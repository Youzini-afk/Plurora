# External Project Operating Plane Guide

> [English](./EXTERNAL_PROJECT_OPERATING_PLANE.en.md) · [中文](./EXTERNAL_PROJECT_OPERATING_PLANE.md)

External Project Operating Plane shows that Plurora does not have to accept only projects that already implement the manifest and capability contract. Unadapted git, npm, local, or archive projects can first be understood, risk-scored, planned, displayed, and wrapped by the platform. Only stable adapters or wrappers enter the ordinary Ygg package and capability world.

The security boundary starts from one fact: install/run means executing untrusted code, and workflow or secret exfiltration are real risks. An unadapted project therefore enters planning, policy, proposal, and audit boundaries first; only user-approved operations constrained by typed executors may produce effects.

## Four object classes

| Object | Meaning | Enters capability registry |
|---|---|---:|
| Ygg Package | Adapted provider with manifest, capabilities, permissions, surfaces, and conformance. | Yes |
| External Project | Unadapted reference such as git/npm/local/archive. Untrusted by default. | No |
| Managed Workspace | Controlled instance/plan/fixture around an External Project, including source ref, workspace state, entrypoints, patch proposals, and audit refs. Not a kernel object. | No |
| Adapter / Wrapper Package | Ordinary Ygg package/capability wrapper around stable external-project operations. | Yes |

This avoids the old plugin-host trap. An external project can remain unchanged while Plurora performs intake, workspace planning, risk summaries, project aggregation UI, patch proposals, and adapter previews around it.

## Implemented packages

### External intake in `official/install-lab`

`plurora install` now detects project kind before attempting package-manifest resolution. A local directory or git source without `project.yaml` / a package manifest no longer fails early with “manifest missing.” Instead, it invokes `official/install-lab/prepare_external_intake` to produce an auditable, zero-package `external_workspace` install plan.

Two ownership modes are explicit:

- `managed` (default): copy a local directory or fetch a git tree into `<data>/workspaces/external/<project_id>/<content_digest>`. The plan records the content digest, so reinstalling the same source and content is idempotent. Uninstall may archive/delete only that host-owned root and never touches the user's source directory.
- `linked_local` (CLI `--link-local`): point the workspace at the canonical local source directory and mark it as user-owned in the descriptor. This is a mutable reference and does not invent a content digest. Uninstall removes only the Ygg project record; it never deletes or archives the linked source.

A managed local copy preserves source metadata such as `.gitignore` while skipping VCS directories, `node_modules`, `target`, virtual environments, and common language caches. A materialized tree defaults to at most 25,000 files, 25,000 directories, and 256 MiB; even direct capability callers cannot raise the hard ceiling above 100,000 files, 100,000 directories, or 1 GiB. Absolute, dangling, or root-escaping symlinks fail closed. Managed storage ancestors must be real directories under the canonical data root. HTTPS Git trees receive the same bounded materialization, hash, size, and symlink checks; unsupported tree modes such as submodule entries fail explicitly. The temporary bare fetch now counts Git pack bytes while gix reads them and interrupts fail-closed above a 512 MiB default. Host deployment workspace cloning explicitly uses the 2 GiB hard ceiling for its 1 GiB tree limit. The transport remains a full bare fetch rather than a claimed shallow clone, but it is no longer unbounded. Inline credentials, query parameters, and fragments are rejected, so any authentication must be supplied out of band by the host and is never embedded in the descriptor.

Project IDs combine a safe slug with a 96-bit source-identity hash, so same-name sources at different paths/URLs do not collide. Descriptors also record `source_kind`, `workspace_ownership`, and `source_digest` when available. An incompatible descriptor at the same ID fails closed; concurrent materialization only reuses a winner whose digest exactly matches.

This step only materializes source and writes a project descriptor. It never runs install/build/test/scripts and never registers the external project as a capability provider. `--wrap-as-adapter` also no longer fabricates a manifest path that does not exist; real adapter authoring is reserved for the later ChangeSet-approved development flow.

### `official/project-intake-lab`

Ordinary official package, no kernel privilege. It exposes these capabilities:

