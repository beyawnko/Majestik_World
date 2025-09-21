//! C ABI bindings for the Majestik World core library.
//!
//! The exported functions provide the `init/tick/shutdown` loop described in
//! `UE5_PLUGIN_MASTER_PLAN.md` Phase 2 and `docs/ue5_plugin_migration_plan.md`
//! §7, enabling Unreal Engine prototypes to call into the Rust simulation.

use std::time::Duration;

use majestic_world_core::{
    CoreInitConfig, GameMode, MajestikCore, TerrainChunkCoord, TerrainDiff, TickConfig,
};

/// Result codes returned by the FFI surface.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MwResult {
    Success = 0,
    NullPointer = 1,
    InvalidMapSize = 2,
    InvalidDayCycle = 3,
    InvalidDeltaTime = 4,
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

impl From<MwGameMode> for GameMode {
    fn from(mode: MwGameMode) -> Self {
        match mode {
            MwGameMode::Server => GameMode::Server,
            MwGameMode::Client => GameMode::Client,
            MwGameMode::Singleplayer => GameMode::Singleplayer,
        }
    }
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

/// Configuration payload consumed by [`mw_core_create`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MwCoreConfig {
    pub map_size_lg_x: u32,
    pub map_size_lg_y: u32,
    pub sea_level: i32,
    pub day_cycle_coefficient: f64,
    pub game_mode: MwGameMode,
}

impl Default for MwCoreConfig {
    fn default() -> Self {
        Self {
            map_size_lg_x: 1,
            map_size_lg_y: 1,
            sea_level: 0,
            day_cycle_coefficient: 1.0,
            game_mode: MwGameMode::Server,
        }
    }
}

impl MwCoreConfig {
    fn into_core_config(self) -> CoreInitConfig {
        CoreInitConfig::from_components(
            self.map_size_lg_x,
            self.map_size_lg_y,
            self.sea_level,
            self.day_cycle_coefficient,
            self.game_mode.into(),
        )
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MwTerrainChunkBuffer {
    pub ptr: *mut MwTerrainChunkCoord,
    pub len: usize,
}

impl MwTerrainChunkBuffer {
    fn from_vec(coords: Vec<MwTerrainChunkCoord>) -> Self {
        if coords.is_empty() {
            Self {
                ptr: std::ptr::null_mut(),
                len: 0,
            }
        } else {
            let len = coords.len();
            let mut boxed = coords.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);
            Self { ptr, len }
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
/// caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_core_create(
    config: *const MwCoreConfig,
    out_state: *mut *mut MwState,
) -> MwResult {
    let cfg = unsafe { config.as_ref() }.copied().unwrap_or_default();

    match MajestikCore::new(cfg.into_core_config()) {
        Ok(core) => write_out_ptr(out_state, Box::new(MwState { inner: core })),
        Err(err) => err.into(),
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

fn with_state_mut<R>(
    state: *mut MwState,
    f: impl FnOnce(&mut MajestikCore) -> R,
) -> Result<R, MwResult> {
    unsafe { state.as_mut() }
        .map(|mw_state| Ok(f(&mut mw_state.inner)))
        .unwrap_or_else(|| Err(MwResult::NullPointer))
}

fn with_state<R>(state: *const MwState, f: impl FnOnce(&MajestikCore) -> R) -> Result<R, MwResult> {
    unsafe { state.as_ref() }
        .map(|mw_state| Ok(f(&mw_state.inner)))
        .unwrap_or_else(|| Err(MwResult::NullPointer))
}

/// Advance the simulation by `dt_seconds` seconds.
///
/// # Safety
/// `state` must be a pointer previously returned by [`mw_core_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_core_tick(
    state: *mut MwState,
    dt_seconds: f32,
    update_terrain: MwBool,
) -> MwResult {
    if !dt_seconds.is_finite() || dt_seconds < 0.0 {
        return MwResult::InvalidDeltaTime;
    }

    with_state_mut(state, |core| {
        let config = TickConfig {
            update_terrain: update_terrain != 0,
        };
        core.tick(Duration::from_secs_f32(dt_seconds), config);
    })
    .map(|_| MwResult::Success)
    .unwrap_or_else(|err| err)
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
    with_state(state, MajestikCore::time_seconds)
        .map(|time| write_scalar(out_time, time))
        .unwrap_or_else(|err| err)
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
    with_state(state, MajestikCore::program_time_seconds)
        .map(|time| write_scalar(out_time, time))
        .unwrap_or_else(|err| err)
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
    with_state(state, MajestikCore::time_of_day_seconds)
        .map(|time| write_scalar(out_time, time))
        .unwrap_or_else(|err| err)
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
    with_state(state, MajestikCore::game_mode)
        .map(MwGameMode::from)
        .map(|mode| write_scalar(out_mode, mode))
        .unwrap_or_else(|err| err)
}

/// Consume and return the terrain diff captured during the previous tick.
///
/// # Safety
/// `state` and `out_diff` must be valid pointers. The caller is responsible for
/// releasing buffers contained in `MwTerrainDiff` via
/// [`mw_terrain_chunk_buffer_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mw_core_last_terrain_diff_take(
    state: *mut MwState,
    out_diff: *mut MwTerrainDiff,
) -> MwResult {
    if out_diff.is_null() {
        return MwResult::NullPointer;
    }

    with_state_mut(state, |core| core.take_last_terrain_diff())
        .map(terrain_diff_into_mw)
        .map(|diff| {
            unsafe { *out_diff = diff };
            MwResult::Success
        })
        .unwrap_or_else(|err| err)
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
        if !buf.ptr.is_null() && buf.len > 0 {
            // SAFETY: The pointer/length pair was produced by `from_vec`, so
            // it is safe to reconstruct the boxed slice.
            let slice = std::ptr::slice_from_raw_parts_mut(buf.ptr, buf.len);
            drop(unsafe { Box::from_raw(slice) });
        }
        buf.ptr = std::ptr::null_mut();
        buf.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn create_tick_and_destroy_round_trip() {
        let mut handle: *mut MwState = ptr::null_mut();
        let config = MwCoreConfig::default();
        assert_eq!(
            unsafe { mw_core_create(&config, &mut handle) },
            MwResult::Success
        );
        assert!(!handle.is_null());

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
        let mut handle: *mut MwState = ptr::null_mut();
        assert_eq!(
            unsafe { mw_core_create(ptr::null(), &mut handle) },
            MwResult::Success
        );
        assert_eq!(
            unsafe { mw_core_tick(handle, f32::NAN, 0) },
            MwResult::InvalidDeltaTime
        );
        unsafe { mw_core_destroy(handle) };
    }

    #[test]
    fn terrain_diff_take_returns_empty_by_default() {
        let mut handle: *mut MwState = ptr::null_mut();
        assert_eq!(
            unsafe { mw_core_create(ptr::null(), &mut handle) },
            MwResult::Success
        );

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
            assert_eq!(new_chunks, &[MwTerrainChunkCoord { x: 1, y: 2 }],);

            let modified_chunks = std::slice::from_raw_parts(
                ffi_diff.modified_chunks.ptr,
                ffi_diff.modified_chunks.len,
            );
            assert_eq!(modified_chunks, &[MwTerrainChunkCoord { x: -4, y: 3 }],);

            mw_terrain_chunk_buffer_free(&mut ffi_diff.new_chunks);
            mw_terrain_chunk_buffer_free(&mut ffi_diff.modified_chunks);
            mw_terrain_chunk_buffer_free(&mut ffi_diff.removed_chunks);
        }
    }
}
