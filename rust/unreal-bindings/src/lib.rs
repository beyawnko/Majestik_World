#![cfg_attr(ffi_use_unsafe_attributes, allow(stable_features))]
#![cfg_attr(ffi_use_unsafe_attributes, feature(unsafe_attributes))]

//! C ABI bindings for the Majestik World core library.
//!
//! The exported functions provide the `init/tick/shutdown` loop described in
//! `UE5_PLUGIN_MASTER_PLAN.md` Phase 2 and `docs/ue5_plugin_migration_plan.md`
//! §7, enabling Unreal Engine prototypes to call into the Rust simulation.
//!
//! For licensing or redistribution questions related to this FFI surface,
//! contact GitHub user `beyawnko`, who retains sole ownership during the UE5
//! migration effort.

use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
    time::Duration,
};

#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use std::sync::atomic::AtomicU64;

use majestic_world_core::{
    CoreInitConfig, GameMode, MajestikCore, TerrainChunkCoord, TerrainDiff, TickConfig,
};

/// Upper bound on per-tick delta time accepted by the FFI.
///
/// UE5 may hand the simulation arbitrarily long pauses after hitch recovery or
/// background suspends. Clamping to ten seconds bounds the amount of work a
/// single tick can demand, prevents denial-of-service attempts that forward
/// unbounded pauses, and keeps physics/integrator stability within the ranges
/// validated in `UE5_PLUGIN_MASTER_PLAN.md`. Exceeding this window risks
/// destabilising downstream systems or overflowing time accumulators, so the
/// runtime returns [`MwResult::InvalidDeltaTime`] instead of advancing the
/// simulation.
const MAX_DELTA_TIME_SECONDS: f32 = 10.0;

/// Result codes returned by the FFI surface.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MwResult {
    Success = 0,
    NullPointer = 1,
    InvalidMapSize = 2,
    InvalidDayCycle = 3,
    InvalidDeltaTime = 4,
    InvalidGameMode = 5,
    BufferTooLarge = 6,
    InternalError = 255,
}

impl From<majestic_world_core::CoreInitError> for MwResult {
    fn from(err: majestic_world_core::CoreInitError) -> Self {
        match err {
            majestic_world_core::CoreInitError::InvalidMapSize => Self::InvalidMapSize,
            majestic_world_core::CoreInitError::InvalidDayCycleCoefficient => Self::InvalidDayCycle,
        }
    }
}

/// Boolean type used across the ABI (0 = false, non-zero = true).
pub type MwBool = u8;

/// UE-facing representation of [`GameMode`].
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MwGameMode {
    Server = 0,
    Client = 1,
    Singleplayer = 2,
}

impl From<GameMode> for MwGameMode {
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::Server => MwGameMode::Server,
            GameMode::Client => MwGameMode::Client,
            GameMode::Singleplayer => MwGameMode::Singleplayer,
        }
    }
}

impl TryFrom<i32> for MwGameMode {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Server),
            1 => Ok(Self::Client),
            2 => Ok(Self::Singleplayer),
            _ => Err(()),
        }
    }
}

impl From<MwGameMode> for GameMode {
    fn from(mode: MwGameMode) -> Self {
        match mode {
            MwGameMode::Server => GameMode::Server,
            MwGameMode::Client => GameMode::Client,
            MwGameMode::Singleplayer => GameMode::Singleplayer,
        }
    }
}

/// Configuration payload consumed by [`mw_core_create`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MwCoreConfig {
    pub map_size_lg_x: u32,
    pub map_size_lg_y: u32,
    pub sea_level: i32,
    pub day_cycle_coefficient: f64,
    /// Integer representation of [`MwGameMode`]. Values outside the declared
    /// discriminants cause [`mw_core_create`] to return
    /// [`MwResult::InvalidGameMode`].
    pub game_mode: i32,
}

impl Default for MwCoreConfig {
    fn default() -> Self {
        Self {
            map_size_lg_x: 1,
            map_size_lg_y: 1,
            sea_level: 0,
            day_cycle_coefficient: 1.0,
            game_mode: MwGameMode::Server as i32,
        }
    }
}

impl MwCoreConfig {
    fn try_game_mode(self) -> Result<MwGameMode, MwResult> {
        MwGameMode::try_from(self.game_mode).map_err(|_| MwResult::InvalidGameMode)
    }

