/**
 * Plurora secure-execution helpers for TypeScript subprocess packages.
 *
 * This module provides thin, protocol-safe helpers for:
 * - Secret reference construction and validation
 * - Network declaration manifest entries
 * - Outbound audit/redaction-safe request payload construction
 * - Stream frame client for faux streaming lifecycle
 *
 * No private kernel internals are exposed. All helpers work through
 * the public subprocess protocol and kernel types.
 */

// ---------------------------------------------------------------------------
// Secret references
// ---------------------------------------------------------------------------

/** Canonical prefix for secret references. */
export const SECRET_REF_PREFIX = "secret_ref:";

/** Alternative recognized prefixes. */
export const SECRET_REF_ALT_PREFIXES = ["secretRef:", "secret-ref:"];

/** Known secret field names that should never contain raw values. */
export const SECRET_FIELD_NAMES = [
  "api_key", "apikey", "api_secret", "apisecret",
  "secret_key", "secretkey", "secret", "token",
  "access_token", "access_secret", "auth_token",
  "password", "passwd", "private_key", "privatekey",
  "credential", "credentials", "bearer_token", "x-api-key",
];

/**
 * Create a secret reference string.
 *
 * @param vault - The vault/source identifier (e.g. "env", "vault", "file")
 * @param key - The key path within the vault
 * @param prefix - The prefix form (default: "secret_ref:")
 * @returns A valid secret reference string like "secret_ref:env:MY_KEY"
 */
export function secretRef(vault: string, key: string, prefix: string = SECRET_REF_PREFIX): string {
  return `${prefix}${vault}:${key}`;
}

/**
 * Check whether a string is a valid secret reference.
 *
 * Valid forms:
 * - `secret_ref:<vault>:<key>` (canonical)
 * - `secretRef:<vault>:<key>` (camelCase)
 * - `secret-ref:<vault>:<key>` (kebab-case)
 * - `host:<key>` (host-injected)
 */
export function isValidSecretRef(s: string): boolean {
  const allPrefixes = [SECRET_REF_PREFIX, ...SECRET_REF_ALT_PREFIXES];
  for (const p of allPrefixes) {
    if (s.startsWith(p)) {
      const afterPrefix = s.slice(p.length);
      return afterPrefix.includes(":") && afterPrefix.length > 2;
    }
  }
  if (s.startsWith("host:")) {
    return s.length > 5;
  }
  return false;
}

/**
 * Check whether a value looks like a raw secret (not a reference).
 * Conservative heuristic — mirrors the Rust `looks_like_raw_secret`.
 */
export function looksLikeRawSecret(value: string): boolean {
  if (isValidSecretRef(value)) return false;
  if (value.startsWith("Bearer ") || value.startsWith("bearer ")) return true;
  if (value.startsWith("sk-") || value.startsWith("sk_")) return true;
  if (value.length >= 32) {
    const alphanum = /^[\w.-]+$/;
    if (alphanum.test(value)) {
      const hasUpper = /[A-Z]/.test(value);
      const hasLower = /[a-z]/.test(value);
      const hasDigit = /[0-9]/.test(value);
      if (hasUpper && hasLower && hasDigit) return true;
      if (/^[0-9a-f]+$/i.test(value) && value.length >= 32) return true;
    }
  }
  return false;
}

/**
 * Check whether a field name is a known secret field name.
 */
export function isSecretFieldName(fieldName: string): boolean {
  const lower = fieldName.toLowerCase();
  return SECRET_FIELD_NAMES.some((n) => lower === n)
    || (lower.includes("secret")
      && !lower.includes("secret_ref")
      && !lower.includes("secretref")
      && !lower.includes("secret-ref"));
}

// ---------------------------------------------------------------------------
// Network declarations
// ---------------------------------------------------------------------------

/** A structured network declaration for manifest permissions. */
export interface NetworkDeclarationEntry {
  host: string;
  methods: string[];
  purpose?: string;
}

