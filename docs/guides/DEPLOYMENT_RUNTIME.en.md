# Deployment Runtime

> [English](./DEPLOYMENT_RUNTIME.en.md) · [中文](./DEPLOYMENT_RUNTIME.md)

Plurora can now host self-hosted AI / agent projects. Deployment is not a Docker concept in the kernel. It is a small set of generic runtime primitives that ordinary packages and Host target drivers compose into Docker, native process, or enrolled remote-Agent targets.

## Boundary

The kernel exposes four generic primitive families:

| Primitive | Protocol family | Role |
|---|---|---|
| target | `platform.target.*` | Describe an execution target. Built-in `local` and enrolled remote Agents enter the same registry/driver contract. |
| exec | `platform.exec.*` | Start, stop, and inspect controlled local execution. Deny-all by default. |
| port | `platform.port.*` | Lease a loopback port from the host. |
| proxy | `platform.proxy.*` | Bind a managed HTTP / WebSocket route to a port lease and explicitly record Host-authenticated or public access. |

Docker, git, installation, secret storage, workspaces, and adapters are not kernel concepts. Ordinary capability packages implement them.

## Current implementation

- `LocalExecExecutor` trait: defaults to `DenyAllLocalExecExecutor`; profiles may opt into `LiveLocalExecExecutor`.
- `LiveLocalExecExecutor`: accepts argv arrays only, never shell strings. cwd, env, logs, timeout, and kill behavior are host-controlled.
- `plurora-service` reverse proxy: `/p/<route_id>/...` remains available inside Host authentication. A route gets an additional unauthenticated `<slug>.apps.example.com/` virtual host only when it explicitly selects `public` and `PLURORA_APP_BASE_DOMAIN=apps.example.com` or `--app-base-domain apps.example.com` is configured, allowing a community app to own `/`. Both entry modes can only point at active loopback port leases. Redirects are disabled, dangerous response headers are stripped or rewritten, response bodies are bounded, and HTTP + WebSocket are supported.
- `plurora/docker-runtime-lab`: an ordinary first-party capability package using `bollard` to manage Docker containers. It fails closed when Docker is unavailable; real Docker smoke requires opt-in.
- Target drivers: built-in `local` and enrolled Agents use the same durable operation, artifact-transfer, declarative-verifier, deployment apply/stop, and receipt model. Agent upstreams remain loopback-only and return to the Host proxy through an authenticated tunnel bound to target/route/lease/epochs.
- Web project console: shows target / exec / port / proxy diagnostics plus host-plane active revision, recovery state, revision history, and recent jobs. If a project declares deployment metadata, the user explicitly chooses Host-authenticated or public route exposure before Deploy / Stop or Build & Deploy, recover, or rollback. The Development area can also move a verified ChangeSet through private preview, separate deployment approval, activation, and interrupted-operation reconciliation. Host-authenticated is the default.
- Persistence and replay: exec / port / proxy registry mutations are written to the event log and replayed to rebuild registries on host restart.
- Restart reconciliation: after a restart, replayed records are first downgraded (exec → unknown, port → reserved, proxy → stale with `ready=false`), then reconciled against the real world.
- Readiness gating: proxy routes register with `ready=false`; the reverse proxy returns 503 for routes that are not yet ready, and only forwards once ready.
- Health supervision: a host background loop periodically probes each active route's upstream, flips `ready=false` on sustained failure and `ready=true` on recovery, and writes an audit event on each transition.
- Build & Deploy broker: `POST /host/v1/build-deploy` creates a durable job intent with an optional `idempotency_key`. The host clones source, builds via Dockerfile / nixpacks, starts the container, registers proxy, and runs readiness probing. The browser observes job status / SSE and may cancel the job.

## Docker deployment descriptor

Until Phase 6 Realization replacement is complete, explicit deployment requests may still submit minimal Docker metadata. These fields are Host-local operation data and never enter WorkRevision:

