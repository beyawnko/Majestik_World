//! Core Rust library surface for the Majestik World UE5 migration.
//!
//! This crate begins the Phase 1a/1d work described in
//! `docs/ue5_plugin_migration_plan.md`, extracting a deterministic simulation
//! interface that can be linked from external runtimes.

use std::{collections::BTreeSet, fmt, sync::Arc, time::Duration};

use specs::{World, world::WorldExt};
use veloren_common::{
    resources::{GameMode as VelorenGameMode, ProgramTime, Time, TimeOfDay},
    terrain::{MapSizeLg, TerrainChunk},
};
use veloren_common_state::{State, TerrainChanges};

/// Integer grid coordinate describing a terrain chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerrainChunkCoord {
    /// Chunk coordinate along the X axis.
    pub x: i32,
    /// Chunk coordinate along the Y axis.
    pub y: i32,
}

impl TerrainChunkCoord {
    /// Create a new chunk coordinate instance.
    pub const fn new(x: i32, y: i32) -> Self { Self { x, y } }
}

/// Snapshot of terrain diffs produced during a simulation tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainDiff {
    /// Coordinates of chunks inserted during the tick.
    pub new_chunks: Vec<TerrainChunkCoord>,
    /// Coordinates of chunks modified during the tick.
    pub modified_chunks: Vec<TerrainChunkCoord>,
    /// Coordinates of chunks removed during the tick.
    pub removed_chunks: Vec<TerrainChunkCoord>,
}

impl TerrainDiff {
    /// Build a diff from the provided terrain change sets, deduplicating chunk
    /// coordinates and returning them in a deterministic sorted order. This
    /// guarantees the FFI surface never reports duplicate entries while relying
    /// on a `BTreeSet` to maintain ordering during collection, avoiding an
    /// additional sorting pass for large updates.
    fn from_terrain_changes(changes: &TerrainChanges) -> Self {
        fn collect_chunks<'a>(
            iter: impl Iterator<Item = &'a vek::Vec2<i32>>,
        ) -> Vec<TerrainChunkCoord> {
            iter.map(|pos| TerrainChunkCoord::new(pos.x, pos.y))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        }

        Self {
            new_chunks: collect_chunks(changes.new_chunks.iter()),
            modified_chunks: collect_chunks(changes.modified_chunks.iter()),
            removed_chunks: collect_chunks(changes.removed_chunks.iter()),
        }
    }

    /// Whether the diff contains no changes.
    pub fn is_empty(&self) -> bool {
        self.new_chunks.is_empty()
            && self.modified_chunks.is_empty()
            && self.removed_chunks.is_empty()
    }
}

/// Configuration used when instantiating a [`MajestikCore`] simulation handle.
#[derive(Clone, Copy, Debug)]
pub struct CoreInitConfig {
    /// Base two logarithm of the desired world dimensions in chunks.
    pub map_size_lg: vek::Vec2<u32>,
    /// Sea level used when building the fallback terrain chunk.
    pub sea_level: i32,
    /// Day/night speed multiplier relative to real time.
    pub day_cycle_coefficient: f64,
    /// Which gameplay mode to initialise the underlying state with.
    pub game_mode: VelorenGameMode,
}

impl Default for CoreInitConfig {
    fn default() -> Self {
        Self {
            map_size_lg: vek::Vec2::new(1, 1),
            sea_level: 0,
            day_cycle_coefficient: 1.0,
            game_mode: VelorenGameMode::Server,
        }
    }
}

impl CoreInitConfig {
    /// Convenience constructor that avoids exposing the `vek` dependency at
    /// FFI boundaries.
    pub fn from_components(
        map_size_lg_x: u32,
        map_size_lg_y: u32,
        sea_level: i32,
        day_cycle_coefficient: f64,
        game_mode: VelorenGameMode,
    ) -> Self {
        Self {
            map_size_lg: vek::Vec2::new(map_size_lg_x, map_size_lg_y),
            sea_level,
            day_cycle_coefficient,
            game_mode,
        }
    }
}

