# Host Project-Scoped Authority

> [English](./HOST_PROJECT_AUTHORITY.en.md) · [中文](./HOST_PROJECT_AUTHORITY.md)

Status: **Candidate implementation**. The project/target authorization boundary defined here is connected to Host Access, RPC, project sessions, deployment, and controlled-development paths. Fields and behavior remain Candidate rather than Stable until cross-platform validation and cross-operation receipt closure are sufficient.

Current implementation:

- HTTP, Cookie, Bearer, and RPC retain the same logical `host_device` identity, grant, delegation chain, actions, and structured resource selectors at the Host Access layer. To preserve the frozen Contract V1 principal union, RPC contexts use a fail-closed `anonymous` sentinel plus a Host-established authority envelope, so an older runtime that ignores the envelope can only deny;
- project/context/journal/change/surface/target/exec/port/proxy paths enforce exact resources or server-side filtering, and every supported public transport reuses the same method policy;
- the pairing journal supports attenuated delegation, expiry, ancestor-revocation cascade, explicit wildcard hydration for earlier stored global grants, and bounded atomic administrator bulk revoke; Web/PWA and the `plurora host access` CLI use the same API;
- device calls append redacted `host/control/v1/authority.decision` allow/deny records without credentials or request payloads;
- global package/capability/asset/projection objects and surface contributions do not yet carry project ownership, so exact-project devices operate through verified project/session paths and may resolve their project bundle but cannot enumerate Host-global catalogues. An opaque-origin frame receives only a five-minute, read-only asset lease bound to its grant and bundle root; raw static paths still require a Host identity;
- deployment jobs and activations persist a redacted authority lease, rehydrate current grant state, reject expiry/revocation before each new effect, and persist revision/direct-route ownership. Full cross-operation effect-receipt linkage remains a Candidate boundary.
- development execute/recover keeps the authenticated identity in memory without credentials, refreshes the grant journal before Docker and managed-workspace effects, and rechecks after blocking verification. Revocation cannot cancel an already in-flight effect, but it prevents cleanup/promotion or any later user effect; explicit recovery by a currently authorized identity reconciles the durable state.

## Goal

The constitutional substrate owns principals, authenticated call context, authority attenuation/delegation/revocation, and audit mechanisms. A Project is a Host Control Plane resource. Project isolation must use the former to protect the latter without promoting `Project` into substrate ontology or treating a caller-supplied `session_id` as proof of authority.

The completed design must ensure that:

- root, devices, CLI, Web/PWA, desktop, package surfaces, and future target agents enter the public protocol through one authenticated context model;
- grants can be constrained to exact projects, targets, and Host actions;
- sessions carry only Host-verified project bindings and cannot amplify authority;
- HTTP, Host stdio, in-process, and subprocess transports cannot bypass resource authorization or invent alternate method identities;
- sensitive allow/deny decisions link subject, grant, delegation, resource, and effect receipt.

## Layer boundary

```text
Constitutional Substrate
  AuthenticatedCallContext / AuthorityRef / ResourceRef / delegation / audit

Host Control Plane
  ProjectId / TargetId / Host action / resource policy / session binding

Experience and package layers
  receive attenuated handles; never receive Host root or device credentials
```

The substrate understands only a structured `ResourceRef { owner, kind, id }`, for example owner=`host`, kind=`project`. It does not interpret Project semantics. The Host policy resolver maps method parameters, paths, session bindings, and object ownership to resource sets.

## Two call contexts

### AuthenticatedCallContext

Every transport constructs the same request-body-independent context after authentication:

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

Bearer, Cookie, stdio, in-process, and future mTLS are authentication adapters. They do not change authorization semantics.

### HostOperationContext

Before dispatching a method, the Host resolves request resources into:

```text
HostOperationContext
  authenticated_call
  action
  resources[]
  verified_project_binding?
  target_ref?
  operation_ref?
  policy_decision_ref
```

When runtime code needs project scope, it receives this verified resource context or an attenuated handle minted from it. It must not derive authority from unverified JSON, URL parameters, or session metadata.

## Grant model

Host grants retain the current opaque bearer credential whose digest alone is stored. The current contract does not require a self-contained token. Authority semantics stabilize before selecting a Biscuit-like encoding.

Candidate shape:

