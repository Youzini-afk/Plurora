import readline from "node:readline";

/**
 * Thin helper for JSON-RPC-over-stdio subprocess capability packages.
 * Packages can handle host-initiated handshake/invoke requests and can also
 * initiate reverse public-contract requests (for example
 * `host.outbound.execute` and `host.outbound.stream`) over the same stdio
 * channel via `pluroraClient`.
 */

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type CapHandleId = string;

export interface JsonRpcRequest {
  [key: string]: unknown;
  jsonrpc?: "2.0";
  id?: string | number | null;
  method?: string;
  params?: Record<string, JsonValue>;
  result?: JsonValue;
  error?: JsonValue;
  kind?: string;
  stream_id?: string;
  data?: JsonValue;
  summary?: JsonValue;
}

export interface CapabilityInvokeParams {
  capability_id: string;
  session_id: string;
  input?: JsonValue;
  /** Opaque, single-activation token minted and validated by the Host. */
  invocation_context_id: string;
  /** Run Binding handles keyed by the importing component's Port id. */
  bindings: Record<string, CapHandleId>;
}

export interface HandshakeParams {
  protocol_version?: string;
  package_id?: string;
  manifest_version?: string;
  permissions?: JsonValue;
  capabilities?: JsonValue;
  bindings?: Record<string, CapHandleId>;
}

export type CapabilityHandler = (params: CapabilityInvokeParams) => JsonValue | Promise<JsonValue>;
export type CapabilityHandlerWithContext = (
  params: CapabilityInvokeParams,
  context: {
    pluroraClient: PluroraClient;
    sessionId: string;
    bindings: Readonly<Record<string, CapHandleId>>;
  },
) => JsonValue | Promise<JsonValue>;
export type HandshakeHandler = (params: HandshakeParams) => JsonValue | Promise<JsonValue>;

export interface SubprocessPackageOptions {
  onHandshake?: HandshakeHandler;
  onInvoke: CapabilityHandler | CapabilityHandlerWithContext;
}

export interface PluroraStreamCallbacks {
  onChunk: (chunk: unknown) => void;
  onEnd?: (summary: unknown) => void;
  onError?: (error: unknown) => void;
  onCancelled?: () => void;
  onTimeout?: () => void;
}

export interface PluroraStreamHandle {
  readonly streamId: string | undefined;
  cancel(): void;
}

export type PluroraWebSocketFrame =
  | { kind: "text"; data: string }
  | { kind: "binary"; data: Uint8Array };

export interface PluroraWebSocketHandle {
  readonly connectionId: string;
  readonly subprotocol?: string;
  send(frame: PluroraWebSocketFrame): Promise<void>;
  close(code?: number, reason?: string): Promise<void>;
}

export interface PluroraWebSocketCallbacks {
  onOpen?: (info: { connectionId: string; subprotocol?: string }) => void;
  onFrame: (frame: PluroraWebSocketFrame & { seq: number; direction: "inbound" }) => void;
  onClose?: (info: { code: number; reason: string }) => void;
  onError?: (err: { code: string; message: string }) => void;
}

export interface PluroraWebSocketOpenParams {
  capability_id: string;
  destination_host: string;
  path?: string;
  purpose?: string;
  subprotocols?: string[];
  secret_refs?: string[];
  metadata?: Record<string, unknown>;
  static_headers?: Record<string, string>;
  secret_headers?: Record<string, { secret_ref: string; scheme?: string }>;
  max_frame_bytes?: number;
  max_total_bytes_inbound?: number;
  max_total_bytes_outbound?: number;
  max_idle_ms?: number;
  max_duration_ms?: number;
}

export interface PluroraClient {
  bindings: Record<string, CapHandleId>;
  sendRequest<T = unknown>(method: string, params: unknown): Promise<T>;
  streamRequest(method: string, params: unknown, callbacks: PluroraStreamCallbacks): PluroraStreamHandle;
  invokeBinding<T = unknown>(name: string, input: unknown): Promise<T>;
  invokeBindingStream(name: string, input: unknown, callbacks: PluroraStreamCallbacks): PluroraStreamHandle;
  openWebSocket(
    params: PluroraWebSocketOpenParams,
    callbacks: PluroraWebSocketCallbacks,
  ): Promise<PluroraWebSocketHandle>;
}

interface PendingPluroraRequest {
  resolve: (value: unknown) => void;
  reject: (error: unknown) => void;
}

interface PendingPluroraStream {
  callbacks: PluroraStreamCallbacks;
  streamId?: string;
}

