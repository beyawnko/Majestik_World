# Majestic World UE5 Core Workspace

This directory hosts the additive crates introduced for the UE5 migration
roadmap documented in `UE5_PLUGIN_MASTER_PLAN.md` and
`docs/ue5_plugin_migration_plan.md`.

- `core/` exposes `majestic-world-core`, a deterministic gameplay façade around
  Veloren's existing `State` type (Phase 1a/1d). It now snapshots terrain chunk
  diffs each tick so downstream bindings can stream world updates without
  inspecting internal ECS resources directly.
- `unreal-bindings/` exposes `majestic-world-ffi`, a stable C ABI surface for
  Unreal Engine integration experiments (Phase 2). The FFI exports terrain
  change buffers as heap-allocated arrays that UE-side callers can consume and
  free explicitly, matching the migration plan's data-contract goals.

Both crates compile as part of the top-level Cargo workspace and deliberately
avoid linking to client rendering or platform code, aligning with the migration
plan's requirement to decouple the simulation from `voxygen`.
