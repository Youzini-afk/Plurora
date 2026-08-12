# Plurora platform shell (`clients/web`)

The Plurora platform's shared Web/PWA chrome — Library, Settings, Installation flow,
Installation frame, scoped Host pairing, and toast/notification system. Built as a React 19 single-page
app with Tailwind v4 and an installable service-worker shell. Styles, layout, and behavior follow the Editorial
Workshop design system in [`../../docs/design/PLATFORM_UI_DESIGN.md`](../../docs/design/PLATFORM_UI_DESIGN.md).

This client is the platform shell only. Package-contributed surfaces (including YdlTavern)
mount inside the Installation frame as iframes through `SurfaceHost`
and own their own visual identity; the shell does not impose a style on them.

---

## Quick start

```bash
# Run the host first (separate terminal)
cargo run -p plurora-cli -- host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml

# Run the web shell
npm install --prefix clients/web
npm run dev   --prefix clients/web   # 127.0.0.1:1420
npm run check --prefix clients/web   # tsc only
npm run build --prefix clients/web   # production bundle to dist/
```

For desktop builds wrap `dist/` with [`../desktop`](../desktop) (Tauri 2.x).

---

## Stack

- **React 19** with `createRoot` + StrictMode.
- **Tailwind v4** as the styling system. All design tokens defined in
  `src/styles/app.css` via the `@theme` directive — there is no `tailwind.config.js`.
  Custom `dark:` variant binds to `data-theme="dark"` on `<html>`.
- **Vite 6** for bundling, dev server, and the Surface bundle dev middleware.
- **vite-plugin-pwa** for manifest generation, service-worker updates, and bounded offline app-shell caching.
- **Motion (formerly Framer Motion) v12** for entrance/exit transitions on
  modals, toasts, and timeline rows. Honors `prefers-reduced-motion`.
- **Radix UI** for `Dialog`, `DropdownMenu`, and `Tooltip` primitives — keeps
  focus-trap, keyboard handling, and ARIA correct without recreating those
  patterns.
- **Phosphor Icons** (regular weight, 16/18px). Re-exported from
  `src/components/icons.tsx` so component code uses semantic names.
- **CVA + clsx + tailwind-merge** for variant-driven components (Button,
  StatusPill).
- **Variable fonts**: `@fontsource-variable/{bricolage-grotesque,geist,jetbrains-mono}`.
  Bundled with the SPA — no CDN at runtime.

The shell talks to the Host exclusively through published boundaries:

- `POST /rpc` for all `platform.*` methods.
- `GET /journal/subscribe/:session_id` (SSE) for event tails.
- `/host/v1/*` for typed Target-Agent adapters, controlled development, and scoped device-access workflows that deliberately remain outside Contract V1.
- `postMessage` bridge for surfaces mounted in sandboxed iframes.

There is no SQLite access and no private runtime call. Shell-owned features that
call platform utility packages still go through ordinary `capability.invoke`
paths; no first-party Package receives a privileged side channel.

---

## Layout

