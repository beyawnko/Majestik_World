# Majestik World Master Plan

## Purpose & Scope
This master plan establishes the end-to-end roadmap for transforming the existing Majestik World (Veloren fork) workspace into a drop-in Unreal Engine 5.6+ plugin while preserving and extending our Rust-based gameplay foundation. It aligns engineering, content, and operations teams around shared milestones, defines guardrails for cross-language integration, and identifies the documentation and CI updates required once the base plan solidifies.

## Strategic Objectives
1. **Deliver a UE 5.6+ plugin** that exposes Majestik World's Rust gameplay, networking, and world simulation through Epic-sanctioned extension points.
2. **Retain deterministic Rust systems** for non-UE-facing logic, exposing them via safe FFI bridges that follow Unreal plugin standards and are backed by automated regression tests that verify identical simulation outputs pre- and post-UE integration.
3. **Modernize assets and pipelines** (Nanite, Control Rig, MetaSound, UE localization) while preserving existing data tables, save formats, and player progression.
4. **Institutionalize CI/CD coverage** for both Rust and Unreal build targets, ensuring plugin packaging, ABI validation, and content cooking are reproducible.
5. **Document and govern the migration** by updating SPECS.md, README.md, and AGENTS.md once the foundational architecture is validated.

## Guiding Principles
- **Spec-first execution**: Reference SPECS.md and future UE migration specs for every change. This plan supersedes the rendering-related sections of the existing `plan.md` while preserving its performance engineering insights for UE integration.
- **Rust core as source of truth**: Simulation, world generation, and data registries remain Rust-driven and are exposed to UE via narrow, testable interfaces.
- **UE-native presentation layer**: Rendering, input, audio, UI, and networking surfaces use Unreal frameworks (Nanite, Enhanced Input, AudioMixer, replication).
- **Additive, testable integration**: Introduce abstraction traits and FFI surfaces incrementally; preserve current gameplay determinism during each phase.
- **CI as contract**: Expand automated checks in lockstep with new build surfaces, preventing regressions across Rust crates and UE modules.

## Phased Roadmap
*Note: This master plan provides the strategic overview. For detailed technical implementation, see [UE5 Plugin Migration Technical Plan](docs/ue5_plugin_migration_plan.md) which contains 7 detailed phases that implement the 6 strategic phases outlined below.*
### Phase 0 — Foundation & Governance
- **GPL-3.0 Legal Review (CRITICAL)**: Conduct comprehensive GPL-3.0 compatibility analysis for UE plugin distribution, as copyleft requirements may fundamentally impact architecture decisions and Epic Marketplace eligibility; document findings and approvals before Phase 1 begins.
- Ratify this UE5 Plugin Master Plan with stakeholders; designate owners for gameplay, UE integration, assets, and CI.
- Freeze non-critical feature work in `voxygen` and other engine-facing crates while planning the migration.
- Stand up a `ue5-migration` branch for documentation, prototypes, and tooling experiments that should not block ongoing Rust improvements.
- Schedule a technical spike comparing `cbindgen` and UniFFI, producing minimal Rust ⇄ UE prototypes that evaluate memory ownership, ergonomics, and build integration trade-offs ahead of Phase 2.

### Phase 1 — Core Extraction & Abstraction
- **Phase 1a – Input & windowing abstraction (≈2 engineer-weeks)**: Audit Rust crates (`common`, `client`, `voxygen`) and remove direct `winit` usage by introducing input/window traits exercised by the existing client as a reference implementation.
- **Phase 1b – Rendering/data surface extraction (≈3 engineer-weeks)**: Isolate `wgpu` dependencies behind feature-gated adapters, consolidate gameplay and simulation crates into a `rust/core` workspace compiled as static libraries with `panic=abort`, and document all render-time data contracts.
- **Phase 1c – Audio & platform shims (≈1 engineer-week)**: Replace `cpal`/platform APIs with trait-driven facades, proving parity through existing audio regression tests and ensuring no direct OS calls remain.
- **Phase 1d – Serialization contract hardening (≈1 engineer-week)**: Define and snapshot serialization contracts for state snapshots, world chunks, player data, and asset tables, ensuring compatibility with Unreal data structures and capturing golden files for deterministic comparison.
- **Exit gate**: Phase 1 concludes only when a determinism regression harness demonstrates identical tick outputs between the pre-refactor client and the abstracted `rust/core` workspace across two consecutive runs using standardized test scenarios: (1) 100-tick world generation with fixed seed, (2) player movement and combat simulation, (3) weather and day/night cycle progression, with byte-for-byte output comparison and golden file validation.

### Phase 2 — FFI & Plugin Skeleton
- Design the C ABI surface (init, tick, shutdown, event queues, data queries) using the FFI approach selected during the Phase 0 spike, then generate headers via `cbindgen` or UniFFI accordingly.
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
- **Security & Compliance**: Assume initial distribution via the existing open-source channels; review GPL-3.0 compatibility and third-party crate licensing, and only escalate Epic Marketplace compliance once a productization decision is made.
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
1. **Stakeholder alignment** — Circulate `UE5_PLUGIN_MASTER_PLAN.md` and `docs/ue5_plugin_migration_plan.md` for review. *Success criteria*: sign-off recorded in the issue tracker with named owners for every phase/workstream.
2. **Dependency abstraction spikes** — File and staff work items to decouple Rust gameplay crates from `winit`/`wgpu`/`cpal`. *Success criteria*: each dependency has a linked tracking issue with scope notes, estimates, and assigned engineers.
3. **Rust static library prototype** — Produce a `rust/core` static library alongside generated headers using the selected FFI tooling, validated by a minimal UE or C harness that executes `mw_state_tick` and matches golden determinism snapshots. *Success criteria*: harness output checked into CI with pass/fail gating.
4. **Documentation update plan** — Draft updates for SPECS.md/README.md/AGENTS.md outlining forthcoming workflow changes once prototypes succeed. *Success criteria*: merged documentation checklist that maps every artifact to an owner and milestone.

---
*This master plan will be revisited after Phase 2 prototypes validate the Rust ⇄ UE integration strategy, at which point documentation updates and downstream tasks will be scheduled.*
