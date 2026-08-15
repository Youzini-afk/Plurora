# Conformance matrix

> [English](./CONFORMANCE_MATRIX.en.md) · [中文](./CONFORMANCE_MATRIX.md)

The conformance suite is the charter's executable guard: it proves both allowed and rejected behavior. The complete named-case list comes from the CLI. Do not hand-write a drifting total or a row-by-row status table here.

The current implementation snapshot is [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md). Construction direction is [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.en.md).

## How to run it

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- conformance --list
cargo run -p plurora-cli -- conformance --case sharing_lab
cargo run -p plurora-cli -- conformance --tag sharing
cargo run -p plurora-cli -- conformance --fail-fast
cargo run -p plurora-cli -- conformance --slowest 3
```

Filtering, timing, and diagnostics are in [`../performance/PERFORMANCE_AND_CODE_HEALTH.md`](../performance/PERFORMANCE_AND_CODE_HEALTH.en.md).

Local acceptance checks for a third-party Package are in [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.en.md) and `plurora conformance package`.

## Domain coverage

`--list` emits tags grouped by domain. Prefer an existing tag when you add a case:

| Domain | Typical tags | What they guard |
|---|---|---|
| Substrate | `substrate` `event` `protocol` `permission` `hook` | Identity, journal, public methods, authority, hooks |
| Host lifecycle | `host` `assembly` `secret` | Installation, Work/Lock, secret_ref |
| Run / Powerbox / Realization | `runtime` `surface` | Run, Exposure/Binding, Realization, Surface |
| Package execution | `package` `capability` `subprocess` `first_party` | In-process / subprocess, no first-party privilege |
| Outbound and streams | `network` `outbound` `stream` | Fail-closed network, audit, stream lifecycle |
| Authoring and experience | `agentic` `experience` `memory` `sharing` | Agents, experiences, memory, sharing |
| Storage and sources | `storage` `asset` `projection` `source_intake` `workspace_lab` `retrieval` | Objects, projections, external sources |
| Replacement proofs | `replacement` | Third parties can replace first-party code; no publisher priority |
| Optional live network | `live` | Real outbound only when explicitly opted in |

Coverage that changes an architectural judgment belongs in architecture or spec prose, not as a second copy of the CLI list.