```
src/
├── app.tsx                     # Provider tree (theme, kernel, tooltip, toast, icons)
├── main.tsx                    # createRoot entry point + font + CSS imports
├── styles/app.css              # Tailwind v4 @theme tokens, base, custom utilities
├── lib/
│   ├── theme.tsx               # Theme provider (system/light/dark, data-theme attr)
│   ├── router.ts               # Hash router — Library / Settings plus Installation path route
│   ├── auth-gate.tsx           # root-token and same-origin device-cookie startup probe
│   ├── plurora-client.tsx       # PluroraProvider, usePlurora, useAsync, useEventTail
│   ├── format.ts               # Shared display helpers (relative time, bytes, etc)
│   ├── home-data.ts            # Legacy sample data helpers; production screens read host protocol
│   └── cn.ts                   # clsx + tailwind-merge composer
├── components/
│   ├── icons.tsx               # Phosphor re-exports with semantic names
│   ├── layout/
│   │   ├── shell.tsx           # Top-level <Shell />
│   │   ├── platform-topbar.tsx # 60px sticky topbar
│   │   └── settings-nav-rail.tsx
│   ├── ui/                     # Reusable primitives
│   │   ├── button.tsx          # CVA variants (primary/secondary/tertiary/destructive/icon)
│   │   ├── card.tsx            # Card, CardSection, CardRow
│   │   ├── modal.tsx           # Radix Dialog wrapper with motion + accent stripe
│   │   ├── dropdown.tsx        # Radix DropdownMenu wrapper
│   │   ├── tooltip.tsx         # Radix Tooltip wrapper
│   │   ├── toast.tsx           # In-house toast queue + viewport
│   │   ├── input.tsx           # Field, Input, InputGroup, Textarea, Checkbox
│   │   ├── status-pill.tsx     # State pills (running/stopped/failed/...)
│   │   ├── skeleton.tsx        # Shimmer placeholder
│   │   ├── empty-state.tsx     # Empty/error placeholder with optional retry
│   │   └── typography.tsx      # Eyebrow, HeroTitle, PageTitle, CardTitle, Mono
│   ├── home/                   # Home-page composition
│   │   ├── hero.tsx
│   │   ├── continue-card.tsx
│   │   ├── utility-strip.tsx
│   │   ├── installation-card.tsx
│   │   ├── install-card.tsx
│   │   ├── activity-timeline.tsx
│   │   └── workshop-utilities.tsx
│   ├── realization/
│   │   └── realization-workbench.tsx # plan/apply/stop/rollback/reconcile with explicit approval
│   └── install/
│       ├── install-modal.tsx   # modal shell around install-lab flow
│       ├── use-install-flow.ts # state machine + public capability calls
│       ├── url-step.tsx
│       ├── plan-step.tsx
│       ├── progress-step.tsx
│       ├── external-wizard-step.tsx
│       └── failure-modal.tsx   # redacted failure diagnostics with deep-rust accent
├── routes/
│   ├── home.tsx
│   ├── home/                   # Library hooks/helpers (Installations, Runs, failure diagnostics)
│   ├── pairing.tsx             # one-time HTTPS device pairing screen
│   ├── installation-frame.tsx       # Work entrypoints, Run/Powerbox controls, and Realization workbench
│   └── settings/
│       ├── index.tsx           # Tab dispatcher
│       ├── api-connections.tsx # secret-store-lab wired
│       ├── installed-packages.tsx # host.package.list wired
│       ├── profiles.tsx        # host.diagnostics wired
│       ├── storage.tsx         # storage areas + event store kind wired
│       ├── host-access.tsx     # scoped pairing, grant expiry, and revoke
│       └── about.tsx
├── client-core/
│   ├── host-access.ts          # typed Host access REST boundary
│   ├── powerbox.ts             # Exposure/Binding candidate and selection client
│   ├── realization.ts          # effect-free plan and explicit Realization mutation helpers
│   └── pairing-credential.ts   # immediate URL scrubbing + memory-only one-time token
├── protocol/
│   └── client.ts               # PluroraProtocolClient — typed RPC + SSE wrappers
└── surfaces/
    ├── surface-host.ts         # iframe SurfaceHost contract
    └── bundle-resolver.ts      # host.surface.bundle.resolve wrapper
```

---

## Routes

| Route | View |
| ----- | ---- |
| `#/` | Home |
| `#/settings/api-connections` | Settings — secrets |
| `#/settings/installed-packages` | Settings — package inventory |
| `#/settings/profiles` | Settings — workshop profiles |
| `#/settings/storage` | Settings — data paths and backend |
| `#/settings/host-access` | Settings — Host identities, pairing, scopes, revoke |
| `#/settings/about` | Settings — version, license, links |
| `/pair?pairing_token=...` | HTTPS one-time device pairing; token is scrubbed immediately |
| `/installation/<id>` | Standalone Installation tab with a full-viewport mounted surface |

Home and Settings keep hash routing because:

- The shell has a small fixed route set; nothing dynamic enough to warrant the
  framework.
- It survives reloads inside Tauri WebView with no server config.
- It composes naturally with the surface iframe (the surface owns its own
  internal navigation independent of the shell route).

Installations use a path route instead. Home opens `/installation/<id>` in a separate
named tab with `noopener,noreferrer`. The Installation page bypasses the platform
topbar and fills the viewport with the sandboxed surface iframe. Closing that
tab does not stop the Run context. The visible Stop action sends an explicit,
revisioned `host.run.stop` request.

---

## Theming

`ThemeProvider` writes `data-theme="light" | "dark"` on `<html>`. Three preferences:

- `system` (default) — follows `prefers-color-scheme`.
- `light` / `dark` — explicit override, persisted in `localStorage` under
  `plurora:theme-preference`.

Tailwind's `dark:` modifier is bound to `[data-theme="dark"]` via
`@custom-variant` in `app.css`. Modal overlay uses a dedicated
`--color-overlay` token that doesn't flip with theme so the scrim stays dark
in both modes. Brass accent shifts to a brighter `aged-brass-glow` in dark
mode for legibility on bark backgrounds.

