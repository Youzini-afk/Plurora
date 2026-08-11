import { activeHostCredentialScope } from "./host-endpoint";
import type { HostAccessIdentity, HostAccessResourceKind } from "./host-access";
import type {
  BindingCandidate,
  BindingCandidatesRequest,
  BindingCandidatesResult,
  BindingGap,
  BindingMutationResult,
  BindingPhase,
  BindingSelectRequest,
  BindingView,
  ExposureId,
  InstallationId,
  PortId,
  ResourceSelector,
  RunRevisionPin,
} from "@/protocol/generated-types";

export type PowerboxPhase = Extract<BindingPhase, "installation" | "launch" | "runtime">;

export interface PowerboxConsumerContext {
  hostScope: string;
  installationId: InstallationId;
  installationRevision: number;
  importPort: PortId;
  phase: PowerboxPhase;
  run?: RunRevisionPin;
}

export interface PowerboxAuthority {
  canObserve: boolean;
  canSelect: (exposureId: ExposureId) => boolean;
  canRevoke: (exposureId: ExposureId, bindingId: string) => boolean;
  reasonCode?: "authority_denied";
  nextStep?: string;
}

export type PowerboxEmptyKind = "absent" | "forbidden" | "unsupported" | "unavailable" | "stale";

export type PowerboxState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "empty"; emptyKind: PowerboxEmptyKind; gaps: BindingGap[] }
  | {
      kind: "ready";
      candidates: BindingCandidate[];
      gaps: BindingGap[];
      preferenceHintDigest?: string;
      explicitChoiceDigest?: string;
    }
  | { kind: "selected"; binding: BindingView }
  | { kind: "stale"; reasonCode: string; nextStep: string }
  | { kind: "invalidated"; reasonCode: string; nextStep: string };

export type PowerboxEvent =
  | { type: "load" }
  | { type: "loaded"; result: BindingCandidatesResult; preferenceProviderInstallationId?: string; nowMs?: number }
  | { type: "choose"; candidateDigest: string }
  | { type: "selected"; result: BindingMutationResult }
  | { type: "stale"; reasonCode: string; nextStep: string }
  | { type: "invalidate"; reasonCode: string; nextStep: string }
  | { type: "reset" };

export function powerboxReducer(state: PowerboxState, event: PowerboxEvent): PowerboxState {
  switch (event.type) {
    case "reset":
      return { kind: "idle" };
    case "load":
      return { kind: "loading" };
    case "loaded": {
      const nowMs = event.nowMs ?? Date.now();
      const candidates = event.result.candidates.filter((candidate) => {
        if (!candidate.effective_expires_at) return true;
        const expiresAt = Date.parse(candidate.effective_expires_at);
        return Number.isNaN(expiresAt) || expiresAt > nowMs;
      });
      if (candidates.length === 0) {
        const hadExpiredCandidate = event.result.candidates.length > 0;
        if (hadExpiredCandidate) {
          return {
            kind: "stale",
            reasonCode: "binding_expired",
            nextStep: "Recompute candidates before selecting another provider.",
          };
        }
        return {
          kind: "empty",
          emptyKind: classifyPowerboxEmpty(event.result.gaps ?? []),
          gaps: event.result.gaps ?? [],
        };
      }
      const preferenceHint = event.preferenceProviderInstallationId
        ? candidates.find((candidate) => candidate.provider.installation.installation_id === event.preferenceProviderInstallationId)
        : undefined;
      return {
        kind: "ready",
        candidates,
        gaps: event.result.gaps ?? [],
        ...(preferenceHint ? { preferenceHintDigest: preferenceHint.candidate_digest } : {}),
      };
    }
    case "choose":
      if (state.kind !== "ready" || !state.candidates.some((candidate) => candidate.candidate_digest === event.candidateDigest)) {
        return state;
      }
      return { ...state, explicitChoiceDigest: event.candidateDigest };
    case "selected":
      return { kind: "selected", binding: event.result.binding };
    case "stale":
      return { kind: "stale", reasonCode: event.reasonCode, nextStep: event.nextStep };
    case "invalidate":
      return { kind: "invalidated", reasonCode: event.reasonCode, nextStep: event.nextStep };
  }
}

