import {
  HOST_EVENT_SESSIONS,
  PluroraProtocolClient,
  ProtocolRpcError,
  type InstallationStateAction,
} from "./client";

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
    case "host.binding.candidates":
      if (body.params?.import_port === "error-port") {
        return Response.json({ error: { code: "binding_unavailable", message: "raw /private/path token=secret stderr", details: { reason_code: "binding_unavailable", next_step: "Retry candidate discovery.", installation_id: "installation-1", port_id: "error-port", stderr: "raw" } } });
      }
      return Response.json({ result: { candidates: [], gaps: [] } });
    case "host.exposure.list":
    case "host.binding.list":
      return Response.json({ result: [] });
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

await client.listExposures({ installation_id: "installation-provider", run_id: "run-provider", status: "active" });
await client.createExposure({
  audience: [{ kind: "installation", id: "installation-1" }],
  expected_installation_revision: 4,
  expected_run_revision: 7,
  expires_at: "2026-01-01T01:00:00Z",
  export_port: "save-export",
  idempotency_key: "exposure-create-1",
  installation_id: "installation-provider",
  run_id: "run-provider",
});
await client.revokeExposure({
  expected_exposure_revision: 2,
  expected_installation_revision: 4,
  expected_run_revision: 7,
  export_port: "save-export",
  exposure_id: "exposure-1",
  idempotency_key: "exposure-revoke-1",
  installation_id: "installation-provider",
  run_id: "run-provider",
});
await client.listBindings({ consumer_installation_id: "installation-1", run_id: "run-1", status: "selected" });
await client.bindingCandidates({
  consumer_installation_id: "installation-1",
  expected_consumer_installation_revision: 5,
  import_port: "save-import",
  phase: "launch",
});
await client.selectBinding({
  candidate_digest: "sha256:" + "d".repeat(64),
  consumer_installation_id: "installation-1",
  expected_consumer_installation_revision: 5,
  expected_exposure_revision: 2,
  expected_provider_installation_revision: 4,
  exposure_id: "exposure-1",
  idempotency_key: "binding-select-1",
  import_port: "save-import",
  phase: "launch",
  provider_installation_id: "installation-provider",
});
await client.revokeBinding({
  binding_id: "binding-1",
  consumer_installation_id: "installation-1",
  expected_binding_revision: 3,
  expected_consumer_installation_revision: 5,
  exposure_id: "exposure-1",
  idempotency_key: "binding-revoke-1",
  import_port: "save-import",
});
const realizationId = "11111111-1111-4111-8111-111111111111";
const rollbackRealizationId = "22222222-2222-4222-8222-222222222222";
const planRef = { artifact_type_uri: "urn:plurora:realization-plan:v1", media_type: "application/json", digest: "sha256:" + "e".repeat(64), size_bytes: 100 };
const approval = { plan_digest: planRef.digest, decision: "approved", accepted_risks: ["public_endpoint"], decided_at: "2026-08-12T00:00:00.000Z" };
await client.listRealizations({ installation_id: "installation-1", target_id: "local" });
await client.getRealization({ installation_id: "installation-1", realization_id: realizationId });
await client.planRealization({
  installation_id: "installation-1",
  expected_installation_revision: 5,
  target_id: "local",
  backends: [{ kind: "oci_image", workload_id: "server", execution_class: "oci-container.v1", image: "example/app@sha256:" + "f".repeat(64), container_port: 8080, port_name: "http", route_id: "installation-1-http", route_access: "host_authenticated", pull_if_missing: false }],
  idempotency_key: "realization-plan-1",
});
await client.applyRealization({ installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 1, plan_ref: planRef, approval, idempotency_key: "realization-apply-1" });
await client.stopRealization({ installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 2, idempotency_key: "realization-stop-1" });
await client.rollbackRealization({ installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 3, rollback_to_realization_id: rollbackRealizationId, approval, idempotency_key: "realization-rollback-1" });
await client.reconcileRealization({ installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 4, idempotency_key: "realization-reconcile-1" });

const expectedMethods = [
  "host.installation.list", "host.installation.get", "host.installation.create", "host.installation.update",
  "host.exposure.list", "host.exposure.create", "host.exposure.revoke",
  "host.binding.list", "host.binding.candidates", "host.binding.select", "host.binding.revoke",
  "host.realization.list", "host.realization.get", "host.realization.plan", "host.realization.apply", "host.realization.stop", "host.realization.rollback", "host.realization.reconcile",
];
if (requests.map((request) => request.method).join(",") !== expectedMethods.join(",")) {
  throw new Error("RPC wrappers did not use the exact public Host methods");
}
if (requests.some((request) => request.headers.get("authorization") !== "Bearer token")) throw new Error("authorization header missing");
const updateRequest = requests.find((request) => request.method === "host.installation.update");
const stateAction = updateRequest?.params.state_action as InstallationStateAction | undefined;
if (stateAction?.kind !== "replace" || !stateAction.replacement_snapshot) throw new Error("state update must carry the replacement snapshot descriptor");
if (Object.keys(stateAction ?? {}).sort().join(",") !== "kind,replacement_snapshot") throw new Error("state update must contain only the public action fields");

