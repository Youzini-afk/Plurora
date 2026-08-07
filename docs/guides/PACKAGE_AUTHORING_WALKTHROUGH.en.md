# Package Authoring Walkthrough

> [English](./PACKAGE_AUTHORING_WALKTHROUGH.en.md) · [中文](./PACKAGE_AUTHORING_WALKTHROUGH.md)

This walkthrough creates a third-party package. It appears in Home, contributes Forge and assistant surfaces, passes local checks, and can be composed with other packages. It deliberately uses the same public manifest, capability, and surface path as first-party Packages.

## 1. Generate a package

```bash
cargo run -p plurora-cli -- init-package /tmp/plurora-seed-package \
  --id example/seed-package \
  --entry subprocess \
  --language typescript \
  --template full-surface
```

The generated manifest includes:

- an `experience_entry` surface for Home;
- a `play_renderer` surface;
- a `forge_panel` surface;
- an `assistant_action` surface;
- an `asset_editor` surface;
- one subprocess JSON-RPC capability that echoes input.

For narrower packages, select another template:

```bash
cargo run -p plurora-cli -- init-package /tmp/plurora-assist \
  --id example/assist \
  --entry subprocess \
  --language typescript \
  --template assistant-action

cargo run -p plurora-cli -- init-package /tmp/plurora-asset-editor \
  --id example/asset-editor \
  --entry subprocess \
  --language python \
  --template asset-editor
```

Available templates are:

- `basic` — capability only, no surfaces.
- `experience` — Home `experience_entry` only.
- `play-renderer` — Play renderer surface.
- `forge-panel` — Forge panel surface.
- `assistant-action` — assistant action surface with approval policy metadata.
- `asset-editor` — asset editor surface.
- `full-surface` — all authoring/play surface slots.
- `networked` — networked capability with declared network permissions (`host`, `methods`, `purpose`), `secret_ref` usage, and outbound audit helper. It embeds no raw secrets and has no implicit network access. Demonstrates `NetworkDeclaration` and `OutboundAuditHelper` from `sdk/typescript/secure-execution`.
- `streaming` — streaming capability with faux frame lifecycle (`StreamFrameClient`). Demonstrates `start`/`chunk`/`end` frames and `redaction_state`. No real model inference. Uses `sdk/typescript/secure-execution`.
- `agent-runtime` — locally replayable agent-like subprocess package. Includes streaming `run` capability, `explain-run` trace summary, `draft-proposal` approval-gated proposal, `echo` capability, and `assistant_action` + `forge_panel` surfaces. Uses `StreamFrameClient` (`sdk/typescript/secure-execution`) and `createTraceEvent`/`createProposalDraft`/`blockRawSecrets` (`sdk/typescript/agent-adapter`). No real model inference, no network calls, no raw secrets.
- `experience-runtime` — locally replayable experience-runtime subprocess package. Includes `describe-contract`, `create-checkpoint`, `inspect-checkpoint`, `draft-recovery`, `bind-agent-run`, and `echo` capabilities, plus all four experience surfaces. Uses the `sdk/typescript/experience-runtime` SDK. No real model inference, no network calls, no raw secrets.
- `playable-board` — locally replayable playable board subprocess package. Includes `launch`, `project_state`, `render_payload`, `record_player_action`, `request_change`, `create_checkpoint`, and `echo` capabilities, plus all four experience surfaces. Closest to the `plurora/playable-creation-board` shape for third-party creators. No real model inference, no network calls, no raw secrets.
- `playable-experience` — locally replayable playable experience subprocess package. Includes all `playable-board` capabilities plus `inspect_checkpoint` and `draft_recovery` for full checkpoint/recovery lifecycle. All four experience surfaces. No real model inference, no network calls, no raw secrets.

`--language typescript-experience` remains supported as a legacy shortcut for a full experience-shaped package.

## 2. Validate the package locally

```bash
cargo run -p plurora-cli -- package check /tmp/plurora-seed-package/manifest.yaml
cargo run -p plurora-cli -- package conformance /tmp/plurora-seed-package/manifest.yaml
cargo run -p plurora-cli -- package run-fixture /tmp/plurora-seed-package/manifest.yaml
cargo run -p plurora-cli -- package reload /tmp/plurora-seed-package/manifest.yaml
```

These commands only inspect the manifest and invoke the package through the ordinary capability path. They do not grant private host access.

`package check` prints authoring diagnostics such as entry kind, trust level, capability count, surfaces by slot, permission summary, sandbox policy, and warnings for packages with no capabilities or no surfaces. `package run-fixture` invokes declared non-streaming capabilities with deterministic fixture input and prints a structured JSON result. `package reload` exercises the local load/restart/unload loop and reports package status and logs.