export function classifyPowerboxEmpty(gaps: BindingGap[]): PowerboxEmptyKind {
  const reasons = new Set(gaps.map((gap) => gap.reason_code));
  if (reasons.has("authority_denied") || reasons.has("forbidden")) return "forbidden";
  if (reasons.has("unsupported_interaction") || reasons.has("port_incompatible") || reasons.has("unsupported")) return "unsupported";
  if (reasons.has("binding_expired") || reasons.has("plan_stale") || reasons.has("stale") || reasons.has("outcome_unknown")) return "stale";
  if (reasons.has("binding_unavailable") || reasons.has("unavailable")) return "unavailable";
  return "absent";
}

export function createPowerboxContext(input: Omit<PowerboxConsumerContext, "hostScope"> & { hostScope?: string }): PowerboxConsumerContext {
  return { ...input, hostScope: input.hostScope ?? activeHostCredentialScope() };
}

export function powerboxContextKey(context: PowerboxConsumerContext): string {
  const run = context.phase === "runtime" && context.run
    ? `${context.run.run_id}:${context.run.run_revision}:${context.run.context_id}`
    : "none";
  return [
    context.hostScope,
    context.installationId,
    context.installationRevision,
    context.importPort,
    context.phase,
    run,
  ].map(encodeURIComponent).join("|");
}

export function createBindingCandidatesRequest(
  context: PowerboxConsumerContext,
  preferenceProviderInstallationId?: string,
): BindingCandidatesRequest {
  assertRunPhase(context);
  return {
    consumer_installation_id: context.installationId,
    expected_consumer_installation_revision: context.installationRevision,
    import_port: context.importPort,
    phase: context.phase,
    ...(context.phase === "runtime" ? { consumer_run: context.run } : {}),
    ...(preferenceProviderInstallationId ? {
      preferences: [{ kind: "installation", id: preferenceProviderInstallationId } satisfies ResourceSelector],
    } : {}),
  };
}

export function createBindingSelectRequest(
  context: PowerboxConsumerContext,
  candidate: BindingCandidate,
  idempotencyKey: string,
): BindingSelectRequest {
  assertRunPhase(context);
  if (candidate.consumer.installation.installation_id !== context.installationId) {
    throw new Error("candidate consumer does not match the active Installation");
  }
  if (candidate.consumer.port.root_port !== context.importPort) {
    throw new Error("candidate consumer does not match the requested import Port");
  }
  if (candidate.phase !== context.phase) {
    throw new Error("candidate phase does not match the active Powerbox phase");
  }
  return {
    candidate_digest: candidate.candidate_digest,
    consumer_installation_id: context.installationId,
    expected_consumer_installation_revision: context.installationRevision,
    expected_exposure_revision: candidate.exposure.revision,
    expected_provider_installation_revision: candidate.provider.installation.installation_revision,
    exposure_id: candidate.exposure.record.exposure_id,
    idempotency_key: idempotencyKey,
    import_port: context.importPort,
    phase: context.phase,
    provider_installation_id: candidate.provider.installation.installation_id,
    ...(context.phase === "runtime" ? { consumer_run: context.run } : {}),
  };
}

function assertRunPhase(context: PowerboxConsumerContext): void {
  if (context.phase === "runtime") {
    if (!context.run?.run_id || !Number.isInteger(context.run.run_revision) || !context.run.context_id) {
      throw new Error("Runtime Powerbox context requires the exact Run ID, revision, and context ID");
    }
  } else if (context.run !== undefined) {
    throw new Error(`${context.phase} Powerbox context must not carry a Run pin`);
  }
}

