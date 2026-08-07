# v1 Versioning Policy

> [English](./VERSIONING.en.md) · [中文](./VERSIONING.md)

## Exact identities

Contract Registry `0.1.0` publishes one owner-based wire ID per method. The v1 tree contains no method aliases or parallel event namespaces. Schema filenames, `$id` URNs, OpenAPI operation IDs, runtime dispatch, and generated SDK methods all derive from the same exact identity.

## Additive v1 rule

`docs/spec/v1/schemas/` is the public v1 contract artifact. Within a published v1 boundary:

- optional fields, methods, event kinds, or open enum values may be added only when existing implementations can safely ignore them;
- fields must not be removed or change type;
- optional fields must not become required;
- closed enums must not be narrowed;
- existing error and event meaning must not change;
- unknown fields that are declared forward-compatible must remain readable.

Schema changes must pass `scripts/validate-schemas.sh`. CI compares against the base schema tree and checks removals plus common structural breakage such as type/const changes, new required fields, enum narrowing, removed properties or definitions, tighter bounds, and incompatible combinator changes.

The serialized-wire guarantee is distinct from crate and SDK semantic versioning. Rust crates and generated SDKs are pre-1.0; an additive wire field can still affect source-level struct literals. Consumers should prefer constructors, builders, or deserialization and follow each artifact's semantic version.

## Breaking changes

A breaking change uses a new explicit contract/profile/version boundary, for example a new `plurora.contract.default/v2` profile or a new per-layer major version. It does not overwrite v1 and does not hide behind an unadvertised alias.

Breaking changes include:

- required-field or existing-field type changes;
- incompatible authority or error semantics;
- event payload reinterpretation;
- removal or renaming of a stable method or event identity;
- incompatible Protocol Commons lifecycle or profile changes.

A stable breaking transition requires migration tooling, readable prior data, explicit negotiation, support policy, and conformance vectors for every supported boundary.

## Negotiation

Clients call `host.info` and inspect `contract_registry_version`, `contract_methods`, profiles, layer versions, supported transports, and Protocol Commons descriptors. They select a supported contract/profile and reject an unsupported version rather than guessing or silently downgrading.

Omitting contract selection chooses `plurora.contract.default/v1`. Explicit requirements must match exactly; unknown profiles, duplicate requirements, unsupported Protocol majors, and version mismatches fail before method dispatch with `protocol/error/unsupported_contract`.

## Pre-release resets

Before a boundary is declared Stable, a coordinated destructive reset may replace an experimental identity set. Such a reset is completed atomically across runtime, schemas, SDKs, clients, tests, data conventions, and documentation. The finished tree keeps only the selected identity set; it does not preserve temporary aliases as permanent architecture.
