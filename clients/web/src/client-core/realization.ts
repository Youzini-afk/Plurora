import type { HostAccessIdentity, HostAccessResourceKind } from "./host-access";
import type {
  ArtifactDescriptor,
  RealizationApplyRequest,
  RealizationPlan,
  RealizationPlanRequest,
  RealizationRevision,
} from "../protocol/generated-types";

export interface OciRealizationDraft {
  installationId: string;
  installationRevision: number;
  targetId: string;
  workloadId: string;
  executionClass: string;
  image: string;
  containerPort: number;
  portName: string;
  routeId: string;
  routeAccess: "host_authenticated" | "public";
  healthPath?: string;
  pullIfMissing: boolean;
}

export interface RealizationDraftFailure {
  reasonCode: string;
  nextStep: string;
}

export interface RealizationAuthority {
  canObserve: boolean;
  canPlan: (targetId: string) => boolean;
  canApply: (targetId: string, realizationId: string) => boolean;
}

const SHA256_OCI_REFERENCE = /^.+@sha256:[0-9a-f]{64}$/;
const ROUTE_TOKEN = /^[A-Za-z0-9._-]+$/;

export function validateOciRealizationDraft(draft: OciRealizationDraft): RealizationDraftFailure | null {
  if (!draft.installationId || !draft.targetId || !draft.workloadId.trim() || !draft.executionClass.trim()) {
    return { reasonCode: "work_invalid", nextStep: "Choose an exact Target and enter the workload and execution class declared by OperationalIntent." };
  }
  if (!SHA256_OCI_REFERENCE.test(draft.image.trim())) {
    return { reasonCode: "artifact_digest_mismatch", nextStep: "Use an immutable OCI reference ending in @sha256:<64 lowercase hex characters>." };
  }
  if (!Number.isInteger(draft.containerPort) || draft.containerPort < 1 || draft.containerPort > 65_535) {
    return { reasonCode: "work_invalid", nextStep: "Container port must be an integer from 1 through 65535." };
  }
  if (!ROUTE_TOKEN.test(draft.portName) || !ROUTE_TOKEN.test(draft.routeId)) {
    return { reasonCode: "work_invalid", nextStep: "Port and route IDs may contain letters, numbers, dot, underscore, and hyphen." };
  }
  const healthPath = draft.healthPath?.trim();
  if (healthPath && (!healthPath.startsWith("/") || healthPath.startsWith("//") || /[\r\n]/.test(healthPath))) {
    return { reasonCode: "work_invalid", nextStep: "Health path must be a single absolute URL path." };
  }
  return null;
}

export function createOciRealizationPlanRequest(
  draft: OciRealizationDraft,
  idempotencyKey: string,
): RealizationPlanRequest {
  const failure = validateOciRealizationDraft(draft);
  if (failure) throw new Error(`${failure.reasonCode}: ${failure.nextStep}`);
  return {
    installation_id: draft.installationId,
    expected_installation_revision: draft.installationRevision,
    target_id: draft.targetId,
    idempotency_key: idempotencyKey,
    backends: [{
      kind: "oci_image",
      workload_id: draft.workloadId.trim(),
      execution_class: draft.executionClass.trim(),
      image: draft.image.trim(),
      container_port: draft.containerPort,
      port_name: draft.portName,
      route_id: draft.routeId,
      route_access: draft.routeAccess,
      ...(draft.healthPath?.trim() ? { health_path: draft.healthPath.trim() } : {}),
      pull_if_missing: draft.pullIfMissing,
    }],
  };
}

export function createRealizationApplyRequest({
  realization,
  targetId,
  plan,
  planRef,
  acceptedRisks,
  idempotencyKey,
  decidedAt = new Date(),
}: {
  realization: RealizationRevision;
  targetId: string;
  plan: RealizationPlan;
  planRef: ArtifactDescriptor;
  acceptedRisks: readonly string[];
  idempotencyKey: string;
  decidedAt?: Date;
}): RealizationApplyRequest {
  const accepted = new Set(acceptedRisks);
  const risks = plan.risk_summary ?? [];
  const missing = risks.filter((risk) => !accepted.has(risk));
  if (missing.length > 0) throw new Error(`approval_required: ${missing.join(", ")}`);
  return {
    installation_id: realization.installation_id,
    target_id: targetId,
    realization_id: realization.realization_id,
    expected_revision: realization.revision,
    plan_ref: planRef,
    approval: {
      plan_digest: planRef.digest,
      decision: "approved",
      accepted_risks: [...risks],
      decided_at: decidedAt.toISOString(),
    },
    idempotency_key: idempotencyKey,
  };
}

export function realizationAuthorityForInstallation(
  identity: HostAccessIdentity | null,
  installationId: string,
): RealizationAuthority {
  if (identity?.kind === "root") {
    return { canObserve: true, canPlan: () => true, canApply: () => true };
  }
  const hasScope = (scope: "observe" | "realization.plan" | "realization.apply") => Boolean(identity?.scopes.includes(scope));
  const has = (kind: HostAccessResourceKind, id: string) => Boolean(identity?.resources?.some(
    (resource) => resource.kind === kind && (resource.id === null || resource.id === id),
  ));
  const installationAllowed = has("installation", installationId);
  return {
    canObserve: hasScope("observe"),
    canPlan: (targetId) => hasScope("realization.plan") && installationAllowed && has("target", targetId),
    canApply: (targetId, realizationId) => hasScope("realization.apply")
      && installationAllowed
      && has("target", targetId)
      && has("realization", realizationId),
  };
}

export function realizationTargetId(realization: RealizationRevision): string | null {
  const targets = new Set((realization.actual_resources ?? []).map((resource) => resource.target_id));
  return targets.size === 1 ? [...targets][0] ?? null : null;
}

export function isRealizationEffectInProgress(status: RealizationRevision["status"]): boolean {
  return status === "applying" || status === "stopping";
}

export function isRealizationActive(status: RealizationRevision["status"]): boolean {
  return status === "active" || status === "degraded";
}
