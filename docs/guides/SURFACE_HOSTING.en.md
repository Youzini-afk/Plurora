# Surface Hosting Guide

> [English](./SURFACE_HOSTING.en.md) · [中文](./SURFACE_HOSTING.md)

This guide explains the two surface forms handled by the platform shell: structured descriptors rendered by the platform, and static Web bundles running inside sandboxed iframes. The current shell uses React 19, but the surface boundary is framework-neutral; third-party code participates only through public contracts and an explicit Host bridge.

## Two surface forms

Capability packages declare surfaces under `contributes.surfaces`. The host chooses the presentation from the descriptor:

- `quick_action`, `workshop_card`, and `home_card` entries with `metadata.shell_schema_version: 1` are rendered directly by the platform;
- projects that own a frontend use `entry.kind: surface_bundle` to provide a static ESM bundle mounted by `SurfaceHost` in an isolated iframe.

Structured descriptors contain only bounded text, icon hints, ordering, and same-package targets. The platform does not load package JavaScript, parse HTML, or create an iframe for them. They are discovery affordances today; any future execution wiring still crosses public protocol, permission, proposal, and audit boundaries.

## Static bundle packages

A minimal manifest:

```yaml
schema_version: 1
id: example/project-surface
version: 0.1.0
license: AGPL-3.0-only
entry:
  kind: surface_bundle
  bundle: dist/bundle.mjs
contributes:
  surfaces:
    - id: example/project-entry
      version: 0.1.0
      slot: experience_entry
      title: Example project
      allowed_capability_ids:
        - example/project/inspect
      activation:
        input_schema: {}
      required_permissions: []
permissions: {}
```

`surface_bundle` is a static, non-executing package entry. The Host does not start it as Rust, subprocess, WASM, or remote package code; installation only places the bundle and sibling static assets into project dist and includes them in the install `tree_hash`.

The raw `/surface-bundles/projects/<project_id>/...` path requires Host identity and exact project authority. After `host.surface.bundle.resolve` succeeds, the Host issues a random, five-minute, read-only `/surface-assets/<lease>/...` URL bound to the current grant and bundle root. Relative modules, stylesheets, fonts, and images must stay under that lease root. Revoking or expiring the grant invalidates the lease immediately.

Do not place secrets, tokens, private configuration, host paths, or source maps in `dist/`. Private data must be reached through capabilities, `secret_ref`, outbound audit, and bridge authority.

## SurfaceHost API

The current Web implementation lives in `clients/web/src/surfaces/surface-host.ts`:

```ts
export interface SurfaceHostOptions {
  containerId: string;
  surfaceId: string;
  bundleUrl: string;
  exportName: string;
  wrapperClass?: string;
  hostBridge?: SurfaceHostBridge;
  initialProps?: unknown;
  stylesheets?: string[];
}

export interface SurfaceHostBridge {
  currentSessionId?: string;
  allowedCapabilityIds?: Iterable<string>;
  callRpc?(method: string, params: unknown): Promise<unknown>;
  subscribeEvents?(callback: (event: unknown) => void): () => void;
}

export interface SurfaceHostHandle {
  surfaceId: string;
  iframe: HTMLIFrameElement;
  unmount(): Promise<void>;
}

export function mountSurface(
  options: SurfaceHostOptions,
): Promise<SurfaceHostHandle>;
```

`mountSurface`:

1. finds the target container;
2. creates an iframe with only `sandbox="allow-scripts"`;
3. waits for the frame's `ready` message;
4. creates a mount-scoped `bridge_token`;
5. sends the bundle URL, export, styles, and sanitized `initialProps`;
6. registers RPC and stream message handlers;
7. closes subscriptions, notifies the frame, removes listeners, and removes the iframe during `unmount()`.

The host injects `currentSessionId` as both `sessionId` and `session_id`, overriding fields with those names supplied through `initialProps`. A surface cannot select another session.

## Bundle mount contract

The bundle must be an ESM module dynamically importable from a same-origin lease URL and expose a named export whose name is a bounded JavaScript identifier. The current frame invokes the export as:

```ts
export function ExampleSurface(
  root: HTMLElement,
  props: Record<string, unknown>,
): void | (() => void) {
  // render into root
  return () => {
    // release listeners and UI state
  };
}
```

