import assert from "node:assert/strict";
import { test } from "node:test";
import { Writable } from "node:stream";

import { pluroraClient, __handlePlatformInboundForTest } from "../index.js";

function captureStdout() {
  const writes: string[] = [];
  const original = process.stdout.write;
  process.stdout.write = ((chunk: string | Uint8Array, ...args: unknown[]) => {
    writes.push(Buffer.isBuffer(chunk) ? chunk.toString("utf8") : String(chunk));
    const cb = args.find((arg) => typeof arg === "function") as (() => void) | undefined;
    cb?.();
    return true;
  }) as Writable["write"];
  return {
    writes,
    frame(index: number) {
      return JSON.parse(writes[index]);
    },
    restore() {
      process.stdout.write = original;
    },
  };
}

const openParams = {
  capability_id: "example/pkg/ws",
  destination_host: "api.example.com",
  path: "/v1/realtime",
  purpose: "test websocket",
  subprotocols: ["json"],
  secret_refs: ["secret_ref:env:TOKEN"],
  metadata: { trace: "abc" },
  static_headers: { "x-test": "yes" },
  secret_headers: { authorization: { secret_ref: "secret_ref:env:TOKEN", scheme: "bearer" } },
  max_frame_bytes: 1024,
  max_total_bytes_inbound: 2048,
  max_total_bytes_outbound: 4096,
  max_idle_ms: 5000,
  max_duration_ms: 10000,
};

async function openHandle(connectionId = "conn-1") {
  const capture = captureStdout();
  const frames: unknown[] = [];
  const promise = pluroraClient.openWebSocket(openParams, { onFrame: (frame) => frames.push(frame) });
  const frame = capture.frame(0);
  __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { connection_id: connectionId, subprotocol_negotiated: "json", status: "ok" } });
  const handle = await promise;
  return { capture, handle, frames, openFrame: frame };
}

test("openWebSocket sends correct host.outbound.websocket.open frame", async () => {
  const capture = captureStdout();
  try {
    const promise = pluroraClient.openWebSocket(openParams, { onFrame: () => undefined });
    const frame = capture.frame(0);
    assert.equal(frame.method, "host.outbound.websocket.open");
    assert.deepEqual(frame.params, openParams);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { connection_id: "conn-open", status: "ok" } });
    await promise;
  } finally {
    capture.restore();
  }
});

test("openWebSocket resolves with connection_id when runtime responds successfully", async () => {
  const capture = captureStdout();
  try {
    const promise = pluroraClient.openWebSocket(openParams, { onFrame: () => undefined });
    const frame = capture.frame(0);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { connection_id: "conn-ok", subprotocol_negotiated: "json", status: "ok" } });
    const handle = await promise;
    assert.equal(handle.connectionId, "conn-ok");
    assert.equal(handle.subprotocol, "json");
  } finally {
    capture.restore();
  }
});

test("openWebSocket rejects when runtime returns error response", async () => {
  const capture = captureStdout();
  try {
    const promise = pluroraClient.openWebSocket(openParams, { onFrame: () => undefined });
    const frame = capture.frame(0);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, error: { message: "websocket denied" } });
    await assert.rejects(promise, /websocket denied/);
  } finally {
    capture.restore();
  }
});

test("openWebSocket onFrame called for inbound frame events filtered by connection_id", async () => {
  const { capture, frames, openFrame } = await openHandle("conn-frame");
  try {
    __handlePlatformInboundForTest({
      jsonrpc: "2.0",
      id: openFrame.id,
      kind: "host/outbound.websocket.frame",
      connection_id: "conn-frame",
      direction: "inbound",
      frame_kind: "text",
      seq: 7,
      frame: { kind: "text", data: "hello" },
    });
    assert.deepEqual(frames, [{ kind: "text", data: "hello", seq: 7, direction: "inbound" }]);
  } finally {
    capture.restore();
  }
});

test("openWebSocket onFrame NOT called for events with different connection_id", async () => {
  const { capture, frames, openFrame } = await openHandle("conn-filter");
  try {
    const handled = __handlePlatformInboundForTest({
      jsonrpc: "2.0",
      id: openFrame.id,
      kind: "host/outbound.websocket.frame",
      connection_id: "conn-other",
      direction: "inbound",
      frame: { kind: "text", data: "ignore" },
      seq: 1,
    });
    assert.equal(handled, false);
    assert.deepEqual(frames, []);
  } finally {
    capture.restore();
  }
});

test("openWebSocket onClose called with code+reason on closed event", async () => {
  const capture = captureStdout();
  let closeInfo: unknown;
  try {
    const promise = pluroraClient.openWebSocket(openParams, { onFrame: () => undefined, onClose: (info) => { closeInfo = info; } });
    const frame = capture.frame(0);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { connection_id: "conn-close", status: "ok" } });
    await promise;
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "host/outbound.websocket.completed", connection_id: "conn-close", code: 1001, reason: "going away" });
    assert.deepEqual(closeInfo, { code: 1001, reason: "going away" });
  } finally {
    capture.restore();
  }
});

