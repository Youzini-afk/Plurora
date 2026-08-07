# Contract Layering Matrix (Candidate)

> [English](./CONTRACT_LAYERING_MATRIX.en.md) · [中文](./CONTRACT_LAYERING_MATRIX.md)

> Status: candidate classification. This document does not change current
> `kernel.v1.*` runtime behavior. The operative specification remains
> [`KERNEL_V1_CONTRACT.md`](KERNEL_V1_CONTRACT.en.md); target principles are in
> [`CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.en.md).

## Purpose

Contract V1 currently carries constitutional mechanisms, host control, deployment, protocol semantics, and product-shell concerns. This document answers, item by item:

1. who owns the contract today;
2. which layer should own it long term;
3. whether it is retained, moved, split, or replaced;
4. how `kernel.v1.*` clients continue working through compatibility adapters.

Target names in this document identify owners and concepts, not frozen final wire method IDs. Final namespaces are selected when the compatibility router is implemented.

## Layer and disposition codes

| Code | Layer | Responsibility |
|---|---|---|
| `S` | Constitutional Substrate | Identity, authority, objects, journals, invocation, streams, transactions, receipts |
| `H` | Host Control Plane | Local installation, processes, ports, proxies, secrets, deployment, diagnostics |
| `C` | Protocol Commons | Shared semantics, state machines, change workflow, projection, and other evolvable protocols |
| `P` | Shell / Product Profile | Home, Forge, surface slots, bundle mounting, and interaction mapping |
| `X` | Split | The current contract mixes multiple owners and must be decomposed |
| `L` | Legacy Adapter | Reads or transforms an old contract only; receives no new semantics |

Dispositions:

- **Retain:** semantics belong to the target layer; only namespace and conformance separation are required.
- **Strengthen:** ownership and object model are substantially correct, but safety, audit, or portability guarantees are incomplete.
- **Reshape:** the capability remains, but the object model or boundary becomes more general.
- **Move:** behavior remains substantially intact while ownership leaves the kernel.
- **Split:** one old method becomes operations owned by multiple layers.
- **Replace:** the old abstraction is available only through an adapter to a new model.

## Current factual baseline

- Code contains 80 `KernelMethod` variants and 80 method schemas.
- Code, schemas, and `EVENT_KIND_REGISTRY.md` all contain 59 kernel events, including `kernel/v1/deployment.health`.
- There are 22 top-level schemas covering contract selection, artifact descriptors, EffectReceipt, Change primitives, protocol descriptors, component/package envelopes/composition locks, World Bundle/World Head/journal ranges, and `protocol-response.schema.json` for additive transport diagnostics.
- Known drift among `KernelMethod::status()`, Contract documentation, and actual dispatch is aligned and test-enforced.
- The Experimental method contract registry, centralized alias resolution, explicit profile/version negotiation, and identity adapters are implemented. The Host Control Plane, host bundle resolver, Shell contributions, Change/Proposal, and Projection currently publish 36 canonical/legacy dual-stack routes.
- The Experimental Protocol Commons registry publishes Change, Shell Default, and World Bundle descriptors, negotiates explicit protocol/profile selections before dispatch, and separates protocol, implementation, and package reports. The concrete World Bundle archive and all five portability vectors now back the `plurora.runtime.world-bundle` implementation claim.
- The Web client now uses canonical IDs in production; generated SDKs derive canonical clients and explicit legacy wrappers from schema metadata, queue transport diagnostics, and reject duplicate wire IDs, function names, or operation IDs before generation.

The first migration requirement is therefore a testable compatibility router, not code deletion.

## 80 methods

### Session and journal (9)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.session.open` | implemented | `S` | Reshape | Open a generic execution/journal scope; old name maps to `context.open` |
| `kernel.v1.session.close` | implemented | `S` | Reshape | Close the scope and freeze writes while retaining historical reads |
| `kernel.v1.session.fork` | partial | `S` | Reshape | Create a causal branch from a head/sequence |
| `kernel.v1.session.branch.list` | partial | `S` | Reshape | Query lineage/heads without binding them to product World semantics |
| `kernel.v1.session.get` | partial | `S` | Retain | Query generic scope metadata; align Contract status with code |
| `kernel.v1.session.list` | planned | `S` | Retain | Substrate scope query; remains Experimental until implemented |
| `kernel.v1.event.append` | implemented | `S` | Reshape | `journal.append`; payloads may reference content-addressed objects |
| `kernel.v1.event.list` | partial | `S` | Retain | `journal.list`; retain stable sequence pagination |
| `kernel.v1.event.subscribe` | planned | `S` | Retain | `journal.subscribe`; unify SSE route and method semantics |

### Package and component lifecycle (7)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.package.load` | partial | `X` | Split | `H` resolves package/artifact; `S` activates a component instance |
| `kernel.v1.package.unload` | partial | `S` | Reshape | Stop a component instance; package envelope is no longer runtime ontology |
| `kernel.v1.package.restart` | partial | `S` | Reshape | Restart a component instance with explicit trust-class support |
| `kernel.v1.package.logs` | partial | `H` | Move | Host observability; logs are not substrate truth |
| `kernel.v1.package.list` | implemented | `X` | Split | `H` package/artifact inventory + `S` active component list |
| `kernel.v1.package.status` | implemented | `X` | Split | Query envelope installation and component runtime state separately |
| `kernel.v1.package.describe` | planned | `X` | Split | Separate artifact descriptor, component descriptor, and protocol claims |

### Project (5)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.project.list` | implemented | `H` | Move | Host project/installation registry; not substrate |
| `kernel.v1.project.get` | implemented | `H` | Move | Host-owned project descriptor |
| `kernel.v1.project.start` | implemented | `H` | Move | Host orchestrates components, scope, and shell entry; old name uses adapter |
| `kernel.v1.project.stop` | implemented | `H` | Move | Host lifecycle control |
| `kernel.v1.project.status` | implemented | `H` | Move | Host state and failure diagnostics |

### Target / exec / port / proxy (17)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.target.list` | partial | `H` | Move | `host.target.list` |
| `kernel.v1.target.status` | partial | `H` | Move | `host.target.status` |
| `kernel.v1.target.register` | partial | `H` | Move | `host.target.register` |
| `kernel.v1.target.unregister` | partial | `H` | Move | `host.target.unregister` |
| `kernel.v1.exec.start` | partial | `H` | Move | `host.exec.start`; `S` still enforces authority and receipts |
| `kernel.v1.exec.stop` | partial | `H` | Move | `host.exec.stop` |
| `kernel.v1.exec.status` | partial | `H` | Move | `host.exec.status` |
| `kernel.v1.exec.logs` | partial | `H` | Move | `host.exec.logs`, preserving redaction |
| `kernel.v1.exec.list` | partial | `H` | Move | `host.exec.list` |
| `kernel.v1.port.lease` | partial | `H` | Move | `host.port.lease`; authority handle supplied by `S` |
| `kernel.v1.port.release` | partial | `H` | Move | `host.port.release` |
| `kernel.v1.port.status` | partial | `H` | Move | `host.port.status` |
| `kernel.v1.port.list` | partial | `H` | Move | `host.port.list` |
| `kernel.v1.proxy.register` | partial | `H` | Move | `host.proxy.register` |
| `kernel.v1.proxy.unregister` | partial | `H` | Move | `host.proxy.unregister` |
| `kernel.v1.proxy.status` | partial | `H` | Move | `host.proxy.status` |
| `kernel.v1.proxy.list` | partial | `H` | Move | `host.proxy.list` |

### Capability and authority handles (8)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.capability.discover` | implemented | `S` | Reshape | Discover component exports and protocol claims, not only package providers |
| `kernel.v1.capability.describe` | planned | `S` | Reshape | Describe export, protocol, schema, trust, and conformance claims |
| `kernel.v1.capability.invoke` | partial | `S` | Retain | Substrate invocation; correct Contract/code status drift |
| `kernel.v1.capability.stream` | partial | `S` | Retain | Substrate streaming invocation |
| `kernel.v1.capability.cancel` | partial | `S` | Retain | Uniform cancellation, deadline, and terminal receipt |
| `kernel.v1.cap.attenuate` | partial | `S` | Strengthen | Verify attenuation is a constraint subset and cannot expand authority |
| `kernel.v1.cap.revoke` | partial | `S` | Strengthen | Add subtree revocation and revocation receipts |
| `kernel.v1.cap.list_for` | partial | `S` | Strengthen | Principal-gated authority introspection; add delegation and lease refresh |

### Extension points and hooks (3)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.extension_point.list` | implemented | `C` | Move | Protocol-registry query; protocol owns extension semantics |
| `kernel.v1.extension_point.describe` | planned | `C` | Move | Protocol descriptor / extension contract |
| `kernel.v1.hook.list` | partial | `C` | Move | Protocol subscription registry; host may expose a runtime diagnostic view |

### Asset and projection (7)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.asset.put` | partial | `S` | Replace | `object.put` / `artifact.commit`, with digest as identity |
| `kernel.v1.asset.get` | partial | `S` | Replace | Retrieve and verify content through descriptor/digest |
| `kernel.v1.asset.list` | partial | `H` | Move | Host object index; substrate does not promise global enumeration |
| `kernel.v1.projection.register` | partial | `C` | Move | Projection protocol registers a derived view |
| `kernel.v1.projection.rebuild` | partial | `C` | Move | Projection-protocol rebuild behavior |
| `kernel.v1.projection.get` | partial | `C` | Move | Projection-profile query |
| `kernel.v1.projection.list` | partial | `C` | Move | Projection-registry query |

### Host (4)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.host.info` | implemented | `H` | Strengthen | Return contract layers, versions, profiles, aliases, and maturity |
| `kernel.v1.host.ping` | partial | `H` | Move | Lightweight host health; not substrate |
| `kernel.v1.host.diagnostics` | partial | `H` | Move | Host diagnostics with path and secret redaction |
| `kernel.v1.host.principal` | planned | `S` | Reshape | Authenticated principal/context introspection |

### Permission, audit, and change workflow (11)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.permission.grant` | partial | `S` | Reshape | Authority mint/delegate + PolicyDecision |
| `kernel.v1.permission.revoke` | partial | `S` | Reshape | Authority revocation with receipt |
| `kernel.v1.permission.list` | partial | `S` | Reshape | Query effective principal authority rather than string grants |
| `kernel.v1.permission.audit` | partial | `S` | Replace | Authority decision/receipt query |
| `kernel.v1.audit.package` | partial | `X` | Replace | `S` authority/effect audit + `H` artifact declared-versus-used report |
| `kernel.v1.proposal.create` | partial | `C` | Replace | Change protocol: create Intent / ChangeSet |
| `kernel.v1.proposal.get` | partial | `C` | Replace | Change-protocol query |
| `kernel.v1.proposal.list` | partial | `C` | Replace | Change-protocol index |
| `kernel.v1.proposal.approve` | partial | `C` | Replace | PolicyDecision / approval profile |
| `kernel.v1.proposal.reject` | partial | `C` | Replace | PolicyDecision / rejection profile |
| `kernel.v1.proposal.apply` | partial | `C` | Replace | Commit + EffectReceipt; old asset/projection operations use adapters |

### Surface (3)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.surface.resolve_bundle` | partial | `X` | Split | `H` resolves/serves bundle; `P` interprets profile and bridge policy |
| `kernel.v1.surface.contribution.list` | partial | `P` | Move | `plurora.shell.default/v1` contribution registry |
| `kernel.v1.surface.contribution.describe` | partial | `P` | Move | Shell-profile descriptor; slot is no longer a substrate enum |

### Outbound (6)

| Current method | Code status | Target | Disposition | Target concept and compatibility behavior |
|---|---:|---:|---|---|
| `kernel.v1.outbound.audit` | partial | `S` | Replace | Query generic EffectReceipts while retaining a network-specific host view |
| `kernel.v1.outbound.execute` | partial | `X` | Split | `H` HTTPS adapter + `S` authority, policy, and receipt |
| `kernel.v1.outbound.stream` | partial | `X` | Split | `H` streaming-network adapter + `S` stream/effect lifecycle |
| `kernel.v1.outbound.websocket.open` | partial | `X` | Split | `H` WebSocket adapter + `S` connection authority/receipt |
| `kernel.v1.outbound.websocket.send` | partial | `X` | Split | Host transport operation that writes an effect receipt |
| `kernel.v1.outbound.websocket.close` | partial | `X` | Split | Host transport operation that produces a terminal receipt |

## 59 events

“Emitted” means a named write site exists in `plurora-runtime`; `—` means only constants/schemas/registry exist or no emission site was found in the inspected scope.

### Session, component, and project (16)

| Current event | Emitted | Target | Disposition and target concept |
|---|---:|---:|---|
| `kernel/v1/session.opened` | ✓ | `S` | Reshape as context/journal scope opened |
| `kernel/v1/session.closed` | ✓ | `S` | Context closed; history remains readable |
| `kernel/v1/session.forked` | ✓ | `S` | Causal head/branch created |
| `kernel/v1/package.loaded` | ✓ | `S` | Component activated; package retained only as source reference |
| `kernel/v1/package.loading` | ✓ | `S` | Component activation requested |
| `kernel/v1/package.starting` | ✓ | `S` | Component starting |
| `kernel/v1/package.ready` | ✓ | `S` | Component ready |
| `kernel/v1/package.stopping` | ✓ | `S` | Component stopping |
| `kernel/v1/package.stopped` | ✓ | `S` | Component stopped |
| `kernel/v1/package.unloaded` | ✓ | `S` | Component deactivated |
| `kernel/v1/package.degraded` | ✓ | `S` | Component health degraded |
| `kernel/v1/package.log` | ✓ | `H` | Host observability event; not canonical history |
| `kernel/v1/project.installed` | — | `H` | Host project lifecycle |
| `kernel/v1/project.started` | — | `H` | Host project lifecycle |
| `kernel/v1/project.stopped` | — | `H` | Host project lifecycle |
| `kernel/v1/project.uninstalled` | — | `H` | Host project lifecycle |

### Object, projection, and change (7)

| Current event | Emitted | Target | Disposition and target concept |
|---|---:|---:|---|
| `kernel/v1/asset.put` | ✓ | `S` | Replace with object/artifact committed receipt |
| `kernel/v1/projection.updated` | ✓ | `C` | Projection-protocol event |
| `kernel/v1/proposal.created` | ✓ | `C` | ChangeSet created |
| `kernel/v1/proposal.approved` | ✓ | `C` | PolicyDecision approved |
| `kernel/v1/proposal.rejected` | ✓ | `C` | PolicyDecision rejected |
| `kernel/v1/proposal.applied` | ✓ | `C` | Commit completed + receipt reference |
| `kernel/v1/proposal.failed` | ✓ | `C` | Change workflow failed |

### Capability, authority, and general error (7)

| Current event | Emitted | Target | Disposition and target concept |
|---|---:|---:|---|
| `kernel/v1/capability.invoked` | ✓ | `S` | Invocation-started receipt/event |
| `kernel/v1/capability.completed` | ✓ | `S` | Terminal EffectReceipt; large output retained by reference |
| `kernel/v1/capability.failed` | ✓ | `S` | Terminal failed receipt |
| `kernel/v1/permission.denied` | ✓ | `S` | Authority decision denied |
| `kernel/v1/permission.granted` | ✓ | `S` | Authority minted/delegated |
| `kernel/v1/permission.revoked` | ✓ | `S` | Authority revoked |
| `kernel/v1/error` | — | `S` | Retain generic protocol/transport error envelope without copying domain errors |

### Outbound and stream (15)

| Current event | Emitted | Target | Disposition and target concept |
|---|---:|---:|---|
| `kernel/v1/outbound.request` | ✓ | `X` | Host network request + substrate EffectReceipt start |
| `kernel/v1/outbound.denied` | ✓ | `X` | PolicyDecision denied + host destination summary |
| `kernel/v1/outbound.execute.completed` | ✓ | `X` | Terminal EffectReceipt |
| `kernel/v1/outbound.stream.completed` | ✓ | `X` | Terminal EffectReceipt |
| `kernel/v1/stream.started` | ✓ | `S` | Retain generic stream lifecycle |
| `kernel/v1/stream.chunk` | ✓ | `S` | Chunk may inline small data or reference an object |
| `kernel/v1/stream.progress` | ✓ | `S` | Generic progress without domain interpretation |
| `kernel/v1/stream.ended` | ✓ | `S` | Terminal success |
| `kernel/v1/stream.error` | ✓ | `S` | Terminal failure |
| `kernel/v1/stream.cancelled` | ✓ | `S` | Terminal cancellation |
| `kernel/v1/stream.timeout` | ✓ | `S` | Terminal timeout |
| `kernel/v1/outbound.websocket.opened` | — | `X` | Host connection event + receipt link |
| `kernel/v1/outbound.websocket.frame` | — | `X` | Host transport telemetry; not canonical world history by default |
| `kernel/v1/outbound.websocket.error` | — | `X` | Host transport error + terminal/partial receipt |
| `kernel/v1/outbound.websocket.completed` | ✓ | `X` | Terminal EffectReceipt |

### Host execution and deployment (14)

| Current event | Emitted | Target | Disposition and target concept |
|---|---:|---:|---|
| `kernel/v1/exec.request` | — | `H` | Host exec lifecycle; references substrate PolicyDecision |
| `kernel/v1/exec.denied` | — | `H` | Host exec denial + receipt reference |
| `kernel/v1/exec.started` | — | `H` | Host exec started |
| `kernel/v1/exec.stopped` | — | `H` | Host exec stopped |
| `kernel/v1/exec.completed` | — | `H` | Host exec completed + EffectReceipt |
| `kernel/v1/exec.failed` | — | `H` | Host exec failed + EffectReceipt |
| `kernel/v1/port.leased` | — | `H` | Host port lifecycle |
| `kernel/v1/port.released` | — | `H` | Host port lifecycle |
| `kernel/v1/port.denied` | — | `H` | Host port denial |
| `kernel/v1/proxy.registered` | — | `H` | Host proxy lifecycle |
| `kernel/v1/proxy.unregistered` | — | `H` | Host proxy lifecycle |
| `kernel/v1/proxy.denied` | — | `H` | Host proxy denial |
| `kernel/v1/deployment.reconciled` | ✓ | `H` | Host deployment reconciliation |
| `kernel/v1/deployment.health` | — | `H` | Host deployment health; add to v1 registry |

## 22 top-level schemas

| Current schema | Target | Disposition | Target shape |
|---|---:|---|---|
| `event-envelope.schema.json` | `S` | Reshape | Journal envelope + object references + explicit causation/receipt references; retain original v1 envelope |
| `protocol-context.schema.json` | `S` | Strengthen | Authenticated principal, contract/profile negotiation, trace, and parent invocation |
| `protocol-response.schema.json` | `S` | Add | Additive result/error envelope diagnostics for Deprecated and Legacy Adapter calls |
| `contract-selection.schema.json` | `S` | Retain | Explicit profile and per-layer version requirements; no silent downgrade |
| `protocol-descriptor.schema.json` | `C` | Add | Shared semantics, lifecycle/errors, authority, vectors, profiles, migrations, and implementation claims |
| `component-descriptor.schema.json` | `S` | Add | Independent implementation identity, behavior digest, trust class, enforced-boundary claims, and references |
| `package-envelope-descriptor.schema.json` | `H` | Add | Retrieval/install envelope joining the manifest to independently addressed components and artifacts |
| `composition-lock.schema.json` | `C` | Add | Separately pinned component artifacts, protocol profiles, and immutable content roots |
| `world-bundle.schema.json` | `C` | Add | Portable manifest, exact v1 envelopes, object inventory, receipts, policies, lineage, and inline transport objects |
| `world-head.schema.json` | `C` | Add | Protocol-defined state/history/composition/policy/provenance roots and parent heads |
| `world-journal-range.schema.json` | `S` | Add | Contiguous per-session sequence range with content-addressed original event envelopes |
| `artifact-descriptor.schema.json` | `S` | Add | Open artifact type, SHA-256 digest, size, references, and annotations; bytes live in ObjectStore |
| `effect-receipt.schema.json` | `S` | Add | Content-addressed terminal evidence referencing input/output/component/authority/policy/approval/parents |
| `intent.schema.json` | `C` | Add | Principal goal and target scope; distinct from a proposal or command |
| `change-set.schema.json` | `C` | Add | Open operations, preconditions, required authority, and idempotency |
| `policy-decision.schema.json` | `S` | Add | allowed/denied/requires_approval with authority evidence |
| `commit.schema.json` | `C` | Add | committed/failed/partial result references and operation receipts |
| `capability-descriptor.schema.json` | `S` | Reshape | Component export + protocol claim + trust/conformance metadata |
| `capability-invocation-request.schema.json` | `S` | Strengthen | Handle-first, idempotency, deadline, input references, requested profile |
| `capability-invocation-result.schema.json` | `S` | Strengthen | Output references, receipt reference, terminal status; avoid permanent large payloads in envelope |
| `permission-set.schema.json` | `X` | Split | Separate host policy request, manifest authority declaration, and runtime capability |
| `manifest.schema.json` | `X` | Split | Separate package envelope, artifact descriptors, component descriptors, protocol claims, and shell contributions |

## V1 compatibility obligations

Every migration implementation satisfies:

1. Old method names route through an explicit alias registry, not scattered `match` special cases.
2. An alias records canonical target, request adapter, response adapter, deprecation state, and support window.
3. `host.info` returns every contract layer, version, profile, alias, and maturity. Clients choose explicitly and do not silently downgrade.
4. Original v1 request/response/event JSON is retained losslessly; unknown fields survive transfer.
5. Old SDKs continue working. New SDKs are split into substrate/host/protocol/shell packages with a legacy umbrella client.
6. Conformance is split into substrate, host, protocol-profile, shell-profile, and legacy-adapter suites.
7. A legacy alias is removed only after migration tooling, support window, and replacement conformance all exist.

Current runtime status is in [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md). This document retains only long-term ownership classification and compatibility obligations; current priorities are in [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.en.md).
