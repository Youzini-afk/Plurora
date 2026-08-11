import type { BindingCandidate, BindingGap, BindingMutationResult, PortDescriptor } from "@/protocol/generated-types";
import {
  BrowserPowerboxPreferenceStore,
  classifyPowerboxEmpty,
  createBindingCandidatesRequest,
  createBindingSelectRequest,
  createPowerboxContext,
  powerboxContextKey,
  powerboxInvalidationForLifecycleEvent,
  powerboxAuthorityForContext,
  powerboxReducer,
  powerboxWorkbenchShouldReloadForLifecycleEvent,
  type PowerboxState,
} from "./powerbox";

function assertEqual<T>(actual: T, expected: T, message = "values differ") {
  if (actual !== expected) throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
}

function assertDeepEqual(actual: unknown, expected: unknown, message = "values differ") {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

const artifact = (kind: string, fill: string) => ({
  artifact_type_uri: kind,
  media_type: "application/json",
  digest: `sha256:${fill.repeat(64)}`,
  size_bytes: 10,
});

function port(portId: string, kind: "import" | "export"): PortDescriptor {
  return {
    port_id: portId,
    contract: {
      protocol_id: "plurora.game-save",
      interface_id: "game-save",
      version: "1.2.0",
      profiles: ["default"],
    },
    interaction: "plurora.interaction.capability-unary/v1",
    role: kind === "import"
      ? {
        kind: "import",
        multiplicity: { min: 1, max: 1 },
        latest_binding_phase: "launch",
        availability: "required",
        accepted_effects: ["deterministic_stateful"],
      }
      : {
        kind: "export",
        multiplicity: { min: 0, max: null },
        effect_class: "deterministic_stateful",
      },
    transport: {
      allowed_classes: ["plurora.transport.capability/v1"],
      local_only: true,
      ordered: true,
      reliable: true,
      same_process: false,
      shared_memory_allowed: false,
      large_payload: false,
    },
  };
}

function candidate(providerInstallationId: string, digestFill: string, phase: "launch" | "runtime" = "launch"): BindingCandidate {
  const run = phase === "runtime" ? { run_id: "run-1", run_revision: 9, context_id: "context-1" } : undefined;
  return {
    availability: "required",
    candidate_digest: `sha256:${digestFill.repeat(64)}`,
    capability: { capability_id: "game/save", capability_version: "1.2.0" },
    consumer: {
      component: {
        behavior_digest: "sha256:" + "c".repeat(64),
        component_artifact: artifact("urn:plurora:component:v1", "c"),
        component_id: "consumer/component",
        node_path: ["consumer"],
        package_id: "consumer/package",
        trust_class: "sandboxed_component",
      },
      installation: {
        assembly_lock: artifact("urn:plurora:assembly-lock:v1", "a"),
        installation_id: "consumer-installation",
        installation_revision: 5,
        work_revision: artifact("urn:plurora:work-revision:v1", "b"),
      },
      port: { canonical_contract_digest: "sha256:" + "d".repeat(64), leaf_port: { node_id: "consumer", port_id: "save-in" }, node_path: ["consumer"], root_port: "save-in" },
      ...(run ? { run } : {}),
    },
    effective_expires_at: "2026-08-11T12:00:00Z",
    exposure: {
      record: {
        audience: [{ kind: "installation", id: "consumer-installation" }],
        expires_at: "2026-08-11T12:00:00Z",
        export_port: "save-out",
        exposure_id: `exposure-${providerInstallationId}`,
        installation_id: providerInstallationId,
        run_id: `run-${providerInstallationId}`,
        status: "active",
      },
      revision: 3,
    },
    phase,
    provider: {
      component: {
        behavior_digest: "sha256:" + digestFill.repeat(64),
        component_artifact: artifact("urn:plurora:component:v1", digestFill),
        component_id: `${providerInstallationId}/component`,
        node_path: ["provider"],
        package_id: providerInstallationId.startsWith("plurora") ? "plurora/provider" : "thirdparty/provider",
        trust_class: "isolated_process",
      },
      installation: {
        assembly_lock: artifact("urn:plurora:assembly-lock:v1", digestFill),
        installation_id: providerInstallationId,
        installation_revision: 4,
        work_revision: artifact("urn:plurora:work-revision:v1", digestFill),
      },
      port: { canonical_contract_digest: "sha256:" + digestFill.repeat(64), leaf_port: { node_id: "provider", port_id: "save-out" }, node_path: ["provider"], root_port: "save-out" },
      ...(run ? { run: { run_id: `run-${providerInstallationId}`, run_revision: 2, context_id: `context-${providerInstallationId}` } } : {}),
    },
    consumer_port: port("save-in", "import"),
    provider_port: port("save-out", "export"),
    provider_component: {
      behavior: artifact("urn:plurora:component-behavior:v1", digestFill),
      claim_status: "declared",
      component_artifact: artifact("urn:plurora:component:v1", digestFill),
      component_id: `${providerInstallationId}/component`,
      enforced_boundaries: {
        filesystem_isolation: true,
        network_isolation: true,
        no_code_execution: false,
        process_failure_isolation: true,
        remote_identity: false,
        resource_limits_enforced: true,
        revocation_enforced: true,
        tenancy_isolation: true,
      },
      entry_kind: "subprocess",
      package_id: providerInstallationId.startsWith("plurora") ? "plurora/provider" : "thirdparty/provider",
      trust_class: "isolated_process",
      version: "1.2.0",
    },
    provider_installation: {
      display_name: `${providerInstallationId} display`,
      installation_id: providerInstallationId,
      installation_revision: 4,
      source: { kind: "git_snapshot" },
    },
    provider_work: { title: "Save Provider", work_id: "tests/save-provider" },
    transport: { class_id: "plurora.transport.capability/v1" },
  };
}

const emptyResult = { candidates: [], gaps: [] };
let state: PowerboxState = { kind: "idle" };
state = powerboxReducer(state, { type: "load" });
assertEqual(state.kind, "loading", "idle must enter loading");
state = powerboxReducer(state, { type: "loaded", result: emptyResult });
assertEqual(state.kind, "empty", "zero candidates must enter empty");
if (state.kind === "empty") assertEqual(state.emptyKind, "absent");

const one = candidate("provider-one", "e");
state = powerboxReducer({ kind: "loading" }, { type: "loaded", result: { candidates: [one] }, nowMs: Date.parse("2026-08-11T11:00:00Z") });
assertEqual(state.kind, "ready");
if (state.kind === "ready") {
  assertEqual(state.explicitChoiceDigest, undefined, "one candidate must still require an explicit choice");
  state = powerboxReducer(state, { type: "choose", candidateDigest: one.candidate_digest });
}
if (state.kind !== "ready" || state.explicitChoiceDigest !== one.candidate_digest) throw new Error("explicit choice was not retained");
state = powerboxReducer(state, {
  type: "selected",
  result: { binding: { record: { binding_id: "binding-1" } }, idempotent: false } as unknown as BindingMutationResult,
});
assertEqual(state.kind, "selected", "successful explicit selection must enter selected");

const firstParty = candidate("plurora-provider", "f");
const thirdParty = candidate("thirdparty-provider", "1");
state = powerboxReducer({ kind: "loading" }, {
  type: "loaded",
  result: { candidates: [thirdParty, firstParty] },
  preferenceProviderInstallationId: firstParty.provider.installation.installation_id,
  nowMs: Date.parse("2026-08-11T11:00:00Z"),
});
if (state.kind !== "ready") throw new Error("multiple candidates should be ready for explicit choice");
assertDeepEqual(state.candidates.map((item) => item.provider.installation.installation_id), ["thirdparty-provider", "plurora-provider"], "first-party providers must not be sorted ahead");
assertEqual(state.preferenceHintDigest, firstParty.candidate_digest);
assertEqual(state.explicitChoiceDigest, undefined, "preference hints must not become authority");

const expired = { ...one, effective_expires_at: "2026-08-11T10:00:00Z" };
state = powerboxReducer({ kind: "loading" }, { type: "loaded", result: { candidates: [expired] }, nowMs: Date.parse("2026-08-11T11:00:00Z") });
assertEqual(state.kind, "stale", "expired candidates must be recomputed");
for (const reasonCode of ["binding_expired", "binding_revoked", "provider_stopped"]) {
  state = powerboxReducer({ kind: "ready", candidates: [one], gaps: [] }, { type: "invalidate", reasonCode, nextStep: "Recompute" });
  assertEqual(state.kind, "invalidated", `${reasonCode} must invalidate instead of rebinding`);
}
assertEqual(powerboxInvalidationForLifecycleEvent(
  { kind: "host/exposure.revoked", payload: { exposure: { record: { exposure_id: one.exposure.record.exposure_id } } } },
  new Set([one.exposure.record.exposure_id]),
  new Set(),
)?.reasonCode, "binding_unavailable", "Exposure revocation must invalidate the visible candidate");
assertEqual(powerboxInvalidationForLifecycleEvent(
  { kind: "host/run.stopped", payload: { run: { record: { run_id: "run-provider" } } } },
  new Set(),
  new Set(["run-provider"]),
)?.reasonCode, "binding_unavailable", "provider stop must invalidate the visible candidate");
assertEqual(powerboxInvalidationForLifecycleEvent(
  { kind: "host/run.stopped", payload: { run: { record: { run_id: "other-run" } } } },
  new Set(),
  new Set(["run-provider"]),
), undefined, "unrelated Host lifecycle events must not cross Powerbox context boundaries");
assertDeepEqual(powerboxInvalidationForLifecycleEvent(
  { kind: "host/binding.expired", payload: { binding: { record: { exposure_id: one.exposure.record.exposure_id } } } },
  new Set([one.exposure.record.exposure_id]),
  new Set(),
), { reasonCode: "binding_expired", nextStep: "Recompute candidates; the selected Binding or Exposure is no longer current." }, "Binding expiry must invalidate the visible candidate");
assertEqual(powerboxInvalidationForLifecycleEvent(
  { kind: "host-private/powerbox.binding-closing", payload: { binding: { record: { exposure_id: one.exposure.record.exposure_id } } } },
  new Set([one.exposure.record.exposure_id]),
  new Set(),
), undefined, "private Powerbox events must never invalidate the chooser");
if (!powerboxWorkbenchShouldReloadForLifecycleEvent(
  { kind: "host/binding.selected", payload: { binding: { record: { consumer: { installation: { installation_id: "consumer" } } } } } },
  "consumer",
)) throw new Error("Binding selection must refresh the matching workbench");
if (!powerboxWorkbenchShouldReloadForLifecycleEvent(
  { kind: "host/run.failed", payload: { run: { record: { installation_id: "provider" } } } },
  "provider",
)) throw new Error("Provider Run failure must refresh the matching workbench");
if (powerboxWorkbenchShouldReloadForLifecycleEvent(
  { kind: "host-private/powerbox.binding-closing", payload: { binding: { record: { consumer: { installation: { installation_id: "consumer" } } } } } },
  "consumer",
)) throw new Error("Private Powerbox events must not refresh the workbench");
state = powerboxReducer(state, { type: "stale", reasonCode: "outcome_unknown", nextStep: "Retry status" });
assertEqual(state.kind, "stale", "outcome_unknown must require an explicit retry");

const gap = (reason_code: string): BindingGap => ({ reason_code, next_step: "next" });
assertEqual(classifyPowerboxEmpty([gap("authority_denied")]), "forbidden");
assertEqual(classifyPowerboxEmpty([gap("unsupported_interaction")]), "unsupported");
assertEqual(classifyPowerboxEmpty([gap("binding_unavailable")]), "unavailable");
assertEqual(classifyPowerboxEmpty([gap("plan_stale")]), "stale");

const launch = createPowerboxContext({ hostScope: "host-a", installationId: "consumer-installation", installationRevision: 5, importPort: "save-in", phase: "launch" });
assertDeepEqual(createBindingCandidatesRequest(launch), {
  consumer_installation_id: "consumer-installation",
  expected_consumer_installation_revision: 5,
  import_port: "save-in",
  phase: "launch",
}, "Launch candidates must not carry a Run");
assertDeepEqual(createBindingCandidatesRequest(launch, "provider-one").preferences, [{ kind: "installation", id: "provider-one" }], "saved preference must be sent only as a typed ordering hint");
const launchSelect = createBindingSelectRequest(launch, one, "select-launch");
if ("consumer_run" in launchSelect) throw new Error("Launch selection must not carry consumer_run");

const runtimeCandidate = candidate("provider-runtime", "2", "runtime");
const runtime = createPowerboxContext({
  hostScope: "host-a",
  installationId: "consumer-installation",
  installationRevision: 5,
  importPort: "save-in",
  phase: "runtime",
  run: { run_id: "run-1", run_revision: 9, context_id: "context-1" },
});
assertDeepEqual(createBindingCandidatesRequest(runtime), {
  consumer_installation_id: "consumer-installation",
  expected_consumer_installation_revision: 5,
  import_port: "save-in",
  phase: "runtime",
  consumer_run: { run_id: "run-1", run_revision: 9, context_id: "context-1" },
}, "Runtime candidates must carry the exact Run pin");
assertDeepEqual(createBindingSelectRequest(runtime, runtimeCandidate, "select-runtime").consumer_run, {
  run_id: "run-1", run_revision: 9, context_id: "context-1",
});

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}
const storage = new MemoryStorage();
const preferences = new BrowserPowerboxPreferenceStore(storage);
preferences.set(launch, "provider-one");
assertEqual(preferences.get(launch), "provider-one");
const otherHost = { ...launch, hostScope: "host-b" };
assertEqual(preferences.get(otherHost), undefined, "Powerbox preferences must be isolated by active Host credential scope");
if (powerboxContextKey(launch) === powerboxContextKey(otherHost)) throw new Error("Host-scoped cache keys collided");

