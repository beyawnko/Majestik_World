# Feature combinations and CI strategy

This quick reference highlights feature interactions that impact CI automation. See [`features.md`](./features.md) for the
full rationale, history, and additional crate-specific notes.

- `airship_maps`: enables the TinySkia-driven airship route map assets; it is intentionally skipped by `--all-features` in
  automation due to mutually exclusive flags and is exercised via a dedicated CI matrix entry instead (see workflow).
- When the CI feature matrix does not supply a custom flag set, the default jobs run with the default features (not
  `--all-features`) to avoid invalid combinations; the matrix enumerates valid exclusive sets—`be-dyn-lib`, `use-dyn-lib`,
  and `airship_maps`—which are tested independently.

Keeping the metadata in `Cargo.toml` (see `[package.metadata.cargo-all-features]`) aligned with these notes ensures contributors
get the same feedback locally as the CI system.
