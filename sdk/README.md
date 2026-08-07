# Plurora Contract SDKs

Generated SDKs for the Plurora public contract live in this directory. The
source of truth is `docs/spec/v1/schemas/`; after changing public Rust types,
method IDs, event kinds, or schema exports, run:

```bash
bash scripts/regen-sdks.sh
```

Every public method has exactly one wire ID. The generated TypeScript and Rust
clients expose that ID directly and can optionally attach an explicit contract
selection. A transport that cannot carry the requested selection fails rather
than silently dropping it.

## TypeScript

```bash
npm install @plurora/contract-sdk
```

The package is also usable by workspace path:

```json
{
  "dependencies": {
    "@plurora/contract-sdk": "file:../plurora/sdk/typescript/contract-sdk"
  }
}
```

## Rust

```toml
plurora-contract-sdk = { path = "../plurora/sdk/rust/plurora-contract-sdk" }
```

## Independent generation

Third-party integrators do not need either first-party SDK. JSON Schema and
OpenAPI remain public inputs for independent generators:

```bash
quicktype --src-lang schema --lang go docs/spec/v1/schemas/methods/*.json
# or generate from sdk/openapi.yaml
```