## 3. Create a composition descriptor

```bash
cargo run -p plurora-cli -- init-composition /tmp/plurora-seed-composition --id example/seed-package
cargo run -p plurora-cli -- composition check /tmp/plurora-seed-composition/composition.yaml
```

A composition descriptor says which packages provide the launchable entry and which surface slots must be present. It is not a kernel `game` or `experience` type.

Composition descriptor v2 fields can also declare optional packages, required capabilities, permission expectations, replacement candidates, default activation metadata, and compatibility notes. `composition check` reports loaded package paths, surfaces by slot, capabilities, missing required surfaces/capabilities, optional-package warnings, and replacement diagnostics.

For a replacement proof, inspect the included third-party example:

```bash
cargo run -p plurora-cli -- package check examples/packages/thirdparty-playable-seed/manifest.yaml
cargo run -p plurora-cli -- composition check examples/compositions/playable-seed-replacement/composition.yaml
```

The package id is `thirdparty/playable-seed`, not `plurora/*`, and it exposes compatible Play/Forging/Assistant/Asset surfaces without official priority.

## 4. Load the package in a host profile

Add the package manifest to a host profile, for example:

```yaml
autoload:
  - /tmp/plurora-seed-package/manifest.yaml
```

Then run:

```bash
cargo run -p plurora-cli -- host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml
```

Home discovers the package through `shell.contribution.list`. Forge discovers panels through the same protocol. The UI does not receive private runtime handles.

Forge now includes lightweight authoring panels over public protocol data:

- package and capability inventory grouped by provider package;
- surface inventory grouped by slot;
- authoring diagnostics for packages, capabilities, surfaces, assets, projections, and entry surfaces;
- copy-ready CLI command guidance for templates, package checks, fixture runs, reloads, and compositions.

## 5. Compare with first-party Packages

First-party Packages under `packages/plurora/` are reference implementations, not privileged routes:

- `plurora/composition-lab` explains launch plans and surface graphs.
- `plurora/asset-lab` previews assets and drafts import plans.
- `plurora/projection-lab` explains projection rebuilds and source events.
- `plurora/playable-seed` proves a reference playable package.

A third-party package should be able to replace any of these when it exposes compatible surfaces and capabilities.

The `examples/packages/thirdparty-playable-seed` package is the current proof. Checks verify that its surfaces are discoverable, capabilities invoke through normal routing, and composition checks pass. Shared capability ids are rejected as ambiguous unless an explicit provider is selected. There is no implicit official priority.

## Invariants

- Packages must not self-assert caller identity.
- Packages must write only inside authorized namespaces.
- Assistant-like packages must return proposals or events, not mutate trusted state directly.
- UI and tooling must use public protocol methods only.
- If a capability needs mutation, route it through permission checks and `platform.proposal.*` when user approval is required.

## 6. Secure execution helpers

The `sdk/typescript/secure-execution` module provides thin, protocol-safe helpers for packages that need secret references, network declarations, outbound audit, and streaming frames. No private kernel internals are exposed.

### Secret references

```ts
import { secretRef, isValidSecretRef, looksLikeRawSecret } from "../../sdk/typescript/secure-execution/index.js";

// Create a secret reference (never embed raw secrets in payloads)
const ref = secretRef("env", "MY_API_KEY"); // → "secret_ref:env:MY_API_KEY"

// Validate
isValidSecretRef("secret_ref:env:KEY"); // true
isValidSecretRef("sk-abc123");           // false
```

### Network declarations

```ts
import { NetworkDeclaration } from "../../sdk/typescript/secure-execution/index.js";

const decl = new NetworkDeclaration({
  host: "api.example.com",
  methods: ["GET", "POST"],
  purpose: "model inference",
});
decl.toManifestEntry(); // manifest-compatible object
decl.matches("api.example.com", "POST"); // true
```

### Outbound audit helper

```ts
import { OutboundAuditHelper, secretRef } from "../../sdk/typescript/secure-execution/index.js";

const audit = new OutboundAuditHelper({
  packageId: "example/my-package",
  capabilityId: "example/my-package/fetch",
});
const payload = audit.buildRequestPayload({
  destinationHost: "api.example.com",
  method: "POST",
  secretRefsUsed: [secretRef("env", "MY_KEY")],
  purpose: "model inference",
});
// payload contains only references — never raw secrets
```

### Stream frame client

