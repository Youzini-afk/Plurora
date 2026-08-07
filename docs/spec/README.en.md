# Specifications

> [English](./README.en.md) · [中文](./README.md)

Executable v1 contracts and the hostile conformance roadmap. These docs are backed by code and tests — not aspirational specs.

- [`KERNEL_V1_CONTRACT.md`](KERNEL_V1_CONTRACT.en.md) — kernel v1 public contract: 80 methods, 59 events, capability handles, Path A / Path B, SDKs, and conformance
- [`CONTRACT_LAYERING_MATRIX.md`](CONTRACT_LAYERING_MATRIX.en.md) — candidate v2 layering matrix for all 80 methods, 59 events, and top-level schemas; it does not change current v1 status
- [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.en.md) — Experimental layered contract registry, identity aliases, and explicit profile/version negotiation
- [`OBJECT_STORE.md`](OBJECT_STORE.en.md) — Experimental SHA-256 ObjectStore, ArtifactDescriptor, v1 asset adapter, and legacy FNV migration
- [`EFFECT_RECEIPTS.md`](EFFECT_RECEIPTS.en.md) — Experimental EffectReceipt, terminal evidence, historical replay, and branch re-execution
- [`CHANGE_WORKFLOW.md`](CHANGE_WORKFLOW.en.md) — Intent/ChangeSet/PolicyDecision/Commit and the v1 Proposal adapter
- [`PROTOCOL_COMMONS.md`](PROTOCOL_COMMONS.en.md) — protocol descriptors, semantic/profile negotiation, adapters, and separate protocol/implementation reports
- [`COMPONENT_IDENTITY.md`](COMPONENT_IDENTITY.en.md) — package envelopes, independent component identity, trust claims, Foreign Capsules, and composition locks
- [`WORLD_BUNDLE.md`](WORLD_BUNDLE.en.md) — cross-host archive integrity, exact journal envelopes, offline replay, lineage, and shell independence
- [`CONFORMANCE_MATRIX.md`](CONFORMANCE_MATRIX.en.md) — hostile conformance case inventory, indexed by tag and domain
- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md) — v1 event kind registry
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.en.md) — v1 error codes
- [`v1/VERSIONING.md`](v1/VERSIONING.en.md) — v1 additive-only versioning strategy
- [`v1/schemas/`](v1/schemas/) — 161 JSON Schemas (80 methods + 59 events + 22 top-level), the SDK source of truth

Run the full suite:

```bash
cargo run -p plurora-cli -- conformance
```