/**
 * Helper for building network permission declarations.
 *
 * Usage:
 * ```ts
 * const decl = new NetworkDeclaration({
 *   host: "api.example.com",
 *   methods: ["GET", "POST"],
 *   purpose: "model inference",
 * });
 * decl.toManifestEntry(); // → { host, methods, purpose }
 * ```
 */
export class NetworkDeclaration {
  constructor(public readonly entry: NetworkDeclarationEntry) {}

  /** Convert to a manifest-compatible entry object. */
  toManifestEntry(): NetworkDeclarationEntry & { methods: string[] } {
    return {
      host: this.entry.host,
      methods: this.entry.methods,
      ...(this.entry.purpose ? { purpose: this.entry.purpose } : {}),
    };
  }

  /**
   * Check whether a given destination and method match this declaration.
   * Mirrors the Rust `check_network_policy` host/method matching logic.
   */
  matches(destinationHost: string, method: string): boolean {
    if (!this.hostMatches(destinationHost)) return false;
    if (this.entry.methods.length === 0) return true;
    return this.entry.methods.some(
      (m) => m.toLowerCase() === method.toLowerCase(),
    );
  }

  /** Check whether the host pattern matches a destination. */
  private hostMatches(destination: string): boolean {
    if (this.entry.host === destination) return true;
    // Wildcard prefix: *.example.com
    if (this.entry.host.startsWith("*.")) {
      const suffix = this.entry.host.slice(2);
      if (destination.endsWith(`.${suffix}`)) return true;
      if (destination === suffix) return true;
    }
    return false;
  }
}

// ---------------------------------------------------------------------------
// Outbound audit / redaction helper
// ---------------------------------------------------------------------------

/** Configuration for the outbound audit helper. */
export interface OutboundAuditConfig {
  packageId: string;
  capabilityId: string;
}

/**
 * Helper for building audit-safe outbound request payloads.
 *
 * This helper ensures:
 * - Raw secrets are never included in payloads
 * - Only `secret_ref` identifiers are used
 * - Redaction state is explicitly declared
 * - No raw body/header/prompt/response fields are present
 */
export class OutboundAuditHelper {
  constructor(private readonly config: OutboundAuditConfig) {}

  /**
   * Build an audit-safe request payload.
   *
   * @param opts - Request options
   * @returns A JSON-safe payload with no raw secrets
   */
  buildRequestPayload(opts: {
    destinationHost: string;
    method: string;
    secretRefsUsed: string[];
    purpose?: string;
  }): Record<string, unknown> {
    // Validate all secret refs
    for (const ref of opts.secretRefsUsed) {
      if (!isValidSecretRef(ref)) {
        throw new Error(
          `Invalid secret reference: "${ref}". ` +
          `Use secretRef(vault, key) to create valid references. ` +
          `Raw secrets must never appear in audit payloads.`,
        );
      }
    }

    return {
      package_id: this.config.packageId,
      capability_id: this.config.capabilityId,
      destination_host: opts.destinationHost,
      method: opts.method,
      purpose: opts.purpose ?? null,
      secret_refs_used: opts.secretRefsUsed,
      redaction_state: "redacted",
      // Explicitly no raw_body, raw_header, raw_prompt, raw_response
    };
  }
}

// ---------------------------------------------------------------------------
// Stream frame client
// ---------------------------------------------------------------------------

/** Redaction state values, mirroring the Rust enum. */
export type RedactionStateValue =
  | "not_captured"
  | "redacted"
  | "policy_ref"
  | "unsafe_blocked"
  | "explicitly_approved";

/** Stream frame type values, mirroring the Rust enum. */
export type StreamFrameTypeValue =
  | "start"
  | "chunk"
  | "progress"
  | "end"
  | "error"
  | "cancelled"
  | "timeout";

/** A generic stream frame envelope. */
export interface StreamFrameEnvelope {
  invocation_id: string;
  stream_id: string;
  frame_type: StreamFrameTypeValue;
  sequence: number;
  redaction_state: RedactionStateValue;
  timestamp: string;
  payload: unknown;
  metadata: Record<string, unknown>;
}

