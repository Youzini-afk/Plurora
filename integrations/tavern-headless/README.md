# TavernHeadless Reference Ledger

This directory records how Plurora studies TavernHeadless without adopting TavernHeadless as product ontology.

TavernHeadless is a reference source for mature headless creative/RP capabilities:

- character/profile import and export edge cases;
- knowledge/worldbook activation behavior;
- preset/context assembly behavior;
- regex/text transform behavior;
- public SDK/OpenAPI update discipline.

Plurora adapts those lessons into general official capability packages:

- `official/persona-lab`
- `official/knowledge-lab`
- `official/context-lab`
- `official/text-transform-lab`
- `official/model-connector-lab`
- `official/model-routing-lab`

No product package in this track should be named `tavern-*`. Compatibility fixtures are inputs for validation, not canonical schemas.

## Update discipline

When TavernHeadless changes:

1. Compare the current upstream commit with `upstream.lock.toml`.
2. Review changed subsystem paths against `capability-map.yaml`.
3. Decide for each change: `adapted`, `adapter_only`, `deferred`, or `rejected`.
4. Add or update compact fixtures only when they protect a Plurora-native behavior.
5. Run Plurora conformance before changing package claims.

The goal is not bit-for-bit parity. The goal is to avoid forgetting useful edge cases while keeping Plurora’s abstractions broader than Tavern.

## Model connectivity note

`model-connectivity-map.yaml` tracks provider/profile and instance-routing lessons from TavernHeadless. The current model-connectivity ledger remains no-network and no-inference: discovery outputs are plans, profile validation is structural, and secrets are represented by references only.
