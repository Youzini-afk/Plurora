import {
  BROWSER_ACCESS_TOKEN_STORAGE_KEY,
  clearBrowserAccessToken,
  ProtocolHttpError,
  resolveBrowserAccessToken,
  storeBrowserAccessToken,
  PluroraProtocolClient,
} from "./client";
import { resolveSurfaceBundle } from "../surfaces/bundle-resolver";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

function assertDeepEqual(actual: unknown, expected: unknown) {
  const actualJson = JSON.stringify(actual);
  const expectedJson = JSON.stringify(expected);
  if (actualJson !== expectedJson) {
    throw new Error(`expected ${expectedJson}, got ${actualJson}`);
  }
}

async function rejectsWithHttpStatus(promise: Promise<unknown>, status: number) {
  try {
    await promise;
  } catch (err: unknown) {
    if (err instanceof ProtocolHttpError) {
      assertEqual(err.status, status);
      assertEqual(err.isAuthError, status === 401);
      return;
    }
    throw err;
  }
  throw new Error(`expected rejection with HTTP status ${status}`);
}

class MemoryStorage {
  private values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

function installWindow(search: string) {
  const storage = new MemoryStorage();
  let replacedUrl = "";

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location: {
        origin: "http://web.test",
        pathname: "/app",
        search,
        hash: "#frag",
      },
      localStorage: storage,
      history: {
        state: { ok: true },
        replaceState: (_state: unknown, _title: string, url: string) => {
          replacedUrl = url;
        },
      },
    },
  });

  return {
    storage,
    replacedUrl: () => replacedUrl,
  };
}

const { storage, replacedUrl } = installWindow("?foo=bar&plurora_token=query-token&access_token=ignored");
assertEqual(resolveBrowserAccessToken(), "query-token");
assertEqual(storage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), null);
assertEqual(replacedUrl(), "/app?foo=bar#frag");

storeBrowserAccessToken("valid-token");
assertEqual(storage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), "valid-token");
clearBrowserAccessToken();
assertEqual(storage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), null);

const secondWindow = installWindow("");
secondWindow.storage.setItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY, "stored-token");
assertEqual(resolveBrowserAccessToken(), "stored-token");
assertEqual(secondWindow.replacedUrl(), "");

const thirdWindow = installWindow("?access_token=bad-token");
assertEqual(resolveBrowserAccessToken(), "bad-token");
clearBrowserAccessToken();
assertEqual(thirdWindow.storage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), null);

const originalFetch = globalThis.fetch;
const originalCrypto = globalThis.crypto;

Object.defineProperty(globalThis, "crypto", {
  configurable: true,
  value: { randomUUID: () => "request-id" },
});

globalThis.fetch = (async () =>
  new Response("missing token", {
    status: 401,
    statusText: "Unauthorized",
  })) as typeof fetch;

await rejectsWithHttpStatus(new PluroraProtocolClient("http://host.test", "bad-token").diagnostics(), 401);
await rejectsWithHttpStatus(new PluroraProtocolClient("http://host.test", "bad-token").listTargetOperations("remote"), 401);

