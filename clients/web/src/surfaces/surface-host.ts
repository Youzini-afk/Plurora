// Iframe-based SurfaceHost.
//
// Mounts third-party surface bundles in a sandboxed iframe with a narrow
// postMessage protocol. The iframe loads /surface-frame.html, which
// dynamically imports the bundle URL passed via mount message.
//
// Security model:
//   - sandbox="allow-scripts" only (no same-origin, no forms, no popups)
//   - host bridge methods are explicit and minimal
//   - bundle URL must be served same-origin or cross-origin with explicit allow
//   - no direct kernel access; surface posts {type: 'rpc.call', method, params, id}
//     and host forwards only a narrow allowlisted bridge policy

const ALLOWED_BRIDGE_METHODS = new Set([
  'host.info',
  'host.ping',
  'capability.invoke',
  'capability.stream',
  'capability.cancel',
]);

const CAPABILITY_METHODS = new Set([
  'capability.invoke',
  'capability.stream',
]);

const MAX_ID_LENGTH = 128;
const MAX_METHOD_LENGTH = 96;
const MAX_CAPABILITY_ID_LENGTH = 256;
const MAX_OWNED_STREAMS = 32;
const MAX_SUBSCRIPTIONS = 8;

// The surface frame is sandboxed without allow-same-origin, so its origin is
// opaque and host->iframe postMessage cannot use the host origin as the target.
// Every host->surface message sent with "*" carries bridge_token instead.
const HOST_TO_SURFACE_TARGET_ORIGIN = '*';

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

interface MountMessage {
  type: 'mount';
  bridge_token: string;
  bundleUrl: string;
  exportName: string;
  wrapperClass?: string;
  initialProps: Record<string, unknown>;
  stylesheets?: string[];
}

interface UnmountMessage {
  type: 'unmount';
  bridge_token: string;
}

interface RpcCallMessage {
  type: 'rpc.call';
  bridge_token?: string;
  id: string;
  method: string;
  params: unknown;
}

interface RpcResultMessage {
  type: 'rpc.result';
  bridge_token: string;
  id: string;
  result?: unknown;
  error?: { code: string; message: string };
}

interface ReadyMessage {
  type: 'ready';
}

interface MountErrorMessage {
  type: 'mount.error';
  bridge_token?: string;
  code?: string;
  message?: string;
}

interface StreamSubscribeMessage {
  type: 'stream.subscribe';
  bridge_token?: string;
  id: string;
  stream_id: string;
  session_id: string;
}

interface StreamUnsubscribeMessage {
  type: 'stream.unsubscribe';
  bridge_token?: string;
  session_id?: string;
  subscription_id: string;
}

interface StreamFrameMessage {
  type: 'stream.frame';
  bridge_token: string;
  session_id: string;
  subscription_id: string;
  kind: 'started' | 'chunk' | 'progress';
  payload: unknown;
}

interface StreamEndedMessage {
  type: 'stream.ended';
  bridge_token: string;
  session_id: string;
  subscription_id: string;
}

interface StreamErrorMessage {
  type: 'stream.error';
  bridge_token: string;
  session_id: string;
  subscription_id: string;
  error: { code: string; message: string };
}

type SurfaceMessage =
  | MountMessage
  | UnmountMessage
  | RpcCallMessage
  | RpcResultMessage
  | ReadyMessage
  | MountErrorMessage
  | StreamSubscribeMessage
  | StreamUnsubscribeMessage
  | StreamFrameMessage
  | StreamEndedMessage
  | StreamErrorMessage;

interface OwnedStreamRecord {
  streamId: string;
  invocationId?: string;
}

interface SurfaceBridgeState {
  ownedStreams: Map<string, OwnedStreamRecord>;
  ownedInvocations: Map<string, OwnedStreamRecord>;
}

interface ActiveSubscriptionRecord {
  bridgeToken: string;
  sessionId: string;
  close: () => void;
}

export class SurfaceBridgeError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

