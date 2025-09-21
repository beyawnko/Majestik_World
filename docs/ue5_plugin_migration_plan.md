# UE5 Plugin Migration Roadmap

## 1. Objectives & Guardrails
- Produce a UE 5.6+ plugin that can be dropped into an Unreal project while preserving Rust-driven gameplay, networking, and data backends.
- Keep non-UE-facing logic in Rust crates where it delivers value (deterministic gameplay, ECS systems, world simulation).
- Replace all engine-facing systems (rendering, audio, input, networking replication) with Unreal-native implementations that integrate cleanly with Epic-sanctioned extension points.
- Maintain compatibility with existing data tables, asset schemas, and save formats; provide conversion or binding layers when a 1:1 import is impossible.
- Preserve current CI habits (format/lint/test) while expanding automation to cover the UE build pipeline and plugin packaging.

## 2. Workspace Inventory & Boundaries
### 2.1 Core crates to preserve as Rust libraries
| Area | Crates/Paths | Notes |
| --- | --- | --- |
| Gameplay ECS & mechanics | `common/src` (combat, resources, weather, world interactions), `common/systems`, `common/ecs` | Pure gameplay logic/components already separated from rendering. |
| Shared state & save logic | `common/state` (ECS world orchestration, serialized state, plugin hooks) | Holds authoritative state container reused by client/server. |
| World simulation & generation | `world/src` (terrain, civ, site, simulation layers) | Deterministic procedural content suited for Rust static library. |
| Networking protocol logic | `network/protocol` (message types, transport abstractions) | Serialization and prioritization rules reusable behind UE NetDriver bridge. |
| Server gameplay | `server/src` and `server/agent` | Contains rules, NPC AI, quest flow to be exposed to UE through bindings. |
| Databases/data tables | `assets/`, `common/assets`, `common/i18n` | Should be exposed to UE as DataAssets or referenced via Rust API. |

### 2.2 Engine-locked crates to replace or wrap
| Area | Crates/Paths | UE Migration Target |
| --- | --- | --- |
| Rendering & windowing | `voxygen` (wgpu renderer, winit input, egui UI), `voxygen/anim`, `voxygen/egui` | Replace with UE viewport, Nanite pipelines, UMG/Slate, Control Rig; leave only data definitions in Rust. |
| Client runtime shell | `client` crate (window bootstrap, input loop) | Rebuild as UE GameInstance + PlayerController using Rust for logic only. |
| Audio backend | `voxygen` uses `kira`+`cpal` | Route through UE AudioMixer and MetaSound assets. |
| Networking transport | `network` uses `quinn` QUIC + tokio runtime | Bridge to UE replication (UDP) via C++ shim calling Rust protocol/state. |
| Platform integrations | Discord SDK, native dialogs, winit-specific clipboard | Replace with UE OnlineSubsystem / platform APIs.

### 2.3 Hybrid integration candidates
| Area | Crates/Paths | Hybrid Approach |
| --- | --- | --- |
| Networking protocol bridging | `network/protocol`, `network/common`, `network/transport` | Keep message serialization, prioritization, and compression in Rust; expose UE-friendly structs while UE NetDriver owns socket lifecycle and bandwidth management. |
| Server orchestration hooks | `server/src`, `server-cli`, `server/agent` | Preserve quest, AI, and rules logic in Rust while delegating matchmaking/session bootstrap to UE subsystems; provide async callbacks for UE-hosted servers. |
| Data ingestion pipelines | `common/assets`, `common/i18n`, `assets/` | Continue authoring and validating data in Rust, but surface UE DataAssets through generated manifests and hot-load hooks maintained on the UE side. |

