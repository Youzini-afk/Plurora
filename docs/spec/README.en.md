# Specifications

> [English](./README.en.md) · [中文](./README.md)

Executable v1 contracts and the hostile conformance roadmap. These docs are backed by code and tests — not aspirational specs.

- [`PUBLIC_CONTRACT.md`](PUBLIC_CONTRACT.en.md) — v1 public contract: 80 methods, 59 events, capability handles, Path A / Path B, SDKs, and conformance
- [`CONTRACT_LAYERING_MATRIX.md`](CONTRACT_LAYERING_MATRIX.en.md) — candidate ownership matrix for the 80 methods, 59 events, and top-level schemas
- [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.en.md) — executable exact-ID registry and explicit contract/profile/version negotiation
- [`OBJECT_STORE.md`](OBJECT_STORE.en.md) — Experimental SHA-256 ObjectStore, ArtifactDescriptor, asset conversion, and verified reads
- [`EFFECT_RECEIPTS.md`](EFFECT_RECEIPTS.en.md) — Experimental EffectReceipt, terminal evidence, historical replay, and branch re-execution
- [`CHANGE_WORKFLOW.md`](CHANGE_WORKFLOW.en.md) — Intent/ChangeSet/PolicyDecision/Commit and the `change.proposal.*` facade
- [`PROTOCOL_COMMONS.md`](PROTOCOL_COMMONS.en.md) — protocol descriptors, portable Work / Assembly contracts, semantic/profile negotiation, adapters, and separate protocol/implementation reports
- [`COMPONENT_IDENTITY.md`](COMPONENT_IDENTITY.en.md) — package envelopes, independent component identity, trust claims, Foreign Capsules, and AssemblyLock
- [`WORLD_BUNDLE.md`](WORLD_BUNDLE.en.md) — cross-host archive integrity, exact journal envelopes, offline replay, lineage, and shell independence
- [`CONFORMANCE_MATRIX.md`](CONFORMANCE_MATRIX.en.md) — hostile conformance case inventory, indexed by tag and domain
- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md) — v1 event kind registry
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.en.md) — v1 error codes
- [`v1/VERSIONING.md`](v1/VERSIONING.en.md) — v1 additive-only versioning strategy
- [`v1/schemas/`](v1/schemas/) — 175 JSON Schemas (80 methods + 59 events + 36 top-level), the SDK source of truth

Run the full suite:

```bash
cargo run -p plurora-cli -- conformance
```