    fn try_into_core_config(self) -> Result<CoreInitConfig, MwResult> {
        let game_mode = self.try_game_mode()?;
        Ok(CoreInitConfig::from_components(
            self.map_size_lg_x,
            self.map_size_lg_y,
            self.sea_level,
            self.day_cycle_coefficient,
            game_mode.into(),
        ))
    }
}

/// Coordinate pair describing a changed terrain chunk.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MwTerrainChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl From<TerrainChunkCoord> for MwTerrainChunkCoord {
    fn from(coord: TerrainChunkCoord) -> Self {
        Self {
            x: coord.x,
            y: coord.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferOwnerEntry {
    owner_addr: usize,
    data_ptr: *mut MwTerrainChunkCoord,
    len: usize,
}

// SAFETY: `BufferOwnerEntry` only carries raw pointers and metadata describing
// a heap allocation tracked by the buffer owner registry. Access to the
// registry is synchronised by a `Mutex`, and entries are never dereferenced
// while the lock is held—callers only compare addresses or convert them back
// into a `Box` after successful removal. Sharing entries across threads
// therefore does not introduce data races or invalid aliasing.
unsafe impl Send for BufferOwnerEntry {}
unsafe impl Sync for BufferOwnerEntry {}

fn buffer_owner_registry() -> &'static Mutex<HashMap<u64, BufferOwnerEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, BufferOwnerEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

const MAX_BUFFER_ID_ATTEMPTS: usize = 64;

#[cfg(test)]
static FORCE_REGISTER_FAILURE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static FORCE_REGISTER_COLLISIONS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
static REGISTRY_POISON_LOGGED: AtomicBool = AtomicBool::new(false);

fn log_registry_poison(operation: &'static str) {
    #[cfg(test)]
    {
        REGISTRY_POISON_LOGGED.store(true, Ordering::SeqCst);
    }

    eprintln!("buffer owner registry mutex poisoned during {operation}; attempting recovery",);
}

fn with_registry_mut<R>(
    operation: &'static str,
    f: impl FnOnce(&mut HashMap<u64, BufferOwnerEntry>) -> R,
) -> R {
    match buffer_owner_registry().lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => {
            log_registry_poison(operation);
            let mut guard = poisoned.into_inner();
            f(&mut guard)
        },
    }
}

fn log_buffer_creation_failure(reason: &str) {
    eprintln!("mw terrain chunk buffer allocation failed: {reason}");
}

fn register_buffer_owner(
    owner: *mut c_void,
    data_ptr: *mut MwTerrainChunkCoord,
    len: usize,
) -> Result<u64, ()> {
    if owner.is_null() || data_ptr.is_null() {
        return Err(());
    }

    #[cfg(test)]
    if FORCE_REGISTER_FAILURE.swap(false, Ordering::SeqCst) {
        return Err(());
    }

    for _ in 0..MAX_BUFFER_ID_ATTEMPTS {
        let id = NEXT_BUFFER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if id == 0 {
            continue;
        }

        #[cfg(test)]
        {
            let collisions = FORCE_REGISTER_COLLISIONS.load(Ordering::SeqCst);
            if collisions > 0 {
                FORCE_REGISTER_COLLISIONS.store(collisions - 1, Ordering::SeqCst);
                continue;
            }
        }

        let entry = BufferOwnerEntry {
            owner_addr: owner as usize,
            data_ptr,
            len,
        };

        if with_registry_mut("register", |registry| registry.insert(id, entry).is_none()) {
            return Ok(id);
        }
    }

    Err(())
}

fn take_buffer_owner(owner_id: u64) -> Option<BufferOwnerEntry> {
    if owner_id == 0 {
        return None;
    }

    with_registry_mut("take", |registry| registry.remove(&owner_id))
}

#[cfg(test)]
fn buffer_owner_registry_len() -> usize { with_registry_mut("inspect", |registry| registry.len()) }

/// Maximum number of terrain chunk coordinates returned in a single buffer.
///
/// This guard prevents untrusted runtimes from forcing the allocator to
/// reserve excessive memory when marshaling large diffs across the FFI
/// boundary.
const MAX_CHUNK_COORDS: usize = 65_536;

/// Buffer returned from terrain diff queries.
///
/// The buffer exposes a borrowed slice of chunk coordinates. Ownership of the
/// allocation remains with the Rust side and must be released via
/// [`mw_terrain_chunk_buffer_free`] when the caller is finished processing the
/// data. The `owner` field is reserved for the allocator and must be treated as
/// opaque by foreign callers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MwTerrainChunkBuffer {
    pub ptr: *mut MwTerrainChunkCoord,
    pub len: usize,
    owner: *mut c_void,
    owner_id: u64,
}

