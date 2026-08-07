# Host Remote Access and Route Exposure

> [English](./HOST_REMOTE_ACCESS.en.md) · [中文](./HOST_REMOTE_ACCESS.md)

Plurora's Web/PWA, Desktop, and CLI are clients of the same Host. Remote access does not create a second mutation interface and does not copy the root token onto a phone. It adds revocable, expiring device identities attenuated by both actions and structured project/target resources in front of the existing Host API and RPC. Public application traffic is a separate, explicit data-plane boundary; configuring a domain does not publish routes implicitly.

## Two planes

```mermaid
flowchart LR
  D["Desktop / root operator"] -->|"root credential"| C["Host control plane"]
  M["Mobile PWA / paired device"] -->|"scoped device cookie"| C
  C --> A["RPC + Host API + authenticated /p routes"]
  V["Public visitor"] -->|"public vhost only"| P["Explicitly public app route"]
  P --> L["Active loopback port lease"]
  A --> L
```

- **Host control plane:** projects, deployment, ChangeSets, access management, and `/p/<route_id>/...`. A root or device identity is required and checked against action scopes.
- **Application data plane:** only a route with `route_access: public` can be reached without Host authentication through `<slug>.<app_base_domain>`.
- Static Web assets and the `/pair` page carry no authority. Reads and mutations remain behind protected APIs. The public pairing endpoints only inspect or claim a high-entropy one-time token already held by the caller.

## Identities

| Identity | Credential | Purpose |
|---|---|---|
| Host root | Bearer token from `PLURORA_HTTP_ACCESS_TOKEN` / `--access-token`; Desktop may exchange a one-time bootstrap nonce for a root cookie | Local administration, first authorization, and recovery; owns every scope |
| Paired device | `plurora_access.*` token; after PWA claim it exists only in the `__Host-plurora_remote_session` cookie | Routine remote control; owns only the grant's scopes and project/target selectors |

Optional authentication with no configured root token is a loopback development mode. `host serve` refuses a non-loopback bind without a non-empty root token. The root token is a root credential and must never enter a pairing URL, browser persistence, application upstream, or logs.

## Scopes

| Scope | Authority boundary |
|---|---|
| `observe` | Read Host / project / package / target / exec / port / proxy state and open Host-authenticated `/p` routes |
| `project_operate` | Start and stop projects and manage project sessions |
| `deploy` | Deploy, cancel deployment jobs, and mutate target / exec / port / proxy state |
| `develop_propose` | Read development ChangeSets and draft a new ChangeSet |
| `develop_approve` | Approve or reject the exact ChangeSet |
| `develop_execute` | Execute or recover an approved ChangeSet |
| `access_manage` | Inspect devices and invitations, create/cancel pairings, and revoke grants |

Unknown HTTP paths, unknown RPC methods, and broad administrative mutations require `access_manage`, so scoped devices fail closed. A new grant must be a subset of the caller's authority and must contain `observe`; only root can delegate `access_manage`. The Web UI selects only `observe` by default, and each additional action is explicit.

## Project and target resources

A grant's `resources` are structured selectors shaped as `{ kind: "project" | "target", id?: string }`. Omitting `id` selects every resource of that kind; an explicit id is compared structurally and exactly, never by string prefix. Legacy journal grants without `resources` rehydrate with the compatible all-projects + all-targets meaning. New Web and CLI pairings submit selectors explicitly.

The server checks or filters HTTP project paths and static project bundles, canonical and legacy RPC, project lists, sessions/events, ChangeSets, deployment jobs/revisions, private `/p` routes, and target/exec/port/proxy objects. A device identity is no longer collapsed into `HostDev` at `/rpc`. The Host writes and verifies `metadata.project_id` on project sessions; forks preserve the binding, and a caller-supplied `session_id` is not authority by itself.

A child grant cannot exceed its parent's scopes, resources, or expiry. Authentication validates the complete delegation chain, so revoking or expiring any ancestor invalidates descendants immediately. Device protocol allow/deny decisions are written to the `host_control_authority` journal with grant, delegation, canonical method, action, structured resources, and correlation—but never tokens, cookies, or raw request parameters.

A sandboxed surface frame has an opaque origin and cannot safely carry a Host Cookie or bearer token. After validating project authority, `host.surface.bundle.resolve` replaces the internal `/surface-bundles/...` location with a random, five-minute, read-only `/surface-assets/<lease>/...` handle constrained to that bundle root; relative modules and CSS stay under the same root. The handle is bound to the device grant and fails immediately after revocation or expiry, while raw bundle paths still require a Host identity. It contains no Host credential and cannot call RPC or read another project.

## Pairing lifecycle

1. A client with `access_manage` calls `POST /host/v1/access/pairings` with a device name, scopes, project/target selectors, and expiration.
2. The Host returns a one-time `plurora_pair.*` token valid for at most ten minutes. The Web UI places it in `/pair` under an operator-supplied HTTPS Host origin.
3. The new device removes the token from the address bar immediately and retains it in memory only. It first calls the public inspect endpoint so the user can verify device name, scopes, and expiry.
4. On confirmation, the public claim endpoint atomically consumes the pairing, creates a grant valid for at most 365 days, and sets a Secure, HttpOnly, SameSite=Strict, host-only cookie.
5. Expired or revoked grants fail on the next authentication check. Revoking the current device also clears its cookie. A pending pairing can be cancelled before claim.

Routes:

| Authentication | Method / route | Purpose |
|---|---|---|
| public + pairing token | `POST /host/v1/access/pair/inspect` | Inspect an invitation before consuming it |
| public + pairing token | `POST /host/v1/access/pair` | Claim a grant once |
| any Host identity | `GET /host/v1/access/me` | Inspect the current identity, scopes, resources, and delegation chain |
| `access_manage` | `GET /host/v1/access` | Inspect grant and pairing projections |
| `access_manage` | `POST /host/v1/access/pairings` | Create an invitation |
| `access_manage` | `POST .../pairings/:id/cancel` | Cancel a pending invitation |
| `access_manage` | `POST .../grants/:id/revoke` | Revoke a device grant |
| `access_manage` | `POST /host/v1/access/grants/revoke` | Atomically revoke up to 256 device grants |
| any Host identity | `POST /host/v1/access/logout` | Clear browser Host cookies |

## Persistence and credential boundary

- Pairing and grant transitions are written to the dedicated `host_control_access` EventStore journal. SQLite and PostgreSQL Hosts rehydrate the same projection after restart.
- The journal stores only domain-separated SHA-256 credential digests, never pairing tokens, access tokens, or cookie values.
- Pairing claim/cancel and grant revoke use expected-tail compare-and-append. Only one concurrent claim can commit; bulk revoke validates every grant before committing one sorted, bounded journal transition and is idempotent on retry.
- Grant revocation and expiry are checked on every authentication, rather than relying on the browser to refresh state.
- Delegated grants retain `parent_grant_id` and `delegation_depth`; authentication walks ancestors fail-closed, and parent revocation cascades.
- Bearer and cookie credentials have explicit precedence. Query credentials are accepted only by `GET /journal/subscribe/:session_id` and `GET /host/v1/build-deploy/:job_id/events`, the two browser SSE entry points; no other route treats a URL token as a credential.

## CLI management

The CLI uses the same Host API as Web/PWA and never writes the grant journal directly:

The CLI permits plaintext HTTP only for loopback. Non-loopback Hosts require HTTPS, Host origins cannot contain paths or credentials, and Host-access requests do not follow redirects. Saved connection profiles contain only a display name, endpoint, and per-Host project/target context; the access token remains an explicit argument or environment variable.

```bash
plurora host connection save workshop --endpoint https://host.example.com
plurora host connection context --project my-project__abc12345 --target remote-builder
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" me
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" projects
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" project-status
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" target-status
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  pair --device-name phone --scopes observe,project_operate,deploy \
  --project my-project__abc12345 --target local
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  revoke <grant-id>
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  bulk-revoke <grant-id> <grant-id> ...
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 list
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 draft --request change.json
```

`changes` also exposes `get`, `bundle`, `approve`, `reject`, `execute`, and `recover`; every command uses the same public ChangeSet API as Web/Desktop/PWA. `--endpoint` / `PLURORA_HOST_URL` still overrides the selected connection for one command. `plurora host connection local` returns to the default loopback Host.

## HTTPS and same-origin requirements

Remote PWA control requires an HTTPS origin. The pairing screen refuses claim over plaintext HTTP because the `__Host-` cookie must be Secure, host-only, and use `Path=/`.

A production topology places the Host behind a TLS reverse proxy or trusted overlay:

```bash
PLURORA_HTTP_ACCESS_TOKEN='<high-entropy-root-token>' \
  plurora host serve --http 0.0.0.0:8787 --static-dir clients/web/dist
```

Firewall the plaintext port so only the proxy/overlay can reach it; expose an origin such as `https://host.example.com`. The proxy must preserve the original `Host` and allow the browser's `Origin` to reach the Host. Cookie pairing remains same-origin and credentialed cross-origin CORS is disabled. The shared Web/PWA/Desktop client may call an explicitly selected remote Host cross-origin with a Host-scoped Bearer token; control routes permit only `GET`/`POST` plus `Authorization`/`Content-Type` and never enable credentialed requests. Proxy and raw surface-bundle routes do not emit this CORS policy. Browser tokens are isolated per Host connection. Project surfaces load their sandbox frame and attenuated assets from the selected Host, so a Host used for surface rendering must serve the matching Web static bundle.

## Application route exposure

`host.proxy.register` and deployment descriptors use:

```yaml
route_access: host_authenticated # default; old descriptors resolve this way
# route_access: public           # requires an explicit user choice
```

- `host_authenticated`: exposes only `/p/<route_id>/...`, inside Host authentication and requiring at least `observe`.
- `public`: when `--app-base-domain` is configured, additionally enables the derived vhost; only that vhost bypasses Host authentication. Without a base domain, the authenticated `/p` fallback is still the only entry.
- Route access is written into proxy registration events and durable deployment revisions; recover and rollback preserve the original choice.
- A public vhost does not forward Host `Authorization`, Plurora query tokens, Host session cookies, or `Referer` to the app. The upstream must remain an active, ready loopback lease.

A public route's application owns internet-input validation, application identity, CSRF protection, rate limiting, and content security. A Plurora Host grant is not an application user system.

## Deliberately absent

- Target-edge ingress, application identity, arbitrary network transport, or remote package transport. Remote deployment ports remain loopback-only and are reached only through the authenticated Target Agent tunnel.
- Multi-user project membership, workload-grade hard sandboxing, and cross-Host delegation chains. Current project selectors are a single-Host control-plane boundary.
- Automatic root-token synchronization to phones, or a local CLI mutation path that bypasses the Host API.
- Deployment, public routes, or side-effect replay without explicit user confirmation.
- Application login, public CORS, or internet-edge protection supplied on behalf of deployed apps.
