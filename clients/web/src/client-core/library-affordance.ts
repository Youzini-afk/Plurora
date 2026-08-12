import type {
  InstallationRecord,
  InstallationView,
  InstallationWorkSummary,
  BindingGap,
  RunGap,
  RunStatus,
  RunView,
  WorkEntrypoint,
} from "@/protocol/generated-types";
import type { HostAccessIdentity } from "./host-access";

export type LibraryAction = "open" | "content" | "play" | "run" | "stop" | "restart" | "backup";

export interface LibraryAuthorityInfo {
  can_observe?: boolean;
  can_run?: boolean;
  can_stop?: boolean;
  can_manage_installation?: boolean;
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
  binding_gaps?: BindingGap[];
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
    return { can_observe: true, can_run: true, can_stop: true, can_manage_installation: true };
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
    can_manage_installation: scopes.has("installation.manage") && hasInstallation,
    ...(!canRun ? {
      reason_code: "authority_denied",
      next_step: "Request Run scope for this exact Installation.",
    } : {}),
  };
}

const BLOCKING_GAPS = new Set([
  "artifact_missing",
  "binding_unavailable",
  "binding_ambiguous",
  "binding_expired",
  "port_incompatible",
  "port_unresolved",
  "unsupported_interaction",
  "plan_stale",
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
  const preflightKnown = input.preflight_gaps !== undefined || input.binding_gaps !== undefined;
  const gaps = [...(input.preflight_gaps ?? []), ...(input.binding_gaps ?? [])];
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
  const executeRight = input.work_summary?.rights_declaration?.execute;
  const canPlay = status === "ready"
    && Boolean(input.entrypoint)
    && authority.can_run !== false
    && preflightKnown
    && gaps.length === 0
    && executeRight !== "denied"
    && executeRight !== "unspecified";
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
    } : { risk: "Creates a new Run attempt; it never applies a missing managed Realization." }),
  });

  const backupRight = input.work_summary?.rights_declaration?.backup;
  const canBackup = status === "ready"
    && authority.can_manage_installation === true
    && backupRight === "allowed";
  affordances.push({
    action: "backup",
    available: canBackup,
    ...(!canBackup ? {
      reason_code: status !== "ready"
        ? "installation_not_ready"
        : authority.can_manage_installation !== true
          ? "authority_denied"
          : backupRight === "denied"
            ? "rights_denied"
            : backupRight === "requires_entitlement"
              ? "entitlement_required"
              : "rights_unspecified",
      next_step: status !== "ready"
        ? "Wait for the Installation to become ready before backing up state."
        : authority.can_manage_installation !== true
          ? "Request installation.manage for this exact Installation."
          : "Review the declared backup Right before capturing opaque state.",
    } : { risk: "Captures immutable opaque state without modifying the Installation state tree." }),
  });

  return affordances;
}

function playBlock(input: LibraryAffordanceInput, gaps: Array<RunGap | BindingGap>): Pick<LibraryAffordance, "reason_code" | "next_step"> {
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
  const executeRight = input.work_summary?.rights_declaration?.execute;
  if (executeRight === "denied" || executeRight === "unspecified") {
    return {
      reason_code: executeRight === "denied" ? "rights_denied" : "rights_unspecified",
      next_step: "Review the Work execute Right before starting this entrypoint.",
    };
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
