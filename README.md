# Majesik: World

[![CI](https://github.com/beyawnko/Majestik_World/actions/workflows/ci.yml/badge.svg)](https://github.com/beyawnko/Majestik_World/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

Majesik: World is an evolving rebrand of the open-source multiplayer voxel RPG derived from Veloren.
The project is transitioning toward a new identity and feature set; this README holds placeholders while the game takes shape.

## Project Status

- ⚠️ Active rebranding in progress
- ✨ Feature list and world lore are being drafted

## Building

This repository requires **Rust nightly-2024-05-09** (or a compatible nightly) and tracks large binary assets with [Git LFS](https://git-lfs.com/).
Ensure typical system dependencies for Rust development (e.g., a C compiler and `pkg-config`) are installed.

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

More detailed build notes will be added as systems migrate from the original Veloren codebase.

## Contributing

Contributions of code, art, design, and testing are welcome.
Please review [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) for workflow guidelines.

## Community

Community channels (Discord, forums, etc.) will be announced as the rebrand progresses.
For now, GitHub issues and pull requests are the best way to get involved.

## License

The project continues under the terms of the [GNU GPLv3](LICENSE).
Assets may carry additional licenses noted in their respective directories.

---

_This repository is a work in progress and will change frequently as Majesik: World develops its identity._