```yaml
project:
  metadata:
    deployment:
      docker:
        image: ghcr.io/example/app:latest
        container_port: 3000
        port_name: web        # optional, default: web
        route_id: my-app-web  # optional, default derived from Installation
        route_access: host_authenticated # optional; host_authenticated | public
        health_path: /healthz # optional, used for the readiness probe
        pull_if_missing: false
```

The current web broker accepts only these fields. Missing `route_access` and older descriptors resolve to `host_authenticated`; public access must also be selected explicitly in the deployment UI. `env`, `volumes`, `mounts`, `binds`, and `secrets` are rejected so the first deployment path cannot silently expand authority.

## Build & Deploy descriptor

If a project has no prebuilt image, it can declare source build metadata under `project.metadata.deployment.build_deploy`:

```yaml
project:
  metadata:
    deployment:
      build_deploy:
        source_url: https://github.com/example/app.git
        ref_name: HEAD
        strategy: dockerfile # dockerfile | nixpacks
        dockerfile_path: Dockerfile # optional
        container_port: 3000
        port_name: web
        route_id: my-app-web
        route_access: host_authenticated # host_authenticated | public
        health_path: /healthz
        runtime_env:
          - name: NODE_ENV
            value: production
          - name: OPENAI_API_KEY
            secret_ref: project:OPENAI_API_KEY
        runtime_mounts:
          - source_host_path: /srv/plurora-data/my-app
            container_path: /app/data
            mode: ro
            approved: true
            high_risk_approved: false
            reason: persistent app data
```

The `dockerfile` strategy uses a Dockerfile from the repository. The `nixpacks` strategy first runs local `nixpacks` to generate Dockerfile / context, then `docker-runtime-lab` builds the image through Docker. If `nixpacks` is unavailable, the build fails closed.

Runtime secrets must be `store:` / `project:` / `env:` `secret_ref`s. Raw secret values are injected by a host-private Docker runner and never cross the `docker-runtime-lab` package boundary or enter events, logs, or job state. Build-time secrets are not supported yet and fail closed.

Volumes may point to arbitrary host paths, but every mount needs explicit approval. Read-only is the default recommendation; read-write mounts require an extra confirmation. The host denies Docker sockets, system directories, secret directories, Plurora secret storage, broad home directories, and ancestor paths that would implicitly include them.

## Explicit Deploy flow

The Deploy button in the project console never runs automatically. After user confirmation, the request is sent to the host-plane `POST /host/v1/deploy`, where the host broker drives the whole chain server-side (the browser is a thin client and no longer orchestrates):

1. The host re-validates the request (client fields are not trusted).
2. `host.port.lease`: lease a loopback port.
3. `capability.invoke` → `plurora/docker-runtime-lab/start_container`: start the Docker container with `approved: true`, `host_port`, and `port_lease_id`.
4. `host.proxy.register`: bind the route and explicit `route_access` to that port lease (registered with `ready=false`).
5. Readiness probe: TCP-connect to the loopback port (with an optional health_path HTTP probe). The route is flipped to `ready=true` and success returned only if the probe passes within a bounded timeout.

If any step fails, the broker rolls back in reverse: unregister proxy, stop the just-started container, release the port lease. Because orchestration is host-side, closing the browser tab does not leave orphan containers or port leases.

Stop deployment (`POST /host/v1/deploy/stop`) finds and stops the container by Docker label (`route_id`). It does not rely on a container id remembered by the browser, and does not stop unknown containers based on a matching `port_name`.

## Virtual-host routes

Path-prefix proxying (`/p/<route_id>/...`) is convenient for platform diagnostics, but real community apps often assume they own `/`. Frontends call `fetch('/api/...')`, load assets from `/assets/...`, and open WebSockets at `/ws`. plurora-service therefore supports an optional virtual-host entry:

```bash
plurora host serve --app-base-domain apps.example.com
# or
PLURORA_APP_BASE_DOMAIN=apps.example.com plurora host serve
```

