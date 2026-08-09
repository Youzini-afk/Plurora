# Project Model

> [English](./PROJECT_MODEL.en.md) · [中文](./PROJECT_MODEL.md)

Project is the model used by the current official Host and distribution to organize installable, runnable instances. Each Project has a stable ID, entry point, state, data, secret policy, and lifecycle, and may run independently beside other Projects.

Project is important, but it is not the permanent root object required by every Plurora product. World, Document, Service, Workspace, Collection, and other protocol objects may retain their own identity; an adapter or product mapping can associate them with a Project when the official Home needs to manage them.

## Layer and boundary

```text
Constitutional Substrate
  identity, authority, objects, journal, invocation, streams, effects
             ↓
Protocols / Components / Content
  a Project may compose them without owning all of their meaning
             ↓
Host Control Plane / Official Distribution
  ProjectDescriptor, ProjectRegistry, data directories, lifecycle, Home mapping
```

Project belongs to the Host and distribution rather than the constitutional substrate. Contract V1 continues to expose Project lifecycle through public methods; third-party clients may use those methods or build a completely different distribution.

## Steam analogy

| Steam | Plurora |
|---|---|
| Steam client | Plurora platform |
| Game library | Home screen |
| Game card | Project card |
| Game save directory | Per-project data directory |
| Steam wallet | Platform secret store |
| Game-specific DLC key | Project secret store |
| Shared OS drivers | Shared capability packages |

## Project types

Three `project.type` values distinguish where a project came from.

### plurora_native

A repository root has `project.yaml` and references Plurora capability packages. This is the preferred form for projects designed for Plurora.

```yaml
schema_version: 1
project:
  id: my-project__abc12345
  title: My Project
  description: A short summary
  type: plurora_native
  entry_surface_id: my-namespace/play
  packages:
    - packages/foo/manifest.yaml
    - packages/bar/manifest.yaml
  secret_policy:
    fallback_to_platform: true
```

### external_wrapped

An external project, such as an ordinary git or npm repository, wrapped by an adapter package that actually exists. The current installer never fabricates an adapter manifest. `--wrap-as-adapter` fails closed and points to the later ChangeSet-approved adapter-authoring flow.

### external_workspace

An external project connected as an agent workspace, without wrapping. This fits temporary use and agent-assisted modification. The default is a host-owned managed copy; a local directory may instead use `--link-local` for an explicit user-owned mutable reference. Neither mode executes project code during intake.

## ProjectDescriptor

The top level of `project.yaml` is a `ProjectDescriptor`. It describes a project instance, not one package by itself.

Common fields:

| Field | Meaning |
|---|---|
| `id` | Stable project id for directories, CLI, and Home cards. |
| `title` | User-facing project name. |
| `description` | Text shown on Home cards and detail views. |
| `type` | `plurora_native` / `external_wrapped` / `external_workspace`. |
| `entry_surface_id` | Surface contribution id opened by Play. |
| `packages` | Required package manifest paths. |
| `optional_packages` | Optional package manifest paths. |
| `required_surfaces` | Surface ids the project expects to exist. |
| `secret_policy` | Project secret resolution policy. |
| `external` | External source, ref, workspace root, `source_kind`, `workspace_ownership`, and optional `source_digest`. |

`entry_surface_id` should match a package manifest surface with `slot: experience_entry`.
For example, YdlTavern uses `ydltavern/play`.

## Project directory layout

```text
~/.plurora/projects/<project_id>/
├── project.yaml          # ProjectDescriptor copy
├── secrets.dat           # age-encrypted project secret store
├── sessions/             # project-level session data
├── state/                # project-level state packages may use
└── lockfile.toml         # package versions locked for this project
```

A managed external workspace is stored separately:

```text
~/.plurora/workspaces/external/<project_id>/<content_digest>/
```

The descriptor's `workspace_ownership` controls uninstall authority. A `managed` path must be contained under that host-owned root before it can be archived/deleted. A `linked_local` source is always preserved.

