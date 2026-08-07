# Experience Runtime Authoring Guide

> [English](./EXPERIENCE_RUNTIME_AUTHORING.en.md) · [中文](./EXPERIENCE_RUNTIME_AUTHORING.md)

This guide explains how to create, inspect, checkpoint, recover, and replace experience-runtime packages in Plurora.

## Overview

This guide uses ordinary Packages and Components under the current Contract V1 to implement an Experience Runtime Product Profile: continuous execution, pause, recovery, checkpoint, and fork. Shared lifecycle meaning may be defined by an optional Protocol or Profile, while concrete experience state belongs to Products and Components rather than the constitutional substrate.

Key constraints:
- No `platform.experience.*`, `platform.world.*`, `platform.turn.*`, `platform.chat.*`, or `platform.memory.*`.
- Experience packages are ordinary packages with no kernel privilege.
- Current Experience descriptors, state projections, checkpoints, and recovery plans are distributed by the corresponding Component or Product Package; they are product artifacts rather than substrate primitives.
- All behavior goes through the public protocol.

## Generating an experience-runtime package

```bash
plurora init-package ./my-experience \
  --id example/my-experience \
  --entry subprocess \
  --language typescript \
  --template experience-runtime
```

The generated package includes:
- 4 surfaces: `experience_entry`, `play_renderer`, `forge_panel`, `assistant_action`
- 6 capabilities: `describe-contract`, `create-checkpoint`, `inspect-checkpoint`, `draft-recovery`, `bind-agent-run`, `echo`
- No network declarations, no raw secrets, no forbidden kernel namespaces

## Experience Descriptor

The experience descriptor (`experience_runtime_descriptor`) is product metadata for the current Profile and is distributed by the corresponding Package:

```typescript
const desc = createExperienceDescriptor({
  package_id: "example/my-experience",
  surfaces: {
    experience_entry: "example/my-experience/entry",
    play_renderer: "example/my-experience/play",
    forge_panel: "example/my-experience/forge",
    assistant_action: "example/my-experience/assist",
  },
  capabilities: {
    describe_contract: "example/my-experience/describe-contract",
    create_checkpoint: "example/my-experience/create-checkpoint",
    inspect_checkpoint: "example/my-experience/inspect-checkpoint",
    draft_recovery: "example/my-experience/draft-recovery",
    bind_agent_run: "example/my-experience/bind-agent-run",
  },
});
```

Validate a descriptor:

```typescript
const errors = validateExperienceDescriptor(desc);
if (errors.length > 0) {
  // descriptor does not conform to the contract
}
```

## State Projection

A state projection is a package-level snapshot of the current experience state:

```typescript
const projection = createStateProjection({
  package_id: "example/my-experience",
  session_id: "session-123",
  state: { health: 100, step_index: 5, location: "forest" },
  capability_id: "example/my-experience/describe-contract",
});
```

## Checkpoint

Checkpoints are persistent snapshots of experience state. They support three formats:

| Format | Description |
|--------|-------------|
| `snapshot` | Full state snapshot |
| `incremental` | Incremental snapshot (based on previous checkpoint) |
| `delta` | Only stores differences |

```typescript
const checkpoint = createCheckpoint({
  package_id: "example/my-experience",
  session_id: "session-123",
  state_snapshot: { health: 100, step_index: 5, location: "forest" },
  asset_refs: ["asset:scene:forest", "asset:character:hero"],
  branch_ref: "branch:target:main",
  sequence: 3,
  capability_id: "example/my-experience/create-checkpoint",
});

// Inspect checkpoint
const inspection = inspectCheckpoint(checkpoint);
// inspection.valid === true/false
```

## Recovery

Recovery is a plan for restoring an experience after failure. It supports five strategies:

| Strategy | Needs checkpoint | Needs approval |
|----------|-----------------|---------------|
| `restore_last_checkpoint` | Yes | No |
| `replay_from_checkpoint` | Yes | Yes |
| `restart_session` | No | Yes |
| `manual_intervention` | No | Yes |
| `discard_and_reset` | No | Yes |

