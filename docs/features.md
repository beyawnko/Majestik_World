# Features

This document explains how workspace crates configure Cargo features, which combinations are encouraged, and where mutual
exclusions exist for technical reasons.

## Principles

- Prefer additive feature flags so that `--all-features` builds stay healthy across the workspace.
- If two flags cannot be enabled together, guard them with `compile_error!` and list the rationale in this document.
- Keep CI feature matrices in sync with this file so incompatibilities are exercised intentionally instead of accidentally.

## veloren-world

- `simd` (default): enables SIMD accelerated math paths via `vek`.
- `airship_maps`: pulls in the PNG/TinySkia asset pipeline for airship route previews.
- `be-dyn-lib`: builds the crate as the dynamic library that downstream clients load.
- `use-dyn-lib`: links against the dynamic library produced by `be-dyn-lib`.

### Mutually exclusive pairs

- `be-dyn-lib` ⟷ `use-dyn-lib`: these represent opposite sides of the dynamic library boundary and cannot both be enabled at
  once. The crate enforces this with `compile_error!`, and CI uses a feature matrix to test each configuration separately.

### CI strategy

- Default lint/test jobs run with `--all-features` to keep additive flags healthy.
- Matrix entries cover `be-dyn-lib` and `use-dyn-lib` individually (with `--no-default-features`) to ensure each constrained
  configuration continues to compile.

### Testing strategies

- Prefer a feature matrix over `--all-features` when flags are mutually exclusive or alter linkage modes.
- For `airship_maps`, avoid forcing it on in global `--all-features`; instead, test it explicitly in matrix entries to prevent
  invalid combinations.
- Where helpful, use `--no-default-features` to isolate feature surfaces and ensure minimal configs compile and test cleanly.
- If using tools like cargo-all-features/cargo-hack, configure allow/deny lists to skip known-conflicting combos and document the
  rationale here.