export async function mountSurface(options: SurfaceHostOptions): Promise<SurfaceHostHandle> {
  const container = document.getElementById(options.containerId);
  if (!container) throw new Error(`SurfaceHost: container ${options.containerId} not found`);

  const bridgeToken = createBridgeToken();
  const currentSessionId = options.hostBridge?.currentSessionId;
  const initialProps = createMountInitialProps(options.initialProps, options.hostBridge, bridgeToken);

  const iframe = document.createElement('iframe');
  iframe.className = 'plurora-surface-iframe';
  iframe.setAttribute('sandbox', 'allow-scripts');
  iframe.style.width = '100%';
  iframe.style.height = '100%';
  iframe.style.border = '0';
  iframe.src = surfaceFrameUrlForTest(options.bundleUrl);

  // Wait for iframe to send {type: 'ready'}
  const ready = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('SurfaceHost: iframe ready timeout')), 5000);
    const onMessage = (e: MessageEvent) => {
      if (e.source !== iframe.contentWindow) return;
      const msg = e.data as SurfaceMessage;
      if (msg.type === 'ready') {
        window.removeEventListener('message', onMessage);
        clearTimeout(timer);
        resolve();
      }
    };
    window.addEventListener('message', onMessage);
  });

  container.appendChild(iframe);
  try {
    await ready;
  } catch (err) {
    iframe.remove();
    throw err;
  }

  // Send mount instruction
  const mountError = waitForSurfaceMountError(iframe, bridgeToken, 1000);
  iframe.contentWindow!.postMessage({
    type: 'mount',
    bridge_token: bridgeToken,
    bundleUrl: options.bundleUrl,
    exportName: options.exportName,
    wrapperClass: options.wrapperClass,
    initialProps,
    stylesheets: options.stylesheets,
  } satisfies MountMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
  try {
    await mountError;
  } catch (err) {
    iframe.remove();
    throw err;
  }

  const bridgeState = createSurfaceBridgeState();

  // Wire host bridge for RPC calls from surface
  const bridgeListener = async (e: MessageEvent) => {
    if (e.source !== iframe.contentWindow) return;
    const msg = e.data as SurfaceMessage;
    if (msg.type !== 'rpc.call') return;
    if (!isAuthorizedSurfaceMessage(e.source, iframe.contentWindow, msg, bridgeToken)) return;
    if (!options.hostBridge?.callRpc) {
      iframe.contentWindow!.postMessage({
        type: 'rpc.result',
        bridge_token: bridgeToken,
        id: msg.id,
        error: { code: 'no_bridge', message: 'host did not configure RPC bridge' },
      } satisfies RpcResultMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
      return;
    }
    try {
      const result = await callBridgeRpc(options.hostBridge, msg, bridgeState);
      iframe.contentWindow!.postMessage({
        type: 'rpc.result',
        bridge_token: bridgeToken,
        id: safeResponseId(msg.id),
        result,
      } satisfies RpcResultMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
    } catch (err) {
      const code = err instanceof SurfaceBridgeError ? err.code : 'rpc_denied';
      iframe.contentWindow!.postMessage({
        type: 'rpc.result',
        bridge_token: bridgeToken,
        id: safeResponseId(msg.id),
        error: {
          code,
          message: sanitizedBridgeMessage(code),
        },
      } satisfies RpcResultMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
    }
  };
  window.addEventListener('message', bridgeListener);

  // Wire stream subscriptions from surface to session SSE events.
  const activeSubs = new Map<string, ActiveSubscriptionRecord>();
  const streamListener = async (e: MessageEvent) => {
    if (e.source !== iframe.contentWindow) return;
    const msg = e.data as SurfaceMessage;

    if (msg.type === 'stream.subscribe') {
      if (!isAuthorizedSurfaceMessage(e.source, iframe.contentWindow, msg, bridgeToken)) return;
      if (msg.session_id !== currentSessionId) return;
      await handleStreamSubscribe(msg, iframe, options.hostBridge, activeSubs, bridgeState.ownedStreams, bridgeToken);
    } else if (msg.type === 'stream.unsubscribe') {
      if (!isAuthorizedSurfaceMessage(e.source, iframe.contentWindow, msg, bridgeToken)) return;
      if (msg.session_id !== currentSessionId) return;
      handleStreamUnsubscribe(msg, activeSubs, bridgeToken, currentSessionId);
    }
  };
  window.addEventListener('message', streamListener);

  return {
    surfaceId: options.surfaceId,
    iframe,
    async unmount() {
      iframe.contentWindow?.postMessage({ type: 'unmount', bridge_token: bridgeToken } satisfies UnmountMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
      await new Promise((resolve) => setTimeout(resolve, 50));
      window.removeEventListener('message', bridgeListener);
      window.removeEventListener('message', streamListener);
      for (const subscription of activeSubs.values()) subscription.close();
      activeSubs.clear();
      iframe.remove();
    },
  };
}

export function surfaceFrameUrlForTest(
  bundleUrl: string,
  currentOrigin: string = window.location.origin,
): string {
  const bundle = new URL(bundleUrl, `${currentOrigin}/`);
  if (bundle.protocol !== 'http:' && bundle.protocol !== 'https:') {
    throw new Error('Surface bundle must use HTTP or HTTPS');
  }
  return new URL('/surface-frame.html', bundle.origin).toString();
}

function waitForSurfaceMountError(
  iframe: HTMLIFrameElement,
  bridgeToken: string,
  graceMs: number,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      window.removeEventListener('message', onMessage);
      resolve();
    }, graceMs);
    const onMessage = (e: MessageEvent) => {
      if (e.source !== iframe.contentWindow) return;
      const msg = e.data as SurfaceMessage;
      if (msg.type !== 'mount.error') return;
      if (msg.bridge_token !== bridgeToken) return;
      window.removeEventListener('message', onMessage);
      clearTimeout(timer);
      reject(new SurfaceBridgeError(
        isBoundedString(msg.code, 64) ? msg.code : 'mount_failed',
        isBoundedString(msg.message, 160) ? msg.message : 'Surface mount failed',
      ));
    };
    window.addEventListener('message', onMessage);
  });
}

