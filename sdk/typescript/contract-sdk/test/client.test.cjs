const assert = require("node:assert/strict");

const { fromHttpRpc, PluroraClient } = require("../dist/client.js");

async function main() {
  const originalFetch = global.fetch;
  const requests = [];
  global.fetch = async (_url, init) => {
    const body = JSON.parse(init.body);
    requests.push(body);
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          id: body.id,
          result: body.method === "host.info"
            ? { protocol_version: "0.1.0", methods: [], supported_transports: ["http_rpc"] }
            : [],
        };
      },
    };
  };

  try {
    const selection = {
      profile: "plurora.contract.default/v1",
      protocols: [{
        protocol_id: "plurora.change",
        version: "1.0.0",
        profile: "plurora.change/default/v1",
      }],
      versions: [{ layer: "host", version: "0.1.0" }],
    };
    const client = fromHttpRpc("http://host.test/rpc");
    await client.negotiateHost(selection);
    await client.invoke("host.target.list", {});
    await client.invoke("host.info", {});
    await client.invoke("host.target.list", {});
    assert.deepEqual(requests, [
      { jsonrpc: "2.0", id: "1", method: "host.info", params: {}, contract: selection },
      { jsonrpc: "2.0", id: "2", method: "host.target.list", params: {}, contract: selection },
      { jsonrpc: "2.0", id: "3", method: "host.info", params: {}, contract: selection },
      { jsonrpc: "2.0", id: "4", method: "host.target.list", params: {}, contract: selection },
    ]);

    const unsupportedTransport = new PluroraClient({
      async invoke() { return {}; },
      async *invokeStream() {},
    });
    await assert.rejects(
      unsupportedTransport.negotiateHost(selection),
      /does not support explicit contract selection/,
    );
  } finally {
    global.fetch = originalFetch;
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
