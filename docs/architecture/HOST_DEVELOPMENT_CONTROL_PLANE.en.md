# Host Development Control Plane

> [English](./HOST_DEVELOPMENT_CONTROL_PLANE.en.md) · [中文](./HOST_DEVELOPMENT_CONTROL_PLANE.md)

The Host development control plane separates “propose a source change for a project” from “run an arbitrary command on the host.” It uses the existing constitutional sequence `Intent -> ChangeSet -> PolicyDecision -> ChangeCommit -> EffectReceipt` for causality, approval, and effects. Project resolution, scratch workspaces, Docker verification, and workspace promotion remain Host control-plane concerns; no `kernel.v1.project.*`, `kernel.v1.workspace.*`, or IDE product ontology is added.

`official/workspace-lab` remains an ordinary planning package with no execution authority. Real changes enter only through the access-token-protected `/host/v1/projects/:project_id/changes` API. Docker verification is performed by the equally ordinary `official/docker-runtime-lab`; it has no kernel privilege.

## Lifecycle

```mermaid
flowchart LR
  I["Intent"] --> C["Drafted ChangeSet"]
  C --> P["RequiresApproval"]
  P -->|"HostAdmin approve"| A["Approved"]
  P -->|"reject"| R["Rejected"]
  A --> S["Host-owned scratch"]
  S --> V["Static or Docker verification"]
  V -->|"managed external"| M["Content-addressed promotion"]
  V -->|"native managed"| B["Verified bundle only"]
  M --> K["ChangeCommit + EffectReceipt"]
  M -->|"interrupted"| X["Recovery required"]
  X --> Q["Descriptor/tree reconciliation"]
  K -->|"committed Docker verification"| D["Private deployment preview"]
  D --> E["Exact-candidate deployment approval"]
  E -->|"approve"| A2["Health-gated activation"]
  A2 --> R2["Durable VerifiedActivate revision"]
  R2 --> O["Drain previous revision"]
  D -->|"interrupted"| X2["Deployment recovery required"]
  A2 -->|"interrupted"| X2
  X2 --> Q2["Adopt exact activation or clean exact candidate"]
```

Source approval and execution are separate requests. Approval covers the exact server-returned operations, verification plan, `required_authority`, and `expected_effects`; ChangeSet content cannot be replaced after approval. The Web project console renders all four next to the approval action.

Deployment is a second independent transaction. Only a committed `managed_external` ChangeSet verified by Docker can create a preview. Once the preview is ready it needs separate approval, whose artifact binds the exact target, candidate receipt, verification/build-context refs, and authority. Source approval never implies deployment approval.

## Host API

| Method | Route | Purpose |
|---|---|---|
| `GET` / `POST` | `/host/v1/projects/:project_id/changes` | List / draft ChangeSets |
| `GET` | `/host/v1/projects/:project_id/changes/:change_set_id` | Read state and durable refs |
| `GET` | `.../:change_set_id/bundle` | Export the artifact-backed JSON patch bundle |
| `POST` | `.../:change_set_id/approve` | Approve or reject the exact ChangeSet once |
| `POST` | `.../:change_set_id/execute` | Stage, verify, and promote according to ownership |
| `POST` | `.../:change_set_id/recover` | Reconcile an interrupted Docker image or managed promotion |
| `POST` | `.../:change_set_id/deployment/preview` | Build a Host-authenticated preview from the verified context on an explicit target |
| `POST` | `.../:change_set_id/deployment/approve` | Approve or reject the exact preview candidate |
| `POST` | `.../:change_set_id/deployment/activate` | Health-check, activate, and commit a durable revision |
| `POST` | `.../:change_set_id/deployment/reconcile` | Explicitly adopt the exact durable activation or clean the exact candidate |

All routes are inside Host authentication middleware. The root token remains the complete Host gate. Source routes attenuate paired-device authority through separate `develop_propose`, `develop_approve`, and `develop_execute` scopes; the four deployment routes require the separate `deploy` scope. Every project route checks the exact project selector, and deployment routes also check the target selector bound by the request or durable record; unknown mutations fail closed. Execute/recover retains the authenticated identity without credentials, refreshes current grant/ancestor state before Docker and managed-workspace effects, and rechecks after blocking verification. A revoked or expired grant cannot begin a later effect; an already in-flight effect may finish and is reconciled through the explicit recovery path. `host serve` still requires a non-empty root token for a non-loopback bind. See [`HOST_REMOTE_ACCESS.md`](HOST_REMOTE_ACCESS.en.md) for device pairing, revocation, HTTPS, and cookie boundaries.

## CLI lifecycle

`plurora host access ... changes` uses only the routes above and the selected Host/project connection context. Draft input is the exact typed JSON request, read from a bounded file or from stdin with `--request -`; the CLI does not read a workspace or write the Host journal directly.

```json
{
  "goal": "Update the application title",
  "operations": [
    {
      "op": "file_write",
      "path": "src/title.txt",
      "content": "Plurora\n"
    }
  ],
  "verification": { "kind": "static_validation" },
  "idempotency_key": "title-update-1"
}
```

```bash
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 draft --request change.json
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 approve <change-set-id> --reason reviewed
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 execute <change-set-id>
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 get <change-set-id>
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 bundle <change-set-id>
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 recover <change-set-id>
```

`list` and `reject` complete the same lifecycle. Execute is asynchronous; its JSON response reports whether work was accepted, and `get`/`list` reads the durable status. Omitting `--project` uses the project selected by `plurora host connection context`.

## Ownership behavior