test("openWebSocket onError called on error event", async () => {
  const capture = captureStdout();
  let errorInfo: unknown;
  try {
    const promise = pluroraClient.openWebSocket(openParams, { onFrame: () => undefined, onError: (err) => { errorInfo = err; } });
    const frame = capture.frame(0);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { connection_id: "conn-error", status: "ok" } });
    await promise;
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "host/outbound.websocket.error", connection_id: "conn-error", error_code: "idle_timeout", message_redacted: "idle timeout" });
    assert.deepEqual(errorInfo, { code: "idle_timeout", message: "idle timeout" });
  } finally {
    capture.restore();
  }
});

test("handle.send writes host.outbound.websocket.send frame with connection_id", async () => {
  const { capture, handle } = await openHandle("conn-send");
  try {
    const sendPromise = handle.send({ kind: "text", data: "hello" });
    const frame = capture.frame(1);
    assert.equal(frame.method, "host.outbound.websocket.send");
    assert.equal(frame.params.connection_id, "conn-send");
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    await sendPromise;
  } finally {
    capture.restore();
  }
});

test("handle.send text frame", async () => {
  const { capture, handle } = await openHandle("conn-text");
  try {
    const sendPromise = handle.send({ kind: "text", data: "hello text" });
    const frame = capture.frame(1);
    assert.equal(frame.params.kind, "text");
    assert.equal(frame.params.data, "hello text");
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    await sendPromise;
  } finally {
    capture.restore();
  }
});

test("handle.send binary frame is encoded as bytes array for runtime parser", async () => {
  const { capture, handle } = await openHandle("conn-binary");
  try {
    const sendPromise = handle.send({ kind: "binary", data: Uint8Array.from([1, 2, 255]) });
    const frame = capture.frame(1);
    assert.equal(frame.params.kind, "binary");
    assert.deepEqual(frame.params.bytes, [1, 2, 255]);
    assert.equal(frame.params.data_b64, undefined);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    await sendPromise;
  } finally {
    capture.restore();
  }
});

test("handle.close writes host.outbound.websocket.close frame", async () => {
  const { capture, handle } = await openHandle("conn-close-send");
  try {
    const closePromise = handle.close(1000, "done");
    const frame = capture.frame(1);
    assert.equal(frame.method, "host.outbound.websocket.close");
    assert.deepEqual(frame.params, { connection_id: "conn-close-send", code: 1000, reason: "done" });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    await closePromise;
  } finally {
    capture.restore();
  }
});

test("handle.send after close rejects with descriptive error", async () => {
  const { capture, handle, openFrame } = await openHandle("conn-after-close");
  try {
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: openFrame.id, kind: "host/outbound.websocket.completed", connection_id: "conn-after-close", code: 1000, reason: "done" });
    await assert.rejects(handle.send({ kind: "text", data: "late" }), /conn-after-close.*closed/);
  } finally {
    capture.restore();
  }
});

test("multiple concurrent openWebSocket calls have distinct connection_ids and listeners", async () => {
  const capture = captureStdout();
  const firstFrames: unknown[] = [];
  const secondFrames: unknown[] = [];
  try {
    const firstPromise = pluroraClient.openWebSocket(openParams, { onFrame: (frame) => firstFrames.push(frame) });
    const secondPromise = pluroraClient.openWebSocket(openParams, { onFrame: (frame) => secondFrames.push(frame) });
    const firstOpen = capture.frame(0);
    const secondOpen = capture.frame(1);
    assert.notEqual(firstOpen.id, secondOpen.id);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: secondOpen.id, result: { connection_id: "conn-two", status: "ok" } });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: firstOpen.id, result: { connection_id: "conn-one", status: "ok" } });
    const first = await firstPromise;
    const second = await secondPromise;
    assert.equal(first.connectionId, "conn-one");
    assert.equal(second.connectionId, "conn-two");

    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: firstOpen.id, kind: "host/outbound.websocket.frame", connection_id: "conn-one", direction: "inbound", seq: 1, frame: { kind: "text", data: "one" } });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: secondOpen.id, kind: "host/outbound.websocket.frame", connection_id: "conn-two", direction: "inbound", seq: 1, frame: { kind: "text", data: "two" } });
    assert.deepEqual(firstFrames, [{ kind: "text", data: "one", seq: 1, direction: "inbound" }]);
    assert.deepEqual(secondFrames, [{ kind: "text", data: "two", seq: 1, direction: "inbound" }]);
  } finally {
    capture.restore();
  }
});