/** Options for starting a streaming invocation. */
export interface StreamStartOptions {
  capabilityId: string;
  metadata?: Record<string, unknown>;
  providerPackageId?: string;
  sessionId?: string;
}

/**
 * Client for building faux stream frame envelopes.
 *
 * This is for no-network readiness proofs: it produces valid frame
 * sequences without any real model inference or network calls.
 *
 * Usage:
 * ```ts
 * const client = new StreamFrameClient();
 * const start = client.start("example/stream/echo", {});
 * const chunk1 = client.chunk({ text: "faux token 1" });
 * const end = client.end();
 * ```
 */
export class StreamFrameClient {
  private invocationId: string;
  private streamId: string;
  private frameCount: number;
  private startedAt: string;
  private ended: boolean;

  constructor() {
    this.invocationId = `inv_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
    this.streamId = `str_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
    this.frameCount = 0;
    this.startedAt = new Date().toISOString();
    this.ended = false;
  }

  /** Start a streaming invocation, returning the start frame. */
  start(capabilityId: string, payload: unknown, metadata?: Record<string, unknown>): StreamFrameEnvelope {
    const frame: StreamFrameEnvelope = {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "start",
      sequence: 0,
      redaction_state: "not_captured",
      timestamp: this.startedAt,
      payload,
      metadata: {
        capability_id: capabilityId,
        ...(metadata ?? {}),
      },
    };
    return frame;
  }

  /** Append a chunk frame. Throws if the invocation is ended/cancelled. */
  chunk(payload: unknown, redactionState: RedactionStateValue = "not_captured"): StreamFrameEnvelope {
    if (this.ended) {
      throw new Error("Cannot append chunk: invocation is in terminal state");
    }
    this.frameCount++;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "chunk",
      sequence: this.frameCount,
      redaction_state: redactionState,
      timestamp: new Date().toISOString(),
      payload,
      metadata: {},
    };
  }

  /** Append a progress frame (no payload). */
  progress(metadata: Record<string, unknown>): StreamFrameEnvelope {
    if (this.ended) {
      throw new Error("Cannot append progress: invocation is in terminal state");
    }
    this.frameCount++;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "progress",
      sequence: this.frameCount,
      redaction_state: "not_captured",
      timestamp: new Date().toISOString(),
      payload: null,
      metadata,
    };
  }

  /** End the invocation normally. */
  end(): StreamFrameEnvelope {
    this.frameCount++;
    this.ended = true;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "end",
      sequence: this.frameCount,
      redaction_state: "not_captured",
      timestamp: new Date().toISOString(),
      payload: null,
      metadata: {},
    };
  }

  /** Error-terminate the invocation. */
  error(errorMessage: string): StreamFrameEnvelope {
    this.frameCount++;
    this.ended = true;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "error",
      sequence: this.frameCount,
      redaction_state: "not_captured",
      timestamp: new Date().toISOString(),
      payload: { error: errorMessage },
      metadata: {},
    };
  }

  /** Cancel the invocation. */
  cancel(): StreamFrameEnvelope {
    this.frameCount++;
    this.ended = true;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "cancelled",
      sequence: this.frameCount,
      redaction_state: "not_captured",
      timestamp: new Date().toISOString(),
      payload: null,
      metadata: {},
    };
  }

  /** Timeout the invocation. */
  timeout(): StreamFrameEnvelope {
    this.frameCount++;
    this.ended = true;
    return {
      invocation_id: this.invocationId,
      stream_id: this.streamId,
      frame_type: "timeout",
      sequence: this.frameCount,
      redaction_state: "not_captured",
      timestamp: new Date().toISOString(),
      payload: null,
      metadata: {},
    };
  }

  /** Get the current invocation id. */
  getInvocationId(): string {
    return this.invocationId;
  }

  /** Get the current stream id. */
  getStreamId(): string {
    return this.streamId;
  }

  /** Get the total frame count. */
  getFrameCount(): number {
    return this.frameCount;
  }
}