const capturedRequests: unknown[] = [];
const capturedFetches: Array<{ input: string; method?: string; body: unknown; headers?: HeadersInit }> = [];
globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
  const inputString = String(input);
  const body = typeof init?.body === "string" ? JSON.parse(init.body) : undefined;
  capturedFetches.push({ input: inputString, method: init?.method, body, headers: init?.headers });
  if (inputString.includes("/host/v1/targets/") && inputString.includes("/operations")) {
    if (inputString.endsWith("/operations")) return Response.json([]);
    return Response.json({
      operation_id: "op-1",
      target_id: "remote/one",
      project_id: "project-1",
      revision: 1,
      status: "succeeded",
      spec: { kind: "health_probe" },
      authority: {},
      receipt: null,
      created_at_ms: 1,
      updated_at_ms: 2,
    });
  }
  if (inputString.endsWith("/host/v1/deploy")) {
    return Response.json({
      route_id: body.route_id,
      public_url: `http://host.test/p/${body.route_id}/`,
      port_lease_id: "lease-1",
      container_id: "container-1",
      container_name: "container-name-1",
    });
  }

  if (inputString.endsWith("/host/v1/deploy/stop")) {
    return Response.json({ route_id: body.route_id, stopped: true, warnings: [] });
  }

  if (inputString.includes("/host/v1/build-deploy")) {
    if (init?.method === "GET") {
      return Response.json({ job_id: "job-1", project_id: "project-1", route_id: "route-1", state: "ready", created_at_ms: 1, updated_at_ms: 2, result: null, error: null, events_url: "/host/v1/build-deploy/job-1/events" });
    }
    if (inputString.endsWith("/cancel")) {
      return Response.json({ job_id: "job-1", state: "cancelled", cancelled: true });
    }
    return Response.json({ job_id: "job-1", status_url: "/host/v1/build-deploy/job-1", events_url: "/host/v1/build-deploy/job-1/events", state: "queued" });
  }

  if (inputString.includes("/host/v1/projects/") && inputString.includes("/changes")) {
    if (init?.method === "GET" && inputString.endsWith("/changes")) {
      return Response.json({ changes: [] });
    }
    if (inputString.endsWith("/execute")) {
      return Response.json({ accepted: true, change: {} });
    }
    if (inputString.endsWith("/bundle")) {
      return Response.json({ schema_version: 1, project_id: "project-1", change_set_id: "chg-1", base_tree_digest: "sha256:test", operations: [] });
    }
    return Response.json({});
  }

  capturedRequests.push(body);

  if (body?.method === "host.info") {
    return Response.json({
      id: body.id,
      result: {
        protocol_version: "0.1.0",
        supported_transports: ["http_rpc"],
        methods: [],
        default_profile: "plurora.contract.default/v1",
      },
    });
  }

  if (body?.method === "context.open") {
    return Response.json({ id: body.id, result: { id: "install-session" } });
  }

  if (body?.method === "capability.invoke") {
    const capabilityId = body.params.capability_id;
    const output = capabilityId.endsWith("/check_for_updates")
      ? { results: [] }
      : capabilityId.endsWith("/update_project")
        ? { status: "current", updated: false, updated_packages: [], check: { results: [] } }
        : capabilityId.endsWith("/start_container")
          ? { kind: "docker_runtime_lab_container_started", container_id: "container-1", container_started: true, docker_performed: true }
          : capabilityId.endsWith("/stop_container")
            ? { kind: "docker_runtime_lab_container_stopped", container_id: "container-1", docker_performed: true }
            : {
                plan: {
                  root_id: "plurora/test-project",
                  packages: [],
                  permissions_summary: {
                    new_capabilities: [],
                    new_network_hosts: [],
                    new_secret_refs: [],
                  },
                  signature_summary: {
                    all_signed: false,
                    unsigned_packages: [],
                  },
                  integrity_summary: {
                    manifest_hashes_match_lockfile: true,
                    drift_detected: [],
                  },
                },
              };
    return Response.json({
      id: body.id,
      result: {
        capability_id: capabilityId,
        correlation_id: "corr-1",
        duration_ms: 1,
        provider_package_id: body.params.provider_package_id,
        output,
      },
    });
  }

  if (body?.method === "host.target.list") {
    return Response.json({ id: body.id, result: [] });
  }

  if (body?.method === "host.target.status") {
    return Response.json({ id: body.id, result: { id: body.params.target_id, name: "Local", reachability: "local_host", status: "available" } });
  }

  if (body?.method === "host.exec.list") {
    return Response.json({ id: body.id, result: { executions: [] } });
  }

  if (body?.method === "host.exec.status") {
    return Response.json({ id: body.id, result: { status: { exec_id: body.params.exec_id, target_id: "local", kind: "running", ready: true } } });
  }

  if (body?.method === "host.exec.logs") {
    return Response.json({ id: body.id, result: { exec_id: body.params.exec_id, lines: [] } });
  }

  if (body?.method === "host.port.list") {
    return Response.json({ id: body.id, result: [] });
  }

  if (body?.method === "host.port.status") {
    return Response.json({ id: body.id, result: { id: body.params.lease_id, target_id: "local", port_name: "web", host: "127.0.0.1", port: 3000, protocol: "tcp", status: "active" } });
  }

  if (body?.method === "host.port.lease") {
    return Response.json({ id: body.id, result: { lease: { id: "lease-1", target_id: body.params.target_id, port_name: body.params.port_name, host: "127.0.0.1", port: 39123, protocol: body.params.protocol ?? "tcp", status: "active" } } });
  }

  if (body?.method === "host.port.release") {
    return Response.json({ id: body.id, result: { id: body.params.lease_id, target_id: "local", port_name: "web", host: "127.0.0.1", port: 39123, protocol: "tcp", status: "released" } });
  }

  if (body?.method === "host.proxy.list") {
    return Response.json({ id: body.id, result: [] });
  }

  if (body?.method === "host.proxy.status") {
    return Response.json({ id: body.id, result: { id: body.params.route_id, protocol: "http", public_url: "http://127.0.0.1/p/r", iframe_url: "http://127.0.0.1/p/r", status: "active", ready: true, upstream: { port_lease_id: "lease-1", port_name: "web" } } });
  }

  if (body?.method === "host.proxy.register") {
    return Response.json({ id: body.id, result: { route: { id: body.params.route_id ?? "route-1", protocol: body.params.protocol ?? "http", public_url: "http://127.0.0.1/p/r", iframe_url: "http://127.0.0.1/p/r", status: "active", ready: false, upstream: body.params.upstream } } });
  }

  if (body?.method === "host.proxy.unregister") {
    return Response.json({ id: body.id, result: { id: body.params.route_id, protocol: "http", public_url: "http://127.0.0.1/p/r", iframe_url: "http://127.0.0.1/p/r", status: "removed", upstream: { port_lease_id: "lease-1", port_name: "web" } } });
  }

  if (body?.method === "host.project.start") {
    return Response.json({ id: body.id, result: { project_id: body.params.project_id, previous_state: "installed", new_state: "running", session_id: "session-1", already_running: false } });
  }

  if (body?.method === "host.project.list") {
    return Response.json({ id: body.id, result: { projects: [] } });
  }

  if (body?.method === "projection.list" || body?.method === "change.proposal.list" || body?.method === "shell.contribution.list") {
    return Response.json({ id: body.id, result: [] });
  }

  if (body?.method === "shell.contribution.describe") {
    return Response.json({ id: body.id, result: { package_id: "plurora/test", entry_kind: "in_process", package_state: "loaded", surface: { id: body.params.surface_id, slot: "experience_entry", kind: "module", source: "bundle.mjs" } } });
  }

  if (body?.method === "host.surface.bundle.resolve") {
    return Response.json({ id: body.id, result: { surface_id: body.params.surface_id, bundle_url: "/surface-bundles/test/bundle.mjs", export_name: "default", stylesheets: [], source: "dev_path" } });
  }

  throw new Error(`unexpected method ${body?.method}`);
}) as typeof fetch;

