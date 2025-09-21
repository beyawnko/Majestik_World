//! C ABI bindings for the Majestik World core library.
//!
//! The exported functions provide the `init/tick/shutdown` loop described in
//! `UE5_PLUGIN_MASTER_PLAN.md` Phase 2 and `docs/ue5_plugin_migration_plan.md`
//! §7, enabling Unreal Engine prototypes to call into the Rust simulation.

use std::{
    collections::HashSet,
    ffi::c_void,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use majestic_world_core::{
    CoreInitConfig, GameMode, MajestikCore, TerrainChunkCoord, TerrainDiff, TickConfig,
};

/// Upper bound on a single simulation step's duration to defend against stalls
/// and untrusted runtimes forwarding arbitrarily long pauses. Values above this
/// can destabilise integrators and overflow downstream systems; 10s is chosen
/// as a conservative ceiling for recovery after hitches while preserving
/// safety.
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

fn buffer_owner_registry() -> &'static Mutex<HashSet<usize>> {
    static REGISTRY: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

fn register_buffer_owner(owner: *mut c_void) -> bool {
    if owner.is_null() {
        return false;
    }

    let owner_addr = owner as usize;

    match buffer_owner_registry().lock() {
        Ok(mut guard) => {
            guard.insert(owner_addr);
            true
        },
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.insert(owner_addr);
            true
        },
    }
}

fn take_buffer_owner(owner: *mut c_void) -> bool {
    if owner.is_null() {
        return false;
    }

    let owner_addr = owner as usize;

    match buffer_owner_registry().lock() {
        Ok(mut guard) => guard.remove(&owner_addr),
        Err(poisoned) => poisoned.into_inner().remove(&owner_addr),
    }
}

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
}

impl MwTerrainChunkBuffer {
    fn from_vec(coords: Vec<MwTerrainChunkCoord>) -> Self {
        if coords.len() > MAX_CHUNK_COORDS {
            return Self::default();
        }

        if coords.is_empty() {
            Self {
                ptr: std::ptr::null_mut(),
                len: 0,
                owner: std::ptr::null_mut(),
            }
        } else {
            let mut boxed_vec = Box::new(coords);
            let ptr = boxed_vec.as_mut_ptr();
            let len = boxed_vec.len();
            let owner = Box::into_raw(boxed_vec) as *mut c_void;
            if register_buffer_owner(owner) {
                Self { ptr, len, owner }
            } else {
                // SAFETY: `owner` originates from `Box::into_raw` above and has not
                // been exposed elsewhere, making this reconstruction sound.
                unsafe {
                    drop(Box::from_raw(owner as *mut Vec<MwTerrainChunkCoord>));
                }
                Self {
                    ptr: std::ptr::null_mut(),
                    len: 0,
                    owner: std::ptr::null_mut(),
                }
            }
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// `dt_seconds` must be finite, non-negative, and no greater than
/// [`MAX_DELTA_TIME_SECONDS`].
///
/// # Safety
/// `state` must be a pointer previously returned by [`mw_core_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_core_tick(
    state: *mut MwState,
    dt_seconds: f32,
    update_terrain: MwBool,
) -> MwResult {
    if !dt_seconds.is_finite() || !(0.0..=MAX_DELTA_TIME_SECONDS).contains(&dt_seconds) {
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_core_last_terrain_diff_take(
    state: *mut MwState,
    out_diff: *mut MwTerrainDiff,
) -> MwResult {
    if out_diff.is_null() {
        return MwResult::NullPointer;
    }

    with_state_mut(state, |core| {
        let diff = core.take_last_terrain_diff();
        if diff.new_chunks.len() > MAX_CHUNK_COORDS
            || diff.modified_chunks.len() > MAX_CHUNK_COORDS
            || diff.removed_chunks.len() > MAX_CHUNK_COORDS
        {
            return MwResult::BufferTooLarge;
        }

        let ffi_diff = terrain_diff_into_mw(diff);
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_terrain_chunk_buffer_free(buffer: *mut MwTerrainChunkBuffer) {
    if let Some(buf) = unsafe { buffer.as_mut() } {
        let ptr_is_null = buf.ptr.is_null();

        if (ptr_is_null && buf.len != 0) || (!ptr_is_null && buf.len == 0) {
            return;
        }

        if buf.owner.is_null() {
            return;
        }

        if take_buffer_owner(buf.owner) {
            let owner = buf.owner as *mut Vec<MwTerrainChunkCoord>;
            // SAFETY: `owner` originates from `Box::into_raw` in
            // `MwTerrainChunkBuffer::from_vec` and has been removed from the
            // registry above, guaranteeing this drop occurs at most once.
            unsafe { drop(Box::from_raw(owner)) };
        }

        buf.ptr = std::ptr::null_mut();
        buf.len = 0;
        buf.owner = std::ptr::null_mut();
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
        assert_eq!(unsafe { mw_core_tick(handle, 0.0, 0) }, MwResult::Success);

        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn accepts_negative_zero_dt() {
        let handle = create_state();

        assert_eq!(
            unsafe { mw_core_tick(handle, -0.0, MwBool::from(true)) },
            MwResult::Success
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

            mw_terrain_chunk_buffer_free(&mut ffi_diff.new_chunks);
            mw_terrain_chunk_buffer_free(&mut ffi_diff.modified_chunks);
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

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };

        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 0);
        assert!(buffer.owner.is_null());
    }

    #[test]
    fn buffer_free_rejects_malformed_and_is_idempotent() {
        let mut buffer = MwTerrainChunkBuffer {
            ptr: ptr::null_mut(),
            len: 1,
            owner: ptr::null_mut(),
        };

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };

        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 1);
        assert!(buffer.owner.is_null());

        unsafe { mw_terrain_chunk_buffer_free(&mut buffer) };
        assert!(buffer.ptr.is_null());
        assert_eq!(buffer.len, 1);
        assert!(buffer.owner.is_null());
    }

    #[test]
    fn take_buffer_owner_recovers_from_poison() {
        let owner = Box::into_raw(Box::new(Vec::<MwTerrainChunkCoord>::new())) as *mut c_void;
        assert!(register_buffer_owner(owner));

        let barrier = Arc::new(Barrier::new(2));
        let waiter = barrier.clone();
        let handle = thread::spawn(move || {
            let _guard = buffer_owner_registry().lock().unwrap();
            waiter.wait();
            panic!("poison");
        });

        barrier.wait();
        let _ = handle.join();

        assert!(take_buffer_owner(owner));

        unsafe {
            drop(Box::from_raw(owner as *mut Vec<MwTerrainChunkCoord>));
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