interface PendingPluroraWebSocketOpen {
  callbacks: PluroraWebSocketCallbacks;
  client: PluroraClient;
  resolve: (handle: PluroraWebSocketHandle) => void;
  reject: (error: unknown) => void;
}

interface ActivePluroraWebSocket {
  callbacks: PluroraWebSocketCallbacks;
  requestId: string;
  connectionId: string;
  subprotocol?: string;
  closed: boolean;
  closeWaiters: Set<(error: Error) => void>;
  client: PluroraClient;
}

type ReverseRequestContext =
  | { kind: "background" }
  | { kind: "invocation"; invocationContextId: string; bindings: Record<string, CapHandleId> };

let nextPluroraRequestId = 1;
const pendingPluroraRequests = new Map<string, PendingPluroraRequest>();
const pendingPluroraStreams = new Map<string, PendingPluroraStream>();
const streamRequestIdsByStreamId = new Map<string, string>();
const pendingPluroraWebSocketOpens = new Map<string, PendingPluroraWebSocketOpen>();
const pluroraWebSocketsByRequestId = new Map<string, ActivePluroraWebSocket>();
const pluroraWebSocketsByConnectionId = new Map<string, ActivePluroraWebSocket>();

function respond(id: JsonRpcRequest["id"], payload: Record<string, JsonValue>) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, ...payload }) + "\n");
}

function contextParams(params: unknown, context: ReverseRequestContext): Record<string, unknown> {
  const value: Record<string, unknown> = asRecord(params) ? { ...asRecord(params) } : { input: params };
  if (context.kind === "invocation") {
    value.invocation_context_id = context.invocationContextId;
  } else {
    value.invocation_context = { kind: "background" };
  }
  return value;
}

function sendPlatformFrame(method: string, params: unknown, context: ReverseRequestContext): string {
  const id = `kreq-${nextPluroraRequestId++}`;
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, method, params: contextParams(params, context) }) + "\n");
  return id;
}

function rejectPlatformError(error: unknown): Error {
  const code = asRecord(error)?.code;
  const safeCode = typeof code === "string" && /^[a-z0-9_./-]+$/i.test(code) ? code : "unknown";
  return new Error(`Host platform request failed (${safeCode})`);
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function isWebSocketEventKind(kind: unknown): kind is string {
  return kind === "host/outbound.websocket.opened"
    || kind === "host/outbound.websocket.frame"
    || kind === "host/outbound.websocket.error"
    || kind === "host/outbound.websocket.closed"
    || kind === "host/outbound.websocket.completed";
}

function getFramePayload(frame: JsonRpcRequest): Record<string, unknown> {
  const record = frame as Record<string, unknown>;
  const payload = asRecord(record.payload) ?? asRecord(record.data) ?? asRecord(record.frame) ?? {};
  return { ...payload, ...record };
}

function getConnectionIdFromFrame(frame: JsonRpcRequest): string | undefined {
  const payload = getFramePayload(frame);
  return typeof payload.connection_id === "string" ? payload.connection_id : undefined;
}

function encodeWebSocketFrame(frame: PluroraWebSocketFrame): Record<string, unknown> {
  if (frame.kind === "text") {
    return { kind: "text", data: frame.data };
  }
  return { kind: "binary", bytes: Array.from(frame.data) };
}

function decodeBinaryData(value: unknown): Uint8Array | undefined {
  if (value instanceof Uint8Array) return value;
  if (Array.isArray(value)) {
    const bytes = value.map((item) => typeof item === "number" ? item : Number.NaN);
    if (bytes.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)) {
      return Uint8Array.from(bytes);
    }
    return undefined;
  }
  if (typeof value === "string") {
    return Uint8Array.from(Buffer.from(value, "base64"));
  }
  return undefined;
}

function decodeInboundWebSocketFrame(payload: Record<string, unknown>): PluroraWebSocketFrame | undefined {
  const nested = asRecord(payload.frame) ?? asRecord(payload.payload) ?? payload;
  const kind = nested.kind ?? payload.frame_kind;
  if (kind === "text") {
    const data = nested.data ?? nested.text ?? payload.data ?? payload.text;
    return typeof data === "string" ? { kind: "text", data } : undefined;
  }
  if (kind === "binary") {
    const data = nested.bytes ?? nested.data ?? nested.data_b64 ?? nested.payload_b64 ?? payload.bytes ?? payload.data ?? payload.data_b64;
    const decoded = decodeBinaryData(data);
    return decoded ? { kind: "binary", data: decoded } : undefined;
  }
  return undefined;
}

