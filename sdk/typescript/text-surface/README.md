# Plurora TypeScript text-surface SDK

Pure TypeScript helpers for third-party UIs that need streaming text
accumulation, stream-frame dispatch, safe text extraction, and scroll
anchoring — without depending on `clients/web` internals.

**This is a frontend SDK, not a capability package.** It ships no protocol
methods, has no kernel coupling, and does not require Pretext. Types are
self-contained (stable minimal shapes copied from `clients/web/src/text-layout`)
so that consumers never need a transitive import into the private web shell.

For the full engine abstraction (registry, preference, Pretext bridge, layout
measurement), see `clients/web/src/text-layout`.

## Usage

```ts
import {
  createTextSurfaceBuffer,
  applyStreamFrame,
  extractTextChunk,
  createScrollAnchor,
  frameStart,
  frameChunk,
  frameEnd,
  frameError,
} from "./index";
```

### Streaming text buffer

```ts
const buf = createTextSurfaceBuffer('14px Inter, sans-serif', 20, 560);
buf.append("Hello ");
buf.append("world");
buf.end();
// buf.text === "Hello world", buf.state === "ended"
```

### Stream frame dispatch

```ts
const buf = createTextSurfaceBuffer('14px Inter, sans-serif', 20, 560);

applyStreamFrame(buf, frameStart());
applyStreamFrame(buf, frameChunk("The quick brown fox"));
applyStreamFrame(buf, frameChunk(" jumps over the lazy dog."));
applyStreamFrame(buf, frameEnd());
// buf.state === "ended"
```

### Text extraction from payloads

```ts
extractTextChunk({ text: "hello", message: "world" }); // "hello"
extractTextChunk({ reason: "timeout" });               // "timeout"
extractTextChunk({ foo: 42 });                         // undefined
```

### Scroll anchoring

```ts
const buf = createTextSurfaceBuffer('14px Inter, sans-serif', 20, 560);
buf.append("Existing text");
const anchor = createScrollAnchor(buf); // atTail: true

buf.append(" new content");
// If anchor.atTail was true, auto-scroll to the new tail.
// Otherwise, hold position.
```

### Font loading helper (from clients/web text-layout)

When running in a browser context, the `clients/web/src/text-layout` module
also provides font-loading helpers:

```ts
import {
  ensureTextSurfaceFontLoaded,
  describeFontLoadState,
} from "./font-helper";

await ensureTextSurfaceFontLoaded("Inter");
const state = describeFontLoadState("Inter");
// state.status === "loaded" | "loading" | "unloaded" | "unsupported"
```

## API reference

| Export | Kind | Description |
|---|---|---|
| `createTextSurfaceBuffer` | function | Create a streaming text accumulator |
| `applyStreamFrame` | function | Feed a stream frame into a buffer |
| `extractTextChunk` | function | Safe plain-text extraction from payloads |
| `createScrollAnchor` | function | Create a scroll-position anchor |
| `frameStart` / `frameChunk` / `frameProgress` / `frameEnd` / `frameError` / `frameCancelled` / `frameTimeout` | function | Stream frame constructors |
| `FontDescriptor` | type | CSS font shorthand string |
| `StreamingBufferState` | type | `"idle" \| "streaming" \| "ended" \| "reset"` |
| `TextSurfaceBuffer` | type | Streaming text buffer interface |
| `StreamFrameKind` | type | Stream frame kind enum |
| `StreamFrame` | type | Generic stream frame |
| `ApplyFrameResult` | type | Result of applying a frame |
| `ScrollAnchor` | type | Scroll position anchor |

## Relationship to clients/web

- `sdk/typescript/text-surface` provides **stable, minimal** helpers that
  third-party UIs can import directly.
- `clients/web/src/text-layout` provides the **full** engine abstraction with
  `TextEngine`, registry, Pretext bridge, fallback layout, and text preview.
  It is a private module of the web shell.
- Types in this SDK are intentionally self-contained. If the shapes evolve,
  this SDK will version independently rather than pulling from the web shell.