A React surface may call `createRoot(root).render(...)` and return a wrapper around `root.unmount()`. A plain DOM surface may mutate `root` directly. `wrapperClass` is assigned to the frame's `#root`; styles should be scoped beneath that class.

## Iframe and CSP

The host creates:

```html
<iframe sandbox="allow-scripts" src="/surface-frame.html"></iframe>
```

There is no `allow-same-origin`, `allow-forms`, `allow-popups`, or top-level navigation authority. The frame therefore has an opaque origin and cannot inherit Host cookies, localStorage, or DOM authority.

`surface-frame.html` uses this CSP:

```text
default-src 'self';
script-src 'self';
connect-src 'none';
style-src 'self' 'unsafe-inline';
img-src 'self' data: blob:;
font-src 'self' data:;
```

The frame bootstrap accepts only same-origin `/surface-assets/`, public `/assets/`, and its own bootstrap script. A bundle cannot load raw `/surface-bundles/` paths or make direct public-network requests from the frame; network effects must cross Host-controlled capability and outbound boundaries.

## postMessage protocol

The main messages are:

```text
frame -> host: ready
host  -> frame: mount | unmount | rpc.result | stream.frame | stream.ended | stream.error
frame -> host: rpc.call | stream.subscribe | stream.unsubscribe | mount.error
```

Other than the initial `ready`, Host and frame messages are bound to the current `bridge_token`. The host also validates `event.source`, the current session, subscription identity, and stream ownership. The asset lease authorizes static reads only; the `bridge_token` authenticates messages for one mount only. Neither is a Host credential.

## RPC bridge

A surface calls `window.pluroraHost.callRpc(method, params)`. Without `hostBridge.callRpc`, the call receives a normalized `no_bridge` error.

The current bridge method allowlist is:

- `host.info`
- `host.ping`
- `capability.invoke`
- `capability.stream`
- `capability.cancel`

Capability invoke and stream calls must satisfy all of the following:

- `capability_id` appears in the surface descriptor's `allowed_capability_ids`;
- `session_id` is rewritten by the host to `currentSessionId`;
- only allowed input, provider, version, and bounded metadata fields survive sanitization;
- returned `stream_id` / `invocation_id` values are recorded as owned by that surface;
- cancel may target only a stream or invocation created by that surface.

The Host does not expose raw runtime objects, administrative methods, secrets, or unfiltered diagnostics. Bridge failures are mapped to a bounded public code and message.

## Stream bridge

A surface may subscribe only to a stream it created through `capability.stream`. The host filters the current project session's event subscription for matching `capability/stream.*` events and maps them to:

- `stream.frame` for `started`, `chunk`, and `progress`;
- `stream.ended`;
- `stream.error` for error, cancelled, and timeout terminals.

The implementation places hard limits on owned streams and concurrent subscriptions per surface, and closes every subscription during unmount. A surface cannot use the subscription API to enumerate other streams in the same session.

## Project-page lifecycle

After Home starts a project, it opens `/project/<project_id>`. The project page omits the platform top bar and retains only the full-screen SurfaceHost and project-console boundary. Closing the tab does not stop the project session automatically; the host page performs stop through `host.project.stop`, not through implicit iframe authority.

Iframe memory is not persistent state. Recoverable state belongs in project capabilities, events, assets, or projections and is reloaded through public contracts. `initialProps` is suitable only for session, descriptor, and read-only startup information.

## Current boundaries

- Bundles are limited to Host-same-origin leased asset URLs; cross-origin bundles still require a separate origin allowlist, integrity pins, and CSP design.
- The frame cannot call Tauri APIs directly; desktop authority must be exposed through a deliberately controlled public boundary.
- Surface lifecycle callbacks such as `onClose` and `onProposalDraft` are not yet a stable contract.
- Structured quick actions remain discovery affordances and do not bypass proposal, permission, or audit to execute directly.

## Related documentation

- [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.en.md) — the architectural position of shells, projects, and packages.
- [`PROJECT_MODEL.md`](PROJECT_MODEL.en.md) — project installation, startup, and session binding.
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.en.md) — capability authority and attenuation.
- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md) — `secret_ref` and secret boundaries.
- [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md) — current implementation status.