## 3. Rust Dependencies Requiring UE Replacement
| Subsystem | Key Crates | Unreal Replacement Strategy |
| --- | --- | --- |
| Graphics | `wgpu`, `shaderc`, `wgpu-profiler`, `glyph_brush` (text), `treeculler` (culling helpers) | Re-implement render flow with UE Render Graph/Nanite. Keep material/mesh metadata in Rust as data descriptors only. |
| Windowing & Input | `winit`, `gilrs`, `iced`, `egui`, `conrod_core`, `window_clipboard` | Use UE Input System, Enhanced Input, Slate/UMG. Provide mapping layer so Rust gameplay systems receive input events via UE dispatch. |
| Audio | `kira`, `cpal`, `mumble-link` | Use UE AudioMixer, MetaSound, and built-in VOIP integration. Rust publishes semantic audio cues consumed by UE. |
| Networking | `quinn`, `tokio`, `tokio-stream`, `async-channel`, `rustls`, `lz-fear` | Replace transport with UE NetDriver/Replication. Rust logic exposes authoritative state diff/compression functions callable from UE. |
| Platform Services | `discord-sdk`, `native-dialog`, `open`, OS-specific memory allocators | Implement via UE OnlineSubsystem, platform frameworks, or optional modules. |

## 4. Game Logic & Systems to Preserve
- **ECS components and systems**: `common/src/comp`, `common/src/systems`, and `common/ecs` define gameplay mechanics, stats, status effects, weather, tethering, etc. These should become the authoritative simulation compiled into a Rust static library that UE calls each tick.
- **State orchestration**: `common/state/src/state.rs` coordinates ECS world creation, time stepping, and plugin hooks; expose this as FFI functions (e.g., `mw_state_init`, `mw_state_tick`) consumable from UE Actors or subsystems.
- **World generation**: `world/src` modules (terrain, civ, sites, sim) remain Rust-driven, generating serialized world chunks that UE converts to Procedural Content (landscape meshes, foliage instances) via translation layers.
- **Server gameplay**: `server/src` logic (missions, AI behaviors) should remain in Rust, callable either in dedicated server builds or via UE server modules linking against the Rust core.
- **Shared data registries**: Items, recipes, weather tables, etc., stored under `common/assets`, `common/i18n`, and `assets/` should feed UE Data Assets or runtime registries by parsing existing RON/JSON/SQL via Rust and handing structured data to UE.

## 5. Serializable Data & Interfaces to Map to UE
- **Authoritative state snapshots** (`common/state/src/state.rs`): provide C ABI functions to fetch serialized ECS component data (player stats, inventories, terrain deltas) for UE replication.
- **Player/account data** (`common/src/character.rs`, `common/src/resources.rs`, `common/src/trade.rs`): define FFI structs mirroring UE `USTRUCT` wrappers so gameplay ability systems can consume them.
- **World terrain data** (`common/src/terrain`, `world/src/land.rs`, `world/src/block.rs`): translate voxel/chunk data into UE Landscape heightmaps or ProceduralMesh data, with Nanite-ready mesh baking.
- **Network messages** (`network/protocol/src/message.rs`, `network/protocol/src/types.rs`): map to UE replicated RPCs and NetSerialize functions, preserving compression/prioritization schemes.
- **Save/Load**: keep Rust serialization (RON/bincode) but expose hooks so UE save games trigger Rust persistence and receive file handles/metadata.

## 6. Rendering Pipeline Extraction Tasks
1. **Freeze Voxygen renderer**: treat `voxygen/src/render` and related shader assets as reference only; no further investment once migration branch starts.
2. **Catalog render passes**: document meshes, terrain, UI, and post-processing passes within `voxygen/src/render/renderer` to know which data streams UE must drive (e.g., chunk meshing, particle emitters, UI overlays).
3. **Define data contracts**: for each renderable (terrain chunk, entity mesh, particle), specify the minimal data Rust must expose (meshes, transforms, animation states) so UE’s Nanite, Niagara, and Material systems can consume them.
4. **Input/event translation**: replace winit event loop with UE input mapping; ensure Rust receives sanitized inputs via FFI (e.g., `mw_apply_input(PlayerId, InputFrame)`).
5. **Animation**: convert Veloren skeletal animation data (currently under `voxygen/anim`) into UE Skeleton/Animation Sequence assets, using data-driven Control Rig or IK solutions.