- `describe_intake_contract`
- `inspect_external_project_ref`
- `detect_project_stack_from_metadata`
- `draft_workspace_plan`
- `draft_security_risk_summary`
- `list_candidate_entrypoints`
- `draft_adapter_plan`
- `generate_adapter_manifest_preview`
- `generate_subprocess_wrapper_preview`
- `generate_adapter_fixture_preview`
- `check_adapter_readiness`

Capability boundaries:

- Static intake, metadata-based stack detection, risk summaries, workspace/adapter planning only.
- No clone, no install, no run, no network, no local filesystem read.
- Blocks raw secrets, path traversal, home paths, and sensitive absolute local paths.
- Detects npm lifecycle scripts (`preinstall`, `install`, `postinstall`, `prepare`, `prepublish`) as `executes_code` / `requires_approval`.
- Adapter previews must use ordinary third-party package ids, never `official/`, and reject path traversal or unsafe characters.
- Capability ids must belong to the adapter package namespace.
- Produces manifest/wrapper/fixture/readiness previews only. No file write, execution, or publishing.

### `official/workspace-lab`

Ordinary official package, no kernel privilege. It exposes 12 capabilities:

- `describe_workspace_contract`
- `draft_workspace_creation`
- `explain_required_permissions`
- `request_workspace_action`
- `summarize_workspace_audit`
- `create_fixture_workspace`
- `inspect_workspace`
- `read_workspace_metadata`
- `plan_workspace_run`
- `record_fixture_process_result`
- `discover_workspace_entrypoints`
- `draft_workspace_patch`

Capability boundaries:

- Action taxonomy covers `clone_project`, `read_metadata`, `install_dependencies`, `run_command`, `run_tests`, `stop_process`, `read_logs`, `discover_entrypoints`, `write_patch`, and `deploy_plan`.
- Each action carries `risk_level`, `requires_approval`, `executes_code`, `network_required`, and `filesystem_write_required`.
- `request_workspace_action` denies by default. Policy/action mismatches fail closed.
- Deterministic fixture workspace capabilities prove workspace descriptors, entrypoints, run plans, fixture results, and patch proposal shapes without creating directories, spawning processes, or reading files.
- Patch output is proposal-only with `file_write_performed=false`.

### Host development control plane

Planning packages and real change execution now follow separate authority paths. `official/workspace-lab` continues to emit deterministic plans and patch proposals only. Approved source changes enter through the access-token-protected `/host/v1/projects/:project_id/changes` API and leave a durable `Intent -> ChangeSet -> PolicyDecision -> ChangeCommit -> EffectReceipt` causal chain. Approval and execution are separate requests. Approval covers exact operations, verification, required authority, and expected effects; content cannot be replaced afterward.

The first version accepts bounded `file_write` / `file_delete` operations only, copies into Host-owned scratch, and performs static validation or a constrained Dockerfile build. Docker defaults to no network and provides no arbitrary host command, scratch Nixpacks build, build secret, or host mount. See [`../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.en.md`](../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.en.md) for the complete boundary.

Ownership determines delivery:

- `managed_external`: after verification, create a new immutable content-digest tree and atomically update the descriptor; the old tree is never edited in place.
- `native_managed`: return a verified bundle only, without automatic in-place write-back.
- `linked_local`: reject the workflow until a managed copy is imported; the Host never mutates the user's directory automatically.

A committed `managed_external` ChangeSet verified with `docker_build` may also enter the verified-deployment transaction. Verification commits an immutable build-context artifact and removes its verification image. Preview revalidates descriptor, tree, and artifact provenance, then the selected `local` or Agent target rebuilds the candidate through typed operations; the preview always remains Host-authenticated. A second deployment approval binds the exact candidate/evidence. Activation drains the previous revision only after health succeeds and a durable `VerifiedActivate` revision commits. Recover and rollback rebuild from durable context on the recorded target without reading the live workspace or refetching source.

Development execute/recover carries the authenticated identity, but never its credential, across the in-memory operation. It refreshes the Host access journal before Docker and managed-workspace effects and again after blocking verification. Revocation cannot interrupt an effect already inside its executor; it does prevent every later user effect, leaving the durable recovery state for an explicitly authorized operator to reconcile.

## Web aggregation entry

