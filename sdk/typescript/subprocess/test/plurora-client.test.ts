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
    restore() {
      process.stdout.write = original;
    },
  };
}

test("pluroraClient.sendRequest unary roundtrip", async () => {
  const capture = captureStdout();
  try {
    const promise = pluroraClient.sendRequest("host.outbound.execute", { ok: true });
    const frame = JSON.parse(capture.writes[0]);
    assert.equal(frame.id, "kreq-1");
    assert.equal(frame.method, "host.outbound.execute");
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    assert.deepEqual(await promise, { status: "ok" });
  } finally {
    capture.restore();
  }
});

test("pluroraClient.sendRequest unary error response", async () => {
  const capture = captureStdout();
  try {
    const promise = pluroraClient.sendRequest("host.outbound.execute", {});
    const frame = JSON.parse(capture.writes[0]);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, error: { message: "denied" } });
    await assert.rejects(promise, /denied/);
  } finally {
    capture.restore();
  }
});

test("pluroraClient.streamRequest emits chunks via callback", () => {
  const capture = captureStdout();
  try {
    const chunks: unknown[] = [];
    let ended: unknown;
    pluroraClient.streamRequest("host.outbound.stream", {}, {
      onChunk: (chunk) => chunks.push(chunk),
      onEnd: (summary) => { ended = summary; },
    });
    const frame = JSON.parse(capture.writes[0]);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { stream_id: "str_1" } });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "capability/stream.chunk", stream_id: "str_1", data: { n: 1 } });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "capability/stream.chunk", stream_id: "str_1", data: { n: 2 } });
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "capability/stream.ended", stream_id: "str_1", summary: { ok: true } });
    assert.deepEqual(chunks, [{ n: 1 }, { n: 2 }]);
    assert.deepEqual(ended, { ok: true });
  } finally {
    capture.restore();
  }
});

test("pluroraClient.streamRequest cancel sends capability.cancel", () => {
  const capture = captureStdout();
  try {
    const handle = pluroraClient.streamRequest("host.outbound.stream", {}, { onChunk: () => undefined });
    const frame = JSON.parse(capture.writes[0]);
    __handlePlatformInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { stream_id: "str_cancel" } });
    handle.cancel();
    const cancelFrame = JSON.parse(capture.writes[1]);
    assert.equal(cancelFrame.method, "capability.cancel");
    assert.equal(cancelFrame.params.stream_id, "str_cancel");
  } finally {
    capture.restore();
  }
});

test("pluroraClient handles request id collisions cleanly", () => {
  const capture = captureStdout();
  try {
    void pluroraClient.sendRequest("host.ping", {});
    void pluroraClient.sendRequest("host.ping", {});
    const first = JSON.parse(capture.writes[0]);
    const second = JSON.parse(capture.writes[1]);
    assert.notEqual(first.id, second.id);
  } finally {
    capture.restore();
  }
});
