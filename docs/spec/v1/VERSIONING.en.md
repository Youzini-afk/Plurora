# v1 Versioning Policy

## Method namespace

The historical v1 schema files retain their `platform.*` names. The Contract Registry may expose a layered canonical wire ID (for example `host.target.list`) while retaining the corresponding `platform.*` ID as an explicit compatibility alias. Both IDs resolve at one boundary to the same handler and v1 payload semantics. Future breaking wire contracts use a separately negotiated major version; they do not overwrite v1.

## Schema rule

`docs/spec/v1/schemas/` is the public v1 contract artifact. v1 evolves additive-only:

- Optional fields, event kinds, methods, or enum variants may be added when old implementations can safely ignore them.
- Fields must not be removed, field types must not change, optional fields must not become required, published enums must not be narrowed, and existing event payload meaning must not change.
- Schema changes must pass `scripts/validate-schemas.sh`; CI sets `BASE_SCHEMA_DIR` and checks removals plus common structural breakage such as type/const changes, new required fields, enum narrowing, removed properties/definitions, tighter bounds, and incompatible combinator changes.

This additive guarantee applies to the serialized v1 wire contract. The Rust crates and generated SDKs are still pre-1.0: adding an optional wire field can add a field to a public Rust struct and therefore break downstream struct literals. Consumers should prefer constructors, builders, or deserialization and follow each crate or SDK's semantic version when upgrading.

## When v2 happens

Use `kernel.v2.*` for breaking changes: required-field changes, existing field type changes, error-code semantic changes, incompatible permission model changes, or incompatible event payload reshaping. v2 does not overwrite v1; hosts may expose multiple versions side by side.

## Negotiation

New clients call canonical `host.info` and inspect `contract_registry_version`, `contract_methods`, `aliases`, profiles, and protocol descriptors in addition to the historical method/status fields. Older clients may still call `host.info`; registry `0.4.0` marked that alias Deprecated, and registry `0.5.0` transitions it to an identity Legacy Adapter while retaining the original support-window metadata and advisory migration diagnostic. That adapter accepts security fixes and data-reading compatibility but no new field semantics. A client needing a method should select a supported contract/profile, prefer the advertised canonical ID, and reject an unsupported version rather than guessing or silently downgrading.
