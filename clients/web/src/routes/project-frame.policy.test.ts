import { allowedSurfaceCapabilityIdsForTest, summarizeConsoleDiagnostics, targetOperationsForProject } from "./project-frame";
import type { PackageRecord, SurfaceContributionRecord, TargetOperationRecord } from "@/protocol/client";

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

const contribution: SurfaceContributionRecord = {
  package_id: "pkg/surface",
  entry_kind: "subprocess",
  package_state: "ready",
  surface: {
    id: "pkg/surface/entry",
    version: "1",
    slot: "experience_entry",
    title: "Entry",
    capability_id: "pkg/surface/render",
    allowed_capability_ids: ["pkg/surface/exact_extra"],
    activation: {
      launch_capability_id: "pkg/surface/launch",
    },
    required_permissions: [],
    metadata: {
      requested_capabilities: ["attacker/metadata_grant"],
      nested: { capability_id: "attacker/nested_metadata_grant" },
    },
  },
};

assertDeepEqual([...allowedSurfaceCapabilityIdsForTest(contribution)].sort(), [
  "pkg/surface/exact_extra",
  "pkg/surface/launch",
  "pkg/surface/render",
]);

assertEqual(allowedSurfaceCapabilityIdsForTest(contribution).has("attacker/metadata_grant"), false);
assertEqual(allowedSurfaceCapabilityIdsForTest(contribution).has("attacker/nested_metadata_grant"), false);
assertEqual(allowedSurfaceCapabilityIdsForTest(contribution).has("pkg/surface/unrelated"), false);

assertDeepEqual([...allowedSurfaceCapabilityIdsForTest(null)], []);

function targetOperation(operationId: string, projectId: string, targetId: string, updatedAt: number): TargetOperationRecord {
  return {
    operation_id: operationId,
    target_id: targetId,
    project_id: projectId,
    revision: 1,
    status: "requested",
    spec: { kind: "health_probe" },
    authority: {
      target_id: targetId,
      operation_id: operationId,
      step_id: "execute",
      project_id: projectId,
      effect: "health_probe",
      artifact_digests: [],
      lease_epoch: 1,
      policy_epoch: 1,
      issued_at_ms: 1,
      expires_at_ms: 2,
      nonce: "nonce",
      request_digest: "sha256:request",
      authority_digest: "sha256:authority",
    },
    created_at_ms: 1,
    updated_at_ms: updatedAt,
  };
}

assertDeepEqual(
  targetOperationsForProject([
    targetOperation("remote-old", "project-a", "remote", 2),
    targetOperation("other-project", "project-b", "remote", 4),
    targetOperation("local", "project-a", "local", 5),
    targetOperation("remote-new", "project-a", "remote", 3),
  ], "project-a", "remote").map((operation) => operation.operation_id),
  ["remote-new", "remote-old"],
);

const packages = [
  {
    id: "pkg/ready",
    version: "1",
    state: "ready",
    entry_kind: "subprocess",
    capability_count: 2,
    hook_count: 0,
  },
  {
    id: "pkg/degraded",
    version: "1",
    state: "degraded",
    entry_kind: "subprocess",
    capability_count: 1,
    hook_count: 1,
    last_failure: {
      package_id: "pkg/degraded",
      reason: "boom",
      failed_at: "now",
      stderr_tail_redacted: [],
      log_tail_redacted: [],
      stderr_truncated: false,
      redaction_state: "redacted",
      state: "degraded",
    },
  },
] satisfies PackageRecord[];

assertDeepEqual(summarizeConsoleDiagnostics({
  projectId: "project-a",
  packages,
  events: [{
    id: "evt-1",
    session_id: "sess-1",
    sequence: 1,
    writer_package_id: "pkg/ready",
    kind: "host/package.ready",
    payload: {},
    metadata: {},
    created_at: "now",
  }],
  updates: {
    results: [
      { package_id: "pkg/ready", status: "current", available: false },
      { package_id: "pkg/degraded", status: "repair_required", available: true },
    ],
  },
  errors: [],
  targets: [
    { id: "local", name: "Local", reachability: "local_host", status: "available" },
  ],
  executions: [
    { exec_id: "exec-1", target_id: "local", kind: "running", ready: true },
    { exec_id: "exec-2", target_id: "local", kind: "exited", ready: false, exit_code: 0 },
  ],
  portLeases: [
    { id: "lease-1", target_id: "local", port_name: "web", host: "127.0.0.1", port: 3000, protocol: "tcp", status: "active" },
    { id: "lease-2", target_id: "local", port_name: "old", host: "127.0.0.1", port: 3001, protocol: "tcp", status: "released" },
  ],
  proxyRoutes: [
    { id: "route-1", protocol: "http", access: "host_authenticated", public_url: "http://127.0.0.1/p/r", iframe_url: "http://127.0.0.1/p/r", status: "active", ready: true, upstream: { port_lease_id: "lease-1", port_name: "web" } },
  ],
  refreshedAt: "now",
}), {
  packageTotal: 2,
  packageHealthy: 1,
  packageProblem: 1,
  recentEvents: 1,
  updateAvailable: 1,
  updateChecked: true,
  targetTotal: 1,
  execTotal: 2,
  execRunning: 1,
  portActive: 1,
  proxyActive: 1,
});

assertDeepEqual(summarizeConsoleDiagnostics(null), {
  packageTotal: 0,
  packageHealthy: 0,
  packageProblem: 0,
  recentEvents: 0,
  updateAvailable: 0,
  updateChecked: false,
  targetTotal: 0,
  execTotal: 0,
  execRunning: 0,
  portActive: 0,
  proxyActive: 0,
});

assertDeepEqual(summarizeConsoleDiagnostics({
  projectId: "project-a",
  packages: [],
  events: [],
  errors: [],
  targets: [
    { id: "local", name: "Local", reachability: "local_host", status: "available" },
    { id: "remote", name: "Remote", reachability: "remote", status: "available" },
  ],
  executions: [
    { exec_id: "local-exec", target_id: "local", kind: "running", ready: true },
    { exec_id: "remote-exec", target_id: "remote", kind: "running", ready: true },
  ],
  portLeases: [
    { id: "local-lease", target_id: "local", port_name: "web", host: "127.0.0.1", port: 3000, protocol: "tcp", status: "active" },
    { id: "remote-lease", target_id: "remote", port_name: "web", host: "127.0.0.1", port: 4000, protocol: "tcp", status: "active" },
  ],
  proxyRoutes: [
    { id: "local-route", protocol: "http", access: "host_authenticated", public_url: "https://local.test", iframe_url: "https://local.test", status: "active", ready: true, upstream: { port_lease_id: "local-lease", port_name: "web" } },
    { id: "remote-route", protocol: "http", access: "host_authenticated", public_url: "https://remote.test", iframe_url: "https://remote.test", status: "active", ready: true, upstream: { port_lease_id: "remote-lease", port_name: "web" } },
  ],
  refreshedAt: "now",
}, "remote"), {
  packageTotal: 0,
  packageHealthy: 0,
  packageProblem: 0,
  recentEvents: 0,
  updateAvailable: 0,
  updateChecked: false,
  targetTotal: 1,
  execTotal: 1,
  execRunning: 1,
  portActive: 1,
  proxyActive: 1,
});
