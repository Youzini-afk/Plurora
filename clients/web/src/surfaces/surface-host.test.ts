import {
  callSurfaceBridgeForTest,
  canSubscribeSurfaceStreamForTest,
  createMountInitialPropsForTest,
  createRpcResultMessageForTest,
  createSurfaceBridgeState,
  createStreamFrameMessageForTest,
  isAuthorizedSurfaceMessageForTest,
  surfaceFrameUrlForTest,
  SurfaceBridgeError,
  type SurfaceHostBridge,
} from "./surface-host";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

function assertDeepEqual(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertOk(value: unknown, message: string) {
  if (!value) throw new Error(message);
}

async function rejectsWithCode(promise: Promise<unknown>, code: string) {
  try {
    await promise;
  } catch (err: unknown) {
    if (err instanceof SurfaceBridgeError && err.code === code) {
      return;
    }
    throw err;
  }
  throw new Error(`expected rejection with code ${code}`);
}

const calls: Array<{ method: string; params: unknown }> = [];
const bridge: SurfaceHostBridge = {
  currentSessionId: "session-current",
  allowedCapabilityIds: ["pkg/cap"],
  callRpc: async (method, params) => {
    calls.push({ method, params });
    if (method === "capability.stream") {
      return { invocation: { invocation_id: "inv-1", stream_id: "stream-1" } };
    }
    return { ok: true };
  },
};

const testWindow = { location: { origin: "http://localhost" } } as Window;
Object.defineProperty(globalThis, "window", { value: testWindow, configurable: true });

assertEqual(
  surfaceFrameUrlForTest("https://remote.example/surface-assets/lease/bundle.mjs"),
  "https://remote.example/surface-frame.html",
);

const mountProps = createMountInitialPropsForTest({ installationId: "installation-1", sessionId: "old-session" }, bridge, "token-1");
assertDeepEqual(mountProps, {
  installationId: "installation-1",
  sessionId: "session-current",
  session_id: "session-current",
  targetOrigin: window.location.origin,
  bridgeToken: "token-1",
  bridge_token: "token-1",
});

const nonObjectMountProps = createMountInitialPropsForTest("not-object", undefined, "token-2");
assertDeepEqual(nonObjectMountProps, {
  targetOrigin: window.location.origin,
  bridgeToken: "token-2",
  bridge_token: "token-2",
});

const expectedSource = testWindow;
assertOk(
  isAuthorizedSurfaceMessageForTest(testWindow, expectedSource, { bridge_token: "token-1" }, "token-1"),
  "expected matching source/token to authorize",
);
assertOk(
  !isAuthorizedSurfaceMessageForTest(testWindow, expectedSource, { bridge_token: "wrong-token" }, "token-1"),
  "expected wrong rpc.call token to be ignored",
);
assertOk(
  !isAuthorizedSurfaceMessageForTest(null, expectedSource, { bridge_token: "token-1" }, "token-1"),
  "expected wrong rpc.call source to be ignored",
);

assertDeepEqual(createRpcResultMessageForTest("rpc-1", "token-1", { ok: true }), {
  type: "rpc.result",
  bridge_token: "token-1",
  id: "rpc-1",
  result: { ok: true },
});

assertDeepEqual(createStreamFrameMessageForTest("sub-1", "chunk", { stream_id: "stream-1" }, "token-1", "session-current"), {
  type: "stream.frame",
  bridge_token: "token-1",
  session_id: "session-current",
  subscription_id: "sub-1",
  kind: "chunk",
  payload: { stream_id: "stream-1" },
});

await rejectsWithCode(
  callSurfaceBridgeForTest(bridge, { id: "1", method: "platform.install.execute", params: {} }),
  "rpc_denied",
);

await callSurfaceBridgeForTest(bridge, {
  id: "2",
  method: "capability.invoke",
  params: { capability_id: "pkg/cap", session_id: "attacker-session", input: { hello: "world" } },
});
assertEqual(calls.at(-1)?.method, "capability.invoke");
assertEqual((calls.at(-1)?.params as { session_id?: string }).session_id, "session-current");

await rejectsWithCode(
  callSurfaceBridgeForTest(bridge, {
    id: "2b",
    method: "capability.invoke",
    params: { capability_id: "pkg/unrelated", input: {} },
  }),
  "capability_denied",
);

const state = createSurfaceBridgeState();
await callSurfaceBridgeForTest(
  bridge,
  { id: "3", method: "capability.stream", params: { capability_id: "pkg/cap", session_id: "other" } },
  state,
);
assertDeepEqual(canSubscribeSurfaceStreamForTest("sub-1", "stream-unknown", [], ["stream-1"]), {
  ok: false,
  code: "not_owned",
});
assertDeepEqual(canSubscribeSurfaceStreamForTest("sub-1", "stream-1", ["sub-1"], ["stream-1"]), {
  ok: false,
  code: "duplicate_subscription",
});

await rejectsWithCode(
  callSurfaceBridgeForTest(
    bridge,
    { id: "4", method: "capability.cancel", params: { stream_id: "not-owned" } },
    state,
  ),
  "not_owned",
);

await callSurfaceBridgeForTest(
  bridge,
  { id: "5", method: "capability.cancel", params: { stream_id: "stream-1" } },
  state,
);
assertEqual((calls.at(-1)?.params as { session_id?: string }).session_id, "session-current");