await new PluroraProtocolClient("http://host.test", "valid-token").resolveInstallPlan({
  root_url: "https://github.com/Youzini-afk/Plurora-Tavern",
});

const sessionOpenRequest = capturedRequests[0] as { method?: string; params?: Record<string, unknown> };
assertEqual(sessionOpenRequest.method, "context.open");
assertDeepEqual(sessionOpenRequest.params?.active_package_set, ["plurora/install-lab"]);
assertDeepEqual(sessionOpenRequest.params?.labels, ["install", "plurora/install-lab"]);

capturedRequests.length = 0;
await new PluroraProtocolClient("http://host.test", "valid-token").uninstallProject("youzini-afk__YdlTavern__d2a47e5c");
const uninstallInvoke = capturedRequests.find(
  (request) => (request as { method?: string }).method === "capability.invoke",
) as { params?: Record<string, unknown> };
assertEqual(uninstallInvoke.params?.capability_id, "plurora/install-lab/uninstall");
assertDeepEqual(uninstallInvoke.params?.input, {
  project_id: "youzini-afk__YdlTavern__d2a47e5c",
  profile: "default",
  delete_project_data: false,
});

capturedRequests.length = 0;
await new PluroraProtocolClient("http://host.test", "valid-token").checkProjectUpdates("youzini-afk__YdlTavern__d2a47e5c");
const updateCheckInvoke = capturedRequests.find(
  (request) => (request as { method?: string }).method === "capability.invoke",
) as { params?: Record<string, unknown> };
assertEqual(updateCheckInvoke.params?.capability_id, "plurora/install-lab/check_for_updates");
assertDeepEqual(updateCheckInvoke.params?.input, {
  project_id: "youzini-afk__YdlTavern__d2a47e5c",
  profile: "default",
});

capturedRequests.length = 0;
await new PluroraProtocolClient("http://host.test", "valid-token").updateProject("youzini-afk__YdlTavern__d2a47e5c");
const updateInvoke = capturedRequests.find(
  (request) => (request as { method?: string }).method === "capability.invoke",
) as { params?: Record<string, unknown> };
assertEqual(updateInvoke.params?.capability_id, "plurora/install-lab/update_project");
assertDeepEqual(updateInvoke.params?.input, {
  project_id: "youzini-afk__YdlTavern__d2a47e5c",
  profile: "default",
  force: false,
});

