# Plurora Kernel SDK

Generated SDKs for the public kernel contract live in this directory. The source
of truth is `docs/spec/v1/schemas/`; run `scripts/regen-sdks.sh` after changing
Rust contract types or schema exports.

Method schemas carry `x-plurora-contract` metadata. Generated source-level method names invoke
the canonical wire ID, while explicit legacy wrappers preserve old wire IDs. Both SDKs support
exact profile/layer-version selection; transports that cannot carry a selection fail rather than
silently downgrading. The generated `ContractDiagnostic` and `ProtocolResponse` types describe
additive transport-level lifecycle warnings, including Deprecated and Legacy Adapter states. TypeScript HTTP/stdio transports expose
`drainContractDiagnostics()`, and Rust transports may implement `drain_contract_diagnostics()`.

Three distribution channels — pick what fits your workflow:

## Channel 1: npm (TypeScript only)

```bash
npm install @plurora/contract-sdk
```

Publishing is opt-in. The package is also usable directly from this repository.

## Channel 2: workspace path reference

```bash
git clone plurora
```

Then in your `package.json`:

```json
{ "dependencies": { "@plurora/contract-sdk": "file:../plurora/sdk/typescript/contract-sdk" } }
```

Rust consumers can depend on the generated crate by path:

```toml
plurora-contract-sdk = { path = "../plurora/sdk/rust/plurora-contract-sdk" }
```

## Channel 3: read schemas, generate yourself

```bash
git clone plurora
# Use docs/spec/v1/schemas/ with your favorite codegen tool:
quicktype --src-lang schema --lang go docs/spec/v1/schemas/methods/*.json
# or oapi-codegen for Go
# or openapi-generator for any of 50+ languages
# from sdk/openapi.yaml
```

Third-party integrators do not need to consume these SDKs; the JSON Schemas and
OpenAPI description are stable inputs for independent code generation.