const exactAuthority = powerboxAuthorityForContext({
  kind: "device",
  device_name: "phone",
  scopes: ["observe", "binding.manage"],
  resources: [
    { kind: "installation", id: "consumer-installation" },
    { kind: "port", id: "consumer-installation/save-in" },
    { kind: "exposure", id: one.exposure.record.exposure_id },
  ],
}, launch);
assertEqual(exactAuthority.canObserve, true);
assertEqual(exactAuthority.canSelect(one.exposure.record.exposure_id), true);
assertEqual(exactAuthority.canRevoke(one.exposure.record.exposure_id, "binding-1"), true);
const noPortAuthority = powerboxAuthorityForContext({
  kind: "device",
  device_name: "phone",
  scopes: ["observe", "binding.manage", "realization.apply"],
  resources: [{ kind: "installation", id: "consumer-installation" }, { kind: "exposure", id: one.exposure.record.exposure_id }],
}, launch);
assertEqual(noPortAuthority.canObserve, false, "missing Port selector must fail closed");
assertEqual(noPortAuthority.canSelect(one.exposure.record.exposure_id), false, "unrelated authority must not substitute for binding.manage resources");
const rawPortAuthority = powerboxAuthorityForContext({
  kind: "device",
  device_name: "phone",
  scopes: ["observe", "binding.manage"],
  resources: [{ kind: "installation", id: "consumer-installation" }, { kind: "port", id: "save-in" }, { kind: "exposure", id: one.exposure.record.exposure_id }],
}, launch);
assertEqual(rawPortAuthority.canObserve, false, "a bare Port ID must not match the Host's Installation-scoped Port selector");

// Binding records expose opaque binding identity, never the injected authority handle.
if ("authority_handle_id" in one) throw new Error("candidate leaked an authority handle");