Configuring a base domain does not publish any route by itself. Only a route registered as `public` gets a DNS-safe slug derived from `route_id` and a public URL such as `https://<slug>.apps.example.com/`. A `host_authenticated` route, or any deployment without a base domain, keeps the Host-authenticated `/p/<route_id>/` URL.

Boundary rules:

- `ProxyRouteAccess` is a generic proxy-route property. Hostname derivation remains service-layer behavior; the kernel does not know DNS.
- Only `<slug>.<app_base_domain>` matches. Arbitrary hosts, the bare base domain, and fake suffixes such as `foo.apps.example.com.evil.com` do not match.
- `X-Forwarded-Host` is not trusted.
- Only a `route_access=public` vhost entry bypasses Plurora Host identity; it is the deployed app's public entry. A private vhost returns 404, while `/p`, RPC, and Host APIs still require Host identity and scopes.
- Upstream must still be a loopback lease, and the route must be active + ready.
- vhost requests set upstream `Host` to the app hostname; `Authorization`, Plurora `access_token` query values, and `Referer` are not forwarded.
- vhost responses strip the `Domain` attribute from `Set-Cookie`, making cookies host-only. `Location` is only rewritten for same-upstream redirects; external absolute redirects are still stripped.

## Build & Deploy flow

Build & Deploy uses `POST /host/v1/build-deploy`. By default it returns immediately with `job_id`, a status URL, and an SSE events URL. Long-running work stays in the host broker:

1. Validate source URL, strategy, runtime env, runtime mounts, and user approvals.
2. Clone into the project workspace through `git-tools-lab`. The project and workspace ancestors must be real directories under the canonical data root; selected-tree materialization fails closed above 100,000 files, 100,000 directories, or 1 GiB. Unsupported tree modes such as submodule entries, absolute/root-escaping symlinks, and symlink entries on platforms that cannot preserve them fail explicitly. The current transport still performs a temporary bare fetch, so these tree limits do not yet constitute a repository-download budget.
3. If strategy is `nixpacks`, generate Dockerfile / context first.
4. Call `plurora/docker-runtime-lab/build_image` and label the image with `installation_id`, `workspace_id`, `build_id`, `source_commit`, `strategy`, and `build_descriptor_hash`.
5. If the Installation already has an active revision, handle its container, route, and lease only after the new image has built. The old revision remains the durable active pointer until the replacement commits, so replacement failure becomes an explicit recovery-required state.
6. Enter the normal deploy chain: port lease → container start → proxy register → readiness probe.
7. After readiness succeeds, append the revision activation event before moving in-memory state to Ready. If the journal commit fails, roll back the new deployment.

Job intent, the latest state snapshot, immutable deployment revisions, and the active pointer are written to the current profile's `EventStore`. SQLite / Postgres profiles therefore restore the control plane across host restarts; the in-memory profile remains development-only. An incomplete job is deterministically marked Failed after restart, and the host never automatically replays clone / build / deploy side effects. The full live log remains a bounded in-memory ring; the journal retains only redacted state and the last event.

Every successful Build & Deploy creates a `DeploymentRevision` containing source ref, build artifact identity, `route_access`, route configuration, and a redacted receipt. It never stores raw secrets or host mount paths. A revision is automatically recoverable only when every runtime env value came from a `secret_ref` and no host mount was used. Plain env values and mounts become explicit blockers that require a manual rebuild. Recover and rollback preserve the revision's route-exposure choice. Journal events remain immutable; the live control-plane projection and API retain the most recent 64 revisions per project so restart memory and response size remain bounded.

## Verified ChangeSet preview and activation

This path accepts only a committed `managed_external` ChangeSet, a `docker_build` verification result, and complete provenance. The verification image is removed after verification. Deployment consumes the immutable build-context artifact, never that image or the live workspace.