impl MwTerrainChunkBuffer {
    fn from_vec(coords: Vec<MwTerrainChunkCoord>) -> Self {
        if coords.is_empty() {
            return Self::default();
        }

        if coords.len() > MAX_CHUNK_COORDS {
            log_buffer_creation_failure("coordinate count exceeds MAX_CHUNK_COORDS");
            return Self::default();
        }

        let mut boxed_vec = Box::new(coords);
        let ptr = boxed_vec.as_mut_ptr();
        let len = boxed_vec.len();
        let owner_candidate = (&mut *boxed_vec) as *mut Vec<MwTerrainChunkCoord> as *mut c_void;

        match register_buffer_owner(owner_candidate, ptr, len) {
            Ok(owner_id) => {
                let owner = Box::into_raw(boxed_vec) as *mut c_void;
                Self {
                    ptr,
                    len,
                    owner,
                    owner_id,
                }
            },
            Err(_) => {
                log_buffer_creation_failure("buffer owner registration failed");
                Self::default()
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MwTerrainDiff {
    pub new_chunks: MwTerrainChunkBuffer,
    pub modified_chunks: MwTerrainChunkBuffer,
    pub removed_chunks: MwTerrainChunkBuffer,
}

/// Opaque handle stored by foreign runtimes.
#[repr(C)]
pub struct MwState {
    inner: MajestikCore,
}

fn write_out_ptr<T>(out: *mut *mut T, value: Box<T>) -> MwResult {
    if let Some(slot) = unsafe { out.as_mut() } {
        *slot = Box::into_raw(value);
        MwResult::Success
    } else {
        MwResult::NullPointer
    }
}

/// Populate a configuration struct with default values.
///
/// # Safety
/// `out_config` must be a valid, writable pointer.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_config_default(out_config: *mut MwCoreConfig) -> MwResult {
    if let Some(out) = unsafe { out_config.as_mut() } {
        *out = MwCoreConfig::default();
        MwResult::Success
    } else {
        MwResult::NullPointer
    }
}

/// Create a new [`MajestikCore`] instance and return an opaque handle.
///
/// # Safety
/// `config` and `out_state` must be null or point to valid memory owned by the
/// caller. Passing a null `config` pointer is allowed and uses default values.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_create(
    config: *const MwCoreConfig,
    out_state: *mut *mut MwState,
) -> MwResult {
    let cfg = unsafe { config.as_ref() }.copied().unwrap_or_default();

    match cfg.try_into_core_config() {
        Ok(core_cfg) => match MajestikCore::new(core_cfg) {
            Ok(core) => write_out_ptr(out_state, Box::new(MwState { inner: core })),
            Err(err) => err.into(),
        },
        Err(err) => err,
    }
}

/// Destroy a previously created [`MwState`].
///
/// # Safety
/// `state` must be a pointer previously returned by [`mw_core_create`].
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_destroy(state: *mut MwState) {
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state) });
    }
}

fn with_state_mut(state: *mut MwState, f: impl FnOnce(&mut MajestikCore) -> MwResult) -> MwResult {
    match unsafe { state.as_mut() } {
        Some(mw_state) => f(&mut mw_state.inner),
        None => MwResult::NullPointer,
    }
}

fn with_state(state: *const MwState, f: impl FnOnce(&MajestikCore) -> MwResult) -> MwResult {
    match unsafe { state.as_ref() } {
        Some(mw_state) => f(&mw_state.inner),
        None => MwResult::NullPointer,
    }
}

/// Advance the simulation by `dt_seconds` seconds.
///
/// # Parameters
/// * `dt_seconds` — must be finite, non-negative, and not exceed
///   [`MAX_DELTA_TIME_SECONDS`]. `+0.0` is accepted as a zero-length step while
///   `-0.0` and negative values are rejected to avoid ambiguous floating-point
///   comparisons. Positive subnormal values are allowed so integrators can
///   represent very small time slices when necessary.
///
/// # Safety
/// `state` must be a pointer previously returned by [`mw_core_create`].
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_tick(
    state: *mut MwState,
    dt_seconds: f32,
    update_terrain: MwBool,
) -> MwResult {
    if !dt_seconds.is_finite() || !(0.0..=MAX_DELTA_TIME_SECONDS).contains(&dt_seconds) {
        return MwResult::InvalidDeltaTime;
    }

    if dt_seconds == 0.0 && dt_seconds.is_sign_negative() {
        return MwResult::InvalidDeltaTime;
    }

    with_state_mut(state, |core| {
        let config = TickConfig {
            update_terrain: update_terrain != 0,
        };
        core.tick(Duration::from_secs_f32(dt_seconds), config);
        MwResult::Success
    })
}

