# Majestik World Master Plan

## Purpose & Scope
This master plan establishes the end-to-end roadmap for transforming the existing Majestik World (Veloren fork) workspace into a drop-in Unreal Engine 5.6+ plugin while preserving and extending our Rust-based gameplay foundation. It aligns engineering, content, and operations teams around shared milestones, defines guardrails for cross-language integration, and identifies the documentation and CI updates required once the base plan solidifies.

## Strategic Objectives
1. **Deliver a UE 5.6+ plugin** that exposes Majestik World's Rust gameplay, networking, and world simulation through Epic-sanctioned extension points.
2. **Retain deterministic Rust systems** for non-UE-facing logic, exposing them via safe FFI bridges that follow Unreal plugin standards.
3. **Modernize assets and pipelines** (Nanite, Control Rig, MetaSound, UE localization) while preserving existing data tables, save formats, and player progression.
4. **Institutionalize CI/CD coverage** for both Rust and Unreal build targets, ensuring plugin packaging, ABI validation, and content cooking are reproducible.
5. **Document and govern the migration** by updating SPECS.md, README.md, and AGENTS.md once the foundational architecture is validated.

## Guiding Principles
- **Spec-first execution**: Reference SPECS.md, the existing `plan.md`, and future UE migration specs for every change.
- **Rust core as source of truth**: Simulation, world generation, and data registries remain Rust-driven and are exposed to UE via narrow, testable interfaces.
- **UE-native presentation layer**: Rendering, input, audio, UI, and networking surfaces use Unreal frameworks (Nanite, Enhanced Input, AudioMixer, replication).
- **Additive, testable integration**: Introduce abstraction traits and FFI surfaces incrementally; preserve current gameplay determinism during each phase.
- **CI as contract**: Expand automated checks in lockstep with new build surfaces, preventing regressions across Rust crates and UE modules.

## Phased Roadmap
### Phase 0 — Foundation & Governance
- Ratify this PLAN.md with stakeholders; designate owners for gameplay, UE integration, assets, and CI.
- Freeze non-critical feature work in `voxygen` and other engine-facing crates while planning the migration.
- Stand up a `ue5-migration` branch for documentation, prototypes, and tooling experiments that should not block ongoing Rust improvements.

### Phase 1 — Core Extraction & Abstraction
- Audit Rust crates (common, world, server, network) to eliminate direct dependencies on winit, wgpu, cpal, and platform APIs by introducing trait-based ports.
- Consolidate reusable gameplay and simulation crates into a dedicated `rust/core` workspace compiled as static libraries with `panic=abort`.
- Define serialization contracts for state snapshots, world chunks, player data, and asset tables, ensuring compatibility with Unreal data structures.

### Phase 2 — FFI & Plugin Skeleton
- Design the C ABI surface (init, tick, shutdown, event queues, data queries) and generate headers via `cbindgen` or UniFFI.
- Scaffold the UE plugin directory (`Plugins/MajestikWorld/`) with module classes, build rules (`MajestikWorld.Build.cs`), and stub subsystems (GameInstance, Actor Components).
- Prototype bidirectional messaging: UE input events converted to Rust-friendly structs; Rust event bundles consumed by UE gameplay actors.

### Phase 3 — System Integration
- **Networking**: Replace the QUIC transport with UE NetDriver replication, calling Rust diff/compression helpers from replication callbacks.
- **Rendering & Animation**: Translate voxel/mesh data into Nanite-ready assets; drive skeletal animation via Control Rig using Rust state IDs.
- **Audio & UI**: Map Rust semantic audio cues to MetaSound graphs; rebuild inventories, quests, and chat with UMG/Slate widgets reading Rust data snapshots.
- **Tooling**: Create UE Editor utilities for importing Veloren assets, verifying localization, and registering data tables.

### Phase 4 — Asset & Data Pipeline Modernization
- Batch-convert existing models, terrain, and particle effects to Nanite and Niagara formats while retaining original collision/physics metadata for Rust.
- Align localization (`common/i18n`) with UE Localization Dashboard exports; ensure save-game compatibility through validation suites.
- Establish automated asset cooking and packaging workflows compatible with Epic’s distribution requirements.

### Phase 5 — QA, CI, and Documentation Lockdown
- Expand GitHub Actions to run `cargo fmt`, `cargo clippy --all-targets`, `cargo test --all`, Unreal `RunUAT BuildPlugin`, ABI regression tests (`ctest`), and asset validation scripts.
- Author integration and gameplay regression tests that tick the Rust simulation through UE harnesses, verifying determinism and replication stability.
- Update SPECS.md, README.md, and AGENTS.md with finalized architecture, coding standards, and contributor workflows for the UE plugin era.
- Define release criteria, packaging steps, and a rolling schedule for hotfixes and content updates post-migration.

## Cross-Cutting Workstreams
- **Security & Compliance**: Review third-party crates and UE modules for licensing; ensure Epic Marketplace submission policies are met.
- **Performance Engineering**: Carry forward existing performance plan (`plan.md`) insights into UE (profiling hooks, GPU/CPU telemetry, scalability tiers).
- **Developer Experience**: Provide reproducible Nix/flake or containerized toolchains for building Rust libraries and UE plugins locally.
- **Community & Documentation**: Prepare contributor guides covering how to extend Rust systems, add UE assets, and test cross-language features.

## Deliverables & Exit Criteria
- ✅ Rust core libraries compiled as platform-ready artifacts and consumable via generated C headers.
- ✅ UE plugin skeleton linking against Rust, demonstrating input → simulation → replication → presentation loop.
- ✅ Automated CI pipeline covering Rust checks, Unreal builds, ABI validation, and asset packaging.
- ✅ Migration guides for assets, networking, and gameplay systems, with documentation updates approved.
- ✅ Sign-off from gameplay, engine, and operations leads confirming readiness for UE-centric development.

## Next Immediate Actions
1. Circulate PLAN.md and `docs/ue5_plugin_migration_plan.md` for stakeholder review; assign owners per phase/workstream.
2. Kick off dependency abstraction spikes to decouple Rust gameplay crates from winit/wgpu/cpal.
3. Prototype the Rust static library build + `cbindgen` header generation to validate integration assumptions.
4. Draft updates for SPECS.md/README.md/AGENTS.md outlining forthcoming workflow changes once prototypes succeed.

---
*This master plan will be revisited after Phase 2 prototypes validate the Rust ⇄ UE integration strategy, at which point documentation updates and downstream tasks will be scheduled.*