Permissions: 0700 directories, 0600 files on Unix.
Encryption: the same master key, from `~/.plurora/secret-store.key` or the OS keyring.

## Soft isolation + platform fallback

“Soft isolation” here describes package/workload secret and data-sharing policy; it does not mean Host control-plane callers may cross project boundaries. Host device grants can now use structured, exact project selectors to limit project lists, sessions/events, development, deployment, and private routes. This is still neither a multi-user membership system nor a hard sandbox for untrusted workloads. Default secret behavior:

- The project's own secret wins (`secret_ref:project:NAME`).
- If missing in the project, fall back to platform when `secret_policy.fallback_to_platform: true`.
- If missing in both, fail closed.

The intent: a user can configure `OPENAI_API_KEY` once at the platform level and all projects can use it. A specific project can override with a project-level key in that project's settings. Both paths are visible to the user; fallback is not hidden.

Strong-isolation projects can disable fallback:

```yaml
secret_policy:
  fallback_to_platform: false
  require_per_project:
    - GITHUB_PAT       # must be configured per-project; platform fallback is not allowed
```

## Lifecycle

```text
plurora install <url>
  ↓ (detect project.yaml / run wizard)
Installed (registered in ProjectRegistry, visible in Home)
  ↓ plurora project start (or Home Play)
Starting → Running
  ↓ plurora project stop
Stopping → Stopped
  ↓ plurora uninstall
(ask what to do with data)
  ├─ Keep: archive project data under ~/.plurora/projects/.archived/<id>/ and archive a managed workspace
  └─ Delete: remove project data and a containment-verified managed workspace
```

A linked-local source does not belong to Plurora, so neither uninstall choice modifies it.

Any state can fail → Failed.

## CLI commands

```bash
# Install projects
plurora install github.com/user/repo
plurora install github.com/user/repo --workspace-only    # external project: workspace
plurora install ./existing-source --link-local           # local external project: keep user ownership
plurora install github.com/user/repo --wrap-as-adapter   # currently fails closed; never fabricates a manifest

# Inspect projects
plurora project list
plurora project info <id>
plurora project status <id>

# Control
plurora project start <id>
plurora project stop <id>
plurora update --project-id <id> [--check-only]

# Uninstall
plurora uninstall <id>                # interactive data prompt
plurora uninstall <id> --keep-data    # keep data (move to .archived)
plurora uninstall <id> --delete-data  # delete immediately
```

## Home screen

The `clients/web` Home route shows cards for all installed projects:

```text
┌─────────────────┐  ┌─────────────────┐
│   YdlTavern     │  │  Coding Agent   │
│   ●Running      │  │  ◯Stopped       │
│   [Play]        │  │  [Play]         │
└─────────────────┘  └─────────────────┘
┌─────────────────┐
│  + Install      │
└─────────────────┘
```

Status indicators:

- ● Running (green)
- ◯ Stopped / Installed (gray)
- ⏳ Starting / Stopping (yellow)
- ❌ Failed (red)

Clicking Play calls `host.project.start`, then navigates to the project's `entry_surface`.

The project page includes a platform-side console for bundle, package, recent-event, update, and deployment diagnostics, plus host-plane durable job / revision / recovery state. Update checks and execution use `plurora/install-lab/check_for_updates` / `update_project` through the public `capability.invoke` path.

## Play flow

After a user clicks Play on a Home card, the web shell and host follow a fixed public-protocol sequence:

1. The user clicks Play on a project card.
2. `clients/web` calls `host.project.start`.
3. The host transitions the project to Running and creates or reuses a project session.
4. The Host stores the verified `project_id` in session `metadata.project_id` and adds a `project:<id>` label.
5. `project.start` returns `session_id` and `already_running`.
6. `clients/web` calls `host.surface.bundle.resolve` to resolve the project's `entry_surface_id` to a surface bundle URL.
7. `mountSurface` mounts a sandboxed iframe.
8. The iframe `initialProps` include `sessionId` and `projectId`.
9. Inside the surface, `callHostRpc` / `invokeCapability` automatically carries `session_id`.
10. The host carries the authenticated principal, resource authority, and server-verified session/project binding into later capability and outbound dispatch.