/// Errors produced when constructing a [`MajestikCore`] instance.
#[derive(Debug, PartialEq, Eq)]
pub enum CoreInitError {
    /// The requested `map_size_lg` violated the invariants enforced by
    /// [`MapSizeLg::new`].
    InvalidMapSize,
    /// The provided day/night cycle multiplier was zero, negative, or not
    /// finite.
    InvalidDayCycleCoefficient,
}

impl fmt::Display for CoreInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMapSize => f.write_str("map_size_lg outside supported range"),
            Self::InvalidDayCycleCoefficient => {
                f.write_str("day_cycle_coefficient must be finite and positive")
            },
        }
    }
}

impl std::error::Error for CoreInitError {}

/// Options that influence how a simulation tick executes.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickConfig {
    /// Whether terrain diffs generated during the tick should be applied
    /// immediately.
    pub update_terrain: bool,
}

/// Deterministic gameplay core that is safe to expose across FFI boundaries.
pub struct MajestikCore {
    state: State,
    server_constants: veloren_common::shared_server_config::ServerConstants,
    game_mode: VelorenGameMode,
    last_terrain_diff: TerrainDiff,
}

impl MajestikCore {
    /// Create a new Majestik simulation core using the supplied configuration.
    pub fn new(config: CoreInitConfig) -> Result<Self, CoreInitError> {
        if !config.day_cycle_coefficient.is_finite() || config.day_cycle_coefficient <= 0.0 {
            return Err(CoreInitError::InvalidDayCycleCoefficient);
        }

        let map_size =
            MapSizeLg::new(config.map_size_lg).map_err(|_| CoreInitError::InvalidMapSize)?;
        let pools = State::pools(config.game_mode);

        let mut state = State::new(
            config.game_mode,
            Arc::clone(&pools),
            map_size,
            Arc::new(TerrainChunk::water(config.sea_level)),
            |_| {},
        );

        // Ensure the `TerrainChanges` resource starts cleared before the first
        // integration step, mirroring the cleanup performed in `tick`.
        state.ecs_mut().write_resource::<TerrainChanges>().clear();

        Ok(Self {
            state,
            server_constants: veloren_common::shared_server_config::ServerConstants {
                day_cycle_coefficient: config.day_cycle_coefficient,
            },
            game_mode: config.game_mode,
            last_terrain_diff: TerrainDiff::default(),
        })
    }

    /// Returns the [`GameMode`] with which this core was initialised.
    pub fn game_mode(&self) -> VelorenGameMode { self.game_mode }

    /// Advance the simulation by the provided duration.
    pub fn tick(&mut self, dt: Duration, config: TickConfig) {
        self.state.tick(
            dt,
            config.update_terrain,
            None,
            &self.server_constants,
            |_, _| {},
        );
        self.snapshot_last_terrain_diff();
        self.state.cleanup();
    }

    /// Read the accumulated simulation time in seconds.
    pub fn time_seconds(&self) -> f64 { self.state.ecs().read_resource::<Time>().0 }

    /// Read the accumulated in-game time-of-day in seconds.
    pub fn time_of_day_seconds(&self) -> f64 { self.state.ecs().read_resource::<TimeOfDay>().0 }

    /// Read the accumulated program time in seconds.
    pub fn program_time_seconds(&self) -> f64 { self.state.ecs().read_resource::<ProgramTime>().0 }

    /// Run a read-only ECS query that must return owned data.
    ///
    /// By constraining the return type to `Send + 'static`, this helper
    /// prevents callers from leaking references tied to the world outside
    /// the closure, eliminating a common class of use-after-free bugs when
    /// integrating with foreign runtimes.
    pub fn query_world_owned<R>(&self, visitor: impl FnOnce(&World) -> R) -> R
    where
        R: Send + 'static,
    {
        visitor(self.state.ecs())
    }

    fn snapshot_last_terrain_diff(&mut self) {
        let diff = {
            let changes = self.state.ecs().read_resource::<TerrainChanges>();
            TerrainDiff::from_terrain_changes(&changes)
        };
        self.last_terrain_diff = diff;
    }

    /// Read the terrain diff captured during the previous tick.
    pub fn last_terrain_diff(&self) -> &TerrainDiff { &self.last_terrain_diff }