fn write_scalar<T: Copy>(out: *mut T, value: T) -> MwResult {
    if let Some(slot) = unsafe { out.as_mut() } {
        *slot = value;
        MwResult::Success
    } else {
        MwResult::NullPointer
    }
}

fn terrain_diff_into_mw(diff: TerrainDiff) -> MwTerrainDiff {
    fn convert(chunks: Vec<TerrainChunkCoord>) -> MwTerrainChunkBuffer {
        let coords = chunks.into_iter().map(MwTerrainChunkCoord::from).collect();
        MwTerrainChunkBuffer::from_vec(coords)
    }

    MwTerrainDiff {
        new_chunks: convert(diff.new_chunks),
        modified_chunks: convert(diff.modified_chunks),
        removed_chunks: convert(diff.removed_chunks),
    }
}

/// Query the accumulated simulation time in seconds.
///
/// # Safety
/// `state` must be a valid pointer returned by [`mw_core_create`], `out_time`
/// must be writable.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_time_seconds(
    state: *const MwState,
    out_time: *mut f64,
) -> MwResult {
    with_state(state, |core| write_scalar(out_time, core.time_seconds()))
}

/// Query the accumulated program time in seconds.
///
/// # Safety
/// `state` must be a valid pointer returned by [`mw_core_create`], `out_time`
/// must be writable.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_program_time_seconds(
    state: *const MwState,
    out_time: *mut f64,
) -> MwResult {
    with_state(state, |core| {
        write_scalar(out_time, core.program_time_seconds())
    })
}

/// Query the accumulated in-game time-of-day in seconds.
///
/// # Safety
/// `state` must be a valid pointer returned by [`mw_core_create`], `out_time`
/// must be writable.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_time_of_day_seconds(
    state: *const MwState,
    out_time: *mut f64,
) -> MwResult {
    with_state(state, |core| {
        write_scalar(out_time, core.time_of_day_seconds())
    })
}

/// Fetch the [`MwGameMode`] currently running inside the state handle.
///
/// # Safety
/// `state` must be a valid pointer returned by [`mw_core_create`], `out_mode`
/// must be writable.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_game_mode(
    state: *const MwState,
    out_mode: *mut MwGameMode,
) -> MwResult {
    with_state(state, |core| {
        let mode = MwGameMode::from(core.game_mode());
        write_scalar(out_mode, mode)
    })
}

/// Consume and return the terrain diff captured during the previous tick.
///
/// # Safety
/// `state` and `out_diff` must be valid pointers. The caller is responsible for
/// releasing buffers contained in `MwTerrainDiff` via
/// [`mw_terrain_chunk_buffer_free`] before mutating or destroying the returned
/// state handle.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_core_last_terrain_diff_take(
    state: *mut MwState,
    out_diff: *mut MwTerrainDiff,
) -> MwResult {
    if out_diff.is_null() {
        return MwResult::NullPointer;
    }

    with_state_mut(state, |core| {
        let last = core.last_terrain_diff();
        if last.new_chunks.len() > MAX_CHUNK_COORDS
            || last.modified_chunks.len() > MAX_CHUNK_COORDS
            || last.removed_chunks.len() > MAX_CHUNK_COORDS
        {
            return MwResult::BufferTooLarge;
        }

        let mut ffi_diff = terrain_diff_into_mw(last.clone());

        let new_failed = !last.new_chunks.is_empty() && ffi_diff.new_chunks.ptr.is_null();
        let modified_failed =
            !last.modified_chunks.is_empty() && ffi_diff.modified_chunks.ptr.is_null();
        let removed_failed =
            !last.removed_chunks.is_empty() && ffi_diff.removed_chunks.ptr.is_null();

        if new_failed || modified_failed || removed_failed {
            unsafe {
                mw_terrain_chunk_buffer_free(&mut ffi_diff.new_chunks);
                mw_terrain_chunk_buffer_free(&mut ffi_diff.modified_chunks);
                mw_terrain_chunk_buffer_free(&mut ffi_diff.removed_chunks);
            }
            return MwResult::InternalError;
        }

        let _ = core.take_last_terrain_diff();
        unsafe { core::ptr::write(out_diff, ffi_diff) };
        MwResult::Success
    })
}