1. `POST /host/v1/development/workspace/<workspace_id>/changes/<change_set_id>/deployment/preview` revalidates the descriptor, tree, verification/build-context artifacts, and Workspace/Installation/Target authority, then performs typed artifact transfer, Docker build, and deployment apply on an explicit `local` or Agent target. The generated preview route is always `host_authenticated`.
2. `POST .../deployment/approve` separately approves or rejects the exact preview. Its approval artifact binds the candidate receipt, artifact refs, target, and authority; source approval never implies deployment approval.
3. `POST .../deployment/activate` revalidates all evidence and readiness, points the requested private or explicitly public route at that same candidate, commits an immutable `VerifiedActivate` revision, and only then drains the previous revision.
4. A Host crash or uncertain effect during preview/activation moves the transaction to `recovery_required`. `POST .../deployment/reconcile` only adopts a provenance-identical durable activation or cleans the exact candidate/route/lease; ambiguous state remains blocked.

Project-scoped host APIs:

- `GET /host/v1/installations/<installation_id>/deployments`: active revision, runtime readiness, recovery requirement, jobs, and revision history.
- `POST /host/v1/installations/<installation_id>/deployments/recover`: explicitly recover the active revision. An ordinary `GitClone` revision reuses its retained local image without cloning/building; a `VerifiedArtifact` revision revalidates evidence and rebuilds from durable build context on its recorded target.
- `POST /host/v1/installations/<installation_id>/deployments/rollback`: activate a historical revision as a new immutable rollback revision. Ordinary revisions reuse retained images; verified revisions rebuild from their durable context on the recorded target. Rollback remains available after explicit stop removes the active pointer, and historical records are never mutated.
- `POST /host/v1/deploy/stop`: clean up resources for a route and append a deactivation event when it belongs to the active durable revision.

Recover and rollback are explicit user actions. Ordinary revisions must be replay-safe, retain their local image, and still resolve referenced secrets. Verified revisions require a valid artifact closure, preview/approval evidence, and current project/target authority. Verified replay never reads the live workspace or refetches source. Failure preserves the prior active pointer and reports recovery required rather than silently claiming success. Direct prebuilt-image `/host/v1/deploy` remains a transient broker operation and does not create a durable revision yet.

## `project.start` does not deploy

Installation `ready` means only that the adoption record is valid. It does not start a process, allocate a port, or register a proxy. Phase 4 introduces independent Run / Exposure lifecycle; Phase 6 replaces this transitional deployment controller with Realization.

Deployment is a separate, explicit host-broker action. This keeps “open project UI” and “run an external service” visibly separate.

## Red lines

- Deny-all by default. No profile opt-in means no real local process start.
- Ports bind to loopback only.
- Proxy upstreams must reference active port leases, and `port_name` must match.
- Proxy routes default to `host_authenticated`. A public vhost requires explicit `route_access: public`; configuring a wildcard domain never publishes existing routes in bulk.
- Path-prefix proxying does not follow upstream redirects and strips `Set-Cookie`, `Location`, CORS, and other dangerous response headers. Vhost proxying only allows host-only cookies and same-upstream `Location` rewrites.
- Prebuilt-image deployment does not accept env / volume / secret fields. Source Build & Deploy accepts only explicitly approved runtime env / volume fields; raw runtime secrets are injected only inside the host-private runner, and build-time secrets are not supported yet.
- Verified ChangeSet deployment never reuses the verification image, builds from the live workspace, or bypasses separate deployment approval; preview always starts Host-authenticated.
- Docker is implemented by an ordinary capability package. No first-party fast path.

## Next

- Native execution remains trusted/dev-oriented. It is not a full OS sandbox.
- **Auto-restart** is not implemented and remains a separate future phase. The host-plane now has durable revisions and explicit recovery, but health supervision still only monitors, flips readiness, and audits; it never replays deployment side effects without user authorization.
- The Remote Target Agent, Project Console, and verified development-to-deployment wiring now form a Candidate loop, with GitHub CI covering faults, Host restart, recovery, and rollback. Target-edge ingress and application identity still need separate designs; this is not an arbitrary network proxy.
- Docker descriptors still lack pull progress and long-term log archival.
- External-project ChangeSets can now add a controlled Dockerfile. Richer guided deployment-descriptor/adapter authoring remains follow-up work, and deployment must retain explicit approval and activation.