| Workspace ownership | Draft | Scratch verification | Automatic write-back |
|---|---:|---:|---:|
| `managed_external` | Yes | Yes | Yes; create a new immutable digest tree, then atomically update the descriptor |
| `native_managed` | Yes | Yes | No; the first version emits a verified bundle only |
| `linked_local` | No | No | Never; import a managed copy before Host verification |

A linked-local directory is user-owned and may change concurrently. The first version does not copy it with a check-then-use path scheme and never writes user source. Although a native workspace is Host-managed, this version still avoids an in-place multi-file transaction and delivers a verified bundle instead. Only a content-addressed managed external tree enters automatic promotion.

## Verified Artifact to deployment

1. Successful Docker verification also commits an immutable build-context artifact. The verification image is removed after ownership-label checks and is never implicitly promoted as a deployment image.
2. Preview revalidates the project descriptor, live managed tree, source digest, verification artifact, and build-context content. Any drift or missing provenance blocks the transaction.
3. The selected `local` or Agent target rebuilds the candidate through the same typed artifact-transfer, declarative Docker-build, and deployment-apply operations. Target ports remain loopback-only and the preview route is always `host_authenticated`.
4. Deployment approval binds the exact preview evidence. Activation revalidates artifacts, approval, authority, target-operation receipts, and readiness; points the requested private or explicitly public route at that same candidate; commits an immutable `VerifiedActivate` revision; and only then drains the previous revision.
5. Recover and rollback never read the live workspace or refetch source. They revalidate the revision's artifact closure, evidence, and current project/target authority, then rebuild from the durable build context on the recorded target.

## File and artifact boundary

- Operations are bounded `file_write` / `file_delete` only. Absolute paths, `..`, backslashes, VCS metadata, `.env`, credential files, and duplicate targets are rejected.
- Source input is limited to 4 MiB per file and 16 MiB per request; a workspace is limited to 25,000 files, 25,000 directories, and 256 MiB.
- Snapshot copy counts bytes actually read, checks file identity/size before and after opening, and rejects hardlinks on Unix. Symlinks and special files fail closed.
- The journal stores structure, state, and artifact descriptors, not source bodies. Metadata says `source_artifact_references`; it does not falsely claim that the entire payload is redacted.
- Source bodies live in the content-addressed artifact store and are materialized by the bundle API inside the authenticated Host boundary. Never submit secrets in a ChangeSet. Fine-grained artifact scopes, encryption policy, retention and reachability GC, plus journal snapshot compaction, remain follow-up work. The current implementation retains the complete in-memory state index so pruning cannot break durable idempotency or replay consistency.

## Verification boundary

`static_validation` checks scratch structure and the final tree digest without executing project code.

`docker_build` is the only first-version project-code execution boundary:

- development scratch supports Dockerfile only; it does not invoke host Nixpacks or an arbitrary command runner;
- context must be exactly `<data>/projects/<project>/development/<change>/workspace`, and the canonical root is checked again immediately before packing;
- `network=none` is the default; `bridge` must be explicit in the ChangeSet and adds `host.network.egress` authority;
- build secrets, secret refs, host mounts, and arbitrary build-time secret parameters are rejected;
- CPU, memory, time, file-count, and byte limits apply;
- only status and a diagnostic-log SHA-256 are persisted, never raw Docker logs;
- the verification image is removed after matching `managed-by`, package, project, build, and change labels. It is not retained as a deployment image.
- container status/log/stop also carries route and port-lease scope and must match `managed-by`, package, route, and lease labels. Stop additionally requires explicit `approved: true`; an arbitrary Docker ID is never treated as a Plurora resource.

## Durability, concurrency, and recovery

- Each project has its own development journal session. Transitions use EventStore `append_with_sequence_if_next` expected-tail compare-and-append; memory, SQLite, and PostgreSQL implement the same atomic semantics.
- With an idempotency key, the change id is deterministically derived from project + key. Different requests using one key conflict in the durable journal instead of relying only on a process-local map.
- The development control plane holds a global 30-second Host lease with a 10-second heartbeat. Missing lease state fails closed. Every change write checks local expiry and the durable lease tail; promotion renews before effects and checks again before descriptor activation. A second Host cannot recover or execute against the shared store concurrently, and approval, execution, and promotion stop after lease loss.
- Interrupted staging or static verification has not promoted a workspace and can fail with scratch cleanup.
- Interrupted Docker verification enters `recovery_required`; recovery uses the stable build id and full ownership labels to remove the image or confirm it is absent before recording a failed terminal state.
- Managed promotion persists old/new digests and whether the destination pre-existed before visible effects. Recovery reads the real descriptor and tree: if the descriptor points at the proposed digest, it completes the success commit; if it still points at the previous digest, it removes only a newly created, digest-matching orphan. Anything else remains recovery-required.
- Interrupted deployment preview or activation never automatically replays target effects. Journal replay marks the transaction `recovery_required` until an explicit reconcile runs with valid project/target authority.
- Reconcile only adopts a durable active revision whose provenance and candidate identity match exactly, or cleans the exact candidate/route/lease. Missing durable identity, ambiguous leases, ownership conflicts, or inconsistent revision provenance remain blocked instead of guessing success.
- The system never replays arbitrary project commands automatically and never disguises uncertain partial effects as an ordinary failure.

## Deliberately absent

- arbitrary shell, install/test command, or host command runner;
- automatic mutation of linked-local or native workspaces;
- implicit use of a verification image for deployment;
- development/project/deploy ontology in the kernel;
- a local CLI mutation path that bypasses the public Host API.

The mobile PWA, Web/Desktop, and remote CLI now use scoped device identity and project/target context through the same public Host API, with no side-channel mutation interface. Remaining work includes fine-grained artifact permission/encryption/retention/GC and richer but still declarative verifier/sandbox backends.