export function createSurfaceBridgeState(): SurfaceBridgeState {
  return {
    ownedStreams: new Map<string, OwnedStreamRecord>(),
    ownedInvocations: new Map<string, OwnedStreamRecord>(),
  };
}

export async function callSurfaceBridgeForTest(
  hostBridge: SurfaceHostBridge,
  message: { id: string; method: string; params: unknown },
  state: SurfaceBridgeState = createSurfaceBridgeState(),
): Promise<unknown> {
  return callBridgeRpc(hostBridge, { type: 'rpc.call', ...message }, state);
}

export function canSubscribeSurfaceStreamForTest(
  subscriptionId: string,
  streamId: string,
  activeSubscriptionIds: Iterable<string>,
  ownedStreamIds: Iterable<string>,
): { ok: true } | { ok: false; code: string } {
  if (!isSafeIdentifier(subscriptionId, MAX_ID_LENGTH) || !isSafeIdentifier(streamId, MAX_ID_LENGTH)) {
    return { ok: false, code: 'invalid_request' };
  }
  const active = new Set(activeSubscriptionIds);
  if (active.has(subscriptionId)) return { ok: false, code: 'duplicate_subscription' };
  if (active.size >= MAX_SUBSCRIPTIONS) return { ok: false, code: 'limit_exceeded' };
  if (!new Set(ownedStreamIds).has(streamId)) return { ok: false, code: 'not_owned' };
  return { ok: true };
}

export function createMountInitialPropsForTest(
  initialProps: unknown,
  hostBridge: Pick<SurfaceHostBridge, 'currentSessionId'> | undefined,
  bridgeToken: string,
): Record<string, unknown> {
  return createMountInitialProps(initialProps, hostBridge, bridgeToken);
}

export function isAuthorizedSurfaceMessageForTest(
  eventSource: MessageEvent['source'],
  expectedSource: Window | null,
  message: { bridge_token?: string },
  bridgeToken: string,
): boolean {
  return isAuthorizedSurfaceMessage(eventSource, expectedSource, message, bridgeToken);
}

export function createRpcResultMessageForTest(
  id: string,
  bridgeToken: string,
  result: unknown,
): RpcResultMessage {
  return {
    type: 'rpc.result',
    bridge_token: bridgeToken,
    id: safeResponseId(id),
    result,
  };
}

export function createStreamFrameMessageForTest(
  subscriptionId: string,
  kind: StreamFrameMessage['kind'],
  payload: unknown,
  bridgeToken: string,
  sessionId: string,
): StreamFrameMessage {
  return createStreamFrameMessage(subscriptionId, kind, payload, bridgeToken, sessionId);
}

function createMountInitialProps(
  initialProps: unknown,
  hostBridge: Pick<SurfaceHostBridge, 'currentSessionId'> | undefined,
  bridgeToken: string,
): Record<string, unknown> {
  const props: Record<string, unknown> = isPlainRecord(initialProps) ? { ...initialProps } : {};
  if (hostBridge?.currentSessionId) {
    props.sessionId = hostBridge.currentSessionId;
    props.session_id = hostBridge.currentSessionId;
  }
  props.targetOrigin = window.location.origin;
  props.bridgeToken = bridgeToken;
  props.bridge_token = bridgeToken;
  return props;
}

