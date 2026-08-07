/**
 * Plurora text-surface SDK — Pure TypeScript helpers for third-party UIs.
 *
 * This module provides reusable text-surface primitives that third-party web
 * clients can import without depending on `clients/web` private modules.
 * It is a **frontend SDK**, not a capability package — it ships no protocol
 * methods and has no kernel coupling.
 *
 * Types are self-contained (copied minimal stable shapes from
 * `clients/web/src/text-layout/types.ts`) so that consumers never need
 * a transitive import into the private web shell.
 *
 * ## API surface
 *
 * - `createTextSurfaceBuffer`  — streaming text accumulator
 * - `applyStreamFrame`         — feed a generic stream frame into a buffer
 * - `extractTextChunk`        — safe plain-text extraction from payloads
 * - `createScrollAnchor`       — scroll-position anchor for streaming views
 *
 * For the full engine abstraction (registry, preference, Pretext bridge),
 * see `clients/web/src/text-layout`.
 */

// ---------------------------------------------------------------------------
// Stable minimal types (no dependency on clients/web internals)
// ---------------------------------------------------------------------------

/** Font style descriptor used for measurement (CSS font shorthand string). */
export type FontDescriptor = string;

/** Streaming buffer lifecycle state. */
export type StreamingBufferState = "idle" | "streaming" | "ended" | "reset";

/** Streaming text buffer that accumulates chunks and can re-layout. */
export type TextSurfaceBuffer = {
  /** Current lifecycle state. */
  state: StreamingBufferState;
  /** Accumulated text so far. */
  text: string;
  /** Maximum layout width in px. */
  maxWidth: number;
  /** Line height in px. */
  lineHeight: number;
  /** CSS font shorthand used for measurement. */
  font: FontDescriptor;
  /** Append a text chunk. Transitions to "streaming" if idle. */
  append(chunk: string): void;
  /** Mark the stream as ended. */
  end(): void;
  /** Clear text and transition to "reset". */
  reset(): void;
};

/** Generic stream frame kinds (mirrors clients/web stream-adapter). */
export type StreamFrameKind =
  | "start"
  | "chunk"
  | "progress"
  | "end"
  | "error"
  | "cancelled"
  | "timeout";

/** A generic stream frame. */
export type StreamFrame = {
  kind: StreamFrameKind;
  text?: string;
  progress?: number;
  error?: string;
  metadata?: Record<string, unknown>;
};

/** Result of applying a stream frame to a buffer. */
export type ApplyFrameResult = {
  accepted: boolean;
  state: StreamingBufferState;
  reason?: string;
};

/** A scroll anchor tracks the reading position in a streaming view. */
export type ScrollAnchor = {
  /** The character offset that the anchor points to. */
  offset: number;
  /** Whether the anchor was at the tail (latest content) when created. */
  atTail: boolean;
  /** Create a new anchor at the current buffer tail. */
  snapToTail(): ScrollAnchor;
  /** Advance the anchor to a given offset. */
  advanceTo(offset: number): ScrollAnchor;
};

// ---------------------------------------------------------------------------
// createTextSurfaceBuffer
// ---------------------------------------------------------------------------

/**
 * Create a lightweight streaming text buffer.
 *
 * The buffer accumulates text chunks and tracks lifecycle state. It does not
 * perform layout itself — pair it with the text-layout engine for measurement
 * when running inside `clients/web`. For pure SDK consumers the buffer is
 * useful as a state machine and text accumulator.
 *
 * @param font      — CSS font shorthand (e.g. `'14px Inter, sans-serif'`)
 * @param lineHeight — line height in px
 * @param maxWidth   — max layout width in px
 * @returns a `TextSurfaceBuffer`
 */
export function createTextSurfaceBuffer(
  font: FontDescriptor,
  lineHeight: number,
  maxWidth: number,
): TextSurfaceBuffer {
  let text = "";
  let state: StreamingBufferState = "idle";

  return {
    get state() {
      return state;
    },
    get text() {
      return text;
    },
    get maxWidth() {
      return maxWidth;
    },
    set maxWidth(value: number) {
      maxWidth = value;
    },
    get lineHeight() {
      return lineHeight;
    },
    get font() {
      return font;
    },
    append(chunk: string) {
      if (state === "ended" || state === "reset") state = "streaming";
      if (state === "idle") state = "streaming";
      text += chunk;
    },
    end() {
      state = "ended";
    },
    reset() {
      text = "";
      state = "reset";
    },
  };
}

// ---------------------------------------------------------------------------
// applyStreamFrame
// ---------------------------------------------------------------------------

