# Host Resource Authority

> [English](./HOST_RESOURCE_AUTHORITY.en.md) · [中文](./HOST_RESOURCE_AUTHORITY.md)

Status: **Candidate implementation**. Host Access attenuates authenticated root, device, CLI, Web/PWA, Desktop, and future Agent identities into action scopes plus structured resource selectors. It protects Host-local Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization resources without promoting those objects into constitutional-substrate concepts.

## Invariants

- HTTP, Cookie, Bearer, stdio, and in-process transports share the same authentication and authorization semantics.
- Request bodies cannot override the authenticated principal, grant, delegation chain, or verified resources.
- The Contract Registry resolves the exact public method before policy; unknown methods have no fallback.
- Lists are filtered by visible resources on the server and are never returned in full for the client to hide.
- Session ids, URL parameters, display names, and local paths are not capabilities.
- Allow/deny decisions can link principal, grant, resources, and later receipts without recording credentials or raw secrets.

## Action scopes

Current action wire values:

```text
observe
installation.manage
run
binding.manage
exposure.manage
realization.plan
realization.apply
develop.propose
develop.approve
develop.execute
access_manage
```

Low-level target/exec/port/proxy adapters are not ordinary-device Realization authority; they are confined to HostAdmin/HostDev or an explicit `access_manage` administration boundary. Managed lifecycle is exposed only through `realization.plan` / `realization.apply`.

Exposure/Binding and Realization methods use these actions:

| Action | Methods | Exact resources |
|---|---|---|
| `observe` | `host.exposure.list`, `host.binding.list`, `host.binding.candidates` | visible provider/consumer Installations, Runs, Exposures, Bindings, and Ports |
| `exposure.manage` | `host.exposure.create`, `host.exposure.revoke` | provider Installation + Run + export Port; revoke also requires the exact Exposure |
| `binding.manage` | `host.binding.select`, `host.binding.revoke` | consumer Installation + import Port + Exposure; revoke also requires the exact Binding; Runtime may carry an exact Run pin |
| `realization.plan` | `host.realization.plan` | exact Installation + Target; creates and persists a content-addressed plan without target effects |
| `observe` | `host.realization.list`, `host.realization.get` | list filters visible Installation/Target/Realization resources; get requires exact Installation + Realization |
| `realization.apply` | `host.realization.apply`, `stop`, `rollback`, `reconcile` | exact Installation + Target + current Realization; rollback also requires the exact historic Realization |

The composite Port resource ID is `<installation-id>/<port-id>`. It is a stable resource-selector composition, not a filesystem path, credential, or handle.

## Resource selectors

Resource kinds are:

```text
work
workspace
installation
run
target
exposure
binding
realization
```

Wire shape:

```json
{"kind":"installation","id":"018f2b74-..."}
{"kind":"installation","id":null}
```

`id: null` is an explicit wildcard. Omitting `id` rejects rather than granting global visibility. A child grant's actions, resources, expiry, and delegation depth must all be subsets of its parent authority. Exposure audiences must also list exact selectors; the candidate relay returns only caller-visible providers whose audience matches.

## Call contexts

After authentication, every transport constructs a context that the request cannot override:

```text
AuthenticatedCallContext
  principal_ref
  credential_kind
  grant_ref?
  delegation_chain[]
  authority_refs[]
  transport
  audience_host_id
  issued_at / expires_at?
  correlation_id / parent_invocation_id?
```

Before dispatch, the Host extracts resources from parsed parameters and server-side projections:

```text
HostOperationContext
  authenticated_call
  action
  resources[]
  installation_ref?
  workspace_ref?
  run_ref?
  target_ref?
  operation_ref?
  policy_decision_ref
```

Runtime code consumes only this verified context or an attenuated handle minted from it. The Host journal stores durable authority; the public relay sends filtered typed projections only. Private intent, hashed-only grant basis, and handles never cross the wire or enter UI/events.

## Fixed authorization order

1. Authenticate the transport and establish call context.
2. Resolve the exact method through the Contract Registry.
3. Extract resources from parameters and server-side projections.
4. Validate ownership and cross-reference consistency.
5. Intersect action × resources × authority.
6. Append a redacted policy decision.
7. Only then enter runtime code or produce an external effect.

Unknown resources, ownership conflicts, missing selectors, expired grants, and revoked ancestors all fail closed.

## Installation and Run

- `host.installation.list|get` require `observe`; list returns only visible Installations.
- `host.installation.create` requires `installation.manage` plus the exact Work named by the requested `work_id`; `WorkRevision.work_id` in canonical CAS must match it. `update|remove` require the exact Installation.
- Runtime mints a non-wire-constructible Host-only sidecar for create, every update state action (including `preserve`), and remove. After waiting for the apply lock and before each durable/effect boundary, the service synchronizes the HostAccess journal and rechecks grant activity, expiry, delegation, action, and exact resource. Request fields never grant authority.
- Installation-local secret scope comes from a Host-verified Installation context.
- Run start requires `run` plus an exact Installation. The Host generates RunId after preflight. Later get/stop calls carry both I/R, and the registry must prove that the Run is a child of that exact Installation before deriving authority for that child request. The rule never crosses Installations and is not a first-party private bypass. Host restart marks active Runs `interrupted`.
- Package surfaces receive only short-lived, method-allowlisted attenuated handles, never root/device credentials.
- Binding runtime injects a least-authority handle only into the selected Component. Provider stop, Exposure revoke/expiry, owner revoke, or version drift is rechecked at every effect barrier and invalidates the Binding.
- A Realization plan pins the Installation revision, Target inventory, and plan digest. Apply/stop/rollback/reconcile revalidate the current grant, exact resources, owner lease, and revision before every durable/effect boundary. A plan or approval is not authority, and private effect checkpoints/receipts never enter the public wire.

## Audit linkage

Sensitive calls link principal, credential kind, grant id, delegation-chain digest, canonical method, action, resource refs, allow/deny reason, correlation/causation, and the resulting receipt or terminal failure. Raw credentials, Cookies, secret values, and complete request payloads never enter the journal.

## Threats and defenses

| Failure mode | Defense |
|---|---|
| Grant for Installation A operates on B | Cross-check exact selectors against server-side projections |
| Missing selector id becomes a wildcard | Wire requires explicit `id: null` |
| Transport-specific alias bypasses policy | Every transport shares resolved `PlatformMethod` and one policy table |
| Device identity becomes unconstrained HostDev | Preserve authenticated principal and grant envelope |
| Lists or event streams leak other resources | Server-side filtering and fixed subscription scope |
| Revoked grants create new effects | Rehydrate grant and ancestor state before every new effect |
| An iframe steals a Host token | Expose only short-lived handles, method allowlists, and bundle-root leases |

## Completion gate

- A device scoped to one exact resource is denied get/update/remove/secret/develop/effect access to another.
- Forged contexts, unknown methods, direct transports, and replayed grants cannot bypass policy.
- Revoke, expiry, delegation attenuation, and bulk revoke have concurrent coverage.
- Audit traces user action through policy decision and effect receipt without credential leakage.