```text
HostGrant
  id
  subject
  actions[]
  resource_selectors[]
  parent_grant_id?
  delegation_depth
  issued_at / expires_at
  revoked_at?
  credential_digest
```

Selectors are structured and never matched by string prefix:

- `host/project/<exact-id>`;
- `host/target/<exact-id>`;
- `host/all-projects` or `host/all-targets`, delegable only by an authority with the same scope;
- explicit Host-wide resources such as `host/access-registry`.

Rules:

1. A child grant's actions, resources, expiry, and delegation depth are subsets of its issuer.
2. Root is the Host root authority, but still uses public APIs and audit paths.
3. A device identity is never collapsed into unconstrained `HostDev` at the RPC boundary.
4. Revocation affects all new calls; operation leases/policies define whether in-flight work is cancelled.
5. Legacy global device grants migrate to explicit `all-projects` / `all-targets` selectors through audited migration.

## Method authorization

Each canonical method registers:

```text
MethodPolicy
  action
  resource_extractor
  project_binding_requirement
  anonymous_allowed = false
  failure_mode = deny
```

The fixed order is:

1. authenticate the transport into `AuthenticatedCallContext`;
2. resolve the exact public method through the Contract Registry and reject unknown identities;
3. extract resources from parsed parameters and server-side projections;
4. validate session/project/object ownership and reject conflicts;
5. intersect action, resources, and authority;
6. record the policy decision;
7. only then invoke runtime code or external effects.

Unknown paths, methods, resources, and missing bindings fail closed. List methods filter server-side instead of returning all resources for the client to hide.

## Session binding

`session_id` is a locator, not a capability:

- the Host writes a controlled project binding when creating a session;
- requested project, session, and object ownership must agree;
- the caller also needs the matching project action;
- package surfaces receive a short-lived, method-allowlisted project handle, not a Host grant;
- fork, restore, archive, and delete preserve or explicitly change the binding with audit;
- resolvers, secrets, events, artifacts, proposals, and deployment consume the verified binding.

## Audit linkage

Sensitive calls record principal, credential kind, grant, delegation-chain digest, canonical method, action, resources, session/project/target/operation references, allow/deny reason, causation, and the resulting receipt or terminal failure. Raw credentials, secret values, and Cookies never enter the journal.

## Threat table

| Threat | Required defense |
|---|---|
| Project A grant submits project B session | Cross-check Host session binding and selectors |
| Transport-specific method mapping escapes policy | Pass the same resolved `PlatformMethod` into one MethodPolicy table |
| HTTP RPC device becomes `HostDev` | Preserve authenticated identity and grant |
| Lists or streams leak other projects | Server-side projection filters and fixed subscription scope |
| Project iframe steals a Host token | Expose only short-lived project handles and allowlisted methods |
| Project-id prefix collision | Structured exact `ResourceRef` comparison |
| Revoked grant creates new effects | Recheck projection per call and lease epoch for long work |
| In-process/stdio bypasses middleware | Require authenticated context in transport-neutral dispatch |

## Evolution and compatibility

- New fields begin optional under Experimental/Candidate schemas; omission preserves the meaning of earlier stored global grants.
- New-grant UI/API must submit selectors after migration.
- Authority remains Host-owned through the Host Access API and the exact public `host.*`, `context.*`, `journal.*`, and `change.*` methods used by each operation.
- Run one authorization conformance table across HTTP RPC, Host stdio, in-process dispatch, subprocess reverse stdio, and direct Host routes.
- Do not remove existing v1 fields or response shapes without an explicit new version boundary and migration path.

## Implementation order

1. Add authenticated context and structured selectors without changing behavior.
2. Propagate root/device identity into runtime dispatch and remove device-to-`HostDev` collapse.
3. Enforce session/project/object binding across events, secrets, artifacts, and resolvers.
4. Extend pairing/grant projection, delegation, and audit.
5. Migrate UI/CLI, then reject selector-less new grants.

## Completion gate

- A project-A-only device is denied project B get/list/event/secret/develop/deploy/route operations.
- Forged Context bindings, unregistered methods, direct transports, and replayed grants cannot bypass policy.
- Root, stored global grants, and clients using the current v1 shape retain explicit tested behavior.
- Revocation, expiry, attenuation, and bulk revocation have concurrency coverage.
- Audit traces user action through policy decision and effect receipt without credential leakage.