`clients/web/src/projects/external-projects.ts` aggregates no-execution outputs from `project-intake-lab` and `workspace-lab` through public protocol/capability invoke.

- Home/Play displays an External Project Operating Plane rail.
- Forge displays an External Projects / Managed Workspaces panel.
- Assistant drawer displays lightweight inspect / draft patch / generate adapter plan entries.
- The project console Development area and `plurora host access ... changes` CLI use the public Host API to draft, review, approve, execute, export, and recover ChangeSets. The console also completes verified private preview, separate deployment approval, activation, and interrupted-operation reconciliation. Neither client reads or writes a workspace directly.
- UI does not read SQLite, runtime internals, local project directories, or process state.

## Security red lines

- Do not add `kernel.v1.project.*`, `kernel.v1.workspace.*`, `kernel.v1.git.*`, `kernel.v1.npm.*`, `kernel.v1.deploy.*`, or `kernel.v1.ide.*`.
- External Project is not a package; Managed Workspace is not a kernel object; Adapter/Wrapper is the package path.
- Unadapted projects do not directly register as capability providers.
- Dangerous actions must be plan-first, policy-checked, proposal/approval-gated, audited, and redacted.
- Do not execute `npm install`, `pip install`, `cargo build`, `make`, or arbitrary project scripts by default.
- Do not inherit host `.env`, SSH keys, browser profiles, home directories, or raw secrets.
- Agents and ordinary packages may draft plans/proposals/patches only; real effects must cross an authenticated Host policy, approval, scratch, verification, and audit chain.
- Web shell remains public-protocol-only.

## Example

`examples/packages/external-project-adapter-preview/manifest.yaml` is the E5 adapter preview fixture. It uses the `thirdparty/example-adapter` namespace and proves that external-project adapters should use the same package path as every other package. It is not a published artifact, does not write to a user project, and does not execute external commands.

Check it with:

```bash
cargo run -p plurora-cli -- package check packages/official/project-intake-lab/manifest.yaml
cargo run -p plurora-cli -- package check packages/official/workspace-lab/manifest.yaml
cargo run -p plurora-cli -- package check examples/packages/external-project-adapter-preview/manifest.yaml
cargo run -p plurora-cli -- conformance --tag project_intake
cargo run -p plurora-cli -- conformance --tag workspace_lab
```

## Continuous real-project acceptance

GitHub CI's [`External project Host operations acceptance`](../../.github/workflows/ci.yml) is a black-box release gate, not a product-specific demo. It uses CLI `install --workspace-only` to intake [`mdn/beginner-html-site-styled`](https://github.com/mdn/beginner-html-site-styled/tree/6c7a360ddb4a0d75be06044bf8a914f260ff10c7) pinned to commit `6c7a360ddb4a0d75be06044bf8a914f260ff10c7`, starts an ordinary SQLite/autoload Host, and then uses only authenticated public RPC/HTTP contracts.

[`scripts/host-operations-acceptance.py`](../../scripts/host-operations-acceptance.py) creates two independently verified revisions for the real project and a third revision for a structurally different [`Python standard-library HTTP fixture`](../../examples/host-operations/python-service/README.md). The gate covers network-none Docker verification, private preview, the production route, container deletion and readiness degradation, explicit recovery, Host crash, durable-lease takeover, SQLite/runtime projection restoration, and rollback. Direct Docker calls are limited to precise fault injection, observation, and cleanup; they never complete a platform operation.

## Next directions

External intake, controlled source ChangeSets, verified local/Agent deployment, and real-project failure recovery now form the first Host loop. The next step is not arbitrary command execution; it is to tighten and extend the same boundary:

- fine-grained artifact read permissions, encryption/retention policy, reachability GC, and journal snapshot compaction;
- more explicit verifiers and sandbox backends, each declaring network, secret, resource, and effect needs instead of collapsing into a generic shell runner;
- human/tool-assisted application of native verified bundles, plus deeper project graphs, dependency-risk analysis, and guided adapter/deployment-descriptor authoring;
- separately designed target-edge ingress and application identity; arbitrary network proxying and a general remote shell remain explicit non-goals.

These should still proceed as ordinary package and Host-executor substrate, not as kernel product ontology.
