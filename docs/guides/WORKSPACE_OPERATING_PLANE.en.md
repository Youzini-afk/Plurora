# Workspace Operating Plane

> [English](./WORKSPACE_OPERATING_PLANE.en.md) · [中文](./WORKSPACE_OPERATING_PLANE.md)

A Workspace is a mutable source location on one Host for inspection, authoring, or builds. It is separate from portable Work identity, Host-local Installation, and later Runs. A repository containing source code is not automatically an executable component.

## Ownership

- `managed`: Host-owned controlled copy under `<data>/workspaces/<workspace-id>/source/`.
- `linked_local`: Workspace binding to an existing user-owned directory; the Host has no authority to delete or rewrite that source.

Installation update/remove, failure cleanup, and recovery never delete a linked-local source. Writes require importing a managed copy and crossing a separate ChangeSet approval.

## Intake

An ordinary repository without explicit Work source first undergoes static inspection: stack, candidate entrypoints, build graph, risk summary, and adapter plan. Source presence does not automatically install dependencies, run scripts, build, test, or access the network.

Install Lab can normalize an external URI, local executable, OCI image, or remote service into a Foreign Capsule WorkRevision. Concrete URLs, executable paths, and credentials enter only Host-local acquisition / bindings, never portable artifacts.

## Filesystem safety

Managed materialization retains existing risk boundaries:

- canonical-root containment;
- symlink/reparse escape rejection at every ancestor and terminal entry;
- 25,000 file/directory and 256 MiB copy budgets;
- HTTPS Git rejects inline credentials, queries, and fragments;
- unsupported tree modes fail explicitly;
- staging plus atomic promotion, with failure preserving the current Workspace.

These boundaries prevent unbounded copying, path escape, and credentials in descriptors; they are not arbitrary product quotas.

## Plans versus effects

Ordinary Packages and agents may produce inspections, Workspace plans, patch proposals, adapter previews, and verifier plans. Those outputs grant no execution authority.

Real file effects cross the Host development control plane:

```text
Intent → ChangeSet → PolicyDecision → ChangeCommit → EffectReceipt
```

Calls route to one subject: `workspace/<workspace-id>` or `installation/<installation-id>`. `develop.propose`, `develop.approve`, and `develop.execute` are distinct scopes. Approval binds exact operations, verification, required authority, and expected effects; content cannot be substituted afterward.

The first real executor accepts only bounded typed file writes/deletes plus explicit verifiers. Docker defaults to no network. There is no ambient shell, Host mount, build secret, or first-party bypass.

## Relationship to Installation

A Workspace can produce a new WorkRevision / AssemblyLock candidate but cannot directly mutate an Installation's active pointer. An operator still calls `host.installation.update` with expected revision and an explicit state action. Failure preserves the previous Work/Lock pointer.

## Web and CLI

Web and CLI use only public protocol / Host APIs for projections, ChangeSet drafts, and approvals. They do not read SQLite, Host filesystems, or executor internals directly.

See [`PACKAGE_INSTALLATION.md`](PACKAGE_INSTALLATION.en.md) for Work packing and Installation adoption, and [`../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md`](../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.en.md) for the controlled-change boundary.
