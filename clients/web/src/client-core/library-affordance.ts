import type {
  InstallationRecord,
  InstallationView,
  InstallationWorkSummary,
  RunGap,
  RunStatus,
  RunView,
  WorkEntrypoint,
} from "@/protocol/generated-types";
import type { HostAccessIdentity } from "./host-access";

export type LibraryAction = "open" | "content" | "play" | "run" | "stop" | "restart";

export interface LibraryAuthorityInfo {
  can_observe?: boolean;
  can_run?: boolean;
  can_stop?: boolean;
  reason_code?: string;
  next_step?: string;
}

export interface LibraryAffordanceInput {
  work_summary?: InstallationWorkSummary | null;
  installation: InstallationRecord | InstallationView;
  active_run?: RunView | null;
  entrypoint?: WorkEntrypoint | null;
  authority?: LibraryAuthorityInfo;
  preflight_gaps?: RunGap[];
}

export interface LibraryAffordance {
  action: LibraryAction;
  available: boolean;
  reason_code?: string;
  risk?: string;
  next_step?: string;
}

/** Stateless UX-only resolver. Host authorization remains authoritative. */
export class LibraryAffordanceResolver {
  resolve(input: LibraryAffordanceInput): LibraryAffordance[] {
    return resolveLibraryAffordances(input);
  }

  forAction(input: LibraryAffordanceInput, action: LibraryAction): LibraryAffordance {
    return affordanceForAction(input, action);
  }
}

export function libraryAuthorityForInstallation(
  identity: HostAccessIdentity | null,
  installationId: string,
): LibraryAuthorityInfo {
  if (identity?.kind === "root") {
    return { can_observe: true, can_run: true, can_stop: true };
  }
  const scopes = new Set(identity?.scopes ?? []);
  const hasInstallation = Boolean(identity?.resources?.some(
    (resource) => resource.kind === "installation" && (resource.id === null || resource.id === installationId),
  ));
  const canObserve = scopes.has("observe") && hasInstallation;
  const canRun = scopes.has("run") && hasInstallation;
  return {
    can_observe: canObserve,
    can_run: canRun,
    can_stop: canRun,
    ...(!canRun ? {
      reason_code: "authority_denied",
      next_step: "Request Run scope for this exact Installation.",
    } : {}),
  };
}

const BLOCKING_GAPS = new Set([
  "artifact_missing",
  "binding_unavailable",
  "target_unsatisfied",
  "approval_required",
  "outcome_unknown",
  "recovery_required",
  "authority_denied",
]);

export function resolveLibraryAffordances(input: LibraryAffordanceInput): LibraryAffordance[] {
  const record = "record" in input.installation ? input.installation.record : input.installation;
  const status = record.status;
  const run = input.active_run ?? null;
  const authority = input.authority ?? {
    can_observe: false,
    can_run: false,
    can_stop: false,
    reason_code: "authority_denied",
    next_step: "Refresh the current Host access identity before choosing an action.",
  };
  const preflightKnown = input.preflight_gaps !== undefined;
  const gaps = input.preflight_gaps ?? [];
  const affordances: LibraryAffordance[] = [];

  const canObserve = authority.can_observe !== false;
  const summaryAvailable = canObserve && Boolean(input.work_summary);
  const missingSummary = summaryAvailable ? {} : {
    reason_code: authority.reason_code ?? "artifact_missing",
    next_step: authority.next_step ?? "Refresh the Installation Work summary before opening it.",
  };
  affordances.push({ action: "open", available: summaryAvailable && Boolean(input.work_summary?.entrypoints.length), ...missingSummary });
  affordances.push({ action: "content", available: summaryAvailable && Boolean(input.work_summary?.content_roots.length), ...missingSummary });

  const activeStatus = run?.record.status;
  const canPlay = status === "ready"
    && Boolean(input.entrypoint)
    && authority.can_run !== false
    && preflightKnown
    && gaps.length === 0;
  affordances.push({
    action: "play",
    available: canPlay,
    ...(!canPlay ? playBlock(input, gaps) : { risk: "Starts a Run for this exact Installation and entrypoint." }),
  });
  affordances.push({
    action: "run",
    available: canPlay,
    ...(!canPlay ? playBlock(input, gaps) : {}),
  });

  const canStop = Boolean(run)
    && ["starting", "running", "degraded", "stopping"].includes(activeStatus ?? "")
    && authority.can_stop !== false;
  affordances.push({
    action: "stop",
    available: canStop,
    ...(!canStop ? {
      reason_code: run ? authority.reason_code ?? "authority_denied" : "run_not_active",
      next_step: run ? authority.next_step ?? "Request stop authority for this Run." : "Start the entrypoint to create an active Run.",
    } : { risk: "Stops only the active Run identified by Installation ID, Run ID, and revision." }),
  });

  const canRestart = Boolean(run) && (activeStatus === "interrupted" || activeStatus === "failed") && authority.can_run !== false;
  affordances.push({
    action: "restart",
    available: canRestart,
    ...(!canRestart ? {
      reason_code: run ? authority.reason_code ?? "run_not_restartable" : "run_not_active",
      next_step: run ? authority.next_step ?? "Inspect the Run failure before restarting." : "Start the entrypoint to create a Run.",
    } : { risk: "Creates a new Run attempt; it never deploys a missing managed Realization." }),
  });

  return affordances;
}

function playBlock(input: LibraryAffordanceInput, gaps: RunGap[]): Pick<LibraryAffordance, "reason_code" | "next_step"> {
  const record = "record" in input.installation ? input.installation.record : input.installation;
  if (record.status !== "ready") {
    return {
      reason_code: record.status === "failed" ? "recovery_required" : "installation_not_ready",
      next_step: "Wait for the Installation to become ready, or inspect its recovery details.",
    };
  }
  if (!input.entrypoint) {
    return { reason_code: "entrypoint_unavailable", next_step: "Choose an entrypoint exposed by this Work." };
  }
  if (input.authority?.can_run === false) {
    return {
      reason_code: input.authority.reason_code ?? "authority_denied",
      next_step: input.authority.next_step ?? "Request Run authority for this Installation.",
    };
  }
  if (input.preflight_gaps === undefined) {
    return { reason_code: "preflight_required", next_step: "Run the entrypoint preflight before starting." };
  }
  const gap = gaps.find((candidate) => BLOCKING_GAPS.has(candidate.reason_code)) ?? gaps[0];
  if (gap) return { reason_code: gap.reason_code, next_step: gap.next_step };
  return { reason_code: "preflight_failed", next_step: "Refresh the entrypoint preflight before starting." };
}

export function affordanceForAction(input: LibraryAffordanceInput, action: LibraryAction): LibraryAffordance {
  return resolveLibraryAffordances(input).find((item) => item.action === action) ?? {
    action,
    available: false,
    reason_code: "unsupported_action",
    next_step: "Inspect the Work entrypoints and Installation state.",
  };
}

export type LibraryRunStatus = RunStatus;