function createBridgeToken(): string {
  const cryptoApi = globalThis.crypto;
  if (typeof cryptoApi?.randomUUID === 'function') return cryptoApi.randomUUID();
  if (typeof cryptoApi?.getRandomValues === 'function') {
    const bytes = new Uint8Array(16);
    cryptoApi.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  return `bridge-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

function isAuthorizedSurfaceMessage(
  eventSource: MessageEvent['source'],
  expectedSource: Window | null,
  message: { bridge_token?: string },
  bridgeToken: string,
): boolean {
  return eventSource === expectedSource && message.bridge_token === bridgeToken;
}

function safeResponseId(value: unknown): string {
  return isBoundedString(value, MAX_ID_LENGTH) ? value : 'invalid';
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function isBoundedString(value: unknown, max: number): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= max;
}

function isSafeIdentifier(value: unknown, max: number): value is string {
  return isBoundedString(value, max) && /^[A-Za-z0-9][A-Za-z0-9._:/@-]*$/.test(value);
}

function sanitizedBridgeMessage(code: string): string {
  switch (code) {
    case 'rpc_denied':
      return 'RPC method is not available to this surface';
    case 'invalid_request':
      return 'Surface bridge request is invalid';
    case 'capability_denied':
      return 'Capability is not available to this surface';
    case 'not_owned':
      return 'Stream is not owned by this surface';
    case 'limit_exceeded':
      return 'Surface bridge limit exceeded';
    default:
      return 'Surface bridge request failed';
  }
}

async function callBridgeRpc(
  hostBridge: SurfaceHostBridge,
  msg: RpcCallMessage,
  state: SurfaceBridgeState,
): Promise<unknown> {
  if (!isBoundedString(msg.method, MAX_METHOD_LENGTH) || !ALLOWED_BRIDGE_METHODS.has(msg.method)) {
    throw new SurfaceBridgeError('rpc_denied', 'method denied');
  }
  if (!isBoundedString(msg.id, MAX_ID_LENGTH)) {
    throw new SurfaceBridgeError('invalid_request', 'invalid id');
  }
  if (!isBoundedString(hostBridge.currentSessionId, MAX_ID_LENGTH)) {
    throw new SurfaceBridgeError('invalid_request', 'invalid session');
  }
  if (msg.method === 'host.ping') {
    return { ok: true };
  }
  if (!hostBridge.callRpc) {
    throw new SurfaceBridgeError('rpc_denied', 'bridge unavailable');
  }

  if (CAPABILITY_METHODS.has(msg.method)) {
    const params = sanitizeCapabilityParams(msg.params, hostBridge);
    const result = await hostBridge.callRpc(msg.method, params);
    if (msg.method === 'capability.stream') {
      recordOwnedStream(result, state.ownedStreams, state.ownedInvocations);
    }
    return result;
  }

  if (msg.method === 'capability.cancel') {
    const params = sanitizeCancelParams(msg.params, hostBridge, state.ownedStreams, state.ownedInvocations);
    const result = await hostBridge.callRpc(msg.method, params);
    removeOwnedStream(params, state.ownedStreams, state.ownedInvocations);
    return result;
  }

  return hostBridge.callRpc(msg.method, {});
}

function sanitizeCapabilityParams(params: unknown, hostBridge: SurfaceHostBridge): Record<string, unknown> {
  if (!isPlainRecord(params)) {
    throw new SurfaceBridgeError('invalid_request', 'params must be an object');
  }
  const capabilityId = params.capability_id;
  if (!isSafeIdentifier(capabilityId, MAX_CAPABILITY_ID_LENGTH)) {
    throw new SurfaceBridgeError('invalid_request', 'invalid capability id');
  }
  const allowed = new Set(hostBridge.allowedCapabilityIds ?? []);
  if (!allowed.has(capabilityId)) {
    throw new SurfaceBridgeError('capability_denied', 'capability denied');
  }
  const out: Record<string, unknown> = {
    capability_id: capabilityId,
    session_id: hostBridge.currentSessionId,
  };
  if ('input' in params) out.input = params.input;
  if (isSafeIdentifier(params.provider_package_id, MAX_CAPABILITY_ID_LENGTH)) out.provider_package_id = params.provider_package_id;
  if (isBoundedString(params.version, 64)) out.version = params.version;
  if (isPlainRecord(params.metadata)) out.metadata = params.metadata;
  return out;
}

function sanitizeCancelParams(
  params: unknown,
  hostBridge: SurfaceHostBridge,
  ownedStreams: Map<string, OwnedStreamRecord>,
  ownedInvocations: Map<string, OwnedStreamRecord>,
): { session_id: string; stream_id?: string; invocation_id?: string } {
  if (!isPlainRecord(params)) {
    throw new SurfaceBridgeError('invalid_request', 'params must be an object');
  }
  const streamId = isSafeIdentifier(params.stream_id, MAX_ID_LENGTH) ? params.stream_id : undefined;
  const invocationId = isSafeIdentifier(params.invocation_id, MAX_ID_LENGTH) ? params.invocation_id : undefined;
  if (!streamId && !invocationId) {
    throw new SurfaceBridgeError('invalid_request', 'cancel requires stream or invocation id');
  }
  if (streamId && !ownedStreams.has(streamId)) {
    throw new SurfaceBridgeError('not_owned', 'stream not owned');
  }
  if (invocationId && !ownedInvocations.has(invocationId)) {
    throw new SurfaceBridgeError('not_owned', 'invocation not owned');
  }
  const sessionId = hostBridge.currentSessionId;
  if (!isBoundedString(sessionId, MAX_ID_LENGTH)) {
    throw new SurfaceBridgeError('invalid_request', 'invalid session');
  }
  return {
    session_id: sessionId,
    ...(streamId ? { stream_id: streamId } : {}),
    ...(invocationId ? { invocation_id: invocationId } : {}),
  };
}

function recordOwnedStream(
  result: unknown,
  ownedStreams: Map<string, OwnedStreamRecord>,
  ownedInvocations: Map<string, OwnedStreamRecord>,
) {
  if (ownedStreams.size >= MAX_OWNED_STREAMS) {
    throw new SurfaceBridgeError('limit_exceeded', 'too many streams');
  }
  const streamId = extractStringField(result, 'stream_id');
  const invocationId = extractStringField(result, 'invocation_id');
  if (!isSafeIdentifier(streamId, MAX_ID_LENGTH)) {
    throw new SurfaceBridgeError('invalid_request', 'stream result missing stream id');
  }
  const record = { streamId, ...(invocationId ? { invocationId } : {}) };
  ownedStreams.set(streamId, record);
  if (invocationId) ownedInvocations.set(invocationId, record);
}

function extractStringField(value: unknown, field: string): string | undefined {
  if (!isPlainRecord(value)) return undefined;
  const direct = value[field];
  if (typeof direct === 'string') return direct;
  for (const nestedKey of ['frame', 'invocation', 'record']) {
    const nested = value[nestedKey];
    if (isPlainRecord(nested) && typeof nested[field] === 'string') return nested[field];
  }
  return undefined;
}

function removeOwnedStream(
  params: { stream_id?: string; invocation_id?: string },
  ownedStreams: Map<string, OwnedStreamRecord>,
  ownedInvocations: Map<string, OwnedStreamRecord>,
) {
  const record = params.stream_id
    ? ownedStreams.get(params.stream_id)
    : params.invocation_id
      ? ownedInvocations.get(params.invocation_id)
      : undefined;
  if (!record) return;
  ownedStreams.delete(record.streamId);
  if (record.invocationId) ownedInvocations.delete(record.invocationId);
}

function payloadStreamId(payload: unknown): string | undefined {
  if (!payload || typeof payload !== 'object') return undefined;
  const streamId = (payload as { stream_id?: unknown }).stream_id;
  return typeof streamId === 'string' ? streamId : undefined;
}

async function handleStreamSubscribe(
  msg: StreamSubscribeMessage,
  iframe: HTMLIFrameElement,
  hostBridge: SurfaceHostBridge | undefined,
  activeSubs: Map<string, ActiveSubscriptionRecord>,
  ownedStreams: Map<string, OwnedStreamRecord>,
  bridgeToken: string,
) {
  const sessionId = hostBridge?.currentSessionId ?? '';
  if (!hostBridge?.subscribeEvents) {
    iframe.contentWindow?.postMessage({
      type: 'stream.error',
      bridge_token: bridgeToken,
      session_id: sessionId,
      subscription_id: msg.id,
      error: { code: 'no_bridge', message: 'host did not configure event subscription bridge' },
    } satisfies StreamErrorMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
    return;
  }
  const decision = canSubscribeSurfaceStreamForTest(msg.id, msg.stream_id, activeSubs.keys(), ownedStreams.keys());
  if (!decision.ok) {
    postStreamError(iframe, safeResponseId(msg.id), decision.code, bridgeToken, sessionId);
    return;
  }

  let close: () => void = () => {};
  try {
    close = hostBridge.subscribeEvents((event: unknown) => {
      const ev = event as { kind?: string; payload?: unknown };
      const kind = ev.kind ?? '';
      if (!kind.startsWith('capability/stream.')) return;
      if (payloadStreamId(ev.payload) !== msg.stream_id) return;

      if (kind === 'capability/stream.started') {
        iframe.contentWindow?.postMessage(
          createStreamFrameMessage(msg.id, 'started', ev.payload, bridgeToken, sessionId),
          HOST_TO_SURFACE_TARGET_ORIGIN,
        );
      } else if (kind === 'capability/stream.chunk') {
        iframe.contentWindow?.postMessage(
          createStreamFrameMessage(msg.id, 'chunk', ev.payload, bridgeToken, sessionId),
          HOST_TO_SURFACE_TARGET_ORIGIN,
        );
      } else if (kind === 'capability/stream.progress') {
        iframe.contentWindow?.postMessage(
          createStreamFrameMessage(msg.id, 'progress', ev.payload, bridgeToken, sessionId),
          HOST_TO_SURFACE_TARGET_ORIGIN,
        );
      } else if (kind === 'capability/stream.ended') {
        iframe.contentWindow?.postMessage({
          type: 'stream.ended',
          bridge_token: bridgeToken,
          session_id: sessionId,
          subscription_id: msg.id,
        } satisfies StreamEndedMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
        const active = activeSubs.get(msg.id);
        if (active?.bridgeToken === bridgeToken && active.sessionId === sessionId) {
          active.close();
          activeSubs.delete(msg.id);
        }
      } else if (
        kind === 'capability/stream.error' ||
        kind === 'capability/stream.cancelled' ||
        kind === 'capability/stream.timeout'
      ) {
        iframe.contentWindow?.postMessage({
          type: 'stream.error',
          bridge_token: bridgeToken,
          session_id: sessionId,
          subscription_id: msg.id,
          error: { code: kind, message: 'Stream failed' },
        } satisfies StreamErrorMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
        const active = activeSubs.get(msg.id);
        if (active?.bridgeToken === bridgeToken && active.sessionId === sessionId) {
          active.close();
          activeSubs.delete(msg.id);
        }
      }
    });
    activeSubs.set(msg.id, { bridgeToken, sessionId, close });
  } catch (err) {
    iframe.contentWindow?.postMessage({
      type: 'stream.error',
      bridge_token: bridgeToken,
      session_id: sessionId,
      subscription_id: msg.id,
      error: { code: 'subscribe_failed', message: 'Stream subscription failed' },
    } satisfies StreamErrorMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
    close();
  }
}

function createStreamFrameMessage(
  subscriptionId: string,
  kind: StreamFrameMessage['kind'],
  payload: unknown,
  bridgeToken: string,
  sessionId: string,
): StreamFrameMessage {
  return {
    type: 'stream.frame',
    bridge_token: bridgeToken,
    session_id: sessionId,
    subscription_id: subscriptionId,
    kind,
    payload,
  };
}

function postStreamError(
  iframe: HTMLIFrameElement,
  subscriptionId: string,
  code: string,
  bridgeToken: string,
  sessionId: string,
) {
  iframe.contentWindow?.postMessage({
    type: 'stream.error',
    bridge_token: bridgeToken,
    session_id: sessionId,
    subscription_id: subscriptionId,
    error: { code, message: sanitizedBridgeMessage(code) },
  } satisfies StreamErrorMessage, HOST_TO_SURFACE_TARGET_ORIGIN);
}

function handleStreamUnsubscribe(
  msg: StreamUnsubscribeMessage,
  activeSubs: Map<string, ActiveSubscriptionRecord>,
  bridgeToken: string,
  sessionId: string | undefined,
) {
  const active = activeSubs.get(msg.subscription_id);
  if (active?.bridgeToken === bridgeToken && active.sessionId === sessionId) {
    active.close();
    activeSubs.delete(msg.subscription_id);
  }
}
