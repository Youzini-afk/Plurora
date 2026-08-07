# Change workflow and Proposal adapter

> [English](./CHANGE_WORKFLOW.en.md) · [中文](./CHANGE_WORKFLOW.md)

Status: Experimental, schema version 1.

The generic `Intent → ChangeSet → PolicyDecision → Commit` chain describes a change's goal, operations, authority decision, and actual commit result without adding world, conversation, model, or other content-domain semantics to the kernel.

## Schemas and type URIs

| Primitive | Type URI / schema |
| --- | --- |
| `Intent` | `urn:yggdrasil:intent:v1` / [`intent.schema.json`](v1/schemas/intent.schema.json) |
| `ChangeSet` | `urn:yggdrasil:change-set:v1` / [`change-set.schema.json`](v1/schemas/change-set.schema.json) |
| `PolicyDecision` | `urn:yggdrasil:policy-decision:v1` / [`policy-decision.schema.json`](v1/schemas/policy-decision.schema.json) |
| `ChangeCommit` | `urn:yggdrasil:change-commit:v1` / [`commit.schema.json`](v1/schemas/commit.schema.json) |

`ChangeOperation` keeps an open `op`, `target`, `input_refs`, and `payload` shape. The kernel does not maintain a closed enum of domain operations.

## v1 Proposal adapter

Existing `kernel.v1.proposal.*` methods and events remain unchanged. Proposal creation now adds:

- `intent`: creator, target session/branch, and goal;
- `change_set`: mapped legacy `operations`, `required_permissions`, and `expected_effects`;
- `policy_decision`: initially `requires_approval`;
- `commit`: created after an apply terminal outcome;
- `receipt`: terminal evidence for rejection, failure, partial completion, or success.

Approval requires `change.proposal.approve`; rejection requires `change.proposal.reject`. HostAdmin/HostDev satisfy these checks intrinsically, while other principals need explicit grants scoped to the proposal. Approval changes the decision to `allowed`. Rejection changes it to `denied` and creates a `change.policy` receipt.

## Apply lifecycle

Apply uses this sequence:

1. Under the proposal-registry write lock, reserve `approved` as `applying` so concurrent duplicate applies cannot start.
2. Recheck `change.proposal.apply` plus every legacy `required_permissions` entry against the applying principal, then preflight every operation without side effects, including operation name, required target, and payload shape.
3. Execute operations in order; each operation produces a `change.operation` receipt.
4. If every operation succeeds, write `ChangeCommit(status=committed)` and a `change.commit/succeeded` receipt.
5. If failure occurs before any operation completes, write a failed commit/receipt.
6. If failure occurs after an operation completed, write a partial commit/receipt and retain the completed operation receipts.

`ChangeCommit.result_refs` points to the final receipt's recorded output objects. `operation_receipts` retains evidence for the operations that actually completed or failed. Raw error bodies are not copied into receipts; protocol results retain a stable failure code plus a SHA-256 message fingerprint. Finalization uses an expected-status compare-and-set, and the public failure transition cannot overwrite an `applying` or terminal proposal.

## Compatibility boundary

- v1 Proposal fields remain readable and writable; new fields are additive and optional.
- Legacy `asset.put` and `projection.rebuild` stay adapter operations rather than permanent substrate enum variants.
- A Proposal is not an Intent, approval is not a Commit, and a receipt proves only an effect that actually occurred.
- Apply does not promise a global ACID transaction across arbitrary external systems. Preflight prevents predictable half-commits; an irreversible mid-execution failure must be recorded explicitly as `partial`.

## Security

Proposal creation continues to reject raw secrets and requires `secret_ref` usage. Intent, ChangeSet, policy, approval, operation output, and final result are stored as separate referenced objects and pass through secret redaction before receipt commit. The receipt envelope retains only descriptors, status, and necessary summaries.