    /// Take the previously captured terrain diff, resetting the internal cache.
    pub fn take_last_terrain_diff(&mut self) -> TerrainDiff {
        std::mem::take(&mut self.last_terrain_diff)
    }
}

#[cfg(feature = "ffi-test-hooks")]
impl MajestikCore {
    /// Inject a preconstructed terrain diff for test instrumentation.
    ///
    /// # Safety
    /// This helper is gated behind the `ffi-test-hooks` feature and must never
    /// be enabled in production builds. Forcing arbitrary diffs into the core
    /// bypasses the normal capture pipeline and can desynchronise terrain state
    /// if abused outside controlled tests.
    pub fn inject_last_terrain_diff_for_test(&mut self, diff: TerrainDiff) {
        self.last_terrain_diff = diff;
    }
}

pub use veloren_common::resources::GameMode;

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn rejects_invalid_map_size() {
        let config = CoreInitConfig {
            map_size_lg: vek::Vec2::new(32, 32),
            ..CoreInitConfig::default()
        };
        assert!(matches!(
            MajestikCore::new(config),
            Err(CoreInitError::InvalidMapSize)
        ));
    }

    #[test]
    fn rejects_invalid_day_cycle() {
        let config = CoreInitConfig {
            day_cycle_coefficient: 0.0,
            ..CoreInitConfig::default()
        };
        assert!(matches!(
            MajestikCore::new(config),
            Err(CoreInitError::InvalidDayCycleCoefficient)
        ));

        let config = CoreInitConfig {
            day_cycle_coefficient: f64::NAN,
            ..CoreInitConfig::default()
        };
        assert!(matches!(
            MajestikCore::new(config),
            Err(CoreInitError::InvalidDayCycleCoefficient)
        ));
    }

    #[test]
    fn tick_advances_time_deterministically() {
        let mut core = MajestikCore::new(CoreInitConfig::default()).expect("core initialises");
        let start_sim = core.time_seconds();
        let start_prog = core.program_time_seconds();

        let dt = Duration::from_millis(16);
        core.tick(dt, TickConfig::default());

        let end_sim = core.time_seconds();
        let end_prog = core.program_time_seconds();

        assert_abs_diff_eq!(end_prog - start_prog, dt.as_secs_f64(), epsilon = 1e-9);
        assert!(end_sim > start_sim);
    }

    #[test]
    fn terrain_diff_sorting_is_stable() {
        let mut changes = TerrainChanges::default();
        changes.new_chunks.insert(vek::Vec2::new(2, -5));
        changes.new_chunks.insert(vek::Vec2::new(-3, 4));
        changes.new_chunks.insert(vek::Vec2::new(-3, 2));

        let diff = TerrainDiff::from_terrain_changes(&changes);
        assert_eq!(diff.new_chunks, vec![
            TerrainChunkCoord::new(-3, 2),
            TerrainChunkCoord::new(-3, 4),
            TerrainChunkCoord::new(2, -5),
        ]);
    }

    #[test]
    fn snapshot_and_take_terrain_diff() {
        let mut core = MajestikCore::new(CoreInitConfig::default()).expect("core initialises");
        {
            let mut terrain_changes = core.state.ecs_mut().write_resource::<TerrainChanges>();
            terrain_changes
                .modified_chunks
                .insert(vek::Vec2::new(7, -1));
        }

        core.snapshot_last_terrain_diff();
        let diff = core.take_last_terrain_diff();
        assert_eq!(diff.modified_chunks, vec![TerrainChunkCoord::new(7, -1)]);
        assert!(core.take_last_terrain_diff().is_empty());
    }

    #[test]
    fn query_world_owned_returns_owned_data() {
        let core = MajestikCore::new(CoreInitConfig::default()).expect("core initialises");
        let (time, program_time) = core.query_world_owned(|world| {
            let time = world.read_resource::<Time>().0;
            let program = world.read_resource::<ProgramTime>().0;
            (time, program)
        });

        assert_eq!(time, core.time_seconds());
        assert_eq!(program_time, core.program_time_seconds());
    }
}
