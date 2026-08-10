# Event Kind Registry (v1)

This table lists the 58 event kinds emitted exclusively by the Plurora platform runtime. Ordinary Package writers use their own Package ID namespace and cannot impersonate a platform-owned kind.

| Event kind | Payload schema | Writer | Trigger | Status |
|---|---|---|---|---|
| `context/opened` | [`./schemas/events/context__opened.schema.json`](./schemas/events/context__opened.schema.json) | `plurora/runtime` | Session opened | implemented |
| `context/closed` | [`./schemas/events/context__closed.schema.json`](./schemas/events/context__closed.schema.json) | `plurora/runtime` | Session closed | implemented |
| `context/forked` | [`./schemas/events/context__forked.schema.json`](./schemas/events/context__forked.schema.json) | `plurora/runtime` | Session fork creates branch lineage | implemented |
| `host/package.loaded` | [`./schemas/events/host__package.loaded.schema.json`](./schemas/events/host__package.loaded.schema.json) | `plurora/runtime` | Package accepted and registered; payload includes `contract_mode` (`v1` or `none`) | implemented |
| `host/package.loading` | [`./schemas/events/host__package.loading.schema.json`](./schemas/events/host__package.loading.schema.json) | `plurora/runtime` | Package enters loading | implemented |
| `host/package.starting` | [`./schemas/events/host__package.starting.schema.json`](./schemas/events/host__package.starting.schema.json) | `plurora/runtime` | Package process/entry starting | implemented |
| `host/package.ready` | [`./schemas/events/host__package.ready.schema.json`](./schemas/events/host__package.ready.schema.json) | `plurora/runtime` | Package ready after startup | implemented |
| `host/package.stopping` | [`./schemas/events/host__package.stopping.schema.json`](./schemas/events/host__package.stopping.schema.json) | `plurora/runtime` | Package execution stopping | implemented |
| `host/package.stopped` | [`./schemas/events/host__package.stopped.schema.json`](./schemas/events/host__package.stopped.schema.json) | `plurora/runtime` | Package execution stopped | implemented |
| `host/package.unloaded` | [`./schemas/events/host__package.unloaded.schema.json`](./schemas/events/host__package.unloaded.schema.json) | `plurora/runtime` | Package removed from registry | implemented |
| `host/package.degraded` | [`./schemas/events/host__package.degraded.schema.json`](./schemas/events/host__package.degraded.schema.json) | `plurora/runtime` | Execution failure or health loss | implemented |
| `host/package.log` | [`./schemas/events/host__package.log.schema.json`](./schemas/events/host__package.log.schema.json) | `plurora/runtime` | Captured subprocess stderr line | implemented |
| `host/installation.created` | [`./schemas/events/host__installation.created.schema.json`](./schemas/events/host__installation.created.schema.json) | `plurora/runtime` | Installation journal entry created and projected | implemented |
| `host/installation.updated` | [`./schemas/events/host__installation.updated.schema.json`](./schemas/events/host__installation.updated.schema.json) | `plurora/runtime` | Installation active Work/Lock pointer or state updated | implemented |
| `host/installation.removed` | [`./schemas/events/host__installation.removed.schema.json`](./schemas/events/host__installation.removed.schema.json) | `plurora/runtime` | Installation removed with an explicit state disposition | implemented |
| `object/put` | [`./schemas/events/object__put.schema.json`](./schemas/events/object__put.schema.json) | `plurora/runtime` | Opaque asset stored | implemented |
| `projection/updated` | [`./schemas/events/projection__updated.schema.json`](./schemas/events/projection__updated.schema.json) | `plurora/runtime` | Projection state rebuilt/updated | implemented |
| `change/proposal.created` | [`./schemas/events/change__proposal.created.schema.json`](./schemas/events/change__proposal.created.schema.json) | `plurora/runtime` | Proposal created | partial |
| `change/proposal.approved` | [`./schemas/events/change__proposal.approved.schema.json`](./schemas/events/change__proposal.approved.schema.json) | `plurora/runtime` | Proposal approved | partial |
| `change/proposal.rejected` | [`./schemas/events/change__proposal.rejected.schema.json`](./schemas/events/change__proposal.rejected.schema.json) | `plurora/runtime` | Proposal rejected | partial |
| `change/proposal.applied` | [`./schemas/events/change__proposal.applied.schema.json`](./schemas/events/change__proposal.applied.schema.json) | `plurora/runtime` | Proposal applied | partial |
| `change/proposal.failed` | [`./schemas/events/change__proposal.failed.schema.json`](./schemas/events/change__proposal.failed.schema.json) | `plurora/runtime` | Proposal apply failed | partial |
| `capability/invoked` | [`./schemas/events/capability__invoked.schema.json`](./schemas/events/capability__invoked.schema.json) | `plurora/runtime` | Capability invocation started | planned |
| `capability/completed` | [`./schemas/events/capability__completed.schema.json`](./schemas/events/capability__completed.schema.json) | `plurora/runtime` | Capability invocation succeeded | planned |
| `capability/failed` | [`./schemas/events/capability__failed.schema.json`](./schemas/events/capability__failed.schema.json) | `plurora/runtime` | Capability invocation failed | planned |
| `authority/denied` | [`./schemas/events/authority__denied.schema.json`](./schemas/events/authority__denied.schema.json) | `plurora/runtime` | Permission check denied | implemented |
| `authority/grant.created` | [`./schemas/events/authority__grant.created.schema.json`](./schemas/events/authority__grant.created.schema.json) | `plurora/runtime` | Permission grant recorded | implemented |
| `authority/grant.revoked` | [`./schemas/events/authority__grant.revoked.schema.json`](./schemas/events/authority__grant.revoked.schema.json) | `plurora/runtime` | Permission grant revoked | implemented |
| `runtime/error` | [`./schemas/events/runtime__error.schema.json`](./schemas/events/runtime__error.schema.json) | `plurora/runtime` | Structured kernel error | planned |
| `host/outbound.request` | [`./schemas/events/host__outbound.request.schema.json`](./schemas/events/host__outbound.request.schema.json) | `plurora/runtime` | Outbound request allowed/audited | partial |
| `host/outbound.denied` | [`./schemas/events/host__outbound.denied.schema.json`](./schemas/events/host__outbound.denied.schema.json) | `plurora/runtime` | Outbound request denied | partial |
| `host/outbound.execute.completed` | [`./schemas/events/host__outbound.execute.completed.schema.json`](./schemas/events/host__outbound.execute.completed.schema.json) | `plurora/runtime` | Outbound execute completed | implemented |
| `host/outbound.stream.completed` | [`./schemas/events/host__outbound.stream.completed.schema.json`](./schemas/events/host__outbound.stream.completed.schema.json) | `plurora/runtime` | Outbound stream completed | implemented |
| `capability/stream.started` | [`./schemas/events/capability__stream.started.schema.json`](./schemas/events/capability__stream.started.schema.json) | `plurora/runtime` | Streaming invocation started | partial |
| `capability/stream.chunk` | [`./schemas/events/capability__stream.chunk.schema.json`](./schemas/events/capability__stream.chunk.schema.json) | `plurora/runtime` | Streaming chunk emitted | partial |
| `capability/stream.progress` | [`./schemas/events/capability__stream.progress.schema.json`](./schemas/events/capability__stream.progress.schema.json) | `plurora/runtime` | Streaming progress emitted | partial |
| `capability/stream.ended` | [`./schemas/events/capability__stream.ended.schema.json`](./schemas/events/capability__stream.ended.schema.json) | `plurora/runtime` | Streaming ended normally | partial |
| `capability/stream.error` | [`./schemas/events/capability__stream.error.schema.json`](./schemas/events/capability__stream.error.schema.json) | `plurora/runtime` | Streaming errored | partial |
| `capability/stream.cancelled` | [`./schemas/events/capability__stream.cancelled.schema.json`](./schemas/events/capability__stream.cancelled.schema.json) | `plurora/runtime` | Streaming cancelled | partial |
| `capability/stream.timeout` | [`./schemas/events/capability__stream.timeout.schema.json`](./schemas/events/capability__stream.timeout.schema.json) | `plurora/runtime` | Streaming timed out | partial |
| `host/outbound.websocket.opened` | [`./schemas/events/host__outbound.websocket.opened.schema.json`](./schemas/events/host__outbound.websocket.opened.schema.json) | `plurora/runtime` | Outbound WebSocket opened | implemented |
| `host/outbound.websocket.frame` | [`./schemas/events/host__outbound.websocket.frame.schema.json`](./schemas/events/host__outbound.websocket.frame.schema.json) | `plurora/runtime` | Outbound WebSocket frame observed | implemented |
| `host/outbound.websocket.error` | [`./schemas/events/host__outbound.websocket.error.schema.json`](./schemas/events/host__outbound.websocket.error.schema.json) | `plurora/runtime` | Outbound WebSocket error | implemented |
| `host/outbound.websocket.completed` | [`./schemas/events/host__outbound.websocket.completed.schema.json`](./schemas/events/host__outbound.websocket.completed.schema.json) | `plurora/runtime` | Outbound WebSocket completed/closed | implemented |
| `host/exec.request` | [`./schemas/events/host__exec.request.schema.json`](./schemas/events/host__exec.request.schema.json) | `plurora/runtime` | Exec requested | implemented |
| `host/exec.denied` | [`./schemas/events/host__exec.denied.schema.json`](./schemas/events/host__exec.denied.schema.json) | `plurora/runtime` | Exec denied | implemented |
| `host/exec.started` | [`./schemas/events/host__exec.started.schema.json`](./schemas/events/host__exec.started.schema.json) | `plurora/runtime` | Exec started | implemented |
| `host/exec.stopped` | [`./schemas/events/host__exec.stopped.schema.json`](./schemas/events/host__exec.stopped.schema.json) | `plurora/runtime` | Exec stopped | implemented |
| `host/exec.completed` | [`./schemas/events/host__exec.completed.schema.json`](./schemas/events/host__exec.completed.schema.json) | `plurora/runtime` | Exec completed | planned |
| `host/exec.failed` | [`./schemas/events/host__exec.failed.schema.json`](./schemas/events/host__exec.failed.schema.json) | `plurora/runtime` | Exec failed | planned |
| `host/port.leased` | [`./schemas/events/host__port.leased.schema.json`](./schemas/events/host__port.leased.schema.json) | `plurora/runtime` | Host port lease created | implemented |
| `host/port.released` | [`./schemas/events/host__port.released.schema.json`](./schemas/events/host__port.released.schema.json) | `plurora/runtime` | Host port lease released | implemented |
| `host/port.denied` | [`./schemas/events/host__port.denied.schema.json`](./schemas/events/host__port.denied.schema.json) | `plurora/runtime` | Host port lease denied | implemented |
| `host/proxy.registered` | [`./schemas/events/host__proxy.registered.schema.json`](./schemas/events/host__proxy.registered.schema.json) | `plurora/runtime` | Proxy route registered | implemented |
| `host/proxy.unregistered` | [`./schemas/events/host__proxy.unregistered.schema.json`](./schemas/events/host__proxy.unregistered.schema.json) | `plurora/runtime` | Proxy route removed | implemented |
| `host/proxy.denied` | [`./schemas/events/host__proxy.denied.schema.json`](./schemas/events/host__proxy.denied.schema.json) | `plurora/runtime` | Proxy registration denied | implemented |
| `host/deployment.reconciled` | [`./schemas/events/host__deployment.reconciled.schema.json`](./schemas/events/host__deployment.reconciled.schema.json) | `plurora/runtime` | Deployment state reconciled after startup | implemented |
| `host/deployment.health` | [`./schemas/events/host__deployment.health.schema.json`](./schemas/events/host__deployment.health.schema.json) | `plurora/runtime` | Host TCP health probe changes deployment ready state | implemented |