function normalizeSendStatus(status: unknown): string {
  return String(status ?? "ok").toLowerCase().replace(/[^a-z0-9]/g, "");
}

function markWebSocketClosed(session: ActivePluroraWebSocket, error: Error) {
  session.closed = true;
  for (const waiter of session.closeWaiters) waiter(error);
  session.closeWaiters.clear();
}

function removeWebSocketSession(session: ActivePluroraWebSocket) {
  pluroraWebSocketsByRequestId.delete(session.requestId);
  pluroraWebSocketsByConnectionId.delete(session.connectionId);
}

function createWebSocketHandle(session: ActivePluroraWebSocket): PluroraWebSocketHandle {
  return {
    get connectionId() {
      return session.connectionId;
    },
    get subprotocol() {
      return session.subprotocol;
    },
    async send(frame: PluroraWebSocketFrame): Promise<void> {
      if (session.closed) {
        throw new Error(`WebSocket connection ${session.connectionId} is closed`);
      }
      let closeReject: ((error: Error) => void) | undefined;
      const closePromise = new Promise<never>((_resolve, reject) => {
        closeReject = reject;
        session.closeWaiters.add(reject);
      });
      try {
        const result = await Promise.race([
          session.client.sendRequest<{ status?: unknown }>("host.outbound.websocket.send", {
            connection_id: session.connectionId,
            ...encodeWebSocketFrame(frame),
          }),
          closePromise,
        ]);
        const status = normalizeSendStatus(result?.status);
        if (status === "ok") return;
        if (status === "bufferfull") throw new Error(`WebSocket connection ${session.connectionId} send buffer is full`);
        if (status === "connectionclosed") throw new Error(`WebSocket connection ${session.connectionId} is closed`);
        if (status === "connectionnotfound") throw new Error(`WebSocket connection ${session.connectionId} was not found`);
        throw new Error(`WebSocket connection ${session.connectionId} send failed with status ${String(result?.status)}`);
      } finally {
        if (closeReject) session.closeWaiters.delete(closeReject);
      }
    },
    async close(code?: number, reason?: string): Promise<void> {
      if (session.closed) return;
      markWebSocketClosed(session, new Error(`WebSocket connection ${session.connectionId} is closed`));
      await session.client.sendRequest("host.outbound.websocket.close", {
        connection_id: session.connectionId,
        code,
        reason,
      });
    },
  };
}

function handleWebSocketEvent(session: ActivePluroraWebSocket, frame: JsonRpcRequest): boolean {
  const kind = frame.kind;
  if (!isWebSocketEventKind(kind)) return false;
  const payload = getFramePayload(frame);
  const connectionId = typeof payload.connection_id === "string" ? payload.connection_id : undefined;
  if (connectionId !== session.connectionId) return false;

  switch (kind) {
    case "host/outbound.websocket.opened": {
      const subprotocol = typeof payload.subprotocol === "string" ? payload.subprotocol : session.subprotocol;
      if (typeof subprotocol === "string") session.subprotocol = subprotocol;
      session.callbacks.onOpen?.({ connectionId: session.connectionId, subprotocol });
      return true;
    }
    case "host/outbound.websocket.frame": {
      const direction = typeof payload.direction === "string" ? payload.direction : "inbound";
      if (direction !== "inbound") return true;
      const decoded = decodeInboundWebSocketFrame(payload);
      const seq = typeof payload.seq === "number" ? payload.seq : Number(payload.sequence ?? 0);
      if (decoded && Number.isFinite(seq)) {
        session.callbacks.onFrame({ ...decoded, seq, direction: "inbound" });
      }
      return true;
    }
    case "host/outbound.websocket.error": {
      const code = String(payload.error_code ?? payload.code ?? "websocket_error");
      const message = String(payload.message_redacted ?? payload.message ?? payload.error ?? "WebSocket error");
      session.callbacks.onError?.({ code, message });
      return true;
    }
    case "host/outbound.websocket.closed":
    case "host/outbound.websocket.completed": {
      const code = typeof payload.code === "number" ? payload.code : Number(payload.code ?? 1000);
      const reason = typeof payload.reason === "string" ? payload.reason : "closed";
      markWebSocketClosed(session, new Error(`WebSocket connection ${session.connectionId} closed: ${code} ${reason}`));
      session.callbacks.onClose?.({ code, reason });
      removeWebSocketSession(session);
      return true;
    }
  }
  return false;
}