/// Release memory owned by a terrain chunk buffer previously returned from
/// [`mw_core_last_terrain_diff_take`].
///
/// # Safety
/// `buffer` must either be null or point to a valid buffer that has not yet
/// been freed.
#[cfg_attr(ffi_use_unsafe_attributes, unsafe(no_mangle))]
#[cfg_attr(not(ffi_use_unsafe_attributes), no_mangle)]
pub unsafe extern "C" fn mw_terrain_chunk_buffer_free(buffer: *mut MwTerrainChunkBuffer) {
    if let Some(buf) = unsafe { buffer.as_mut() } {
        let owner_ptr = buf.owner;
        let owner_id = buf.owner_id;

        if owner_ptr.is_null() || owner_id == 0 {
            buf.ptr = std::ptr::null_mut();
            buf.len = 0;
            buf.owner = std::ptr::null_mut();
            buf.owner_id = 0;
            return;
        }

        if let Some(entry) = take_buffer_owner(owner_id) {
            let owner_matches = owner_ptr as usize == entry.owner_addr;
            let data_matches = buf.ptr == entry.data_ptr && buf.len == entry.len;
            let is_empty_buffer = buf.ptr.is_null() || buf.len == 0;

            if owner_matches && (data_matches || is_empty_buffer) {
                // SAFETY: `entry.owner_addr` originates from `Box::into_raw` in
                // `MwTerrainChunkBuffer::from_vec` and has been removed from the
                // registry above, guaranteeing this drop occurs at most once.
                unsafe {
                    drop(Box::from_raw(
                        entry.owner_addr as *mut Vec<MwTerrainChunkCoord>,
                    ));
                }
            } else {
                eprintln!(
                    "WARNING: mw_terrain_chunk_buffer_free validation failed for owner ID {}. The \
                     buffer was not freed to prevent memory corruption. This may indicate a bug \
                     in the FFI caller.",
                    owner_id
                );
                with_registry_mut("restore", |registry| {
                    if registry.insert(owner_id, entry).is_some() {
                        eprintln!(
                            "ERROR: Buffer owner registry collision on restore for ID {}. This \
                             indicates a buffer validation bug. Memory will be leaked to prevent \
                             use-after-free. Please report this issue with reproduction steps.",
                            owner_id
                        );
                    }
                });
            }
        }

        buf.ptr = std::ptr::null_mut();
        buf.len = 0;
        buf.owner = std::ptr::null_mut();
        buf.owner_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        convert::TryFrom,
        ffi::c_void,
        ptr,
        sync::{Arc, Barrier},
        thread,
    };

    fn create_state() -> *mut MwState {
        let mut handle: *mut MwState = ptr::null_mut();
        assert_eq!(
            unsafe { mw_core_create(ptr::null(), &mut handle) },
            MwResult::Success
        );
        assert!(!handle.is_null());
        handle
    }

    #[test]
    fn create_tick_and_destroy_round_trip() {
        let handle = create_state();

        assert_eq!(unsafe { mw_core_tick(handle, 0.016, 0) }, MwResult::Success);

        let mut time = 0.0;
        assert_eq!(
            unsafe { mw_core_time_seconds(handle, &mut time) },
            MwResult::Success
        );
        assert!(time > 0.0);

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn rejects_invalid_delta_time() {
        let handle = create_state();

        assert_eq!(
            unsafe { mw_core_tick(handle, f32::NAN, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, -0.1, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, MAX_DELTA_TIME_SECONDS + 1.0, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, f32::INFINITY, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, f32::NEG_INFINITY, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(unsafe { mw_core_tick(handle, 0.0, 0) }, MwResult::Success);
        assert_eq!(
            unsafe { mw_core_tick(handle, MAX_DELTA_TIME_SECONDS, 0) },
            MwResult::Success
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn allows_subnormal_delta_time() {
        let handle = create_state();
        let subnormal = f32::from_bits(1); // smallest positive subnormal
        let smallest_normal = f32::MIN_POSITIVE;

        assert_eq!(
            unsafe { mw_core_tick(handle, subnormal, 0) },
            MwResult::Success
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, -subnormal, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, smallest_normal, 0) },
            MwResult::Success
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn rejects_negative_zero_dt() {
        let handle = create_state();

        assert_eq!(
            unsafe { mw_core_tick(handle, -0.0, MwBool::from(true)) },
            MwResult::InvalidDeltaTime
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn validates_dt_before_duration_conversion() {
        let handle = create_state();

        assert_eq!(
            unsafe { mw_core_tick(handle, f32::INFINITY, 0) },
            MwResult::InvalidDeltaTime
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, f32::NEG_INFINITY, 0) },
            MwResult::InvalidDeltaTime
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn rejects_oversize_dt() {
        let handle = create_state();

        assert_eq!(
            unsafe { mw_core_tick(handle, MAX_DELTA_TIME_SECONDS + 0.001, 0) },
            MwResult::InvalidDeltaTime
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn terrain_diff_take_returns_empty_by_default() {
        let handle = create_state();

        let mut diff = MwTerrainDiff::default();
        assert_eq!(
            unsafe { mw_core_last_terrain_diff_take(handle, &mut diff) },
            MwResult::Success
        );
        assert_eq!(diff.new_chunks.len, 0);
        assert!(diff.new_chunks.ptr.is_null());

        unsafe {
            mw_terrain_chunk_buffer_free(&mut diff.new_chunks);
            mw_terrain_chunk_buffer_free(&mut diff.modified_chunks);
            mw_terrain_chunk_buffer_free(&mut diff.removed_chunks);
            mw_core_destroy(handle);
        }
    }

    #[test]
    fn terrain_diff_take_returns_error_for_oversize_buffers() {
        let handle = create_state();
        let oversize = TerrainDiff {
            new_chunks: vec![TerrainChunkCoord::new(0, 0); MAX_CHUNK_COORDS + 1],
            modified_chunks: Vec::new(),
            removed_chunks: Vec::new(),
        };

        assert_eq!(
            with_state_mut(handle, move |core| {
                core.inject_last_terrain_diff_for_test(oversize);
                MwResult::Success
            }),
            MwResult::Success
        );

        let mut diff = MwTerrainDiff::default();
        assert_eq!(
            unsafe { mw_core_last_terrain_diff_take(handle, &mut diff) },
            MwResult::BufferTooLarge
        );

        assert_eq!(
            with_state_mut(handle, |core| {
                assert_eq!(
                    core.last_terrain_diff().new_chunks.len(),
                    MAX_CHUNK_COORDS + 1
                );
                MwResult::Success
            }),
            MwResult::Success
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn terrain_diff_take_preserves_data_on_buffer_failure() {
        let handle = create_state();
        let test_diff = TerrainDiff {
            new_chunks: vec![TerrainChunkCoord::new(1, 2)],
            modified_chunks: Vec::new(),
            removed_chunks: Vec::new(),
        };

        assert_eq!(
            with_state_mut(handle, |core| {
                core.inject_last_terrain_diff_for_test(test_diff.clone());
                MwResult::Success
            }),
            MwResult::Success
        );

        FORCE_REGISTER_FAILURE.store(true, Ordering::SeqCst);

        let mut out = MwTerrainDiff::default();
        assert_eq!(
            unsafe { mw_core_last_terrain_diff_take(handle, &mut out) },
            MwResult::InternalError
        );

        assert_eq!(
            with_state_mut(handle, |core| {
                assert_eq!(core.last_terrain_diff().new_chunks, test_diff.new_chunks);
                MwResult::Success
            }),
            MwResult::Success
        );

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn terrain_diff_conversion_allocates_buffers() {
        let diff = TerrainDiff {
            new_chunks: vec![TerrainChunkCoord::new(1, 2)],
            modified_chunks: vec![TerrainChunkCoord::new(-4, 3)],
            removed_chunks: vec![],
        };

        let mut ffi_diff = terrain_diff_into_mw(diff);
        unsafe {
            let new_chunks =
                std::slice::from_raw_parts(ffi_diff.new_chunks.ptr, ffi_diff.new_chunks.len);
            assert_eq!(new_chunks, &[MwTerrainChunkCoord { x: 1, y: 2 }]);

            let modified_chunks = std::slice::from_raw_parts(
                ffi_diff.modified_chunks.ptr,
                ffi_diff.modified_chunks.len,
            );
            assert_eq!(modified_chunks, &[MwTerrainChunkCoord { x: -4, y: 3 }]);
            assert!(!ffi_diff.new_chunks.owner.is_null());
            assert!(!ffi_diff.modified_chunks.owner.is_null());
            assert!(ffi_diff.removed_chunks.owner.is_null());
            assert_ne!(ffi_diff.new_chunks.owner_id, 0);
            assert_ne!(ffi_diff.modified_chunks.owner_id, 0);
            assert_eq!(ffi_diff.removed_chunks.owner_id, 0);

            mw_terrain_chunk_buffer_free(&mut ffi_diff.new_chunks);
            assert_eq!(ffi_diff.new_chunks.owner_id, 0);
            mw_terrain_chunk_buffer_free(&mut ffi_diff.modified_chunks);
            assert_eq!(ffi_diff.modified_chunks.owner_id, 0);
            mw_terrain_chunk_buffer_free(&mut ffi_diff.removed_chunks);
        }
    }

    #[test]
    fn oversized_coordinate_vectors_are_rejected() {
        let coords = vec![MwTerrainChunkCoord { x: 0, y: 0 }; MAX_CHUNK_COORDS + 1];
        let buffer = MwTerrainChunkBuffer::from_vec(coords);
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);
    }

    #[test]
    fn rejects_invalid_game_mode() {
        let config = MwCoreConfig {
            game_mode: 42,
            ..Default::default()
        };
        let mut handle: *mut MwState = ptr::null_mut();
        assert_eq!(
            unsafe { mw_core_create(&config, &mut handle) },
            MwResult::InvalidGameMode
        );
        assert!(handle.is_null());
    }

    #[test]
    fn buffer_free_releases_owner_only_once() {
        let mut buffer = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 1, y: 2 }]);
        assert!(!buffer.ptr.is_null());
        assert_eq!(buffer.len, 1);
        assert!(!buffer.owner.is_null());
        assert_ne!(buffer.owner_id, 0);

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };

        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);
    }

    #[test]
    fn buffer_free_rejects_malformed_and_is_idempotent() {
        let mut buffer = MwTerrainChunkBuffer {
            ptr: ptr::null_mut(),
            len: 1,
            owner: ptr::null_mut(),
            owner_id: 0,
        };

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };

        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);

        let mut inconsistent_len =
            MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 5, y: 6 }]);
        assert!(!inconsistent_len.owner.is_null());
        assert_ne!(inconsistent_len.owner_id, 0);
        inconsistent_len.len = 0;

        unsafe { mw_terrain_chunk_buffer_free(&mut inconsistent_len) };
        assert!(inconsistent_len.ptr.is_null());
        assert_eq!(inconsistent_len.len, 0);
        assert!(inconsistent_len.owner.is_null());
        assert_eq!(inconsistent_len.owner_id, 0);

        let mut inconsistent_ptr =
            MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: -2, y: 3 }]);
        inconsistent_ptr.ptr = ptr::null_mut();

        unsafe { mw_terrain_chunk_buffer_free(&mut inconsistent_ptr) };
        assert!(inconsistent_ptr.ptr.is_null());
        assert_eq!(inconsistent_ptr.len, 0);
        assert!(inconsistent_ptr.owner.is_null());
        assert_eq!(inconsistent_ptr.owner_id, 0);
    }

    #[test]
    fn buffer_free_mismatch_protected() {
        let mut first = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 0, y: 0 }]);
        let mut second = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 1, y: 1 }]);

        let second_owner = second.owner;
        let second_owner_id = second.owner_id;

        // Corrupt the exposed buffer to describe the wrong allocation.
        second.ptr = first.ptr;
        second.len = first.len;

        unsafe { mw_terrain_chunk_buffer_free(&mut second) };

        assert!(second.ptr.is_null());
        assert_eq!(second.len, 0);
        assert!(second.owner.is_null());
        assert_eq!(second.owner_id, 0);

        // Clean up the original allocations.
        unsafe { mw_terrain_chunk_buffer_free(&mut first) };

        // Recover the preserved owner entry and drop it to avoid polluting later tests.
        if let Some(restored) = take_buffer_owner(second_owner_id) {
            assert_eq!(restored.owner_addr, second_owner as usize);
            unsafe {
                drop(Box::from_raw(
                    restored.owner_addr as *mut Vec<MwTerrainChunkCoord>,
                ))
            };
        }
    }

    #[test]
    fn buffer_ids_are_unique() {
        let mut first = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 2, y: 3 }]);
        let mut second = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 4, y: 5 }]);

        assert_ne!(first.owner_id, 0);
        assert_ne!(second.owner_id, 0);
        assert_ne!(first.owner_id, second.owner_id);

        unsafe {
            mw_terrain_chunk_buffer_free(&mut first);
            mw_terrain_chunk_buffer_free(&mut second);
        }
        assert_eq!(buffer_owner_registry_len(), 0);
    }

    #[test]
    fn stale_buffer_cannot_free_new_owner() {
        let mut original = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 6, y: 7 }]);
        let mut stale_copy = original;

        unsafe { mw_terrain_chunk_buffer_free(&mut original) };
        assert_eq!(buffer_owner_registry_len(), 0);

        let mut replacement =
            MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 8, y: 9 }]);
        assert_ne!(replacement.owner_id, 0);
        let registry_before = buffer_owner_registry_len();
        let replacement_owner = replacement.owner;
        let replacement_id = replacement.owner_id;

        unsafe { mw_terrain_chunk_buffer_free(&mut stale_copy) };

        assert_eq!(buffer_owner_registry_len(), registry_before);
        assert_eq!(replacement.owner, replacement_owner);
        assert_eq!(replacement.owner_id, replacement_id);

        unsafe { mw_terrain_chunk_buffer_free(&mut replacement) };
        assert_eq!(buffer_owner_registry_len(), 0);
    }

    #[test]
    fn buffer_registration_failure_returns_default() {
        FORCE_REGISTER_FAILURE.store(true, std::sync::atomic::Ordering::SeqCst);

        let buffer = MwTerrainChunkBuffer::from_vec(vec![MwTerrainChunkCoord { x: 9, y: 9 }]);
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
        assert_eq!(buffer.owner_id, 0);
        assert_eq!(buffer_owner_registry_len(), 0);
    }

    #[test]
    fn register_buffer_owner_respects_attempt_limit() {
        FORCE_REGISTER_COLLISIONS
            .store(MAX_BUFFER_ID_ATTEMPTS, std::sync::atomic::Ordering::SeqCst);

        let mut boxed = Box::new(vec![MwTerrainChunkCoord { x: 1, y: 1 }]);
        let ptr = boxed.as_mut_ptr();
        let len = boxed.len();
        let owner_handle = (&mut *boxed) as *mut Vec<MwTerrainChunkCoord> as *mut c_void;

        assert!(register_buffer_owner(owner_handle, ptr, len).is_err());
        assert_eq!(
            FORCE_REGISTER_COLLISIONS.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        FORCE_REGISTER_COLLISIONS.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn register_buffer_owner_rejects_null_data_ptr() {
        let mut owner_box = Box::new(vec![MwTerrainChunkCoord { x: 1, y: 1 }]);
        let owner_handle = (&mut *owner_box) as *mut Vec<MwTerrainChunkCoord> as *mut c_void;

        assert!(register_buffer_owner(owner_handle, std::ptr::null_mut(), 1).is_err());
    }

    #[test]
    fn take_buffer_owner_recovers_from_poison() {
        let mut owner_box = Box::new(vec![MwTerrainChunkCoord { x: 0, y: 0 }]);
        let owner_handle = (&mut *owner_box) as *mut Vec<MwTerrainChunkCoord> as *mut c_void;
        let owner_ptr = owner_box.as_mut_ptr();
        let owner_len = owner_box.len();
        let owner_id =
            register_buffer_owner(owner_handle, owner_ptr, owner_len).expect("owner must register");
        let _owner = Box::into_raw(owner_box);

        REGISTRY_POISON_LOGGED.store(false, std::sync::atomic::Ordering::SeqCst);

        let barrier = Arc::new(Barrier::new(2));
        let waiter = barrier.clone();
        let handle = thread::spawn(move || {
            let _guard = buffer_owner_registry().lock().unwrap();
            waiter.wait();
            panic!("poison");
        });

        barrier.wait();
        let _ = handle.join();

        let taken = take_buffer_owner(owner_id).expect("owner removed despite poison");
        assert_eq!(taken.owner_addr, owner_handle as usize);
        assert!(REGISTRY_POISON_LOGGED.swap(false, std::sync::atomic::Ordering::SeqCst));

        unsafe {
            drop(Box::from_raw(
                taken.owner_addr as *mut Vec<MwTerrainChunkCoord>,
            ));
        }
    }

    #[test]
    fn game_mode_discriminant_validation() {
        assert!(MwGameMode::try_from(0).is_ok());
        assert!(MwGameMode::try_from(1).is_ok());
        assert!(MwGameMode::try_from(2).is_ok());
        assert!(MwGameMode::try_from(42).is_err());
    }
}