## 7. Target Unreal Plugin Architecture
### 7.1 Repository Layout Proposal
```
/Plugins/MajestikWorld/
  MajestikWorld.uplugin
  /Content           # UE assets (Nanite meshes, animations, Blueprints)
  /Source
    /MajestikWorld   # UE C++ layer: module, subsystem, Actor components
      MajestikWorld.Build.cs
      Public/
      Private/
    /MajestikWorldRust # Header-only shim generated via cbindgen for Rust APIs
  /ThirdParty/RustCore
    /lib              # Cargo builds output static/dynamic libs per platform
    /include          # Generated C headers for Rust types/APIs
/rust/
  /core              # New Cargo workspace housing extracted gameplay/world crates
  /unreal-bindings   # FFI crate exposing UE-facing API surface
```

### 7.2 Build & Toolchain Integration
- Use `cargo` to build `rust/core` crates into static libs (`.a`/`.lib`) per platform during CI, compiling with `-C panic=abort` to match UE expectations, and publish artifacts into `Plugins/MajestikWorld/ThirdParty/RustCore/<platform>`.
- **Memory Safety Requirements**: Implement comprehensive error handling with Result types throughout FFI boundary, add panic hooks for graceful error handling, and establish validation harness using Valgrind/AddressSanitizer for memory leaks, double-free errors, and proper cleanup of Rust-allocated resources.
- Generate C headers with `cbindgen` (or UniFFI if selected) and bundle them under `Source/MajestikWorldRust` for UE consumption, versioning them alongside the prebuilt libraries.
- Configure `MajestikWorld.Build.cs` to select the correct prebuilt artifact per platform, **with hybrid approach: fail fast with guidance for CI/release builds when platform-specific library missing, but for development builds, invoke script that checks if Rust library is up-to-date and rebuilds if necessary to streamline local development workflow**.
- Support Windows, Linux, and future console targets by extending CI pipelines and Unreal Build Tool (UBT) switches to copy the appropriate ThirdParty libraries and include directories into packaged plugins.

### 7.3 Runtime Ownership Model
- UE `UGameInstanceSubsystem` (C++) owns a pointer to Rust `State` (`mw_state_handle`).
- Each tick, subsystem calls `mw_state_tick(delta)`; the Rust core returns event bundles (entity spawns/despawns, component updates) processed by UE to drive visuals and replication.
- Use UE `UActorComponent` wrappers to mirror ECS components that need blueprint access (e.g., health, equipment). Components subscribe to update streams from Rust via **memory-safe message passing through serialized data or atomic operations with clear ownership semantics and documented synchronization primitives**.
- **Synchronization Requirements**: Document specific memory ordering guarantees, implement proper atomic operations for cross-language data sharing, and establish clear ownership semantics to prevent data races.

### 7.4 Networking & Replication Plan
- Retire Rust’s QUIC socket management; designate UE server authoritative using existing NetDriver for connection/auth.
- **Performance and Security Requirements**: Establish specific performance benchmarks comparing QUIC vs UE NetDriver (latency, throughput, packet loss handling). Document security considerations including encryption parity, DDoS protection, and congestion control before committing to migration.
- Rust networking crates degrade to deterministic state diff/compression utilities invoked by UE replication callbacks (e.g., `GetLifetimeReplicatedProps`).
- Provide bridging layer translating `network/protocol` message structs into UE `FStruct`s, letting UE’s replication automatically deliver to clients while Rust handles simulation updates.
- Benchmark UE NetDriver throughput and latency against the existing QUIC pipeline early in Phase 3 using representative workloads and the defined metrics; treat any performance regression or missing security parity beyond agreed tolerances as a blocker for full migration.

### 7.5 Input, Audio, and UI Hooks
- UE input actions produce normalized `InputFrame` data forwarded to Rust (per player) through FFI.
- Rust emits semantic audio events (e.g., `PlaySound(SoundEventId, Position)`) captured by UE audio subsystem and played via MetaSound graphs.
- Replace egui/iced UIs with UE UMG/Slate, reading Rust-provided data models (inventories, vendor lists) via asynchronous queries.

### 7.6 Asset & Animation Strategy
- Convert Voxel/VOX assets into Nanite meshes during content cooking; maintain original data for deterministic physics/hitboxes in Rust.
- **Asset Validation Pipeline**: Establish validation pipeline verifying converted assets maintain gameplay-critical properties (collision detection, physics calculations) and implement rollback mechanism for problematic conversions.
- Use Veloren animation timelines as data-driven references to author UE Animation Sequences or Control Rig logic, storing mapping tables linking Rust animation state IDs to UE assets.
- Keep localization tables by parsing `common/i18n` with Rust, then exporting to UE `Localization Dashboard` resources during build.