capturedRequests.length = 0;
const protocolClient = new PluroraProtocolClient("http://host.test", "valid-token");
await protocolClient.listTargets();
await protocolClient.targetStatus("local");
await protocolClient.listExecs();
await protocolClient.execStatus("exec-1");
await protocolClient.execLogs("exec-1", 80);
await protocolClient.listPortLeases();
await protocolClient.portStatus("lease-1");
await protocolClient.listProxyRoutes();
await protocolClient.proxyStatus("route-1");
await protocolClient.leasePort({ target_id: "local", port_name: "web", protocol: "tcp", requested_port: 39123 });
await protocolClient.releasePort("lease-1");
await protocolClient.registerProxy({ route_id: "route-1", protocol: "http", upstream: { port_lease_id: "lease-1", port_name: "web" } });
await protocolClient.unregisterProxy("route-1");
assertDeepEqual(capturedRequests.map((request) => (request as { method?: string }).method), [
  "host.target.list",
  "host.target.status",
  "host.exec.list",
  "host.exec.status",
  "host.exec.logs",
  "host.port.list",
  "host.port.status",
  "host.proxy.list",
  "host.proxy.status",
  "host.port.lease",
  "host.port.release",
  "host.proxy.register",
  "host.proxy.unregister",
]);
assertDeepEqual((capturedRequests[4] as { params?: Record<string, unknown> }).params, { exec_id: "exec-1", limit: 80 });

capturedRequests.length = 0;
await protocolClient.call("host.target.list");
await protocolClient.listTargets();
assertDeepEqual(capturedRequests.map((request) => (request as { method?: string }).method), [
  "host.target.list",
  "host.target.list",
]);

capturedRequests.length = 0;
await protocolClient.startDockerContainer({
  image: "example/app:latest",
  container_port: 8080,
  host_port: 39123,
  port_lease_id: "lease-1",
  route_id: "route-1",
  approved: true,
});
await protocolClient.stopDockerContainer({
  approved: true,
  container_id: "container-1",
  route_id: "route-1",
  port_lease_id: "lease-1",
  timeout_secs: 5,
});
const dockerSessionOpen = capturedRequests.filter((request) => (request as { method?: string }).method === "context.open") as Array<{ params?: Record<string, unknown> }>;
const dockerInvokes = capturedRequests.filter((request) => (request as { method?: string }).method === "capability.invoke") as Array<{ params?: Record<string, unknown> }>;
assertDeepEqual(dockerSessionOpen[0].params?.active_package_set, ["plurora/docker-runtime-lab"]);
assertDeepEqual(dockerSessionOpen[0].params?.labels, ["deploy", "plurora/docker-runtime-lab"]);
assertEqual(dockerInvokes[0].params?.provider_package_id, "plurora/docker-runtime-lab");
assertEqual(dockerInvokes[0].params?.capability_id, "plurora/docker-runtime-lab/start_container");
assertEqual(dockerInvokes[1].params?.capability_id, "plurora/docker-runtime-lab/stop_container");

capturedFetches.length = 0;
await protocolClient.deployProject({
  project_id: "project-test",
  image: "example/app:latest",
  container_port: 8080,
  port_name: "web",
  route_id: "route-1",
  route_access: "host_authenticated",
  health_path: "/healthz",
  pull_if_missing: false,
});
await protocolClient.stopProjectDeployment({ route_id: "route-1" });
await protocolClient.buildDeployProject({ project_id: "project-1", source_url: "https://example.com/repo.git", ref_name: "main", strategy: "dockerfile", container_port: 3000, port_name: "web", route_id: "route-1", route_access: "public", approved: true });
await protocolClient.getBuildDeployJob("job-1");
await protocolClient.cancelBuildDeployJob("job-1");
assertEqual(capturedFetches[0].input, "http://host.test/host/v1/deploy");
assertEqual(capturedFetches[1].input, "http://host.test/host/v1/deploy/stop");
assertEqual(capturedFetches[2].input, "http://host.test/host/v1/build-deploy");
assertEqual(capturedFetches[3].input, "http://host.test/host/v1/build-deploy/job-1");
assertEqual(capturedFetches[4].input, "http://host.test/host/v1/build-deploy/job-1/cancel");
assertDeepEqual(capturedFetches.map((request) => request.body), [
  { project_id: "project-test", image: "example/app:latest", container_port: 8080, port_name: "web", route_id: "route-1", route_access: "host_authenticated", health_path: "/healthz", pull_if_missing: false },
  { route_id: "route-1" },
  { project_id: "project-1", source_url: "https://example.com/repo.git", ref_name: "main", strategy: "dockerfile", container_port: 3000, port_name: "web", route_id: "route-1", route_access: "public", approved: true },
  undefined,
  {},
]);

