# Creative Capability Kit

> [English](./CREATIVE_CAPABILITY_KIT.en.md) · [中文](./CREATIVE_CAPABILITY_KIT.md)

Creative Capability Kit turns mature headless creative and RP workflows into general capability packages. These packages follow Plurora's public protocol and manifest rules.

TavernHeadless informs the edge cases. The first-party Packages are not `tavern-*` wrappers:

- `plurora/persona-lab` handles persona-like structured profiles.
- `plurora/knowledge-lab` handles structured knowledge collections and match traces.
- `plurora/context-lab` handles bounded context block assembly and budget diagnostics.
- `plurora/text-transform-lab` handles replayable text transform previews and pipeline explanations.

## Rules

- The kernel does not know persona, knowledge, prompt, worldbook, chat, character, or model-call concepts.
- The packages are ordinary manifest/capability/surface packages.
- Compatibility input formats are adapters and fixtures, not canonical Plurora ontology.
- Mutation must be represented as explicit asset/projection/proposal plans, not hidden package state writes.
- Outputs should include provenance and diagnostics.

## Reference tracking

`integrations/tavern-headless/` records the reviewed TavernHeadless commit, capability map, and compact fixtures. Use it as a review ledger when TavernHeadless changes.

The decision vocabulary is:

- `adapted`: generalized into a Plurora package.
- `adapter_only`: useful for import/export, not canonical.
- `deferred`: valuable but not yet part of this kit.
- `rejected`: intentionally not inherited.

## Typical flow

1. Import a profile-like payload with `plurora/persona-lab/import_profile`.
2. Import a knowledge collection with `plurora/knowledge-lab/import_collection`.
3. Match knowledge entries with `plurora/knowledge-lab/match_entries`.
4. Assemble generic context blocks with `plurora/context-lab/assemble_preview`.
5. Preview deterministic transforms with `plurora/text-transform-lab/apply_preview`.
6. If persistence is desired, create an approval-gated proposal that writes assets or rebuilds projections through public protocol.

The flow is intentionally package-level. A third-party package can replace any first-party lab by exposing compatible capabilities and surfaces.
