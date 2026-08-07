import assert from "node:assert/strict";
import { test } from "node:test";
import { Writable } from "node:stream";

import { kernelClient, __handleKernelInboundForTest } from "../index.js";

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

test("kernelClient.sendKernelRequest unary roundtrip", async () => {
  const capture = captureStdout();
  try {
    const promise = kernelClient.sendKernelRequest("kernel.v1.outbound.execute", { ok: true });
    const frame = JSON.parse(capture.writes[0]);
    assert.equal(frame.id, "kreq-1");
    assert.equal(frame.method, "kernel.v1.outbound.execute");
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { status: "ok" } });
    assert.deepEqual(await promise, { status: "ok" });
  } finally {
    capture.restore();
  }
});

test("kernelClient exposes contract legacy-adapter diagnostics", async () => {
  const capture = captureStdout();
  try {
    kernelClient.drainContractDiagnostics();
    const promise = kernelClient.sendKernelRequest("kernel.v1.host.info", {});
    const frame = JSON.parse(capture.writes[0]);
    __handleKernelInboundForTest({
      jsonrpc: "2.0",
      id: frame.id,
      result: { protocol_version: "0.1.0" },
      diagnostics: [{
        code: "plurora.contract.alias.legacy_adapter",
        severity: "warning",
        requested_id: "kernel.v1.host.info",
        canonical_id: "host.info",
        maturity: "legacy_adapter",
        message: "use host.info; no new field semantics will be added",
        replacement: "host.info",
      }],
    });
    await promise;
    const followup = kernelClient.sendKernelRequest("kernel.v1.host.ping", {});
    const followupFrame = JSON.parse(capture.writes[1]);
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: followupFrame.id, result: { ok: true } });
    await followup;
    const diagnostic = kernelClient.drainContractDiagnostics()[0];
    assert.equal(diagnostic?.code, "plurora.contract.alias.legacy_adapter");
    assert.equal(diagnostic?.maturity, "legacy_adapter");
    assert.equal(diagnostic?.replacement, "host.info");
    assert.deepEqual(kernelClient.drainContractDiagnostics(), []);
  } finally {
    capture.restore();
  }
});

test("kernelClient.sendKernelRequest unary error response", async () => {
  const capture = captureStdout();
  try {
    const promise = kernelClient.sendKernelRequest("kernel.v1.outbound.execute", {});
    const frame = JSON.parse(capture.writes[0]);
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, error: { message: "denied" } });
    await assert.rejects(promise, /denied/);
  } finally {
    capture.restore();
  }
});

test("kernelClient.streamKernelRequest emits chunks via callback", () => {
  const capture = captureStdout();
  try {
    const chunks: unknown[] = [];
    let ended: unknown;
    kernelClient.streamKernelRequest("kernel.v1.outbound.stream", {}, {
      onChunk: (chunk) => chunks.push(chunk),
      onEnd: (summary) => { ended = summary; },
    });
    const frame = JSON.parse(capture.writes[0]);
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { stream_id: "str_1" } });
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "kernel/v1/stream.chunk", stream_id: "str_1", data: { n: 1 } });
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "kernel/v1/stream.chunk", stream_id: "str_1", data: { n: 2 } });
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, kind: "kernel/v1/stream.ended", stream_id: "str_1", summary: { ok: true } });
    assert.deepEqual(chunks, [{ n: 1 }, { n: 2 }]);
    assert.deepEqual(ended, { ok: true });
  } finally {
    capture.restore();
  }
});

test("kernelClient.streamKernelRequest cancel sends capability.cancel", () => {
  const capture = captureStdout();
  try {
    const handle = kernelClient.streamKernelRequest("kernel.v1.outbound.stream", {}, { onChunk: () => undefined });
    const frame = JSON.parse(capture.writes[0]);
    __handleKernelInboundForTest({ jsonrpc: "2.0", id: frame.id, result: { stream_id: "str_cancel" } });
    handle.cancel();
    const cancelFrame = JSON.parse(capture.writes[1]);
    assert.equal(cancelFrame.method, "kernel.v1.capability.cancel");
    assert.equal(cancelFrame.params.stream_id, "str_cancel");
  } finally {
    capture.restore();
  }
});

test("kernelClient handles request id collisions cleanly", () => {
  const capture = captureStdout();
  try {
    void kernelClient.sendKernelRequest("kernel.v1.host.ping", {});
    void kernelClient.sendKernelRequest("kernel.v1.host.ping", {});
    const first = JSON.parse(capture.writes[0]);
    const second = JSON.parse(capture.writes[1]);
    assert.notEqual(first.id, second.id);
  } finally {
    capture.restore();
  }
});