capturedFetches.length = 0;
await protocolClient.listTargetOperations("remote/one");
await protocolClient.getTargetOperation("remote/one", "op/1");
assertDeepEqual(capturedFetches.map((request) => request.input), [
  "http://host.test/host/v1/targets/remote%2Fone/operations",
  "http://host.test/host/v1/targets/remote%2Fone/operations/op%2F1",
]);
assertDeepEqual(capturedFetches.map((request) => request.method), ["GET", "GET"]);
assertDeepEqual(capturedFetches.map((request) => request.body), [undefined, undefined]);
assertDeepEqual(
  capturedFetches.map((request) => new Headers(request.headers).get("authorization")),
  ["Bearer valid-token", "Bearer valid-token"],
);

capturedFetches.length = 0;
await protocolClient.listProjectChanges("project-1");
await protocolClient.getProjectChange("project-1", "chg-1");
await protocolClient.draftProjectChange("project-1", {
  goal: "update source",
  operations: [{ op: "file_write", path: "src/app.ts", content: "export {};\n" }],
  verification: { kind: "static_validation" },
});
await protocolClient.approveProjectChange("project-1", "chg-1", true, "reviewed");
await protocolClient.executeProjectChange("project-1", "chg-1");
await protocolClient.recoverProjectChange("project-1", "chg-1");
await protocolClient.getProjectChangeBundle("project-1", "chg-1");
await protocolClient.createProjectDeploymentPreview("project-1", "chg-1", {
  target_id: "remote/one",
  container_port: 8080,
  port_name: "web",
  route_id: "route-1",
  route_access: "public",
  health_path: "/healthz",
  idempotency_key: "preview-1",
});
await protocolClient.approveProjectDeployment("project-1", "chg-1", {
  approved: true,
  reason: "preview reviewed",
});
await protocolClient.activateProjectDeployment("project-1", "chg-1");
await protocolClient.reconcileProjectDeployment("project-1", "chg-1");
assertDeepEqual(capturedFetches.map((request) => request.input), [
  "http://host.test/host/v1/projects/project-1/changes",
  "http://host.test/host/v1/projects/project-1/changes/chg-1",
  "http://host.test/host/v1/projects/project-1/changes",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/approve",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/execute",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/recover",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/bundle",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/deployment/preview",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/deployment/approve",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/deployment/activate",
  "http://host.test/host/v1/projects/project-1/changes/chg-1/deployment/reconcile",
]);
assertDeepEqual(capturedFetches.map((request) => request.body), [
  undefined,
  undefined,
  {
    goal: "update source",
    operations: [{ op: "file_write", path: "src/app.ts", content: "export {};\n" }],
    verification: { kind: "static_validation" },
  },
  { approved: true, reason: "reviewed" },
  {},
  {},
  undefined,
  {
    target_id: "remote/one",
    container_port: 8080,
    port_name: "web",
    route_id: "route-1",
    route_access: "public",
    health_path: "/healthz",
    idempotency_key: "preview-1",
  },
  { approved: true, reason: "preview reviewed" },
  {},
  {},
]);
assertDeepEqual(
  capturedFetches.map((request) => new Headers(request.headers).get("authorization")),
  Array(11).fill("Bearer valid-token"),
);

capturedRequests.length = 0;
await protocolClient.startProject("project-1");
assertDeepEqual(capturedRequests.map((request) => (request as { method?: string }).method), ["host.project.start"]);

capturedRequests.length = 0;
await protocolClient.listProjects();
await protocolClient.projections();
await protocolClient.proposals();
await protocolClient.surfaceContributions();
await protocolClient.describeSurface("plurora/test.entry");
const remoteBundle = await resolveSurfaceBundle(protocolClient, "plurora/test.entry");
assertEqual(remoteBundle.bundleUrl, "http://host.test/surface-bundles/test/bundle.mjs");
assertDeepEqual(capturedRequests.map((request) => (request as { method?: string }).method), [
  "host.project.list",
  "projection.list",
  "change.proposal.list",
  "shell.contribution.list",
  "shell.contribution.describe",
  "host.surface.bundle.resolve",
]);

capturedRequests.length = 0;
const negotiatedClient = new PluroraProtocolClient("http://host.test", "valid-token");
const contract = {
  profile: "plurora.contract.default/v1",
  versions: [{ layer: "host" as const, version: "0.1.0" }],
};
await negotiatedClient.negotiateHost(contract);
await negotiatedClient.listTargets();
assertDeepEqual(capturedRequests, [
  { id: "request-id", method: "host.info", params: {}, contract },
  { id: "request-id", method: "host.target.list", params: {}, contract },
]);

globalThis.fetch = originalFetch;
Object.defineProperty(globalThis, "crypto", {
  configurable: true,
  value: originalCrypto,
});
