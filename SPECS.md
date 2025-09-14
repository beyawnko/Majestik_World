# SPECS.md

## Purpose

This document serves as the central specification for forking and extending the Veloren codebase to create an enhanced version of the game. It is meant as a complete navigation and reference guide for engineers and automation agents, with direct mapping to Veloren’s existing frameworks, files, and project organization as of September 2025. It outlines planned improvements focusing on maintainability, modular character customization, and enhancements to the rendering pipeline, while preserving the base structure of Veloren’s architecture.

---

## Table of Contents

1. [Project Overview](#1-project-overview)  
2. [Engine, Frameworks, & Dependencies](#2-engine-frameworks-dependencies)  
3. [Project Structure (Key Directories & Files)](#3-project-structure-key-directories-files)  
4. [Core Systems to Extend or Replace](#4-core-systems-to-extend-or-replace)  
5. [Rendering Pipeline Specification](#5-rendering-pipeline-specification)  
6. [Asset Pipeline Specification](#6-asset-pipeline-specification)  
7. [Networking & Multiplayer](#7-networking-multiplayer)  
8. [Build, Tooling, and Workflow](#8-build-tooling-and-workflow)  
9. [Naming, Versioning, and Git Procedures](#9-naming-versioning-and-git-procedures)  
10. [Contribution & Documentation Guidelines](#10-contribution-documentation-guidelines)  
11. [Error Handling Philosophy](#11-error-handling-philosophy)  
12. [Modular Character System Overview](#12-modular-character-system-overview)  
13. [Gameplay System Extensibility Hooks](#13-gameplay-system-extensibility-hooks)  

---

## 1. Project Overview

**Name (placeholder):** Veloren-Fork  
**Goal:**  
- Deliver a visually and technically advanced voxel RPG, upgrading performance, graphics fidelity, and extensibility from Veloren.  
- Improve code maintainability and modularity to support long-term development and easier integration of new features.  
- Introduce a flexible **modular character customization** system, allowing extensive player/NPC appearance personalization and equipment changes.  
- Base all new development on the existing Veloren codebase (as found at [github.com/veloren/veloren/tree/master](https://github.com/veloren/veloren/tree/master)), preserving core architecture while making targeted enhancements.

**Core Technologies:** Rust, wgpu (WebGPU), ECS (Entity-Component-System), PBR (Physically-Based Rendering), advanced procedural world generation, modular asset pipeline.

---

## 2. Engine, Frameworks, & Dependencies

This section enumerates and describes all core technologies, frameworks, and third-party crates used in Veloren (as of September 2025), based directly on up-to-date `Cargo.toml` files and the project organization.

---

### 2.1 Programming Language & Edition
- **Language:** Rust  
- **Edition:** Rust 2024 (as specified by `[workspace.package] edition = "2024"`)

---

### 2.2 Graphics, Windowing, and Audio

- **Graphics API/Backend:**  
  - `wgpu` — Safe, cross-platform graphics abstraction supporting Vulkan, DirectX 12/11, Metal, and OpenGL backends.  
  - **Shaders:** Primarily custom GLSL shaders, compiled and managed via wgpu.
- **Windowing & Events:**  
  - `winit` — Cross-platform window creation and input (mouse/keyboard) event handling for desktop targets.
- **UI Framework:**  
  - `egui` — Immediate mode GUI library for Rust, integrated in the `voxygen/egui/` module for in-game HUD and menus.
- **Audio:**  
  - `rodio` (or an equivalent audio crate) for client-side audio playback (check `client/Cargo.toml` for the specific audio backend).

---

### 2.3 ECS & Game Systems

- **Entity-Component-System:**  
  - `hecs` — Primary ECS crate for game logic (Veloren may use a custom fork of `hecs` under `/common/ecs/` for performance tuning).  
  - `specs` — Older ECS crate used in certain simulation or legacy systems (patched version likely used for specific subsystems).
- **Physics:**  
  - `rapier` (or possibly a custom physics engine) for 3D rigid body physics and collision simulation.
- **Math & Vectors:**  
  - `vek` — High-performance math library (vector/matrix types) for game and graphics calculations.

---

### 2.4 Asset & Data Management

- **Serialization:**  
  - `serde` + `bincode` — Used for fast serialization/deserialization of game data (binary and JSON/RON as needed).
- **Textures & Image I/O:**  
  - `image` — Handles loading and processing PNG, JPEG, and other image formats for textures and sprites.
- **Config & Save Formats:**  
  - `ron`, `toml` — Human-editable configuration (e.g., game settings, world data) and possibly save game or asset manifests.

---

### 2.5 Networking

- **General Networking:**  
  - Custom UDP-based protocol for real-time gameplay, optimized for low latency.  
  - `tokio` for async networking tasks and runtime.  
  - `quinn` (QUIC over UDP) for cases requiring reliable ordered streams (e.g. world state sync or large data transfer) in `/common/net/` and `/server/`.
- **Serialization/Protocol:**  
  - `bincode` for compact, fast binary encoding of network packets and messages.

---

### 2.6 Build, Tooling, and Miscellaneous

- **Parallelism & Concurrency:**  
  - `rayon` — for parallel iteration and data processing on multi-core CPUs.  
  - `crossbeam` — for lock-free data structures, channels, and thread coordination.
- **Profiling & Metrics:**  
  - `criterion` — used for benchmarking critical code to ensure performance regressions are caught.  
  - `prometheus` + `prometheus-hyper` — for exposing runtime metrics (especially on the server) for monitoring.
- **Randomness:**  
  - `rand` (with `rand_chacha`) — for RNG needs in procedural generation and gameplay randomness.
- **Databases:**  
  - `rusqlite` — lightweight SQLite database for persistent world data or player data storage.
- **Utilities:**  
  - `clap` — command-line argument parsing (for server/client CLI options and tools).  
  - `regex` — for advanced text parsing needs (possibly in chat commands or data import).

---

### 2.7 Project & Dependency Management

- **Rust Workspace:**  
  - The project is organized as a Cargo workspace (root `Cargo.toml` lists all member crates and common build profiles). This allows all sub-crates (client, server, common, etc.) to be built together and share versions.
- **Nix Support:**  
  - A `/nix/` directory exists with Nix configuration for a reproducible development environment. Developers can use `nix-shell` to get all required system dependencies for building the project.

---

### 2.8 Key Reference Files

- Key workspace manifest: the `[workspace]` section in the root `/Cargo.toml` (lists member crates and global build profiles).  
- Per-crate manifests: `/client/Cargo.toml`, `/server/Cargo.toml`, `/voxygen/Cargo.toml` define dependencies and feature flags specific to each major component.  
- Shared code directories: `/common/`, `/plugin/`, `/network/` contain code and dependencies shared across client and server (e.g., ECS components, protocol definitions, plugin interface).

---

#### Example: Graphics Crate Dependencies (`voxygen/Cargo.toml`)
```toml
[dependencies]
wgpu = "0.20"                        # Main graphics abstraction  
winit = "0.29"  
egui = "0.26"  
vek = "0.17"  
image = "0.25"  

# ... and more as detailed above
```
> *Update exact versions in future as dependencies evolve; check each `[dependencies]` section for up-to-date versions and feature lists.*

---

### 2.9 Upstream/Patched Dependencies

- In some cases, Veloren uses patched versions of crates from non-standard repositories to enable custom features or fixes. For example, the ECS crate `specs` might be pulled from a Git revision instead of crates.io:  
  ```toml
  [patch.crates-io]
  specs = { git = "https://github.com/amethyst/specs.git", rev = "<commit-hash>" }
  ```  
  Always check the `Cargo.toml` for `[patch.crates-io]` entries to understand if a crate is using a forked version.

---

### 2.10 Build Profiles

- The root `/Cargo.toml` defines custom build profiles (in addition to default dev and release). Profiles like `dev` (fast compile), `release` (optimized), and others such as `debuginfo`, `no_overflow`, or `thin-lto` are configured to balance speed and safety. For instance, a `no_overflow` profile might enable integer overflow checks even in an optimized build. A `thin LTO` profile might be used to slim down binary size. Adjust these profiles as needed for the fork’s requirements.

---

All framework and dependency details in this section are synchronized with the latest Veloren codebase, ensuring a meticulous, transparent, and future-proof baseline for the fork and its ongoing enhancements.

---

## 3. Project Structure (Key Directories & Files)

**Top-Level Layout:** Overview of the primary project directories and important files.

- `/assets/` — Game data assets (art, voxel models, textures, music, sound effects, localization files, etc.).  
- `/client/` — Client-side game application (the game client executable logic).  
- `/server/` — Server-side game application (game server logic for multiplayer).  
- `/common/` — Shared code library (common data types, utilities, and network protocol definitions used by client and server).  
- `/plugin/` — Plugin/mod support framework (WASM plugin runtime and API for extending game functionality).  
- `/voxygen/` — Main client engine crate (graphics rendering, world display, input handling, and UI integration for the client).  
  - `src/` — Source code of the Voxygen engine:
    - `render/` — Rendering subsystem directory:  
      - `mod.rs` — Render pipeline orchestrator (entry point for rendering, sets up frame rendering sequence).  
      - `renderer/` — Core renderer module (manages render passes, wgpu context, pipelines).  
      - `mesh.rs` / `model.rs` — Data structures and logic for meshes and models (voxel model handling, mesh generation, drawing routines).  
      - `texture.rs` — Texture loading and management (including streaming textures in/out of memory).  
      - `instances.rs` — GPU instancing management (batching of instance data for drawing many similar objects).  
      - `shaders/` — Shader programs (GLSL files or embedded shader code) for various rendering stages.  
    - `egui/` — UI subsystem integration (the client’s UI code using Egui for HUD, menus, etc.).  
- `/nix/` — Nix package manager configurations (for setting up a consistent development environment).  
- `README.md` — High-level introduction, build instructions, and quick start guide for the project.  
- `Cargo.toml` / `Cargo.lock` — Rust workspace manifest and lockfile (lists all crates and exact versions of dependencies).  
- `.github/workflows/` — Continuous Integration (CI) pipeline definitions (GitHub Actions workflows for building, testing, etc.).

---

## 4. Core Systems to Extend or Replace

This section outlines the core areas of Veloren’s engine that we plan to enhance, refactor, or replace in the fork. These targets are chosen to improve performance, maintainability, and extensibility:

- **Rendering:** Refactor the rendering engine to enable batched draw calls and instanced rendering, utilize wgpu render bundles (pre-recorded command buffers), implement GPU-driven culling (compute shader based frustum/occlusion culling), and add enhanced post-processing effects (see [Section 5](#5-rendering-pipeline-specification) for details). This will modernize the graphics pipeline for better performance and visuals.  
- **Asset Pipeline:** Add support for meshlet-based mesh processing, a formal asset registration system, and background streaming of high-detail assets. This means models will be broken into meshlets for GPU efficiency, and assets (textures, etc.) can load on-demand with prioritization.  
- **Worldgen:** Maintain a flexible and extensible world generation pipeline, allowing easy addition of new biomes, structures, or world features (potentially via mods). The procedural world generator should be structured so that developers can plug in new generators or parameters without rewriting core logic.  
- **Entities (ECS):** Upgrade the ECS storage and architecture for better performance and flexibility. This may involve switching to a more performant ECS crate or optimizing the existing one. We aim to enable **runtime-addable components** (dynamically introducing new component types or adding components to entities at runtime) and ensure all components are serializable for saving/loading game state. This will support modding scenarios and complex runtime behavior changes.  
- **Networking:** Update and optimize the network protocol as needed to improve bandwidth usage and latency handling. If new entity types or game mechanics are added, extend the protocol to support them (with proper versioning). Also, consider third-party connectivity needs (such as integration with external services or bridging servers). Maintaining backward compatibility with the base protocol is not required since this is a fork, but the fork’s client and server must remain in sync on any protocol changes.  
- **Characters/Avatars:** Overhaul the character subsystem to support a **modular character customization** system. This includes refactoring how player/NPC models are composed (e.g., separating equipment and cosmetic layers) and ensuring the character creation process is data-driven. The goal is to allow swapping of body parts, armor, clothing, etc., at runtime in a maintainable way (see [Section 12](#12-modular-character-system-overview)).  
- **Modding & Plugins:** Expand the plugin/modding system. Veloren’s current plugin support is limited; we will broaden the **WASM plugin API** and scripting hooks to cover more gameplay aspects (events, UI, AI, etc.). This involves creating clear extension points in the engine where external modules can hook in (see [Section 13](#13-gameplay-system-extensibility-hooks)). By doing so, third-party developers or automation agents can add new content and mechanics without modifying the core engine.

---

## 5. Rendering Pipeline Specification

**Required features:**
- Physically-Based Rendering (PBR) material pipeline (supporting multiple texture maps per material: e.g., albedo, normal, roughness, metalness, ambient occlusion, clearcoat, detail maps).
- Real-time dynamic lighting and baked/static lighting, with support for multiple shadow-casting lights and multi-cascade shadow maps for sun shadows.
- Modern rendering architecture using either a multi-pass **Deferred** shading pipeline or a **Forward+** (clustered forward) pipeline, leveraging clustered light culling for many lights.
- Instanced drawing of repeated objects (vegetation, props, etc.) to reduce draw call overhead.
- GPU-based frustum and occlusion culling (using compute shaders to cull objects on the GPU before drawing).
- Advanced anti-aliasing options: TAA (Temporal AA) for high quality or FXAA for lower cost, configurable by the user.
- A modular post-processing stack (bloom, screen-space reflections (SSR), depth-of-field (DOF), tone mapping, color grading, etc.) that can be enabled/disabled or reordered easily.
- Volumetric effects such as volumetric clouds, fog, and light shafts (god rays) to enhance atmosphere.
- Particle effects and other VFX simulated or computed on the GPU for performance (e.g., GPU particles for magic effects, weather).
- Texture streaming and Level of Detail (LOD) support for both meshes and textures (mipmapped textures, simplified models at distance) to maintain performance in large scenes.
- HDR rendering pipeline with proper tone mapping to SDR output, allowing high dynamic range lighting.
- Profiling hooks in each render stage (to measure frame time distribution and help optimize bottlenecks).

**Reference Implementation:**
- Primary rendering code entry point: `/voxygen/src/render/mod.rs`  
- Core mesh handling: `/voxygen/src/render/mesh.rs`  
- Renderer logic and passes: `/voxygen/src/render/renderer/`  
- Instancing utility: `/voxygen/src/render/instances.rs`  
- Shaders: `/voxygen/src/render/shaders/` (GLSL shader programs used by the engine)

### 5.1 Rendering Pipeline Flow

To meet the above requirements, the rendering pipeline will be structured in clear stages. The following describes an **example Forward+ pipeline** (clustered forward rendering) that the engine could use:

1. **Shadow Map Pass:** At the start of each frame, render shadow depth maps for all significant light sources. This typically includes the sun (directional light) with cascaded shadow maps (covering near to far view frustum slices) and any major dynamic lights (e.g., a bright torch or spell effect) that cast shadows. These depth textures will be used later when lighting the scene to produce correct shadows.  
2. **Depth Pre-Pass & Clustering:** Render all opaque geometry to produce a scene depth buffer (writing only depths, no color). This pre-pass optimizes later shading by allowing early z-cull of hidden fragments. After the depth pre-pass, run a compute shader to divide the camera’s view frustum into a 3D grid of clusters (tiles spanning screen space and depth) and determine which lights affect each cluster. Each cluster will get a list of lights (by ID or index) that need to be considered for objects inside that cluster. This clustered light list is stored for use during the main lighting pass.  
3. **Opaque Geometry & Lighting Pass:** Render all opaque objects again in a single geometry pass, this time with full lighting and materials. In a Forward+ approach, the fragment shader will use the cluster light lists to efficiently compute lighting: for each fragment, it finds the cluster it belongs to and iterates only over the lights influencing that cluster (rather than all scene lights). This applies PBR shading using the material’s textures (albedo, normal, roughness, etc.), and uses the shadow maps from step 1 to shadow appropriate lights. The output of this stage is an HDR image with all opaque geometry lit. (If a **Deferred** path were used instead, this stage would be split into a G-buffer pass and a separate lighting pass – see note below.)  
4. **Transparent & Particle Pass:** Render transparent objects and particle effects in one or more passes. Transparent objects (water, glass, etc.) cannot be handled in deferred shading easily, so even in a deferred pipeline they’d be drawn in a forward manner. Sort transparent geometry roughly from back-to-front and render it using the available lighting (in Forward+, transparent fragments can still use clustered lighting). Particle systems (e.g., fire, smoke) that are GPU-driven can also be rendered in this stage. They may use simpler shading or additive blending. This pass writes to the same HDR buffer, blending with the opaque image from step 3.  
5. **Post-Processing:** Apply the sequence of post-processing effects on the HDR scene image. This can include bloom (filter bright areas and blur to add glow), screen-space reflections (SSR) for reflective surfaces, ambient occlusion (SSAO) if implemented, depth of field blur, motion blur, etc., as configured. These are done in a controlled order (for example, HDR bloom before tone mapping). Each effect reads the image (and possibly depth buffer) and writes back into the image or a new buffer, chaining the results. Finally, apply tone mapping and color grading to convert the HDR image to SDR output format, and apply the chosen anti-aliasing (TAA or FXAA) to smooth jagged edges.  
6. **UI & Final Composition:** In the final step, render the 2D user interface on top of the scene. This involves drawing Egui UI elements (HUD, menus) by compositing them over the now-tone-mapped frame. Because the UI is rendered in screen space, it is drawn last. Once UI rendering is complete, the fully composed frame is presented to the screen (swap chain present). Input for the next frame is processed while the GPU works on the current frame, maintaining a pipeline of CPU-GPU work.

**Note:** We are evaluating both a Forward+ (clustered forward) and a Deferred shading approach for the final implementation. In a deferred pipeline, the flow would differ by first rendering a G-buffer (storing material properties per pixel) and then doing a lighting pass that reads that G-buffer to calculate lighting, after which transparencies are handled in a forward manner. Both approaches leverage clustered light culling for efficiency. Forward+ tends to be simpler in terms of not needing multiple buffers, while deferred can simplify managing many lights at the cost of more memory bandwidth. We will prototype and profile both. Regardless of the approach, the engine will meet the same feature requirements (multiple dynamic lights, shadows, post-effects, etc.), and the architecture is designed such that switching between Forward+ and Deferred is possible if needed.

**Reference Implementation:** For context, Veloren’s existing renderer code can be found in the `voxygen` crate (see files listed above). The new pipeline will be implemented by modifying or extending those files, or replacing them with a new rendering module, while keeping in mind integration with the rest of the engine (ECS, asset system, etc.).

---

## 6. Asset Pipeline Specification

The asset pipeline covers how game assets (models, textures, sounds, etc.) are imported, processed, and managed at runtime. Key points of the asset pipeline in the fork:

- **Asset Directory Structure:** All static assets reside in the `/assets/` directory (as in Veloren). Within this directory, assets are organized by category (for example, there may be sub-folders for `models/` (voxel model files), `textures/` (images), `audio/` (sound effects and music), etc.). The fork will maintain this structure for compatibility and easy discovery. New assets should be added to the appropriate subdirectory and referenced in code or data by their path.  
- **Meshlet Conversion:** 3D models must be converted to a **meshlet** structure for the planned GPU-optimized rendering. A *meshlet* is a small cluster of triangles (or voxels) that can be processed efficiently by modern GPUs. We will introduce an offline preprocessing step or tool that takes each model (especially complex or high-polygon models) and subdivides it into meshlets (for example, using mesh optimization techniques inspired by Nvidia’s Mesh Shading pipeline). These meshlets are stored (perhaps in a custom binary format or within the asset metadata) so that at runtime the engine can perform culling and rendering per-meshlet, greatly reducing the vertex shader workload for large models. (For voxel models, meshlet generation might involve grouping voxels into chunks or simplifying geometry.) This preprocessing will integrate with the build pipeline or asset loading stage.  
- **Texture Mipmaps & Streaming:** All texture assets should include pre-generated mipmaps. Mipmaps (multiple resolution levels of the texture) will either be generated at asset import time or provided in the asset package. The engine’s asset manager will **register textures with a streaming system**, assigning each texture a priority or LOD group. “Registering” a texture means the asset pipeline knows about its existence and importance (e.g., GUI icons might be high priority to load at full resolution, whereas a terrain texture far away might be lower priority). At runtime, a background loader will load higher-resolution mipmaps of textures as needed based on camera distance or other heuristics, and unload or use lower mipmaps for far or unseen textures to save memory. This ensures that detailed textures are only used when they matter, preventing stalls.  
- **Audio and Effects:** Audio files (sound effects, music) and other media are handled via Rust-native loaders (e.g., `rodio` for audio decoding). The asset pipeline will treat these similarly to Veloren: as soon as an audio asset is needed (for example, a sound effect for a sword swing), it will be loaded either from a packed resource or disk. We will continue to support common formats like OGG for compressed audio. There isn’t extensive preprocessing needed for audio beyond ensuring files are included and their paths are known to the game (some could be packaged into a pack file). The existing system in Veloren is likely sufficient, but we will document any custom steps if, say, audio is to be streamed or volume-normalized.

*Note:* Veloren’s asset formats and tools will remain in use. For instance, Veloren uses MagicaVoxel `.vox` files for 3D models (characters, items), PNG images for textures and sprites, OGG files for audio, and RON/TOML files for configuration and item definitions. Our fork will retain support for these formats to leverage the existing asset library. We will layer the new meshlet and streaming mechanisms on top of this. When adding new assets, developers must ensure the game is aware of them (through either hardcoded references or data files). Missing asset references are a common source of runtime errors, so an asset manifest may be introduced to list all expected assets and validate their presence on load.

---

## 7. Networking & Multiplayer

- **Protocol and Performance:** The fork will maintain and enhance Veloren’s UDP-based multiplayer networking model. This involves keeping an authoritative server model with clients sending inputs and receiving world updates. We will continue using UDP for its low-latency benefits, augmented by **QUIC** (via the `quinn` crate) for reliability where needed (e.g., important messages that must arrive in order). The networking stack will be optimized to handle higher update rates and more players/NPCs if possible, by refining the data replication logic and culling (only send relevant data to each client).  
- **Protocol Evolution & Documentation:** If we add new entity types, world events, or other gameplay features, the network protocol will be extended accordingly. All protocol changes will be documented clearly (e.g., in a protocol specification document). We will use versioning for the protocol: the client and server will perform a version handshake on connection to prevent mismatched versions. This means if the protocol is updated (breaking compatibility), older clients will be cleanly rejected with a message rather than causing undefined behavior. Every significant network message (packets for movement, combat, chat, etc.) will be described in documentation to aid debugging and future modifications.  
- **Robustness & Security:** The network code will be built with security and stability in mind. The server will validate inputs from clients (e.g., movement commands will be checked against physical constraints to prevent speed hacks or teleporting). Malformed or unexpected packets will be handled gracefully (the server will ignore or log and not crash). We will also consider basic anti-cheat or anti-abuse measures where possible given an open-source environment (for example, rate-limiting certain actions from clients to prevent denial-of-service). Additionally, integration with third-party network services (such as metrics or community servers list) can be improved as needed, but core gameplay traffic remains peer-to-peer (client-server).  
- **Reference:** Relevant code is in `/common/network/` (shared networking library) and `/server/` (server application logic). The fork’s networking will largely reuse Veloren’s structure with the above enhancements. Any new networking utility (like encryption, if considered, or improved compression) will be added in these modules.

---

## 8. Build, Tooling, and Workflow

**Tooling:**
- All builds are managed via **Cargo** (the Rust package manager). Developers can build each crate (client, server, etc.) or the whole workspace with standard Cargo commands. We will maintain helpful Cargo aliases or Makefile tasks if needed (for example, a `cargo run-client` vs `cargo run-server` if those are set up).  
- A **Nix** environment is provided (in the `/nix/` directory) for those who want deterministic builds or are on systems that can leverage Nix. Using `nix-shell` will load a development environment with all necessary system dependencies (like the exact compiler version, SDL libraries if needed by audio, etc.). This is supported but not strictly required; non-Nix users can install dependencies manually as described in the README.  
- **Rust Toolchain:** The project currently targets a specific Rust toolchain. Veloren often requires **Rust nightly** (due to using unstable features like maybe `generic_const_exprs` or others). We will include a `rust-toolchain.toml` file to pin the recommended toolchain (e.g., nightly-2025-09-01) so contributors automatically use the correct compiler. Our long-term aim is to migrate to stable Rust if possible (as Rust stabilizes features we need), to lower the barrier for contributors. Until then, contributors should use the nightly toolchain specified.  
- **Code Formatting & Linting:** We use `rustfmt` for automatic code formatting and `clippy` for linting. All code should be formatted according to the standard Rust style (enforced by CI), and should not produce any critical clippy warnings. This ensures a consistent codebase style and catches common mistakes early. Developers should run `cargo fmt` and `cargo clippy` before commits; the CI pipeline will reject commits that do not pass these checks.

**Hot-reload:**
- We plan to support hot-reloading of certain assets to speed up development iterations. Specifically, shader code and game assets will be watchable for changes. During development, if a shader file in `/assets/shaders/` (or a source snippet in the code) is changed, the client can detect this and recompile or reload that shader without restarting the game. Similarly, for assets like textures or models, an in-editor or in-engine “asset watch” could be enabled to reload an asset file on disk when it changes. This will be implemented using a file-watching crate (like `notify`) and careful reinitialization of GPU resources. (Hot-reloading might be disabled or limited in release builds for stability.)

**CI/CD:**
- Continuous Integration is set up via workflows in `.github/workflows/`. Each pull request or push will trigger automated build and test jobs. The CI ensures the project builds on all supported platforms (likely Windows, Linux, MacOS) and that all tests pass. It also runs the formatter and linter to enforce code quality (failing the build if code is misformatted or has serious lint issues). We may integrate additional CI steps such as running the game’s test suite (if integration tests exist), checking for outdated dependencies, etc.  
- For releases (Continuous Deployment), we will use git tags and GitHub release artifacts. Semantic versioning will be followed (see Section 9), and the CI can automatically build release binaries for distribution when a new tag is pushed.

---

## 9. Naming, Versioning, and Git Procedures

- **Project and Crate Naming:** Since this is a fork of Veloren, the project must use a new unique name to avoid confusion. All relevant crates should be renamed in their `Cargo.toml` to reflect the new project name. For example, if the fork is called *Astral*, the crate `veloren-common` might become `astral-common`, `veloren-voxygen` -> `astral-voxygen`, etc. This prevents conflicts with original Veloren crates and clearly distinguishes our fork in logs and backtraces. Any user-facing references to Veloren (in the UI, documentation) should also be updated to use the new name, while giving proper credit to the original project where appropriate.  
- **Semantic Versioning:** The fork will adopt semantic versioning for its releases (e.g., 0.1.0, 0.2.0, ... until a stable 1.0.0). Since it’s a new project, we may start at 0.1.0 for the initial forked release. Each new release that adds features will increment the minor version, bug fixes increment the patch, and any potentially incompatible changes increment the major version (pre-1.0, we’ll treat minor bumps as possibly breaking). This versioning applies to each crate as well as the overall game.  
- **Git Workflow:** We will use a modern git branching strategy to manage development. Feature development should occur in feature branches (named by feature or issue, e.g., `feature/render-refactor`). When ready, a pull request (PR) will be opened to merge into the main branch. We prefer **squash merging** to keep the main history clean (each feature becomes one commit). The repository includes templates for PR descriptions and issues (see `.github/` directory) to ensure all contributions are well-documented. Before merging, all commits should pass CI checks. Code reviews by maintainers or automation agents are required for quality control.  
- **Changelog and References:** All changes, especially those generated or suggested by automation agents, must be recorded in the `CHANGELOG.md`. When an agent or developer makes a change that implements part of this specification, the commit or PR description should reference the relevant section of `SPECS.md` (for example, “Implemented meshlet processing as per Section 6. Asset Pipeline Specification”). This cross-linking ensures traceability from spec to implementation. The changelog will include entries for each release highlighting new features, changes, and bug fixes, so testers and players know what’s updated.

---

## 10. Contribution & Documentation Guidelines

- **Code Documentation:** All public types, functions, and modules must include Rustdoc comments or Doxygen-style comments explaining their purpose and usage. We aim for thorough documentation so that anyone (or any AI agent) reading the code can understand it without guessing. Major subsystems (Rendering, Worldgen, ECS, Networking, etc.) should also have overview documentation in the `/docs/` directory. For example, if we overhaul the rendering, a document like `docs/RENDERING.md` should describe the new pipeline at a high level.  
- **Module Guides:** In addition to API docs, we will maintain high-level guides for complex systems. Veloren already has a “Book” for contributors; similarly, this fork will keep developer guides (in `/docs/`) updated. For instance, if a new **modular character system** is implemented, a guide explaining its design (and how to add new character assets or options) should be added. These guides make it easier for new contributors to get up to speed.  
- **Contributing Guidelines:** The `CONTRIBUTING.md` file (and similarly the `README.md`) will be updated to reflect the fork’s processes. This includes how to set up the development environment (mentioning Rust nightly, Nix, etc.), the coding style (we follow rustfmt/clippy as mentioned), and how to run tests. New contributors should read this for a smooth start. We will also include information about how to respect this spec when contributing (e.g., ensuring features align with the specification or discussing spec changes in issues).  
- **Testing:** Wherever feasible, contributions should include tests. For any new gameplay system or engine feature, unit tests or integration tests help prevent regressions. For example, if adding a new inventory system, include tests for adding/removing items. Our CI will run these tests on each PR. We also encourage playtesting for features that can’t be easily unit-tested (document steps for manual testing in the PR if needed).  
- **Style & Quality:** We have automated style checks (rustfmt, clippy) as noted. Contributors should also strive to match the existing code style in areas not covered by rustfmt (like module organization, naming conventions). Significant code contributions should be accompanied by an update to this `SPECS.md` if they deviate from or add to the specification. In other words, the spec and the code should remain in sync.  
- **Changelog Updates:** When contributing a user-facing feature or a major change, update the `CHANGELOG.md` under the “Unreleased” section (or appropriate version heading) with a brief description. This keeps the project history transparent. For larger changes, consider also writing a brief design note or using the `/docs/NEW-FEATURE.md` template to describe the rationale and usage of the feature.

---

## 11. Error Handling Philosophy

Robust error handling is critical for maintainability. The project adopts a philosophy of **never crashing or panicking in production due to predictable errors** – instead, errors are propagated or handled gracefully. Key guidelines include:

- **Prefer Results over Panics:** Use `Result<T, E>` (or `Option<T>`) to represent recoverable errors, and propagate them to calling code using the `?` operator. Library and engine code should avoid calling `panic!()` or using `.unwrap()`/`.expect()` on Results. Panics should be reserved for truly unrecoverable situations (e.g., memory corruption, impossible logic states) or during development to catch bugs. Even in those cases, using `debug_assert!` or logging an error is preferred in release builds so that the server/client attempts to continue running.  
- **No Silent Failures:** Do not ignore error return values. Every `Result` that arises must be either handled or propagated. For example, if a file load fails, handle it (maybe try a default asset or notify the user) or propagate the error up to a context that can handle it. Code like `let _ = some_result;` is discouraged unless there is a very good reason (and in such case, comment why it’s safe to ignore). This prevents hidden issues and makes debugging easier.  
- **Contextual Logging:** When an error is caught and cannot be propagated further (for instance, at the top of a thread or in the main game loop), it should be logged with enough context to diagnose the problem. We use the `log` crate macros (`error!`, `warn!`, etc.) to record these events. E.g., if loading a texture fails, log which texture and why (include the `io::Error` message). This way, if something goes wrong in production or during automated testing, we have breadcrumbs to investigate. Logs should be user-friendly for known error cases (e.g., “Failed to connect to server – host unreachable”), and highly technical for unexpected ones (including backtrace or error codes).  
- **Graceful Degradation:** Wherever possible, the game will degrade gracefully on errors rather than abort. If an asset is missing, we might substitute a placeholder model/texture rather than crashing the renderer. If the server encounters a malformed packet or a plugin error, it should catch that, log it, and isolate the issue (e.g., drop that packet or disable that plugin) while keeping the rest of the game running. The idea is to keep the game/server up even if some subsystems encounter issues. Critical errors that make the game unplayable (like “failed to initialize graphics device”) will of course be reported and cause shutdown, but those are rare and happen early.  
- **Unified Error Types:** We will introduce structured error types for subsystems using libraries like `thiserror` for easy implementation of `Error`. For example, a `AssetError` enum might classify errors as `AssetError::NotFound(path)`, `AssetError::ParseFailed(path, source)`, etc. This allows matching on error kinds and handling them programmatically if needed. At subsystem boundaries, we might convert errors into a higher-level error (using the `From` trait or `.map_err()`) to avoid leaking low-level errors upward without context. When propagating errors up, attach context strings (`anyhow` or manual error messages) so that at the top level we know **what** failed and **why**.  
- **Testing and Debugging:** Our error handling approach will be verified by writing tests for failure scenarios. For instance, we can test that if a required config file is missing, the game doesn’t panic but instead uses defaults and logs a warning. We also use Rust’s backtrace support: in debug builds, enabling `RUST_BACKTRACE=1` will be recommended when running the game to get backtraces for any panic. We ensure that our custom error types implement `std::error::Error` and provide sources, so if an error bubbles up to an anyhow, the backtrace and cause chain are intact.  
- **Avoiding Gotchas:** Developers should be cautious with certain Rust behaviors – for example, integer overflow in release builds (we might use the `overflow-checks = true` in critical profiles to catch math bugs), and be careful with `.expect()` in threads (since a panic there can terminate the whole program if not caught). We set up global panic hooks in the client and server to log panics (with stack traces) so that even in unexpected crashes we have insight. 

By following these practices, we aim to make the fork’s codebase robust against errors, easier to debug, and safe for automation agents to work with (since errors will manifest as explicit `Result` values or logged messages rather than unpredictable crashes).

---

## 12. Modular Character System Overview

One of the major enhancements in this fork is a **modular character customization system**. The goal is to allow dynamic, data-driven customization of player and NPC appearance (species, body parts, clothing, armor, etc.) and to make the character-related code more maintainable. In Veloren, character models and their equipment were relatively static or handled with hardcoded logic. We plan to redesign this as follows:

- **Multi-Part Character Models:** Characters will be composed of multiple model parts rather than a single monolithic model. For example, a player character might consist of a base body model plus separate models for headgear, chest armor, pants, gloves, boots, and perhaps attachments like backpacks or weapons. Each part is an individual voxel model asset aligned to a common **character skeleton template**. Veloren already uses a character template to ensure armor fits on characters; we will formalize this. All species (human, orc, etc.) will share a compatible rig structure or have a defined mapping so that equipment can be shared across them. This means adding a new armor piece or clothing item is as simple as creating a new voxel model for that slot – the game will attach it to the character at runtime. The base model plus its attachments are combined for rendering. This modular approach greatly increases the variety of appearances and makes extending content easier.  
- **ECS Representation:** In the ECS, each character entity will have components that define its appearance and equipment. For instance, a `BodyComponent` might specify base race/gender and body model, and an `EquipmentComponent` could hold a list of equipped item identifiers or model references (helmet model, chest model, etc.). There may also be a `SkeletonComponent` that holds the skeletal pose/animation state for that character. Systems will be responsible for constructing the final rendered mesh from these pieces. For example, a **CharacterRenderSystem** will take an entity’s body model and all attached equipment models and produce a combined mesh or draw commands for rendering. Thanks to ECS, we can add or remove equipment components at runtime to change a character’s appearance (e.g., when a player equips a new sword, a `WeaponComponent` with the sword’s model is added). This dynamic aspect is directly supported by ECS – adding/removing components on an entity at runtime will alter its behavior/appearance. Serialization of characters (for saving game or sending over network) will include all these components so that a character’s full appearance is reconstructable.  
- **Rigging and Animation:** All character parts share a common animation rig (skeleton). Veloren uses a skeletal animation system for voxel models (each part of the model can be assigned to a bone and animated). In our modular system, the base body and all equipment pieces must be rigged to the same set of bones or at least have transforms that follow the base skeleton. For example, the “hand” bone moves, and the glove model attached to the hand moves with it. We will either use a parent-child relationship (equipments are child objects of the body in the scene graph following the bone transforms) or merge the meshes with bone weights. The key is that an animation (running, jumping, attack swing, etc.) will apply uniformly. We will provide guidelines for asset creators: e.g., any armor model should be created using the character template rig, so that when placed on a character it lines up correctly. In implementation, when an animation plays, the system updates the skeleton pose (joint matrices) which is then applied to all sub-meshes. This might involve updating uniform buffers for each mesh or using a unified skeleton uniform for all parts of a character.  
- **Customization Options:** The system will expose many customization options in a data-driven way. Players might be able to choose species (human, dwarf, etc.), gender/body type, hair style, hair color, skin color, and starting clothes. These options will be defined in configuration files (for example, a `characters.ron` that lists all valid species and the corresponding model to use, available hair models, etc.). During character creation, the UI will allow cycling through these options, and it will simply assign the appropriate components to the character entity based on selection. For instance, picking a hair style adds a `HairComponent` that references a hair model asset. Color choices could be implemented by variant textures or palette swaps — since voxel models often use a palette, we could decide certain palette indices are customizable colors. Our documentation will clarify how to add new customization options: because it’s data-driven, adding a new hair style might be as easy as dropping the `.vox` file in `assets/models/hair/` and updating the config, without touching Rust code.  
- **Content Extensibility:** This modular approach means adding new character-related content is straightforward and doesn’t require engine changes. For example, a contributor (or modder) could introduce a new armor set. They would create voxel models for each armor piece (helmet, chest, legs, etc.) following the scale and rig conventions, place them in the assets, and update item definitions (so the game knows an item “Iron Helmet” uses that model) and perhaps the character customization options if it should be selectable. The engine will automatically handle equipping it: when an entity equips “Iron Helmet,” the code will add the appropriate helmet model to that entity’s equipment component and it will appear. We aim to minimize assumptions in code about specific gear – instead of hardcoding “if wearing X, do Y,” we’ll rely on data tags and maybe scriptable effects (for example, an armor could add a “FireResistanceComponent” to the entity when equipped, rather than game logic having a special case for that armor). This makes the system flexible and open for extension by mods.  
- **Potential Gotchas & Solutions:** In a modular system, one challenge is performance – drawing a character composed of 10 separate models could increase draw calls or matrix palette updates. We will mitigate this by **batching** character sub-meshes where possible. For instance, we might merge the base body and all armor into one GPU mesh at load time or in a background thread, especially for NPCs where customization doesn’t change often. Another issue is ensuring all parts align correctly: if the base model rig changes (say we adjust arm length), all associated items must be updated. We will version the character rig and document it, so any model targeting that rig version is compatible. We’ll likely keep compatibility with Veloren’s existing rig initially to reuse assets. Also, tools for artists (like a template Blender or MagicaVoxel file with layers for each slot) will be provided to reduce alignment errors. In summary, careful asset guidelines and perhaps validation tooling (to check model alignment) will be part of this effort. When done, the result will be a highly **maintainable and extensible character system**: adding content is easy, and code-wise it’s mainly about assembling components, which is clear and less error-prone than large if/else logic.

---

## 13. Gameplay System Extensibility Hooks

To support modders and future development, the fork will include explicit hooks and extension points throughout the gameplay systems. The idea is to make it possible to extend or customize game logic without modifying the core engine, through plugins or configuration, wherever feasible. This benefits both human developers and automation agents that might generate new game content. The extensibility strategy includes:

- **WASM Plugin API Expansion:** Veloren introduced a WebAssembly-based plugin system for server and client mods. Currently, this API is limited (only a few event types). We will significantly **extend the plugin API**. Concretely, we’ll expose more game events and data to plugins: e.g., player join/leave events, entity spawn/despawn, damage events, inventory change events, world generation hooks, etc., and allow plugins to perform a wider range of actions (spawning entities, modifying stats, triggering custom UI). The plugin runtime (`veloren-plugin-rt` crate) will be updated to include these new hooks. We’ll also ensure the plugin API is documented and versioned. The sandbox nature of WASM will be retained (plugins run in a sandbox for safety), but with more capabilities. Our goal is that many gameplay changes could be done as an *optional plugin* rather than a fork – for example, a total conversion mod could add new items and quests via plugins and data, without touching core code. This also means internal features might sometimes be implemented using the same API (dogfooding it to ensure it’s sufficient). The plugin system will manage synchronization: server-side plugins can send data or invoke effects on clients (with permission). We also plan to support the **server distributing required plugins to clients** on connect for seamless modded server experience (so if you join a modded server, your client can automatically load the mod plugin).  
- **Game Event Hooks:** Within the core engine, we will implement an event dispatcher or observer pattern for key game events. This is useful both for plugins and for the core game code to remain decoupled. For example, events could include “EntityDamaged”, “EntityDied”, “PlayerChatMessage”, “DayNightCycleChanged”, etc. Core systems will **emit events** at appropriate times. Other systems or plugins can subscribe to these events. This design means, for instance, a new **achievement system** could subscribe to the “EntityDied” event and check if the player killed a special boss, then grant an achievement – all without altering the combat system that generates the event. Similarly, an AI system could emit an event when an NPC finishes a patrol route, and a plugin could listen to that to maybe spawn an ambush. We will likely implement this with a global or ECS-based event bus. Performance considerations mean we won’t go overboard on events (emitting thousands of events per second), but for game-level events this is fine. Documentation will list all available events and their data payloads.  
- **Data-Driven Gameplay:** We will push more gameplay logic into data definitions to allow tuning and extending without recompiling. Veloren already uses external files for item stats, recipes, etc. We will expand this: for example, **abilities and skills** could be defined in data (a config describes what a skill does in terms of effects, cooldown, etc.). The engine would read this and instantiate skill behaviors accordingly. If an automation agent wants to create a new skill, it could just append a new entry in the skills data file and provide any needed script or effect reference. We might use a lightweight scripting for certain behaviors – for instance, a quest might be scripted in a Lua or RON logic format, or a dialogue could be in JSON. The point is to reduce hardcoded gameplay logic. Another angle is **AI behavior**: potentially allow NPC behavior trees to be defined in data or via a scripting interface, so new AI patterns can be added without Rust coding. We will evaluate adding a simple scripting language (like a Lua interpreter or a Rust mini-language) for certain subsystems if it greatly improves flexibility; however, this must be balanced with performance and complexity. The existing WASM plugin approach might suffice for most needs, so we might expose scripting via that (i.e., writing a plugin in Rust, which gets compiled to WASM, for heavy logic).  
- **Modular ECS Systems:** Our ECS-centric design inherently supports adding new systems. We will organize the game loop (the schedule of ECS systems each frame/tick) to have clear **extension points**. For example, after the core systems update (movement, physics, etc.), we might have a phase where custom systems (from mods or optional features) can run. This could be as simple as allowing plugins to register new systems at startup (for server-side logic changes). We will ensure that ordering and data access of systems are well-defined so that adding a new system doesn’t inadvertently break invariants. For example, we might guarantee that all mod systems run after core physics but before rendering, etc., and restrict them from conflicting with core systems’ data usage without explicit coordination. This way, an agent can introduce a new feature by writing an independent system that observes and modifies components, rather than editing existing system code. It also makes merging upstream changes easier, since fewer core systems need modification for new features.  
- **World Generation Hooks:** Extensibility will also extend to the world generator. We will allow the insertion of new worldgen steps via configuration or code. For instance, if a mod wants to add floating islands in the sky, we could expose a hook in the world generation sequence where a plugin can inject custom terrain manipulation after the base terrain is generated. Perhaps the worldgen uses a pipeline (noise generation -> biome assignment -> structure placement -> etc.); we can let plugins hook at specific points or provide callback registration (e.g., “onChunkGenerated” event). We’ll also make the list of worldgen parameters data-driven (so new biomes or structures can be added by adding entries to a file). The server will coordinate worldgen, so any mod that affects worldgen would ideally run on the server and inform clients (which just receive the final world data). As mentioned, distributing plugins to clients will enable them to handle custom world content smoothly.  
- **Example Use-Cases:** To illustrate, consider adding a new **magic system** via these hooks. We could add new “Spell” items in a data file, define their effects (maybe in data or as plugin code). We add an event hook for “PlayerUsedItem” that plugins can catch. A magic mod plugin catches that event for spell items, then executes custom logic (like apply a fireball effect, spawn particle entities, apply damage to targets). The plugin could also define a new component `Burning` that when present on entities, a modded system reduces health over time – that system runs as part of ECS schedule because the plugin registered it. None of the core engine needed to know about “Burning” or “fireball”; it’s all extensions. The result is a new gameplay feature implemented cleanly alongside the base game. Our fork aims to facilitate this kind of extension for a wide range of gameplay elements, effectively making the engine *mod-friendly and future-proof*.  
- **Stability and Backwards Compatibility:** When extending these systems, we will keep an eye on not breaking existing content. Built-in game features will also use these hooks (for instance, base quests might use the same event system that mods use), to ensure we exercise and maintain them. We’ll version the plugin API such that older plugins can declare a compatibility version. We’ll also write tests for the extensibility layer: e.g., a dummy plugin that registers an event handler to ensure it runs, or a dummy system that adds a component to verify scheduling works. As the project evolves, if we change internal data structures, we’ll update the extension interfaces accordingly, documenting any changes in the plugin API (so mod authors know how to update). Our intent is to cultivate a community (or an ecosystem of automation scripts) that can create content for the game, with our specification and implementation providing a stable foundation for them to build on.

---

## Additional Resources

- Veloren’s development documentation, the [Veloren Book](https://book.veloren.net/), is a valuable reference for understanding the original architecture and reasoning behind systems. Many concepts in this spec build upon or modify what’s described in the Veloren Book.  
- The repository’s own `/README.md` and `/CONTRIBUTING.md` (from Veloren, and updated for this fork) provide baseline project principles, build instructions, and contribution workflows. Contributors should read these for general guidance not duplicated in this spec.  
- The `plan.md` file in the repo root (if present, as in Veloren) or our project board should be used for tracking current tasks, features in progress, and near-term roadmap. It complements this spec by breaking down the implementation steps and assignments in an agile manner.

---

**Note:** Always verify file paths and directory names mentioned in this document, as the upstream Veloren project may change structure over time. This `SPECS.md` is intended to be kept up-to-date as the project evolves – whenever the codebase changes in a way that deviates from the spec (or when new decisions are made), this document should be revised to reflect the new reality. This ensures that engineers and automation agents relying on `SPECS.md` always have an accurate blueprint of the project.