```typescript
const recoveryPlan = draftRecoveryPlan({
  package_id: "example/my-experience",
  session_id: "session-123",
  failure_kind: "state_corruption",
  last_checkpoint_ref: "checkpoint:session-123:3",
  capability_id: "example/my-experience/draft-recovery",
});
```

## Play Surface Subscription

The Play surface supports three subscription types:

| Type | Description |
|------|-------------|
| `state_change` | State change notifications |
| `checkpoint` | Checkpoint creation notifications |
| `lifecycle` | Lifecycle event notifications |

```typescript
const subscription = createPlaySurfaceSubscription({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/play",
  subscription_type: "state_change",
  capability_id: "example/my-experience/describe-contract",
});
```

## Forge/Assist Bindings

### Forge Binding

Forge panel bindings connect the Forge surface to an experience session, supporting inspection and proposals:

```typescript
const forgeBinding = createForgeBinding({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/forge",
  inspect_capabilities: ["example/my-experience/describe-contract"],
  proposal_capabilities: ["example/my-experience/draft-recovery"],
  capability_id: "example/my-experience/bind-agent-run",
});
```

### Assist Binding

Assist bindings use `fork_then_approve` policy to ensure all modifications go through proposals:

```typescript
const assistBinding = createAssistBinding({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/assist",
  action_capabilities: ["example/my-experience/draft-recovery"],
  capability_id: "example/my-experience/bind-agent-run",
});
```

## Agent Run Binding

Experiences can connect to Agentic Forge via `bind_agent_run`:

- Agent run is scoped to a branch
- Agent explores in a scratch branch
- Agent produces candidates/proposals (never directly modifies target)
- Experience inspects/approves/rejects through Forge/Assist

## Third-Party Replacement

Experience-runtime packages are ordinary packages. Any third-party package that satisfies the same surface and capability contract can replace them. Replacement rules:

- Same surface slots (experience_entry, play_renderer, forge_panel, assistant_action)
- Same capability shape
- No official priority
- Declare replacement via composition descriptor

## Red Lines

The following are strictly prohibited:

1. Kernel experience namespace: events, proposals, checkpoints, or any output must never contain `platform.experience.*`, `platform.world.*`, `platform.turn.*`, `platform.chat.*`, or `platform.memory.*`.
2. Raw secrets: all secrets must use `secret_ref` references. Checkpoints, recovery plans, and state projections must not contain raw secrets.
3. Direct target branch mutation: agent changes to experiences must go through the proposal lifecycle. They must not directly modify the target branch.
4. Network access: experience-runtime packages do not use the network by default. If network is needed, `permissions.network.declarations` must be declared.
5. Kernel privilege: experience packages have no kernel privilege.

## TypeScript SDK

`sdk/typescript/experience-runtime` provides a pure TypeScript SDK. It has no dependencies and exposes no private runtime.

```typescript
import {
  createExperienceDescriptor,
  validateExperienceDescriptor,
  createStateProjection,
  createCheckpoint,
  inspectCheckpoint,
  draftRecoveryPlan,
  createPlaySurfaceSubscription,
  createForgeBinding,
  createAssistBinding,
  blockRawSecrets,
  hasKernelExperienceNamespace,
} from "../../sdk/typescript/experience-runtime/index.js";
```

## Reference Implementation

- `packages/plurora/experience-runtime-lab/` — Official experience-runtime lab package
- `crates/plurora-runtime/src/inproc/experience_runtime_lab.rs` — Replayable in-process handler
- `sdk/typescript/experience-runtime/` — TypeScript SDK
- `docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md` — This document

## Further Reading

- `docs/CHARTER.md` — Immutable founding principles
- `docs/product/PLAY_CREATION_MODEL.md` — stance and boundary of the optional play-creation Product Profile
- `docs/guides/AGENTIC_FORGE_PACKAGE_AUTHORING.md` — Agentic Forge authoring guide
- `docs/roadmap/NEXT_STEPS.md` — Roadmap
