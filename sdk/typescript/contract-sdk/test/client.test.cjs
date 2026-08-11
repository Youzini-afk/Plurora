const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { fromHttpRpc, PluroraClient } = require("../dist/client.js");
const methods = require("../dist/methods.js");

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
    for (const name of [
      "hostExposureList", "hostExposureCreate", "hostExposureRevoke",
      "hostBindingList", "hostBindingCandidates", "hostBindingSelect", "hostBindingRevoke",
    ]) {
      assert.equal(typeof methods[name], "function", `${name} must be generated`);
    }
    const eventsSource = fs.readFileSync(path.join(__dirname, "../src/events.ts"), "utf8");
    for (const eventName of [
      "HostExposureCreatedEvent", "HostExposureRevokedEvent", "HostExposureExpiredEvent",
      "HostBindingSelectedEvent", "HostBindingRevokedEvent", "HostBindingExpiredEvent",
    ]) {
      assert.match(eventsSource, new RegExp(`interface ${eventName}\\b`));
    }
    const typesSource = fs.readFileSync(path.join(__dirname, "../src/types.ts"), "utf8");
    for (const typeName of [
      "BindingCandidate", "BindingComponentDisclosure",
      "BindingInstallationDisclosure", "BindingWorkDisclosure",
    ]) {
      assert.match(typesSource, new RegExp(`interface ${typeName}\\b`));
    }
    for (const requiredField of [
      "exposure", "consumer_port", "provider_port", "provider_work",
      "provider_installation", "provider_component", "phase",
    ]) {
      assert.match(typesSource, new RegExp(`"${requiredField}":`));
    }
    for (const typeName of [
      "BindingCandidate", "BindingComponentDisclosure",
      "BindingInstallationDisclosure", "BindingWorkDisclosure",
    ]) {
      const start = typesSource.indexOf(`export interface ${typeName} {`);
      assert.notEqual(start, -1, `${typeName} must exist`);
      const end = typesSource.indexOf("\n}\n", start);
      assert.notEqual(end, -1, `${typeName} must have a complete body`);
      const body = typesSource.slice(start, end + 3);
      for (const privateField of [
        "authority_handle_id", "authority_basis", "grant_reference", "raw_handle",
        "host_path", "secret_value", "stderr",
      ]) {
        assert.doesNotMatch(body, new RegExp(privateField));
      }
    }
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
