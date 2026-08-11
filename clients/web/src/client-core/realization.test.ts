import {
  createOciRealizationPlanRequest,
  createRealizationApplyRequest,
  realizationAuthorityForInstallation,
  realizationTargetId,
  validateOciRealizationDraft,
  type OciRealizationDraft,
} from "./realization";

const descriptor = (digest: string) => ({
  artifact_type_uri: "urn:plurora:test:v1",
  media_type: "application/json",
  digest,
  size_bytes: 1,
});

const draft: OciRealizationDraft = {
  installationId: "installation-1",
  installationRevision: 7,
  targetId: "local",
  workloadId: "server",
  executionClass: "oci-container.v1",
  image: `registry.example/app@sha256:${"a".repeat(64)}`,
  containerPort: 8080,
  portName: "http",
  routeId: "installation-1-http",
  routeAccess: "host_authenticated",
  healthPath: "/healthz",
  pullIfMissing: false,
};

if (validateOciRealizationDraft(draft) !== null) throw new Error("valid immutable OCI draft rejected");
if (validateOciRealizationDraft({ ...draft, image: "registry.example/app:latest" })?.reasonCode !== "artifact_digest_mismatch") {
  throw new Error("mutable OCI tag must be rejected before planning");
}
const planRequest = createOciRealizationPlanRequest(draft, "plan-key");
if (planRequest.backends[0]?.kind !== "oci_image" || planRequest.target_id !== "local" || planRequest.backends[0].image !== draft.image) {
  throw new Error("plan request did not preserve exact explicit inputs");
}

const planRef = descriptor(`sha256:${"b".repeat(64)}`);
const plan = {
  schema: "plurora.realization-plan.v1" as const,
  installation_id: "installation-1",
  work_revision: descriptor(`sha256:${"c".repeat(64)}`),
  assembly_lock: descriptor(`sha256:${"d".repeat(64)}`),
  operational_intent: descriptor(`sha256:${"e".repeat(64)}`),
  inventory_refs: [descriptor(`sha256:${"f".repeat(64)}`)],
  required_authority: ["realization.apply"],
  risk_summary: ["public_endpoint", "remote_effect"],
};
const realization = {
  realization_id: "11111111-1111-4111-8111-111111111111",
  installation_id: "installation-1",
  plan_ref: planRef,
  status: "planned" as const,
  health: { status: "unknown" as const },
  revision: 1,
  actual_resources: [],
  receipts: [],
  created_at: "2026-08-12T00:00:00Z",
  updated_at: "2026-08-12T00:00:00Z",
};
let approvalRejected = false;
try {
  createRealizationApplyRequest({ realization, targetId: "local", plan, planRef, acceptedRisks: ["public_endpoint"], idempotencyKey: "apply-key" });
} catch (error) {
  approvalRejected = String(error).includes("approval_required");
}
if (!approvalRejected) throw new Error("partial risk approval must fail closed");
const apply = createRealizationApplyRequest({
  realization,
  targetId: "local",
  plan,
  planRef,
  acceptedRisks: plan.risk_summary,
  idempotencyKey: "apply-key",
  decidedAt: new Date("2026-08-12T00:00:00Z"),
});
if (apply.approval.plan_digest !== planRef.digest || apply.approval.accepted_risks?.length !== 2) {
  throw new Error("approval must bind the exact plan digest and every declared risk");
}

const device = realizationAuthorityForInstallation({
  kind: "device",
  device_name: "test",
  scopes: ["observe", "realization.plan", "realization.apply"],
  resources: [
    { kind: "installation", id: "installation-1" },
    { kind: "target", id: "local" },
    { kind: "realization", id: realization.realization_id },
  ],
}, "installation-1");
if (!device.canObserve || !device.canPlan("local") || !device.canApply("local", realization.realization_id) || device.canApply("other", realization.realization_id)) {
  throw new Error("Realization affordance authority must remain exact");
}
if (realizationTargetId({ ...realization, actual_resources: [{ backend_id: "docker", resource_id: "c1", resource_type: "container", target_id: "local" }] }) !== "local") {
  throw new Error("active realization target should derive from exact realized resources");
}