```ts
import { StreamFrameClient } from "../../sdk/typescript/secure-execution/index.js";

const client = new StreamFrameClient();
const startFrame = client.start("example/stream/echo", {});
const chunk1 = client.chunk({ text: "faux token 1" });
const endFrame = client.end();
// Frames carry invocation_id, stream_id, sequence, redaction_state
```

## 7. No-network readiness proof

For packages that want to prove their readiness to work with the secure execution substrate (secret refs, network permissions, streaming) without making real network calls or performing model inference, see the included examples:

```bash
cargo run -p plurora-cli -- package check examples/packages/faux-model-readiness/manifest.yaml
cargo run -p plurora-cli -- package check examples/packages/faux-agent-readiness/manifest.yaml
```

- `example/faux-model-readiness` declares network permissions, uses `secret_ref` for credentials, returns discovery plans (not real API responses), and produces faux streaming frames. No real inference or network calls.
- `example/faux-agent-readiness` produces proposals/traces/plans only, emphasizes public protocol/capability/proposal patterns, has no network permissions, and produces faux streaming trace frames. No connection to pi runtime or model inference.

These packages prove the substrate shape without coupling to any specific model or agent implementation.

## 8. Playable package walkthrough — from template to playable

This walkthrough shows how a new creator can go from a template to a playable package in under a day, using only docs, templates, and Forge — without reading Plurora source code.

### 8.1 Generate a playable board package

```bash
cargo run -p plurora-cli -- init-package /tmp/my-playable-board \
  --id thirdparty/my-playable-board \
  --entry subprocess \
  --language typescript \
  --template playable-board
```

This generates a package skeleton that mirrors the `plurora/playable-creation-board` shape:

- 4 experience surfaces: `experience_entry`, `play_renderer`, `forge_panel`, `assistant_action`
- 7 capabilities: `launch`, `project_state`, `render_payload`, `record_player_action`, `request_change`, `create_checkpoint`, `echo`
- No network declarations; locally replayable by default
- A `package.ts` with local stubs for each capability

### 8.2 Validate locally

```bash
cargo run -p plurora-cli -- package check /tmp/my-playable-board/manifest.yaml
cargo run -p plurora-cli -- package conformance /tmp/my-playable-board/manifest.yaml
cargo run -p plurora-cli -- package run-fixture /tmp/my-playable-board/manifest.yaml
cargo run -p plurora-cli -- package reload /tmp/my-playable-board/manifest.yaml
```

`package check` now prints creator-facing diagnostics:

- Experience surface coverage: warns if `experience_entry` is present but `play_renderer`, `forge_panel`, or `assistant_action` is missing
- Checkpoint/recovery capability coverage: warns if `create_checkpoint` or `draft_recovery` capability is missing for experience packages
- Dangerous permissions: warns about wildcard `capabilities.invoke: ["*"]` or network declarations with empty method lists
- Non-deterministic hint: warns if network access is requested

`package run-fixture` now provides error-specific fix hints when capabilities fail (e.g., "check that the capability id in the surface's capability_id field matches a provided capability").

`package reload` now warns if the package is degraded after restart.

### 8.3 Compose with other packages

```bash
cargo run -p plurora-cli -- init-composition /tmp/my-board-composition --id thirdparty/my-playable-board
cargo run -p plurora-cli -- composition check /tmp/my-board-composition/composition.yaml
```

`composition check` now prints experience-specific diagnostics:

- Experience surface coverage: shows which surface slots are covered or missing
- Replacement candidates: shows declared candidates and whether they are loaded
- Replacement hint: if multiple packages provide the same slot, suggests declaring `replacement_candidates`
- State capability coverage: shows `create_checkpoint` and `draft_recovery` provider counts
- Optional package coverage: hints about `memory-lab` and `experience-observability-lab` for richer experiences

### 8.4 Compare with the official reference

The official `plurora/playable-creation-board` package has the same surfaces and capabilities. Your third-party package uses the same public manifest, capability, and surface path. It has no privilege and no special routing. When both are loaded, the kernel does not prefer the first-party Package. If you want to replace it in a composition, declare your package as the primary provider and the first-party Package as a `replacement_candidate`.

### 8.5 For a richer lifecycle: playable-experience template

If your experience needs checkpoint inspection and recovery planning (save/restore mid-session, recover from failures), use the `playable-experience` template instead:

```bash
cargo run -p plurora-cli -- init-package /tmp/my-playable-experience \
  --id thirdparty/my-playable-experience \
  --entry subprocess \
  --language typescript \
  --template playable-experience
```

This adds `inspect_checkpoint` and `draft_recovery` capabilities (9 total) for the full save/inspect/recover lifecycle.