This chain lets project-level secret resolution find the project scope from session metadata, and it keeps real model calls in the same project session. For the end-to-end path, see [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.en.md).

Note: this `sessionId` is then used for:

- All RPC calls, which carry it automatically (`callHostRpc` reads it through `setActiveSessionId`).
- Streaming calls (`streamCapability`), which use it as the subscription scope for receiving `capability/stream.*` events.

## Explicit deployment

`project.start` does not start external processes. It only opens a project session and marks the project Running.

If a project needs a Docker HTTP service, it can declare a minimal descriptor under `project.metadata.deployment.docker`. The web project console then shows Deploy / Stop buttons. After user confirmation, the `plurora-service` host broker runs the chain while the browser remains a thin client:

1. `host.port.lease` leases a loopback port.
2. `plurora/docker-runtime-lab/start_container` starts the container.
3. `host.proxy.register` registers the HTTP/WebSocket reverse-proxy route.

This path is explicit. It never runs automatically when opening a project. See [`DEPLOYMENT_RUNTIME.md`](DEPLOYMENT_RUNTIME.en.md).

## Protocol

Host project-management methods allow HostAdmin/HostDev, or a logical HostDevice with the corresponding action and exact project selector. Its Contract V1 context uses the `anonymous` sentinel plus an authority envelope; ordinary packages cannot call these methods:

```text
host.project.list      list installed projects
host.project.get       get project details
host.project.start     start a project
host.project.stop      stop a project
host.project.status    get project status
```

Deployment runtime protocols allow HostAdmin/HostDev, or a logical HostDevice with `deploy` / `observe` and a matching target selector; ordinary packages cannot call them:

```text
platform.target.*   execution targets
platform.exec.*     controlled local execution
platform.port.*     loopback port leases
platform.proxy.*    HTTP/WebSocket routes
```

Lifecycle events:

```text
host/project.installed
host/project.started
host/project.stopped
host/project.uninstalled
```

## Boundary with Work / Assembly

| Work / Assembly | Project (transitional legacy Host lifecycle) |
|---|---|
| Portable content and component-assembly Artifact DAG | Runtime record and state on one Host |
| Validated and packed by `plurora work check/pack` | Temporarily managed by `plurora project list/start/stop` |
| Shared as an Artifact closure without host paths or secrets | May contain Host-local runtime information |
| Work, Assembly, Ports, and AssemblyLock | Legacy Project identity; never the permanent Work identity |

One WorkRevision can produce multiple independent Installations and Runs on different Hosts. The current Project API remains a transitional Host-lifecycle implementation; it does not replace WorkRevision and must not write host-local state back into Work content identity.

## Install detection

`plurora install <url>` detects source and project kind before deciding whether to resolve a package manifest.

- Present with `type: plurora_native`: install as a native project.
- A valid package manifest: resolve and install as a package source.
- No project/package manifest: invoke `plurora/install-lab/prepare_external_intake` and create an `external_workspace`.
- Present but invalid: fail closed and require descriptor fixes.

An external project defaults to a managed `external_workspace`, copied/fetched into a host workspace isolated by project id and content digest. `--link-local` is local-source-only and explicitly preserves user ownership. Reinstalling the same source/content is idempotent, and intake never generates wrapper code or executes project scripts.

## Non-goals (deferred)

- Multi-user project membership / access control
- Project import/export bundles (sharing-lab already has bundle formats)
- Multi-user project membership, workload-grade hard sandboxing, and cross-Host project authority
- Automatic project archive cleanup (manual for now)
- Project marketplace (against the platform's open principle)