function resolveWebSocketOpen(requestId: string, pending: PendingPluroraWebSocketOpen, result: unknown) {
  const record = asRecord(result) ?? {};
  const connectionId = record.connection_id;
  if (typeof connectionId !== "string") {
    pending.reject(new Error("host.outbound.websocket.open response missing connection_id"));
    return;
  }
  const subprotocol = typeof record.subprotocol_negotiated === "string"
    ? record.subprotocol_negotiated
    : typeof record.subprotocol === "string"
      ? record.subprotocol
      : undefined;
  const session: ActivePluroraWebSocket = {
    callbacks: pending.callbacks,
    requestId,
    connectionId,
    subprotocol,
    closed: false,
    closeWaiters: new Set(),
    client: pending.client,
  };
  pluroraWebSocketsByRequestId.set(requestId, session);
  pluroraWebSocketsByConnectionId.set(connectionId, session);
  pending.resolve(createWebSocketHandle(session));
}

function getBindingHandle(bindings: Record<string, CapHandleId>, name: string): CapHandleId {
  const handle = bindings[name];
  if (!handle) throw new Error(`unknown capability binding: ${name}`);
  return handle;
}

function createPluroraClient(context: ReverseRequestContext): PluroraClient {
  const bindings = context.kind === "invocation" ? { ...context.bindings } : {};
  return {
  bindings,
  sendRequest<T = unknown>(method: string, params: unknown): Promise<T> {
    const id = sendPlatformFrame(method, params, context);
    return new Promise<T>((resolve, reject) => {
      pendingPluroraRequests.set(id, { resolve: resolve as (value: unknown) => void, reject });
    });
  },

  streamRequest(method: string, params: unknown, callbacks: PluroraStreamCallbacks): PluroraStreamHandle {
    const id = sendPlatformFrame(method, params, context);
    const pending: PendingPluroraStream = { callbacks };
    pendingPluroraStreams.set(id, pending);
    let cancelled = false;

    return {
      get streamId() {
        return pending.streamId;
      },
      cancel() {
        if (cancelled) return;
        cancelled = true;
        const streamId = pending.streamId;
        if (!streamId) return;
        sendPlatformFrame("capability.cancel", { stream_id: streamId, invocation_id: streamId, session_id: `subprocess_reverse_${streamId}` }, context);
      },
    };
  },

  async invokeBinding<T = unknown>(name: string, input: unknown): Promise<T> {
    if (context.kind !== "invocation") throw new Error("Run Binding invocation requires a current Host activation context");
    const result = await this.sendRequest<{ output?: T } & Record<string, unknown>>("capability.invoke", {
      handle: getBindingHandle(bindings, name),
      consumer_port: name,
      input,
    });
    return (result && typeof result === "object" && "output" in result) ? (result.output as T) : (result as T);
  },

  invokeBindingStream(name: string, input: unknown, callbacks: PluroraStreamCallbacks): PluroraStreamHandle {
    if (context.kind !== "invocation") throw new Error("Run Binding stream requires a current Host activation context");
    return this.streamRequest("capability.stream", {
      handle: getBindingHandle(bindings, name),
      consumer_port: name,
      input,
      session_id: `subprocess_binding_${name}`,
    }, callbacks);
  },

  openWebSocket(params: PluroraWebSocketOpenParams, callbacks: PluroraWebSocketCallbacks): Promise<PluroraWebSocketHandle> {
    const id = sendPlatformFrame("host.outbound.websocket.open", params, context);
    return new Promise<PluroraWebSocketHandle>((resolve, reject) => {
      pendingPluroraWebSocketOpens.set(id, { callbacks, client: this, resolve, reject });
    });
  },
  };
}

/** Client for ordinary package background work. Run Bindings require the per-invocation client. */
export const pluroraClient: PluroraClient = createPluroraClient({ kind: "background" });
export const __createInvocationClientForTest = (
  invocationContextId: string,
  bindings: Record<string, CapHandleId>,
): PluroraClient => createPluroraClient({ kind: "invocation", invocationContextId, bindings });

