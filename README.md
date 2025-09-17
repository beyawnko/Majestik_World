# Majestik: World

[![CI](https://github.com/beyawnko/Majestik_World/actions/workflows/ci.yml/badge.svg)](https://github.com/beyawnko/Majestik_World/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

Majestik: World is an evolving rebrand of the open-source multiplayer voxel RPG
derived from Veloren.
The project is transitioning toward a new identity and feature set;
this README holds placeholders while the game takes shape.

## Project Status

- ⚠️ Active rebranding in progress
- ✨ Feature list and world lore are being drafted

## Building

> **Rust edition 2024 (experimental):** This project enables `edition2024` in
> `Cargo.toml` and requires a specific nightly toolchain
> (**`nightly-2025-09-14`** or a compatible nightly). Pin the toolchain via
> `rust-toolchain.toml` to ensure reproducible builds:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2025-09-14"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

All crates in the workspace must be compatible with edition 2024.
If you hit dependency issues, try updating crates or file an issue with details.

This repository also tracks large binary assets with [Git LFS](https://git-lfs.com/).
Ensure typical system dependencies for Rust development (e.g., a C compiler
and `pkg-config`) are installed.

```bash
# Install Git LFS hooks for this repository
git lfs install --local

# Build the project
cargo build

# Check formatting
cargo fmt --all -- --check

# Run tests
cargo test --workspace --all-features
```

More detailed build notes will be added as systems migrate from the original
Veloren codebase.

## Contributing

Contributions of code, art, design, and testing are welcome.

Please review [CONTRIBUTING.md](CONTRIBUTING.md) and
[AGENTS.md](AGENTS.md) for workflow guidelines.

## Development

- Toolchain: `nightly-2025-09-14` (see `rust-toolchain.toml`).
- Components: `rustfmt`, `clippy`.
- Branches: `feat/<scope>` for features, `fix/<scope>` for bug fixes,
  `chore/<scope>` for maintenance.
- PRs: include summary, rationale, and tests. CI must pass `fmt`, `clippy`, and `test`.
- Commits: follow Conventional Commits.

## Community

Community channels (Discord, forums, etc.) will be announced as the rebrand progresses.
For now, GitHub issues and pull requests are the best way to get involved.

## License

The project continues under the terms of the [GNU GPLv3](LICENSE).
Assets may carry additional licenses noted in their respective directories.

---

_This repository is a work in progress and will change frequently as
Majestik: World develops its identity._