## 8. Workflow, Testing, and CI Expansion
- **Rust CI (existing):** keep `cargo fmt`, `cargo clippy --all-targets`, `cargo test --all`. Run inside `rust/` workspace.
- **Security Scanning:** Add automated security scanning tools for C/Rust FFI boundaries analyzing buffer overflows, null pointer dereferences, and improper memory management across language boundaries.
- **UE Build Verification:** add GitHub Actions job using Unreal Automation Tool (UAT) or `RunUAT BuildPlugin` to compile the plugin against UE 5.6 headless build (Linux container).
- **Binding Consistency Test:** add integration test crate that links against generated C headers and ensures ABI compatibility (e.g., using `ctest`).
- **Simulation Regression Tests:** keep deterministic worldgen/save-game tests in Rust; use golden outputs to ensure parity before and after UE integration.
- **Packaging:** create CI artifact packaging plugin zip with built Rust libs for supported platforms.
- **Branch Strategy:** create `ue5-migration` analysis branch for documentation and tooling while leaving existing Rust gameplay development on `main`; merge plan once executable path validated.

## 9. Phased Migration Plan
1. **Discovery & Documentation**
   - Freeze `voxygen` feature work; document render/input/audio contracts.
   - Produce binding specification for ECS components and network messages.
2. **Rust Core Extraction**
   - Factor gameplay/world crates into `rust/core` workspace with clean `no_std`-friendly boundaries where feasible.
   - Remove direct dependencies on winit/wgpu/cpal from logic crates by introducing trait-based interfaces.
3. **FFI Surface Definition**
   - Design `extern "C"` API for state lifecycle, tick, event subscription, and data queries.
   - Implement serialization bridging (RON/JSON/Bincode) to UE `FBufferArchive` / `FMemoryReader` wrappers.
4. **UE Plugin Skeleton**
   - Scaffold plugin folder with module classes, build script invoking cargo, and placeholder Blueprints/Content.
   - Implement input forwarding, tick loop, and sample actor replicating Rust-driven NPC state.
5. **Subsystem Migration**
   - Replace networking: UE authoritative server calling Rust simulation.
   - Replace audio: map Rust events to UE MetaSound.
   - Replace UI/inventory: build UMG screens backed by Rust data snapshots.
6. **Asset Conversion & Visual Upgrade**
   - Batch convert meshes to Nanite, integrate Niagara particle systems using Rust event triggers.
   - Port animation sets with Control Rig.
7. **Stabilization & QA**
   - Establish automated integration tests and performance baselines.
   - Update documentation (`SPECS.md`, `README.md`, `AGENTS.md`) with new plugin workflow and guardrails.

## 10. Immediate Next Actions
1. **Roadmap approval & ownership** — Approve this roadmap and create the `ue5-migration` branch dedicated to planning and tooling without blocking active Rust development. *Success criteria*: decision log entry with named engineering, content, and operations leads plus branch provisioning.
2. **Dependency audit execution** — Kick off dependency audit PRs that mark `wgpu`/`winit`/`cpal` usage sites for removal or abstraction. *Success criteria*: merged audit document enumerating every call site with proposed abstraction patterns and effort estimates.
3. **Rust core prototype** — Prototype a `rust/core` static library exposing `mw_state_init/tick/shutdown` and validate linking from a minimal UE (or C) harness via the selected FFI tooling. *Success criteria*: automated test harness producing deterministic tick snapshots stored under CI artifacts.
4. **Asset pipeline experiment** — Begin asset pipeline research for VOX → Nanite conversion (e.g., MagicaVoxel → FBX/GLTF → UE Nanite) while preserving collision data for Rust physics. *Success criteria*: documented pipeline prototype with sample asset conversion and comparison screenshots/metrics.
5. **Documentation scheduling** — Schedule documentation updates once the base FFI layer and plugin skeleton stabilize (per instructions for `SPECS.md`, `README.md`, `AGENTS.md`). *Success criteria*: shared checklist mapping each document update to a milestone and owner within the migration tracker.