function handlePlatformInbound(frame: JsonRpcRequest): boolean {
  if (typeof frame.id !== "string" || !frame.id.startsWith("kreq-")) {
    const connectionId = getConnectionIdFromFrame(frame);
    const session = connectionId ? pluroraWebSocketsByConnectionId.get(connectionId) : undefined;
    return session ? handleWebSocketEvent(session, frame) : false;
  }
  const requestId = frame.id;

  const pendingWebSocketOpen = pendingPluroraWebSocketOpens.get(requestId);
  if (pendingWebSocketOpen) {
    if (frame.result && typeof frame.result === "object") {
      pendingPluroraWebSocketOpens.delete(requestId);
      resolveWebSocketOpen(requestId, pendingWebSocketOpen, frame.result);
      return true;
    }
    if (frame.error) {
      pendingPluroraWebSocketOpens.delete(requestId);
      pendingWebSocketOpen.reject(rejectPlatformError(frame.error));
      return true;
    }
  }

  const websocket = pluroraWebSocketsByRequestId.get(requestId);
  if (websocket && handleWebSocketEvent(websocket, frame)) return true;

  const pendingStream = pendingPluroraStreams.get(requestId);
  if (pendingStream) {
    if (frame.result && typeof frame.result === "object") {
      const streamId = (frame.result as { stream_id?: unknown }).stream_id;
      if (typeof streamId === "string") {
        pendingStream.streamId = streamId;
        streamRequestIdsByStreamId.set(streamId, requestId);
      }
      return true;
    }
    if (frame.error) {
      pendingPluroraStreams.delete(requestId);
      pendingStream.callbacks.onError?.(frame.error);
      return true;
    }

    switch (frame.kind) {
      case "capability/stream.chunk":
      case "stream.chunk":
        pendingStream.callbacks.onChunk(frame.data);
        return true;
      case "capability/stream.ended":
      case "stream.ended":
        pendingPluroraStreams.delete(requestId);
        if (pendingStream.streamId) streamRequestIdsByStreamId.delete(pendingStream.streamId);
        pendingStream.callbacks.onEnd?.(frame.summary);
        return true;
      case "capability/stream.error":
      case "stream.error":
        pendingPluroraStreams.delete(requestId);
        if (pendingStream.streamId) streamRequestIdsByStreamId.delete(pendingStream.streamId);
        pendingStream.callbacks.onError?.(frame.error);
        return true;
      case "capability/stream.cancelled":
      case "stream.cancelled":
        pendingPluroraStreams.delete(requestId);
        if (pendingStream.streamId) streamRequestIdsByStreamId.delete(pendingStream.streamId);
        pendingStream.callbacks.onCancelled?.();
        return true;
      case "capability/stream.timeout":
      case "stream.timeout":
        pendingPluroraStreams.delete(requestId);
        if (pendingStream.streamId) streamRequestIdsByStreamId.delete(pendingStream.streamId);
        pendingStream.callbacks.onTimeout?.();
        return true;
      default:
        return false;
    }
  }

  const pending = pendingPluroraRequests.get(requestId);
  if (!pending) return false;
  pendingPluroraRequests.delete(requestId);
  if (frame.error) pending.reject(rejectPlatformError(frame.error));
  else pending.resolve(frame.result);
  return true;
}

export const __handlePlatformInboundForTest = handlePlatformInbound;

export function serveSubprocessPackage(options: SubprocessPackageOptions) {
  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", async (line) => {
    let request: JsonRpcRequest;
    try {
      request = JSON.parse(line);
    } catch (error) {
      respond(null, { error: { code: "invalid_json", message: String(error) } as JsonValue });
      return;
    }

    if (handlePlatformInbound(request)) return;

    try {
      if (request.method === "package.handshake") {
        const params = (request.params ?? {}) as HandshakeParams;
        const result = options.onHandshake
          ? await options.onHandshake(params)
          : { ready: true, package_protocol_version: "0.1.0" };
        respond(request.id, { result: result as JsonValue });
      } else if (request.method === "capability.invoke") {
        const params = (request.params ?? {}) as unknown as CapabilityInvokeParams;
        if (!params.invocation_context_id || !params.session_id || !params.bindings || typeof params.bindings !== "object") {
          respond(request.id, { error: { code: "invalid_invocation_context", message: "Host invocation context is incomplete" } as JsonValue });
          return;
        }
        const invocationClient = createPluroraClient({
          kind: "invocation",
          invocationContextId: params.invocation_context_id,
          bindings: params.bindings,
        });
        const output = await options.onInvoke(params, {
          pluroraClient: invocationClient,
          sessionId: params.session_id,
          bindings: Object.freeze({ ...params.bindings }),
        });
        respond(request.id, { result: { output } as JsonValue });
      } else {
        respond(request.id, { error: { code: "unknown_method", message: request.method ?? "<missing>" } as JsonValue });
      }
    } catch (error) {
      respond(request.id, { error: { code: "package_error", message: "package request failed" } as JsonValue });
    }
  });
}
