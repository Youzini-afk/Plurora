import { PluroraProtocolClient, type InstallationStateAction } from "./client";

const requests: Array<{ method: string; params: Record<string, unknown>; headers: Headers }> = [];
const originalFetch = globalThis.fetch;
const workRevision = { artifact_type_uri: "urn:plurora:work-revision:v1", media_type: "application/json", digest: "sha256:" + "a".repeat(64), size_bytes: 0 };
const assemblyLock = { artifact_type_uri: "urn:plurora:assembly-lock:v1", media_type: "application/json", digest: "sha256:" + "b".repeat(64), size_bytes: 0 };
const installationRecord = () => ({
  schema_version: 1,
  installation_id: "installation-1",
  work_revision: workRevision,
  assembly_lock: assemblyLock,
  display_name: "Demo",
  source: { kind: "git_snapshot" },
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  status: "ready",
});
globalThis.fetch = async (input, init) => {
  const body = JSON.parse(String(init?.body ?? "{}")) as { method?: string; params?: Record<string, unknown> };
  requests.push({ method: body.method ?? "http", params: body.params ?? {}, headers: new Headers(init?.headers) });
  switch (body.method) {
    case "host.installation.list":
      return Response.json({ result: [] });
    case "host.installation.get":
      return Response.json({ result: { record: installationRecord(), revision: 1 } });
    case "host.installation.create":
      return Response.json({ result: { installation: { record: installationRecord(), revision: 1 }, idempotent: false, receipts: [] } });
    case "host.installation.update":
      return Response.json({ result: { installation: { record: installationRecord(), revision: 2 }, idempotent: false, receipts: [] } });
    default:
      return Response.json({ result: {} });
  }
};

const client = new PluroraProtocolClient("https://host.test", "token");
await client.listInstallations();
await client.getInstallation("installation-1");
await client.createInstallation({
  work_id: "tests/demo",
  work_revision: workRevision,
  assembly_lock: assemblyLock,
  display_name: "Demo",
  source: { kind: "git_snapshot" },
  idempotency_key: "create-1",
});
await client.updateInstallation({
  installation_id: "installation-1",
  expected_revision: 1,
  work_revision: workRevision,
  assembly_lock: assemblyLock,
  display_name: "Demo",
  source: { kind: "git_snapshot" },
  idempotency_key: "update-1",
  state_action: {
    kind: "replace",
    replacement_snapshot: { artifact_type_uri: "urn:plurora:installation-state-snapshot:v1", media_type: "application/json", digest: "sha256:" + "c".repeat(64), size_bytes: 42 },
  },
});

if (requests.map((request) => request.method).join(",") !== "host.installation.list,host.installation.get,host.installation.create,host.installation.update") {
  throw new Error("Installation RPC wrappers did not use the exact host.installation methods");
}
if (requests.some((request) => !request.method.startsWith("host.installation"))) throw new Error("unexpected non-installation RPC was called");
if (requests.some((request) => request.headers.get("authorization") !== "Bearer token")) throw new Error("authorization header missing");
const updateRequest = requests.find((request) => request.method === "host.installation.update");
const stateAction = updateRequest?.params.state_action as InstallationStateAction | undefined;
if (stateAction?.kind !== "replace" || !stateAction.replacement_snapshot) throw new Error("state update must carry the replacement snapshot descriptor");
if (Object.keys(stateAction ?? {}).sort().join(",") !== "kind,replacement_snapshot") throw new Error("state update must contain only the public action fields");

globalThis.fetch = originalFetch;