function hasExactResource(identity: HostAccessIdentity, kind: HostAccessResourceKind, id: string): boolean {
  return Boolean(identity.resources?.some((resource) => resource.kind === kind && (resource.id === null || resource.id === id)));
}

/** Public Host access selector for a Port is scoped to its owning Installation. */
export function installationPortResourceId(installationId: InstallationId, portId: PortId): string {
  return `${installationId}/${portId}`;
}

export function powerboxAuthorityForContext(
  identity: HostAccessIdentity | null,
  context: PowerboxConsumerContext,
): PowerboxAuthority {
  if (identity?.kind === "root") {
    return { canObserve: true, canSelect: () => true, canRevoke: () => true };
  }
  if (!identity) return deniedPowerboxAuthority();
  const scopes = new Set(identity.scopes);
  const exactConsumer = hasExactResource(identity, "installation", context.installationId)
    && hasExactResource(identity, "port", installationPortResourceId(context.installationId, context.importPort))
    && (context.phase !== "runtime" || Boolean(context.run && hasExactResource(identity, "run", context.run.run_id)));
  const canObserve = scopes.has("observe") && exactConsumer;
  return {
    canObserve,
    canSelect: (exposureId) => canObserve
      && scopes.has("binding.manage")
      && hasExactResource(identity, "exposure", exposureId),
    canRevoke: (exposureId, _bindingId) => canObserve
      && scopes.has("binding.manage")
      && hasExactResource(identity, "exposure", exposureId),
    ...(!canObserve ? {
      reasonCode: "authority_denied" as const,
      nextStep: "Request observe authority for the exact consumer Installation, import Port, phase, and Run when Runtime-bound.",
    } : {}),
  };
}

function deniedPowerboxAuthority(): PowerboxAuthority {
  return {
    canObserve: false,
    canSelect: () => false,
    canRevoke: () => false,
    reasonCode: "authority_denied",
    nextStep: "Refresh the current Host identity and request exact Powerbox resources.",
  };
}

export interface PowerboxPreferenceStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

const POWERBOX_PREFERENCE_PREFIX = "plurora_powerbox_preference_v1:";

export class BrowserPowerboxPreferenceStore {
  constructor(private readonly storage: PowerboxPreferenceStorage | undefined = browserStorage()) {}

  get(context: PowerboxConsumerContext): string | undefined {
    try {
      return this.storage?.getItem(this.key(context)) ?? undefined;
    } catch {
      return undefined;
    }
  }

  set(context: PowerboxConsumerContext, providerInstallationId: InstallationId): void {
    try {
      this.storage?.setItem(this.key(context), providerInstallationId);
    } catch {
      // Preferences are optional UI hints; selection remains explicit.
    }
  }

  clear(context: PowerboxConsumerContext): void {
    try {
      this.storage?.removeItem(this.key(context));
    } catch {
      // Best effort only.
    }
  }

  private key(context: PowerboxConsumerContext): string {
    return `${POWERBOX_PREFERENCE_PREFIX}${powerboxContextKey(context)}`;
  }
}

function browserStorage(): PowerboxPreferenceStorage | undefined {
  try {
    return typeof window === "undefined" ? undefined : window.localStorage;
  } catch {
    return undefined;
  }
}

export function isBindingGapForPowerbox(gap: { reason_code: string; port_id?: string | null }): boolean {
  return Boolean(gap.port_id) && new Set([
    "binding_ambiguous",
    "binding_unavailable",
    "binding_expired",
    "port_incompatible",
    "port_unresolved",
    "unsupported_interaction",
    "outcome_unknown",
    "plan_stale",
  ]).has(gap.reason_code);
}

