# Feature combinations and CI strategy

This quick reference highlights feature interactions that impact CI automation. See [`features.md`](./features.md) for the
full rationale, history, and additional crate-specific notes.

- `airship_maps`: enables the TinySkia-driven airship route map assets. It is skipped by default when invoking
  `cargo --all-features` in automation because other mutually exclusive flags (such as dynamic library modes) cannot build
  alongside it. Dedicated CI matrix entries exercise `airship_maps` explicitly.
- When the CI feature matrix does not supply a custom flag set, the default jobs run `--all-features` to keep additive
  combinations healthy. Matrix entries instead enumerate the valid exclusive sets so conflicts like
  `be-dyn-lib` vs. `use-dyn-lib` are tested independently.

Keeping the metadata in `Cargo.toml` (see `[package.metadata.cargo-all-features]`) aligned with these notes ensures contributors
get the same feedback locally as the CI system.