---

## Real data wiring

| Page | Source |
| ---- | ------ |
| Library | `host.installation.list` + exact per-Installation `host.run.list` |
| Home — shell contributions | `shell.contribution.list` filtered to `quick_action`, `workshop_card`, and schema-versioned `home_card` |
| Settings — API Connections | `plurora/secret-store-lab/{list,put,delete}_secret` + `health` |
| Settings — Installed Packages | `host.package.list` + `host.installation.list` (installation flag) |
| Settings — Profiles | `host.diagnostics` (active profile, packages_loaded, allowlist) |
| Settings — Storage | storage-area summary + event store kind |
| Settings — Host Access | `/host/v1/access*` identity, pairing, grant, and revoke APIs |
| Installation tab | `host.installation.get` + `host.run.get/status`; Run controls use explicit `host.run.*` |
| Powerbox | `host.exposure.*` + `host.binding.*` public chooser |
| Realization | `host.realization.plan/apply/get/list/stop/rollback/reconcile`; shown only when Work declares OperationalIntent |
| Install Modal | `host.installation.create` with a typed Installation DTO |
| Failure Modal | `host.package.list/status/logs` redacted failure summaries |

All async views show a shimmer skeleton during load and an `EmptyState` with a
retry action when the call fails. Mutating actions (delete secret, installation create) push toast feedback and re-query the underlying resource.

The shell never reads raw secret values. Provider keys move from the secret
store into outbound requests via host-injected `secret_ref` references; the UI
sees only names, scopes, and counts.

Structured shell descriptors are rendered by the platform, not by package code.
`quick_action`, `workshop_card`, and `home_card` entries with
`metadata.shell_schema_version: 1` may provide bounded localized text, a shell
icon hint, display order, and same-package targets. The shell does not import a
bundle, parse HTML, or create an iframe for these entries. Package-contributed
quick actions are discovery affordances in the current slice; executable wiring
must still cross proposal, permission, and audit boundaries.

Run lifecycle uses only explicit `host.run.start|stop|status` calls. Starting a
Run never invokes Realization, builds an image, or publishes a route. Powerbox
uses the public Exposure/Binding chooser and exact Installation/Port/Run
selectors. Managed resources are a separate explicit flow through
`host.realization.plan` followed by approval-bound `apply`; stop, rollback, and
reconcile remain explicit revisioned actions.

### Realization workbench

The Installation frame renders the Realization workbench only when the Work
declares OperationalIntent. Planning is effect-free and displays the stable plan
digest, resource actions, target placement, risk/approval requirements, and
structured gaps before an apply button becomes available. Apply binds the exact
plan reference, digest, Installation revision, Target, approval decision, and
idempotency key. The UI lists current and historical revisions and exposes
explicit stop, rollback, and reconcile controls; it never treats closing a tab,
starting a Run, or selecting a Binding as implicit apply/stop.

Local Docker and enrolled Target Agents execute through the same typed Host
Realization contract. Raw Target/exec/port/proxy adapters are not surfaced as a
second lifecycle, and the shell has no fallback managed-resource path outside
`host.realization.*`.

### Powerbox chooser

The Installation frame calls `host.binding.candidates` as an effect-free query,
showing the explicit phase, exact Exposure/audience/expiry, both PortContracts,
provider Work/Installation source, Component trust/claims/boundaries/evidence,
and candidate digest/stale state. Zero or multiple candidates require an
explicit user or policy choice; preferences are ordering hints only. The shell
never displays runtime handles, credentials, or private intent, and it never
implicitly applies or rebinds. Candidate visibility is bounded at 256 with a
structured overflow diagnostic.

`host.exposure.create|revoke` and `host.binding.select|revoke` use typed public
DTOs and exact resource selectors. Host journal is durable authority; the Web
shell receives filtered projections. Closing a tab, iframe, or PWA connection
does not stop the Run or revoke its Exposure/Binding. Host/Installation caches
are isolated for remote PWA use.

The Host exposes authenticated routes through `/p/<route_id>/...`. When a route
is explicitly public and `PLURORA_APP_BASE_DOMAIN` / `--app-base-domain` is
configured, it also exposes a virtual host such as
`https://<slug>.apps.example.com/`. Merely configuring the wildcard domain does
not publish private routes. The Web shell displays and opens the URL returned by
the Host broker; enforcement stays service-side.

### Remote PWA control

