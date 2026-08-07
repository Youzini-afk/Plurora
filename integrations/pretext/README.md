# Pretext Reference Ledger

This directory records how Plurora studies [Pretext](https://github.com/chenglou/pretext) without adopting Pretext into the kernel, package protocol, or product ontology.

## What Pretext is

Pretext (`@chenglou/pretext`) is a pure JavaScript/TypeScript library for multiline text measurement and layout. It uses the browser's Canvas 2D text engine as ground truth and `Intl.Segmenter` for Unicode-aware segmentation, avoiding DOM-based measurements that trigger synchronous layout reflow.

- **License:** MIT
- **Version observed:** `0.0.7`
- **Repository:** `https://github.com/chenglou/pretext`

## Core API shape (client-side)

```ts
import { prepare, layout } from '@chenglou/pretext'

const prepared = prepare('Some text…', '16px Inter')
const { height, lineCount } = layout(prepared, 320, 20)
```

- `prepare(text, font, options?)` — one-time text analysis + measurement pass. Returns an opaque handle.
- `layout(prepared, maxWidth, lineHeight)` — pure arithmetic resize hot path. No DOM, no canvas, no allocations.
- `prepareWithSegments(text, font, options?)` / `layoutWithLines(prepared, maxWidth, lineHeight)` — rich-path manual layout with per-line text and cursors.
- Options: `{ whiteSpace?: 'normal' | 'pre-wrap', wordBreak?: 'normal' | 'keep-all', letterSpacing?: number }`

## Suitable for (UI proof scope)

- Streaming agent/model text output: progressive chunks arriving into a prepared buffer, relayout on every resize without reflow.
- Long text measurement: height prediction for virtualization, occlusion, and scroll anchoring before DOM paint.
- Resize stability: layout-only hot path after a one-time prepare pass.
- Multilingual app text: `Intl.Segmenter` handles CJK, Thai, Arabic, emoji, mixed-script punctuation.

## Not suitable for (explicitly out of scope)

- Markdown engine, kernel content runtime, or package protocol feature. Pretext is a layout primitive; it does not parse markup.
- Full rich-text inline formatting engine (CSS inline formatting, nested trees, font-optical-sizing, font-feature-settings).
- Server-side rendering commitment today (Pretext may add it later; Plurora will evaluate when available).
- Typography final design system. The text surface proof is about measurement stability, not visual polish.

## Risks and constraints

- **`system-ui` font risk:** Canvas and DOM can resolve different `system-ui` optical variants on macOS. Pretext recommends named fonts (e.g., `Inter`, `Helvetica Neue`). The web shell text surface proof uses explicit font variables to avoid this.
- **Canvas 2D availability:** Headless/SSR environments without a Canvas implementation are unsupported. The web proof targets browsers only.
- **`Intl.Segmenter` availability:** Runtimes without `Intl.Segmenter` are unsupported. Modern browsers are fine.
- **Emoji correction:** Auto-detected per font size; font-independent constant inflation on Chrome/Firefox macOS at sizes <24px.
- **Dependency commitment:** Pretext is optional. The web shell adds `PretextTextEngine` behind dynamic import and compiles/runs without `@chenglou/pretext` installed. If installed, the registry can activate it at runtime; if not, fallback is automatic.

## Integration discipline

When Pretext changes:

1. Compare the current upstream commit with `upstream.lock.toml`.
2. Review changed API paths against `ui-map.yaml`.
3. Decide for each change: `adapted`, `adapter_only`, `deferred`, or `rejected`.
4. Add or update compact fixtures only when they protect a Plurora-native behavior.
5. Run `tsc -p clients/web/tsconfig.json --noEmit` before changing claims.

The goal is not to wrap Pretext as a kernel feature. The goal is to prove that client-side streaming text surfaces can be stable, measurable, and resize-safe before committing to a dependency.

## Web shell integration

The web shell adds the optional Pretext engine integration:

- **`pretext-shim.ts`**: Local type definitions mirroring the Pretext API surface (v0.0.7). Allows TypeScript compilation without the package. Defines `PretextModuleShape` for safe dynamic import casting.
- **`pretext-bridge.ts`**: Isolated mapping between Plurora text-layout types and Pretext shapes. Option/result conversion and opaque handle bridging.
- **`pretext-engine.ts`**: `PretextTextEngine implements TextEngine` with async `initialize()`. Uses dynamic import (`import("@chenglou/pretext")`) with unknown-safe casting. If the module is unavailable, throws a diagnostic error for registry fallback.
- **`config.ts`**: Runtime engine preference resolution (`auto|fallback|pretext`) from URL, localStorage, and global env. Default is `auto`.
- **Registry additions**: `initializeTextEnginePreference()` (async init with fallback), `getInitializationResult()`, `isPretextEngineAvailable()`.
- **Assistant Drawer**: Shows engine preference, Pretext availability, and fallback reason badges.