const expectedBodies: Record<string, Record<string, unknown>> = {
  "host.exposure.list": { installation_id: "installation-provider", run_id: "run-provider", status: "active" },
  "host.exposure.create": { audience: [{ kind: "installation", id: "installation-1" }], expected_installation_revision: 4, expected_run_revision: 7, expires_at: "2026-01-01T01:00:00Z", export_port: "save-export", idempotency_key: "exposure-create-1", installation_id: "installation-provider", run_id: "run-provider" },
  "host.exposure.revoke": { expected_exposure_revision: 2, expected_installation_revision: 4, expected_run_revision: 7, export_port: "save-export", exposure_id: "exposure-1", idempotency_key: "exposure-revoke-1", installation_id: "installation-provider", run_id: "run-provider" },
  "host.binding.list": { consumer_installation_id: "installation-1", run_id: "run-1", status: "selected" },
  "host.binding.candidates": { consumer_installation_id: "installation-1", expected_consumer_installation_revision: 5, import_port: "save-import", phase: "launch" },
  "host.binding.select": { candidate_digest: "sha256:" + "d".repeat(64), consumer_installation_id: "installation-1", expected_consumer_installation_revision: 5, expected_exposure_revision: 2, expected_provider_installation_revision: 4, exposure_id: "exposure-1", idempotency_key: "binding-select-1", import_port: "save-import", phase: "launch", provider_installation_id: "installation-provider" },
  "host.binding.revoke": { binding_id: "binding-1", consumer_installation_id: "installation-1", expected_binding_revision: 3, expected_consumer_installation_revision: 5, exposure_id: "exposure-1", idempotency_key: "binding-revoke-1", import_port: "save-import" },
  "host.realization.list": { installation_id: "installation-1", target_id: "local" },
  "host.realization.get": { installation_id: "installation-1", realization_id: realizationId },
  "host.realization.plan": { installation_id: "installation-1", expected_installation_revision: 5, target_id: "local", backends: [{ kind: "oci_image", workload_id: "server", execution_class: "oci-container.v1", image: "example/app@sha256:" + "f".repeat(64), container_port: 8080, port_name: "http", route_id: "installation-1-http", route_access: "host_authenticated", pull_if_missing: false }], idempotency_key: "realization-plan-1" },
  "host.realization.apply": { installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 1, plan_ref: planRef, approval, idempotency_key: "realization-apply-1" },
  "host.realization.stop": { installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 2, idempotency_key: "realization-stop-1" },
  "host.realization.rollback": { installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 3, rollback_to_realization_id: rollbackRealizationId, approval, idempotency_key: "realization-rollback-1" },
  "host.realization.reconcile": { installation_id: "installation-1", target_id: "local", realization_id: realizationId, expected_revision: 4, idempotency_key: "realization-reconcile-1" },
};
for (const [method, expected] of Object.entries(expectedBodies)) {
  const actual = requests.find((request) => request.method === method)?.params;
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${method} did not send its exact generated body`);
}

try {
  await client.bindingCandidates({ consumer_installation_id: "installation-1", expected_consumer_installation_revision: 5, import_port: "error-port", phase: "launch" });
  throw new Error("expected structured RPC error");
} catch (error) {
  if (!(error instanceof ProtocolRpcError)) throw error;
  if (error.reasonCode !== "binding_unavailable" || error.nextStep !== "Retry candidate discovery.") throw new Error("structured reason and next step were not preserved");
  if (error.message.includes("private") || error.message.includes("secret") || JSON.stringify(error.details).includes("stderr")) throw new Error("raw RPC message or detail leaked through ProtocolRpcError");
}

const originalEventSource = globalThis.EventSource;
const eventSourceUrls: string[] = [];
let eventSourceClosed = 0;
class FakeEventSource {
  constructor(readonly url: string) { eventSourceUrls.push(url); }
  addEventListener(_type: string, _listener: EventListenerOrEventListenerObject) {}
  close() { eventSourceClosed += 1; }
}
globalThis.EventSource = FakeEventSource as unknown as typeof EventSource;
const eventsAbort = new AbortController();
const closeEvents = client.subscribeHostEvents(
  [HOST_EVENT_SESSIONS.powerbox, HOST_EVENT_SESSIONS.installationLifecycle],
  () => undefined,
  { signal: eventsAbort.signal },
);
const closeDefault = client.subscribeEvents(undefined, () => undefined);
if (!eventSourceUrls.some((url) => {
  const parsed = new URL(url);
  return parsed.pathname === `/journal/subscribe/${HOST_EVENT_SESSIONS.powerbox}`
    && parsed.searchParams.get("access_token") === "token";
})) {
  throw new Error("Powerbox events must use the public host_powerbox relay session");
}
if (!eventSourceUrls.some((url) => {
  const parsed = new URL(url);
  return parsed.pathname === `/journal/subscribe/${HOST_EVENT_SESSIONS.installationLifecycle}`
    && parsed.searchParams.get("access_token") === "token";
})) {
  throw new Error("Installation lifecycle events must use the public lifecycle relay session");
}
eventsAbort.abort();
if (eventSourceClosed !== 2) throw new Error("aborting public relay subscriptions must close both EventSources");
closeEvents();
closeDefault();
if (Number(eventSourceClosed) !== 3) throw new Error("subscription cleanup must be idempotent");
globalThis.EventSource = originalEventSource;

globalThis.fetch = originalFetch;
