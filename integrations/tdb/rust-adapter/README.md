# Plurora TDB Rust Adapter

This is an opt-in TriviumDB/TDB adapter proof.

- Default build: JSON-RPC stdio shell, no TriviumDB dependency, no backend open.
- Real crate build: `../rust-adapter-real-crate/Cargo.toml` depends on the published `triviumdb = "0.7.0"` crate and the `real-tdb` feature to call real Rust APIs while reusing this adapter source.
- Package role: ordinary subprocess package adapter, not kernel storage and not `EventStore`.

Default check:

```bash
cargo check --manifest-path integrations/tdb/rust-adapter/Cargo.toml
```

Real local smoke test:

```bash
cargo test --manifest-path integrations/tdb/rust-adapter-real-crate/Cargo.toml --features real-tdb
```

This real-crate manifest is intentionally separate from the default manifest so ordinary Plurora builds keep TDB disabled by default while platform users can still compile the real published crate path. Developers who need to test an unpublished local checkout should use their own uncommitted Cargo patch/override.