The PWA is a same-Host remote client, not a detached demo. An administrator
creates an HTTPS pairing link in Settings → Host Access and selects action
scopes. The `/pair` route scrubs the one-time credential from browser history,
shows the exact grant before claim, and receives a Secure/HttpOnly/SameSite
device cookie. The root token is never copied into mobile storage. Mutations
continue through the same Host API/RPC and cookie requests are origin-checked.

See [`../../docs/architecture/HOST_REMOTE_ACCESS.md`](../../docs/architecture/HOST_REMOTE_ACCESS.md)
for TLS topology, scope mapping, revocation, and application-route exposure.

---

## Surface hosting

`src/surfaces/surface-host.ts` mounts third-party surface bundles in sandboxed
iframes using `/surface-frame.html`. Installation tabs use the same host, only without
the shell chrome around it. Surface bundles are ESM modules with a
named export that is either callable as `(root, props) => void` or exposes
`{ mount(root, props) }`.

The iframe uses `sandbox="allow-scripts"`. Host access is opt-in through the
explicit postMessage RPC bridge (`callRpc` and `subscribeEvents`). See
[`../../docs/guides/SURFACE_HOSTING.md`](../../docs/guides/SURFACE_HOSTING.md)
for the full contract.

Surface stream subscription is supported through additive postMessage messages
(`stream.subscribe`, `stream.frame`, `stream.ended`, `stream.error`,
`stream.unsubscribe`) bridged from host `capability/stream.*` events. YdlTavern
uses this for live model token streaming.

### ST URL layout (for SillyTavern extension compatibility)

YdlTavern surfaces serve SillyTavern-compatible ESM modules at standard ST URLs:

- `/script.js` — ST core globals shim
- `/scripts/extensions.js` — Extension manager shim
- `/scripts/events.js`, `/scripts/st-context.js`, `/scripts/group-chats.js`,
  `/scripts/secrets.js`, `/scripts/power-user.js`

These are served by the `ydltavern-st-compat-server` Vite plugin during dev,
reading from `../../YdlTavern/packages/ydltavern-surface/dist/st-compat/`.
Production hosting still needs a static fileserver route (deferred).

---

## Keyboard shortcuts

| Shortcut | Action |
| -------- | ------ |
| `⌘ N` / `Ctrl N` | Open Install modal (Home only) |
| `⌘ F` / `Ctrl F` | Focus package filter input (Settings → Installed Packages) |
| `Esc` | Close modal |
| `↵` | Confirm primary action in modals |

---

## Accessibility

- `:focus-visible` draws a 2px Aged Brass outline with offset on every
  focusable. Buttons use an inner ring; modal close uses the global ring.
- All interactive elements have `aria-label` when icon-only.
- Status pills include leading text in addition to color-coded dots.
- Toast queue uses `role="status"` + `aria-live="polite"`.
- `prefers-reduced-motion: reduce` zeroes animation/transition durations.

---

## What this shell is not

- It is not a Studio. There are no privileged tools, private Powerbox bridges, or routes that bypass public protocol.
- It is not a chat UI. Package-contributed surfaces own conversational behavior.
- It is not a marketplace. Settings → Installed Packages shows local
  inventory only; the web install flow accepts public HTTPS Git URLs, never a
  curated catalog.
- It is not a content runtime. Experience-level state remains owned by the installed Work and its explicit Installation state bindings.

---

## Related docs

- [`../../docs/design/PLATFORM_UI_DESIGN.md`](../../docs/design/PLATFORM_UI_DESIGN.md)
  — Editorial Workshop design system reference.
- [`../../docs/guides/SURFACE_HOSTING.md`](../../docs/guides/SURFACE_HOSTING.md)
  — Surface bundle contract and mount lifecycle.
- [`../../docs/guides/INSTALLATION_MODEL.md`](../../docs/guides/INSTALLATION_MODEL.md)
  — Work / Installation lifecycle and Home card semantics.
- [`../../docs/guides/REALIZATION.md`](../../docs/guides/REALIZATION.md)
  — OperationalIntent planning, approval, execution, recovery, and rollback.
- [`../../docs/guides/SECRET_MANAGEMENT.md`](../../docs/guides/SECRET_MANAGEMENT.md)
  — `secret_ref` contract and platform/Installation scoping.
- [`../../docs/architecture/HOST_REMOTE_ACCESS.md`](../../docs/architecture/HOST_REMOTE_ACCESS.md)
  — Host root/device identity, HTTPS pairing, and explicit public routes.
- [`../../docs/spec/PUBLIC_CONTRACT.md`](../../docs/spec/PUBLIC_CONTRACT.md)
  — Public protocol that the shell consumes.