/**
 * Apply a generic stream frame to a TextSurfaceBuffer.
 *
 * Frame semantics:
 * - `start`:     resets the buffer
 * - `chunk`:     appends text (if streaming)
 * - `progress`:  no-op (confirms stream is alive)
 * - `end`:       marks the buffer as ended
 * - `error`:     marks the buffer as ended
 * - `cancelled`: marks the buffer as ended
 * - `timeout`:   marks the buffer as ended
 *
 * @param buffer — the TextSurfaceBuffer to feed
 * @param frame  — the stream frame
 * @returns ApplyFrameResult
 */
export function applyStreamFrame(
  buffer: TextSurfaceBuffer,
  frame: StreamFrame,
): ApplyFrameResult {
  switch (frame.kind) {
    case "start": {
      buffer.reset();
      return { accepted: true, state: buffer.state };
    }
    case "chunk": {
      if (buffer.state === "ended") {
        return { accepted: false, state: buffer.state, reason: "stream already ended" };
      }
      const text = frame.text ?? "";
      if (text.length > 0) {
        buffer.append(text);
      }
      return { accepted: true, state: buffer.state };
    }
    case "progress": {
      if (buffer.state !== "streaming") {
        return { accepted: false, state: buffer.state, reason: "not in streaming state" };
      }
      return { accepted: true, state: buffer.state };
    }
    case "end": {
      if (buffer.state === "idle") {
        return { accepted: false, state: buffer.state, reason: "stream not started" };
      }
      buffer.end();
      return { accepted: true, state: buffer.state };
    }
    case "error":
    case "cancelled":
    case "timeout": {
      if (buffer.state === "streaming" || buffer.state === "idle") {
        buffer.end();
      }
      return {
        accepted: true,
        state: buffer.state,
        reason: frame.error ?? `stream ${frame.kind}`,
      };
    }
    default: {
      return {
        accepted: false,
        state: buffer.state,
        reason: `unknown frame kind: ${(frame as any).kind}`,
      };
    }
  }
}

// ---------------------------------------------------------------------------
// extractTextChunk
// ---------------------------------------------------------------------------

/** Common field names that typically carry human-readable text. */
const COMMON_TEXT_FIELDS = [
  "text",
  "message",
  "summary",
  "reason",
  "content",
] as const;

/**
 * Extract a safe plain-text chunk from an arbitrary payload object.
 *
 * Scans common field names (`text`, `message`, `summary`, `reason`, `content`)
 * and returns the first non-empty string found. Returns `undefined` if no
 * suitable text is found.
 *
 * This is a defensive extraction — it never throws and treats the payload as
 * opaque.
 *
 * @param payload — an unknown record to scan
 * @returns the extracted text, or undefined
 */
export function extractTextChunk(payload: unknown): string | undefined {
  if (typeof payload !== "object" || payload === null) return undefined;
  const record = payload as Record<string, unknown>;
  for (const field of COMMON_TEXT_FIELDS) {
    const value = record[field];
    if (typeof value === "string" && value.length > 0) {
      return value;
    }
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// createScrollAnchor
// ---------------------------------------------------------------------------

/**
 * Create a scroll anchor for a streaming text view.
 *
 * An anchor tracks a character offset and whether it was at the tail
 * (latest content) when created. UI components can use this to maintain
 * scroll position when new chunks arrive — if the anchor was at the tail,
 * auto-scroll; otherwise hold position.
 *
 * @param buffer  — the TextSurfaceBuffer to anchor against
 * @param options — optional initial offset
 * @returns a ScrollAnchor
 */
export function createScrollAnchor(
  buffer: TextSurfaceBuffer,
  options?: { offset?: number },
): ScrollAnchor {
  const initialOffset = options?.offset ?? buffer.text.length;
  const atTail = initialOffset >= buffer.text.length;

  return {
    offset: initialOffset,
    atTail,
    snapToTail(): ScrollAnchor {
      return {
        offset: buffer.text.length,
        atTail: true,
        snapToTail: this.snapToTail,
        advanceTo: this.advanceTo,
      };
    },
    advanceTo(newOffset: number): ScrollAnchor {
      return {
        offset: newOffset,
        atTail: newOffset >= buffer.text.length,
        snapToTail: this.snapToTail,
        advanceTo: this.advanceTo,
      };
    },
  };
}

// ---------------------------------------------------------------------------
// Frame convenience constructors
// ---------------------------------------------------------------------------

/** Create a start frame. */
export function frameStart(metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "start", metadata };
}

/** Create a chunk frame. */
export function frameChunk(text: string, metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "chunk", text, metadata };
}

/** Create a progress frame. */
export function frameProgress(progress: number, metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "progress", progress, metadata };
}

/** Create an end frame. */
export function frameEnd(metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "end", metadata };
}

/** Create an error frame. */
export function frameError(error: string, metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "error", error, metadata };
}

/** Create a cancelled frame. */
export function frameCancelled(metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "cancelled", metadata };
}

/** Create a timeout frame. */
export function frameTimeout(error?: string, metadata?: Record<string, unknown>): StreamFrame {
  return { kind: "timeout", error, metadata };
}
