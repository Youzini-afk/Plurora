# Packages, Work, and Installation

> [English](./PACKAGE_INSTALLATION.en.md) · [中文](./PACKAGE_INSTALLATION.md)

Plurora no longer combines Package loading, source workspaces, content identity, installed instances, and runtime state into one “install” operation. The current flow separates source recognition and Work packing, Host Installation create/update, and a later independent Run.

## Fast path

```bash
# Create or check Work source
plurora work init ./my-work --id example/my-work
plurora work check ./my-work

# Write the content-addressed ObjectStore; do not install or run
plurora work pack ./my-work

# Create a Host-local Installation
plurora installation create ./my-work --idempotency-key install-my-work-v1

# Inspect and update
plurora installation list
plurora installation info <installation-id>
plurora installation update <installation-id> \
  --expected-revision 1 --state preserve \
  --idempotency-key update-my-work-v2

# Removal requires an explicit state decision
plurora installation remove <installation-id> \
  --state keep --idempotency-key remove-my-work
```

Installation `ready` still does not mean running; CLI and Web must call explicit `host.run.start` to create a Run. Opening detail never starts one, and an unavailable local implementation returns structured gaps. `host.run.stop` stops only that Run and does not unload global Packages.

## Accepted sources

| Source | Normalized result |
|---|---|
| `work.yaml` + `assembly.yaml` | Parse explicit Work / Assembly source. |
| Package manifest | Produce a single-node Assembly and synthetic WorkRevision while preserving Component identity. |
| Ordinary source repository | First becomes a Workspace / inspection / BuildGraph candidate; source visibility does not imply executability. |
| External URI, local executable, OCI, remote service | Produce a Foreign Capsule WorkRevision; concrete location exists only in Host-local bindings. |
| Content-only bundle | Produce content Work with no executable node. |

A Package is a replaceable component and capability distribution unit. `provides` projects to export Capability Ports and `consumes` projects to import Capability Ports. Resolution checks protocol, interface, version, profile, interaction, transport, effect, and multiplicity. Multiple equivalent providers are ambiguous; publisher identity is not a tie-breaker.

## Work pack

`plurora work pack`:

1. opens source descriptors through safe file handles;
2. parses Package Envelope / Component Descriptor or Foreign/content source;
3. emits and validates AssemblyRevision;
4. resolves authoring bindings and nested Assembly exposed ports;
5. emits AssemblyLock;
6. emits WorkRevision;
7. writes canonical bytes plus the complete closure to ObjectStore;
8. reports digests, closure, and structured diagnostics.

It does not write the Installation journal, create a Run, allocate a port, or mutate a profile.

## Installation create

`host.installation.create` accepts an explicit `work_id`, WorkRevision and AssemblyLock descriptors, display name, acquisition, state bindings, secret policy, and an idempotency key. A device call requires `installation.manage` plus the exact `host/work/<work_id>` selector; the CLI obtains that WorkId from the packed canonical WorkRevision. The Host revalidates that:

- artifacts exist and match descriptor SHA-256;
- `WorkRevision.work_id` decoded from canonical CAS bytes exactly matches the requested `work_id`;
- Work and AssemblyLock type URIs are correct;
- portable artifacts contain no local paths, raw secrets, or Host runtime facts;
- state slots and bindings are unique;
- Installation-local secret refs use an exact allowlist.

After the initial protocol check, the Host mints a mutation-authority sidecar that cannot be constructed on the wire. After acquiring the apply lock, and before closure/projection preparation and journal terminal commit, the service resynchronizes the HostAccess journal and revalidates the grant, expiry, delegation chain, action, and exact Work. Revocation, expiry, or a Work mismatch fails closed before a directory or terminal event is created. Success appends a journal event and atomically refreshes the `installation.json` projection. The projection is not authority and is rebuilt from the journal after restart.

## Update

Update requires `expected_revision` plus an explicit state action:

- `preserve`: retain state when contracts remain compatible;
- `replace`: switch to a validated state artifact;
- `reset`: explicitly abandon incompatible state.

The Host snapshots state before switching active Work/Lock pointers. CAS or later failure preserves the old pointer and records auditable rollback / recovery evidence.

`preserve`, `replace`, and `reset` all use the same Host-only sidecar and revalidate exact `host/installation/<installation-id>` authority at apply-lock, CAS/closure, projection, state-effect, and terminal-commit boundaries. Request fields never grant authority.

## Workspace ownership

A Workspace is mutable and Host-local:

- `managed`: Host-owned copy under `~/.plurora/workspaces/<workspace-id>/source/`;
- `linked_local`: binding to an existing user-owned directory.

Managed operations enforce canonical containment, symlink/reparse checks, and directory ownership. Installation update/remove never deletes, archives, or rewrites a linked-local source.

## Path and size boundaries

These limits correspond to identified failure modes rather than product quotas:

- source descriptors are capped at 1 MiB so arbitrary large files cannot masquerade as control-plane YAML/JSON;
- managed trees retain intake's 25,000 file/directory and 256 MiB budgets to prevent unbounded Host copies;
- source files, ancestor directories, and ObjectStore roots fail closed on symlink/reparse or containment change;
- logical safe-ID grammar never substitutes for filesystem containment.

## Secrets

Installation records store only references and policy, never raw keys:

```yaml
secret_policy:
  allow_platform_fallback: false
  allowed_secret_refs:
    - secret_ref:installation:OPENAI_API_KEY
```

Values resolve only inside Host executors. See [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md).

## Local layout

```text
~/.plurora/
├── objects/
├── installations/<installation-id>/
├── workspaces/<workspace-id>/
└── runtime/installations.sqlite3
```

New code does not read retired directories, descriptors, lockfiles, or profiles as Installation authority. There are no aliases, fallback readers, or migration paths. Tests use fresh temporary data directories.

## Checks

```bash
cargo test -p plurora-work
cargo test -p plurora-runtime install_lab
cargo test -p plurora-service installations
cargo test -p plurora-cli --test install_commands
```

See [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) for the full object boundary and journal semantics.