export function powerboxInvalidationForLifecycleEvent(
  event: { kind: string; payload: unknown },
  exposureIds: ReadonlySet<string>,
  providerRunIds: ReadonlySet<string>,
  providerInstallationIds: ReadonlySet<string> = new Set(),
): Pick<Extract<PowerboxEvent, { type: "invalidate" }>, "reasonCode" | "nextStep"> | undefined {
  if (event.kind === "host/exposure.revoked" || event.kind === "host/exposure.expired") {
    const exposureId = nestedString(event.payload, ["exposure", "record", "exposure_id"]);
    if (exposureId && exposureIds.has(exposureId)) {
      return {
        reasonCode: event.kind.endsWith("expired") ? "binding_expired" : "binding_unavailable",
        nextStep: "Recompute candidates; the selected Exposure is no longer active.",
      };
    }
  }
  if (
    event.kind === "host/binding.selected"
    || event.kind === "host/binding.revoked"
    || event.kind === "host/binding.expired"
  ) {
    const exposureId = nestedString(event.payload, ["binding", "record", "exposure_id"]);
    const providerRunId = nestedString(event.payload, ["binding", "record", "provider", "run", "run_id"]);
    const providerInstallationId = nestedString(event.payload, ["binding", "record", "provider", "installation", "installation_id"]);
    if (
      (exposureId && exposureIds.has(exposureId))
      || (providerRunId && providerRunIds.has(providerRunId))
      || (providerInstallationId && providerInstallationIds.has(providerInstallationId))
    ) {
      return {
        reasonCode: event.kind.endsWith("expired") ? "binding_expired" : "binding_unavailable",
        nextStep: "Recompute candidates; the selected Binding or Exposure is no longer current.",
      };
    }
  }
  if (event.kind === "host/run.stopped" || event.kind === "host/run.failed") {
    const runId = nestedString(event.payload, ["run", "record", "run_id"]);
    if (runId && providerRunIds.has(runId)) {
      return {
        reasonCode: "binding_unavailable",
        nextStep: "Recompute candidates; the provider Run stopped.",
      };
    }
  }
  if (
    event.kind === "host/installation.created"
    || event.kind === "host/installation.updated"
    || event.kind === "host/installation.removed"
  ) {
    const installationId = nestedString(event.payload, ["view", "record", "installation_id"]);
    if (installationId && providerInstallationIds.has(installationId)) {
      return {
        reasonCode: "plan_stale",
        nextStep: "Recompute candidates; the provider Installation changed.",
      };
    }
  }
  return undefined;
}

/** Pure event filter used by the Installation Powerbox workbench. */
export function powerboxWorkbenchShouldReloadForLifecycleEvent(
  event: { kind: string; payload: unknown },
  installationId: string,
): boolean {
  if (
    event.kind === "host/exposure.created"
    || event.kind === "host/exposure.revoked"
    || event.kind === "host/exposure.expired"
  ) {
    return nestedString(event.payload, ["exposure", "record", "installation_id"]) === installationId;
  }
  if (
    event.kind === "host/binding.selected"
    || event.kind === "host/binding.revoked"
    || event.kind === "host/binding.expired"
  ) {
    return nestedString(event.payload, ["binding", "record", "consumer", "installation", "installation_id"]) === installationId
      || nestedString(event.payload, ["binding", "record", "provider", "installation", "installation_id"]) === installationId;
  }
  if (event.kind === "host/run.stopped" || event.kind === "host/run.failed") {
    // A provider Run may belong to another Installation. The public Powerbox
    // relay normally follows with Binding/Exposure terminal events, but the
    // lifecycle relay is still the recovery signal when that transition races
    // the workbench refresh.
    return Boolean(nestedString(event.payload, ["run", "record", "installation_id"]));
  }
  if (
    event.kind === "host/installation.created"
    || event.kind === "host/installation.updated"
    || event.kind === "host/installation.removed"
  ) {
    return Boolean(nestedString(event.payload, ["view", "record", "installation_id"]));
  }
  return false;
}

function nestedString(value: unknown, path: string[]): string | undefined {
  let current: unknown = value;
  for (const key of path) {
    if (!current || typeof current !== "object" || Array.isArray(current)) return undefined;
    current = (current as Record<string, unknown>)[key];
  }
  return typeof current === "string" ? current : undefined;
}
