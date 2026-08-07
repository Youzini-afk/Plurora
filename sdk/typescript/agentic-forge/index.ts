/**
 * Plurora Agentic Forge SDK — Package-owned run lifecycle, plan graph,
 * working state, and candidate shapes.
 *
 * This module defines the **agentic forge contract** at the package/SDK layer.
 * It does NOT enter the kernel, does NOT add Rust protocol methods, and does
 * NOT add `platform.agent.*`, `platform.model.*`, `platform.prompt.*`,
 * `platform.memory.*`, or `platform.turn.*`.
 *
 * ## Design principles
 *
 * - **Package-owned**: Run lifecycle, plan graph, working state are ordinary
 *   package artifacts, not kernel primitives.
 * - **Deterministic**: No network, no real model inference, no random.
 * - **Secret-safe**: Uses `secret_ref` identifiers; rejects raw secrets.
 * - **No kernel agent namespace**: Output never contains `platform.agent.*`,
 *   `platform.model.*`, `platform.prompt.*`, `platform.memory.*`, `platform.turn.*`.
 *
 * ## API surface
 *
 * Types:
 * - `AgentRunLifecycleState` — lifecycle state enum
 * - `PlanNode` / `PlanEdge` — plan graph elements
 * - `PlanGraph` — full plan graph artifact
 * - `WorkingState` — run working state artifact
 * - `RunEvent` — trace event
 * - `CandidateShell` — candidate shape (Phase A shell)
 * - `Candidate` — full branch-aware candidate (Phase B)
 * - `CandidateComparison` — candidate comparison result
 * - `PromoteProposalDraft` — promote proposal draft
 * - `ObservabilitySummary` — run observability summary
 * - `BranchPolicy` — scratch/target branch policy
 *
 * Helpers:
 * - `createRunEvent()` — build a valid run event
 * - `validatePlanGraph()` — validate a plan graph
 * - `createPlanGraph()` — build a minimal plan graph
 * - `createWorkingState()` — build a working state
 * - `createCandidateShell()` — build a candidate shell (Phase A)
 * - `createCandidate()` — build a branch-aware candidate (Phase B)
 * - `compareCandidate()` — compare scratch vs target with stale detection
 * - `createPromoteProposalDraft()` — build a promote proposal draft
 * - `archiveCandidate()` — build an archived candidate shape
 * - `validateCandidate()` — validate a candidate structure
 * - `blockRawSecrets()` — check for raw-secret-like content
 * - `runAgenticForgeSelfTest()` — pure-TS self-test
 */

// ---------------------------------------------------------------------------
// Lifecycle states
// ---------------------------------------------------------------------------

/** Package-owned agent run lifecycle states. */
export type AgentRunLifecycleState =
  | "created"
  | "prepared"
  | "running"
  | "paused"
  | "waiting_for_approval"
  | "completed"
  | "failed"
  | "cancelled"
  | "archived";

const VALID_LIFECYCLE_STATES: AgentRunLifecycleState[] = [
  "created", "prepared", "running", "paused", "waiting_for_approval",
  "completed", "failed", "cancelled", "archived",
];

export function isValidLifecycleState(s: string): s is AgentRunLifecycleState {
  return (VALID_LIFECYCLE_STATES as string[]).includes(s);
}

// ---------------------------------------------------------------------------
// Plan graph types
// ---------------------------------------------------------------------------

export type PlanNodeKind = "observe" | "infer" | "tool_call" | "inspect" | "branch_op" | "compare" | "propose" | "wait";

export interface PlanNode {
  node_id: string;
  kind: PlanNodeKind;
  label: string;
  status: "pending" | "running" | "completed" | "failed" | "skipped";
  input_refs?: string[];
  output_refs?: string[];
}

export interface PlanEdge {
  from_node_id: string;
  to_node_id: string;
  kind: "sequential" | "conditional" | "parallel";
}

export interface RetryPolicy {
  max_retries: number;
  backoff: "none" | "linear" | "exponential";
}

export interface PlanGraph {
  nodes: PlanNode[];
  edges: PlanEdge[];
  status: "prepared" | "running" | "completed" | "failed";
  revision: number;
  input_refs: string[];
  output_refs: string[];
  approval_policy: string;
  retry_policy: RetryPolicy;
  deterministic_mode: boolean;
}

// ---------------------------------------------------------------------------
// Working state
// ---------------------------------------------------------------------------

export interface PolicyState {
  approval_policy: string;
  retry_budget_remaining: number;
  deterministic_mode: boolean;
}

export interface WorkingState {
  run_id: string;
  owner_package: string;
  target_branch_ref: string;
  scratch_branch_ref: string;
  current_objective: string;
  local_context_refs: string[];
  plan_graph_ref: string;
  candidate_refs: string[];
  tool_observation_refs: string[];
  inference_trace_refs: string[];
  policy_state: PolicyState;
}

// ---------------------------------------------------------------------------
// Run event
// ---------------------------------------------------------------------------

export type RunEventType =
  | "run_created"
  | "run_prepared"
  | "run_running"
  | "run_paused"
  | "run_waiting_for_approval"
  | "run_completed"
  | "run_failed"
  | "run_cancelled"
  | "run_archived"
  | "node_started"
  | "node_completed"
  | "node_failed";

export interface RunEvent {
  event_type: RunEventType;
  run_id: string;
  timestamp: number;
  payload: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Candidate shell (Phase A placeholder)
// ---------------------------------------------------------------------------

export interface CandidateShell {
  candidate_id: string;
  run_id: string;
  target_branch_ref: string;
  scratch_branch_ref: string;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

// ---------------------------------------------------------------------------
// Phase B: full candidate, comparison, promote, archive
// ---------------------------------------------------------------------------

export type CandidateState =
  | "draft"
  | "ready"
  | "comparing"
  | "promoting"
  | "promoted"
  | "rejected"
  | "archived"
  | "failed";

const VALID_CANDIDATE_STATES: CandidateState[] = [
  "draft", "ready", "comparing", "promoting", "promoted",
  "rejected", "archived", "failed",
];

export function isValidCandidateState(s: string): s is CandidateState {
  return (VALID_CANDIDATE_STATES as string[]).includes(s);
}

/** Full branch-aware candidate artifact (Phase B). */
export interface Candidate {
  candidate_id: string;
  run_id: string;
  target_branch_ref: string;
  scratch_branch_ref: string;
  changed_asset_refs: string[];
  projection_refs: string[];
  diff_summary: string;
  inspection_refs: string[];
  confidence: number;
  uncertainty: number;
  provenance: {
    package_id: string;
    capability_id: string;
  };
  status: CandidateState;
  target_revision: number;
}

/** Result of comparing a candidate's scratch branch against target. */
export interface CandidateComparison {
  candidate_id: string;
  target_branch_ref: string;
  scratch_branch_ref: string;
  diff_summary: string;
  affected_assets: string[];
  affected_projections: string[];
  lineage_impact: {
    target_branch_modified: boolean;
    scratch_branch_source: string;
    requires_rebase: boolean;
  };
  stale: boolean;
  candidate_target_revision: number;
  current_target_revision: number;
}

/** Promote proposal draft — never directly mutates target. */
export interface PromoteProposalDraft {
  requires_user_approval: boolean;
  operations: Array<{
    op: string;
    payload: Record<string, unknown>;
  }>;
  required_permissions: string[];
  expected_effects: string[];
  source_candidate: string;
  source_run: string;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

/** Branch policy explanation. */
export interface BranchPolicy {
  scratch_branch_intent: string;
  promote_requires_proposal: boolean;
  stale_target_blocks_promote: boolean;
  target_revision_must_match: boolean;
  reject_leaves_target_unchanged: boolean;
  archive_does_not_modify_target: boolean;
}

// ---------------------------------------------------------------------------
// Phase C: inference node, replay, validation, failure taxonomy
// ---------------------------------------------------------------------------

/** Inference provider kinds. */
export type ProviderKind =
  | "deterministic"
  | "recorded"
  | "cloud_adapter_plan"
  | "local_fake";

export const VALID_PROVIDER_KINDS: ProviderKind[] = [
  "deterministic", "recorded", "cloud_adapter_plan", "local_fake",
];

export function isValidProviderKind(s: string): s is ProviderKind {
  return (VALID_PROVIDER_KINDS as string[]).includes(s);
}

/** Plan node kinds (explicit Phase C coverage). */
export type ExplicitPlanNodeKind =
  | "observe"
  | "infer"
  | "tool_call"
  | "inspect"
  | "branch_op"
  | "compare"
  | "propose"
  | "wait";

export const EXPLICIT_PLAN_NODE_KINDS: ExplicitPlanNodeKind[] = [
  "observe", "infer", "tool_call", "inspect", "branch_op", "compare", "propose", "wait",
];

/** Allowed inference output actions. */
export type AllowedInferenceAction =
  | "candidate_seed"
  | "proposal_seed"
  | "observation"
  | "needs_repair";

export const ALLOWED_INFERENCE_ACTIONS: AllowedInferenceAction[] = [
  "candidate_seed", "proposal_seed", "observation", "needs_repair",
];

/** Forbidden inference output actions. */
export type ForbiddenInferenceAction =
  | "privilege_escalation"
  | "auto_promote"
  | "secret_request"
  | "target_branch_write"
  | "unknown_action";

export const FORBIDDEN_INFERENCE_ACTIONS: ForbiddenInferenceAction[] = [
  "privilege_escalation", "auto_promote", "secret_request", "target_branch_write", "unknown_action",
];

/** Inference failure taxonomy kinds. */
export type InferenceFailureKind =
  | "rate_limit"
  | "quota"
  | "timeout"
  | "auth"
  | "network_denied"
  | "invalid_output"
  | "malformed_output"
  | "replay_mismatch"
  | "policy_reject";

export const INFERENCE_FAILURE_KINDS: InferenceFailureKind[] = [
  "rate_limit", "quota", "timeout", "auth", "network_denied",
  "invalid_output", "malformed_output", "replay_mismatch", "policy_reject",
];

export function isInferenceFailureKind(s: string): s is InferenceFailureKind {
  return (INFERENCE_FAILURE_KINDS as string[]).includes(s);
}

/** Inference node result from run_inference_node. */
export interface InferenceNodeResult {
  status: string;
  output_action: string;
  content_hint: string;
  target_branch_unchanged: boolean;
  direct_mutation: boolean;
}

/** Inference trace record. */
export interface InferenceTrace {
  provider_kind: ProviderKind;
  model_performed: boolean;
  network_performed: boolean;
  output_action: string;
  fingerprint: string;
}

/** Full run_inference_node response shape. */
export interface RunInferenceNodeResponse {
  kind: string;
  run_id: string;
  node_id: string;
  provider_kind: ProviderKind;
  node_result: InferenceNodeResult;
  inference_trace: InferenceTrace;
  inference_performed: boolean;
  network_performed: boolean;
}

/** Replay result (match or mismatch). */
export interface ReplayInferenceNodeResponse {
  kind: string;
  run_id: string;
  node_id: string;
  fingerprint_match: boolean;
  fingerprint?: string;
  expected_fingerprint?: string;
  actual_fingerprint?: string;
  inference_performed: boolean;
  network_performed: boolean;
}

/** Inference output validation result. */
export interface InferenceOutputValidation {
  action: string;
  validation_result: "accepted" | "rejected";
  allowed: boolean;
  reason: string;
  allowed_actions: string[];
  forbidden_actions: string[];
}

/** Inference failure explanation. */
export interface InferenceFailureExplanation {
  failure_kind: string;
  is_known: boolean;
  recovery_hint: string;
  taxonomy: string[];
}

// ---------------------------------------------------------------------------
// Observability summary
// ---------------------------------------------------------------------------

export interface ObservabilitySummary {
  run_id: string;
  lifecycle_state: AgentRunLifecycleState;
  trace_event_count: number;
  plan_node_count: number;
  candidate_count: number;
  tool_observation_count: number;
  inference_trace_count: number;
  inference_performed: boolean;
  network_performed: boolean;
  summary: string;
}

// ---------------------------------------------------------------------------
// Raw-secret detection (conservative)
// ---------------------------------------------------------------------------

const SECRET_FIELD_NAMES = [
  "api_key", "secret", "token", "password", "private_key",
  "access_token", "refresh_token", "auth_token",
];

const SECRET_VALUE_PREFIXES = ["sk-", "Bearer ", "bearer "];

function isSecretRefValue(val: string): boolean {
  return val.startsWith("secret_ref:") ||
         val.startsWith("secretRef:") ||
         val.startsWith("secret-ref:") ||
         val.startsWith("host:");
}

export function looksLikeRawSecret(val: string): boolean {
  for (const prefix of SECRET_VALUE_PREFIXES) {
    if (val.startsWith(prefix)) return true;
  }
  if (val.length >= 32) {
    const hasUpper = /[A-Z]/.test(val);
    const hasLower = /[a-z]/.test(val);
    const hasDigit = /[0-9]/.test(val);
    if (hasUpper && hasLower && hasDigit && val.length >= 40) return true;
  }
  return false;
}

export function isSecretFieldName(name: string): boolean {
  const lower = name.toLowerCase();
  return SECRET_FIELD_NAMES.some(sn => lower === sn || lower.includes(sn));
}

/** Recursively check a value for raw-secret-like content. Returns true if blocked. */
export function blockRawSecrets(value: unknown): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === "string") return looksLikeRawSecret(value);
  if (typeof value !== "object") return false;

  if (Array.isArray(value)) {
    return value.some(item => blockRawSecrets(item));
  }

  const obj = value as Record<string, unknown>;
  for (const [key, val] of Object.entries(obj)) {
    if (isSecretFieldName(key) && typeof val === "string") {
      if (!isSecretRefValue(val) && val !== "") {
        return true;
      }
    }
    if (typeof val === "string" && looksLikeRawSecret(val)) return true;
    if (blockRawSecrets(val)) return true;
  }
  return false;
}

/** Check that a JSON-serializable output contains no platform.agent/model/prompt/memory/turn namespace. */
export function hasPlatformAgentNamespace(value: unknown): boolean {
  const str = JSON.stringify(value);
  return str.includes("platform.agent") ||
         str.includes("platform.model") ||
         str.includes("platform.prompt") ||
         str.includes("platform.memory") ||
         str.includes("platform.turn");
}

// ---------------------------------------------------------------------------
// Helper constructors
// ---------------------------------------------------------------------------

/** Build a valid run event. */
export function createRunEvent(
  eventType: RunEventType,
  runId: string,
  timestamp: number,
  payload: Record<string, unknown>,
): RunEvent {
  return { event_type: eventType, run_id: runId, timestamp, payload };
}

/** Validate a plan graph structure. Returns diagnostics. */
export function validatePlanGraph(pg: Partial<PlanGraph>): string[] {
  const diagnostics: string[] = [];
  if (!pg.nodes || !Array.isArray(pg.nodes)) {
    diagnostics.push("plan_graph.nodes must be an array");
  } else {
    const nodeIds = new Set(pg.nodes!.map(n => n.node_id));
    for (const edge of pg.edges ?? []) {
      if (!nodeIds.has(edge.from_node_id)) {
        diagnostics.push(`edge references unknown from_node_id: ${edge.from_node_id}`);
      }
      if (!nodeIds.has(edge.to_node_id)) {
        diagnostics.push(`edge references unknown to_node_id: ${edge.to_node_id}`);
      }
    }
  }
  if (pg.approval_policy && pg.approval_policy !== "fork_then_approve" && pg.approval_policy !== "approve_then_fork" && pg.approval_policy !== "none") {
    diagnostics.push(`unknown approval_policy: ${pg.approval_policy}`);
  }
  return diagnostics;
}

/** Build a minimal plan graph. */
export function createPlanGraph(runId: string, objective: string): PlanGraph {
  return {
    nodes: [
      { node_id: `${runId}_node_observe`, kind: "observe", label: "Observe context", status: "pending" },
      { node_id: `${runId}_node_plan`, kind: "tool_call", label: objective, status: "pending" },
      { node_id: `${runId}_node_propose`, kind: "propose", label: "Produce candidate", status: "pending" },
    ],
    edges: [
      { from_node_id: `${runId}_node_observe`, to_node_id: `${runId}_node_plan`, kind: "sequential" },
      { from_node_id: `${runId}_node_plan`, to_node_id: `${runId}_node_propose`, kind: "sequential" },
    ],
    status: "prepared",
    revision: 1,
    input_refs: [],
    output_refs: [],
    approval_policy: "fork_then_approve",
    retry_policy: { max_retries: 0, backoff: "none" },
    deterministic_mode: true,
  };
}

/** Build a working state. */
export function createWorkingState(
  runId: string,
  ownerPackage: string,
  options: Partial<WorkingState> = {},
): WorkingState {
  return {
    run_id: runId,
    owner_package: ownerPackage,
    target_branch_ref: options.target_branch_ref ?? "branch:target:default",
    scratch_branch_ref: options.scratch_branch_ref ?? "branch:scratch:default",
    current_objective: options.current_objective ?? "deterministic agentic forge run",
    local_context_refs: options.local_context_refs ?? [],
    plan_graph_ref: options.plan_graph_ref ?? `plan_graph:${runId}`,
    candidate_refs: options.candidate_refs ?? [],
    tool_observation_refs: options.tool_observation_refs ?? [],
    inference_trace_refs: options.inference_trace_refs ?? [],
    policy_state: options.policy_state ?? {
      approval_policy: "fork_then_approve",
      retry_budget_remaining: 0,
      deterministic_mode: true,
    },
  };
}

/** Build a candidate shell (Phase A placeholder). */
export function createCandidateShell(
  candidateId: string,
  runId: string,
  targetBranchRef: string,
  scratchBranchRef: string,
  packageId: string,
  capabilityId: string,
): CandidateShell {
  return {
    candidate_id: candidateId,
    run_id: runId,
    target_branch_ref: targetBranchRef,
    scratch_branch_ref: scratchBranchRef,
    provenance: { package_id: packageId, capability_id: capabilityId },
  };
}

// ---------------------------------------------------------------------------
// Phase B helpers
// ---------------------------------------------------------------------------

/** Build a full branch-aware candidate (Phase B). */
export function createCandidate(
  candidateId: string,
  runId: string,
  targetBranchRef: string,
  scratchBranchRef: string,
  packageId: string,
  capabilityId: string,
  options: Partial<Omit<Candidate, "candidate_id" | "run_id" | "target_branch_ref" | "scratch_branch_ref" | "provenance">> = {},
): Candidate {
  return {
    candidate_id: candidateId,
    run_id: runId,
    target_branch_ref: targetBranchRef,
    scratch_branch_ref: scratchBranchRef,
    changed_asset_refs: options.changed_asset_refs ?? [],
    projection_refs: options.projection_refs ?? [],
    diff_summary: options.diff_summary ?? "deterministic diff: no real changes",
    inspection_refs: options.inspection_refs ?? [],
    confidence: options.confidence ?? 0.5,
    uncertainty: options.uncertainty ?? 0.5,
    provenance: { package_id: packageId, capability_id: capabilityId },
    status: options.status ?? "draft",
    target_revision: options.target_revision ?? 1,
  };
}

/** Compare scratch vs target, detecting stale branches. */
export function compareCandidate(
  candidate: Candidate,
  currentTargetRevision: number,
): CandidateComparison {
  const stale = candidate.target_revision !== currentTargetRevision;
  return {
    candidate_id: candidate.candidate_id,
    target_branch_ref: candidate.target_branch_ref,
    scratch_branch_ref: candidate.scratch_branch_ref,
    diff_summary: candidate.diff_summary,
    affected_assets: candidate.changed_asset_refs,
    affected_projections: candidate.projection_refs,
    lineage_impact: {
      target_branch_modified: false,
      scratch_branch_source: candidate.scratch_branch_ref,
      requires_rebase: stale,
    },
    stale,
    candidate_target_revision: candidate.target_revision,
    current_target_revision: currentTargetRevision,
  };
}

/** Build a promote proposal draft. Never directly mutates target. */
export function createPromoteProposalDraft(
  candidate: Candidate,
  packageId: string,
  capabilityId: string,
): PromoteProposalDraft {
  return {
    requires_user_approval: true,
    operations: [
      {
        op: "asset.put",
        payload: {
          ref: candidate.changed_asset_refs,
          source_branch: candidate.scratch_branch_ref,
          target_branch: candidate.target_branch_ref,
        },
      },
    ],
    required_permissions: [],
    expected_effects: [
      "candidate assets promoted to target branch via proposal approval",
    ],
    source_candidate: candidate.candidate_id,
    source_run: candidate.run_id,
    provenance: { package_id: packageId, capability_id: capabilityId },
  };
}

/** Build an archived candidate shape. */
export function archiveCandidate(
  candidate: Candidate,
): Candidate {
  return {
    ...candidate,
    status: "archived",
  };
}

/** Validate a candidate structure. Returns diagnostics. */
export function validateCandidate(c: Partial<Candidate>): string[] {
  const diagnostics: string[] = [];
  if (!c.candidate_id) diagnostics.push("candidate must have candidate_id");
  if (!c.run_id) diagnostics.push("candidate must have run_id");
  if (!c.target_branch_ref) diagnostics.push("candidate must have target_branch_ref");
  if (!c.scratch_branch_ref) diagnostics.push("candidate must have scratch_branch_ref");
  if (c.status && !isValidCandidateState(c.status)) {
    diagnostics.push(`unknown candidate status: ${c.status}`);
  }
  if (c.confidence !== undefined && (c.confidence < 0 || c.confidence > 1)) {
    diagnostics.push("confidence must be between 0 and 1");
  }
  if (c.uncertainty !== undefined && (c.uncertainty < 0 || c.uncertainty > 1)) {
    diagnostics.push("uncertainty must be between 0 and 1");
  }
  return diagnostics;
}

// ---------------------------------------------------------------------------
// Phase C helpers: inference node / replay / validation / failure
// ---------------------------------------------------------------------------

/** Compute a deterministic fingerprint from input content. */
export function computeDeterministicFingerprint(input: Record<string, unknown>): string {
  const objective = (typeof input.objective === "string") ? input.objective : "default";
  const len = objective.length;
  const hash = ((len * 31) + 0xaf) >>> 0;  // unsigned 32-bit
  return `fp_${hash.toString(16).padStart(4, "0")}`;
}

/** Build a run_inference_node result (deterministic/default mode). */
export function runInferenceNode(
  runId: string,
  nodeId: string,
  providerKind: ProviderKind = "deterministic",
  objective: string = "deterministic inference",
): RunInferenceNodeResponse {
  // cloud_adapter_plan: return plan shape, no network
  if (providerKind === "cloud_adapter_plan") {
    return {
      kind: "agentic_forge_inference_node_plan",
      run_id: runId,
      node_id: nodeId,
      provider_kind: providerKind,
      node_result: {
        status: "needs_host_policy",
        output_action: "observation",
        content_hint: "cloud adapter requires host-managed network policy and outbound execution; no network performed by package",
        target_branch_unchanged: true,
        direct_mutation: false,
      },
      inference_trace: {
        provider_kind: providerKind,
        model_performed: false,
        network_performed: false,
        output_action: "observation",
        fingerprint: computeDeterministicFingerprint({ objective, run_id: runId, node_id: nodeId }),
      },
      inference_performed: false,
      network_performed: false,
    };
  }

  const outputAction = objective.includes("proposal") ? "proposal_seed" : "candidate_seed";
  const modelPerformed = providerKind === "local_fake";

  return {
    kind: "agentic_forge_inference_node_result",
    run_id: runId,
    node_id: nodeId,
    provider_kind: providerKind,
    node_result: {
      status: "completed",
      output_action: outputAction,
      content_hint: `deterministic ${outputAction} from ${providerKind}`,
      target_branch_unchanged: true,
      direct_mutation: false,
    },
    inference_trace: {
      provider_kind: providerKind,
      model_performed: modelPerformed,
      network_performed: false,
      output_action: outputAction,
      fingerprint: computeDeterministicFingerprint({ objective, run_id: runId, node_id: nodeId }),
    },
    inference_performed: modelPerformed,
    network_performed: false,
  };
}

/** Replay a recorded inference node. Mismatches are flagged, never silently passed. */
export function replayInferenceNode(
  runId: string,
  nodeId: string,
  expectedFingerprint: string,
  objective: string = "default",
): ReplayInferenceNodeResponse {
  const actualFingerprint = computeDeterministicFingerprint({ objective, run_id: runId, node_id: nodeId });
  const match = expectedFingerprint === actualFingerprint;

  if (match) {
    return {
      kind: "agentic_forge_replay_ok",
      run_id: runId,
      node_id: nodeId,
      fingerprint_match: true,
      fingerprint: expectedFingerprint,
      inference_performed: false,
      network_performed: false,
    };
  } else {
    return {
      kind: "agentic_forge_replay_mismatch",
      run_id: runId,
      node_id: nodeId,
      fingerprint_match: false,
      expected_fingerprint: expectedFingerprint,
      actual_fingerprint: actualFingerprint,
      inference_performed: false,
      network_performed: false,
    };
  }
}

/** Validate an inference output action against the allowlist. */
export function validateInferenceOutput(action: string): InferenceOutputValidation {
  const isAllowed = (ALLOWED_INFERENCE_ACTIONS as string[]).includes(action);
  const isForbidden = (FORBIDDEN_INFERENCE_ACTIONS as string[]).includes(action);
  const result = (isForbidden || !isAllowed) ? "rejected" : "accepted";

  let reason: string;
  if (isForbidden) {
    reason = `action '${action}' is in the forbidden list; model output cannot escalate privileges, auto-promote, request secrets, write target branches, or execute unknown actions`;
  } else if (!isAllowed) {
    reason = `action '${action}' is not in the allowed list; only candidate_seed, proposal_seed, observation, needs_repair are permitted`;
  } else {
    reason = "action is permitted";
  }

  return {
    action,
    validation_result: result,
    allowed: isAllowed && !isForbidden,
    reason,
    allowed_actions: [...ALLOWED_INFERENCE_ACTIONS],
    forbidden_actions: [...FORBIDDEN_INFERENCE_ACTIONS],
  };
}

/** Explain an inference failure with recovery hint. */
const FAILURE_RECOVERY_HINTS: Record<string, string> = {
  rate_limit: "reduce request frequency or implement backoff; consider recorded replay for deterministic re-runs",
  quota: "check usage limits; switch to deterministic or recorded provider for quota-free runs",
  timeout: "increase timeout budget or use recorded replay to avoid network dependency",
  auth: "verify secret_ref resolves correctly; do not embed raw credentials; check provider identity",
  network_denied: "network access was denied by policy; use deterministic or local_fake provider; cloud_adapter_plan only returns plan shape",
  invalid_output: "model output failed validation; run validate_inference_output to check; repair with needs_repair action",
  malformed_output: "model output could not be parsed; treat as node_failed; generate repair proposal",
  replay_mismatch: "recorded output fingerprint does not match expected; re-run with correct recorded output or update expected fingerprint",
  policy_reject: "inference output action was rejected by policy; only candidate_seed/proposal_seed/observation/needs_repair are allowed; model output cannot escalate or auto-promote",
};

export function explainInferenceFailure(failureKind: string): InferenceFailureExplanation {
  const isKnown = isInferenceFailureKind(failureKind);
  const recoveryHint = FAILURE_RECOVERY_HINTS[failureKind]
    ?? "unknown failure kind; consult inference_failure_taxonomy for valid kinds";

  return {
    failure_kind: failureKind,
    is_known: isKnown,
    recovery_hint: recoveryHint,
    taxonomy: [...INFERENCE_FAILURE_KINDS],
  };
}

// ---------------------------------------------------------------------------
// Phase D: tool bridge v2 — scoped toolchain / risk / replay
// ---------------------------------------------------------------------------

/** Tool risk categories. */
export type ToolRiskCategory =
  | "prompt_injection"
  | "secret_exfiltration"
  | "branch_write"
  | "outbound_expansion"
  | "nested_delegation"
  | "large_output";

export const TOOL_RISK_CATEGORIES: ToolRiskCategory[] = [
  "prompt_injection", "secret_exfiltration", "branch_write",
  "outbound_expansion", "nested_delegation", "large_output",
];

/** A single risk finding from summarize_tool_risk. */
export interface ToolRiskFinding {
  category: ToolRiskCategory;
  severity: "critical" | "high" | "medium" | "low";
  description: string;
  mitigation: string;
}

/** Tool call context shape. */
export interface ToolCallContext {
  requesting_package: string | null;
  run_id: string | null;
  plan_node_id: string | null;
  target_branch_scope: string | null;
  scratch_branch_scope: string | null;
  asset_scope: string | null;
  capability_grant: string[];
  approval_policy: string;
  audit_context: Record<string, unknown>;
}

/** Toolchain plan step. */
export interface ToolchainStep {
  step_index: number;
  capability_id: string;
  provider_package_id: string | null;
  grant_scope: string[];
  approval_policy: string;
  status: "planned" | "blocked";
  reason?: string;
  no_execution: boolean;
  no_ambient_authority: boolean;
}

/** Tool observation record. */
export interface ToolObservation {
  observation_ref: string;
  run_id: string;
  plan_node_id: string;
  provider_package_id: string | null;
  untrusted: boolean;
  output_recommendation: "inline" | "asset_ref";
}

/** Create a tool call context. */
export function createToolCallContext(options: Partial<ToolCallContext> = {}): ToolCallContext {
  return {
    requesting_package: options.requesting_package ?? null,
    run_id: options.run_id ?? null,
    plan_node_id: options.plan_node_id ?? null,
    target_branch_scope: options.target_branch_scope ?? null,
    scratch_branch_scope: options.scratch_branch_scope ?? null,
    asset_scope: options.asset_scope ?? null,
    capability_grant: options.capability_grant ?? [],
    approval_policy: options.approval_policy ?? "fork_then_approve",
    audit_context: options.audit_context ?? {},
  };
}

/** Compute a deterministic tool plan fingerprint. */
export function computeToolPlanFingerprint(input: Record<string, unknown>): string {
  const objective = (typeof input.objective === "string") ? input.objective : "default";
  const len = objective.length;
  const hash = ((len * 37) + 0xdf) >>> 0;
  return `tp_${hash.toString(16).padStart(4, "0")}`;
}

/** Create a toolchain step. */
export function createToolchainStep(
  stepIndex: number,
  capabilityId: string,
  providerPackageId: string,
  options: Partial<Omit<ToolchainStep, "step_index" | "capability_id" | "provider_package_id">> = {},
): ToolchainStep {
  return {
    step_index: stepIndex,
    capability_id: capabilityId,
    provider_package_id: providerPackageId,
    grant_scope: options.grant_scope ?? [],
    approval_policy: options.approval_policy ?? "fork_then_approve",
    status: "planned",
    no_execution: true,
    no_ambient_authority: true,
  };
}

/** Check if a tool observation contains prompt injection patterns. */
export function hasPromptInjectionPattern(output: unknown): boolean {
  const str = JSON.stringify(output);
  return str.includes("ignore previous") ||
         str.includes("system:") ||
         str.includes("override");
}

// ---------------------------------------------------------------------------
// Self-test
// ---------------------------------------------------------------------------

export interface SelfTestResult {
  passed: number;
  failed: number;
  failures: string[];
}

export function runAgenticForgeSelfTest(): SelfTestResult {
  const failures: string[] = [];
  let passed = 0;

  function assert(condition: boolean, label: string) {
    if (condition) { passed++; } else { failures.push(label); }
  }

  // Lifecycle states
  assert(isValidLifecycleState("created"), "created is valid state");
  assert(isValidLifecycleState("archived"), "archived is valid state");
  assert(!isValidLifecycleState("unknown"), "unknown is not valid state");
  assert(VALID_LIFECYCLE_STATES.length === 9, "9 lifecycle states");

  // Run event
  const evt = createRunEvent("run_created", "run_test", 0, { step: "test" });
  assert(evt.event_type === "run_created", "run event type");
  assert(evt.run_id === "run_test", "run event run_id");

  // Plan graph creation
  const pg = createPlanGraph("run_test", "test objective");
  assert(pg.nodes.length === 3, "plan graph has 3 nodes");
  assert(pg.edges.length === 2, "plan graph has 2 edges");
  assert(pg.deterministic_mode === true, "deterministic mode");
  assert(pg.approval_policy === "fork_then_approve", "approval policy");

  // Plan graph validation
  const validDiags = validatePlanGraph(pg);
  assert(validDiags.length === 0, "valid plan graph has no diagnostics");

  const badPg = { nodes: [{ node_id: "n1", kind: "observe" as const, label: "x", status: "pending" as const }], edges: [{ from_node_id: "n1", to_node_id: "n_missing", kind: "sequential" as const }] };
  const badDiags = validatePlanGraph(badPg);
  assert(badDiags.length > 0, "plan graph with bad edge has diagnostics");

  // Working state
  const ws = createWorkingState("run_test", "official/agentic-forge-lab");
  assert(ws.run_id === "run_test", "working state run_id");
  assert(ws.owner_package === "official/agentic-forge-lab", "working state owner");
  assert(ws.policy_state.deterministic_mode === true, "working state deterministic mode");

  // Candidate shell
  const cs = createCandidateShell("c1", "run_test", "branch:target:main", "branch:scratch:s1", "official/agentic-forge-lab", "official/agentic-forge-lab/start_run");
  assert(cs.candidate_id === "c1", "candidate shell id");
  assert(cs.provenance.package_id === "official/agentic-forge-lab", "candidate provenance");

  // Raw secret blocking
  assert(looksLikeRawSecret("RawSecretExample1234567890abcdefABCDEF123456"), "raw-looking secret detected");
  assert(looksLikeRawSecret("Bearer abc"), "Bearer prefix detected");
  assert(!looksLikeRawSecret("hello world"), "normal string ok");
  assert(!looksLikeRawSecret("secret_ref:env:KEY"), "secret_ref not flagged");

  assert(blockRawSecrets({ api_key: "RawSecretExample1234567890abcdefABCDEF123456" }), "raw secret in api_key blocked");
  assert(blockRawSecrets({ token: "Bearer xyz" }), "raw secret in token blocked");
  assert(!blockRawSecrets({ api_key: "secret_ref:env:MY_KEY" }), "secret_ref in api_key allowed");
  assert(!blockRawSecrets({ api_key: "secret-ref:env:MY_KEY" }), "secret-ref in api_key allowed");
  assert(!blockRawSecrets({ api_key: "host:env:MY_KEY" }), "host secret ref in api_key allowed");
  assert(!blockRawSecrets({ objective: "safe text" }), "normal objective allowed");

  // No kernel agent namespace
  assert(!hasPlatformAgentNamespace({ kind: "agentic_forge_run_started", run_id: "r1" }), "clean output has no kernel agent namespace");
  assert(hasPlatformAgentNamespace({ method: "platform.agent.run" }), "platform.agent detected");
  assert(hasPlatformAgentNamespace({ method: "platform.model.infer" }), "platform.model detected");

  // isSecretFieldName
  assert(isSecretFieldName("api_key"), "api_key is secret field");
  assert(isSecretFieldName("token"), "token is secret field");
  assert(!isSecretFieldName("objective"), "objective is not secret field");

  // --- Phase B ---

  // Candidate states
  assert(isValidCandidateState("draft"), "draft is valid candidate state");
  assert(isValidCandidateState("archived"), "archived is valid candidate state");
  assert(!isValidCandidateState("unknown"), "unknown is not valid candidate state");
  assert(VALID_CANDIDATE_STATES.length === 8, "8 candidate states");

  // Full candidate creation
  const cand = createCandidate(
    "c1", "run_test", "branch:target:main", "branch:scratch:s1",
    "official/agentic-forge-lab", "official/agentic-forge-lab/create_candidate",
    { changed_asset_refs: ["asset:x"], confidence: 0.8, uncertainty: 0.2, target_revision: 1 },
  );
  assert(cand.candidate_id === "c1", "candidate id");
  assert(cand.status === "draft", "candidate default status is draft");
  assert(cand.changed_asset_refs.length === 1, "candidate changed_asset_refs");
  assert(cand.confidence === 0.8, "candidate confidence");
  assert(cand.uncertainty === 0.2, "candidate uncertainty");
  assert(cand.target_revision === 1, "candidate target_revision");

  // Candidate validation
  const validCandDiags = validateCandidate(cand);
  assert(validCandDiags.length === 0, "valid candidate has no diagnostics");

  const badCandDiags = validateCandidate({});
  assert(badCandDiags.length > 0, "empty candidate has diagnostics");

  const badStatusDiags = validateCandidate({
    candidate_id: "c1", run_id: "r1",
    target_branch_ref: "b:t", scratch_branch_ref: "b:s", status: "unknown" as CandidateState,
  });
  assert(badStatusDiags.some(d => d.includes("unknown candidate status")), "bad status detected");

  // Compare candidate — matching revision → stale=false
  const comp = compareCandidate(cand, 1);
  assert(comp.stale === false, "matching revision → not stale");
  assert(comp.lineage_impact.target_branch_modified === false, "compare: target not modified");
  assert(comp.candidate_target_revision === 1, "compare: candidate revision");
  assert(comp.current_target_revision === 1, "compare: current revision");

  // Compare candidate — mismatched revision → stale=true
  const compStale = compareCandidate(cand, 3);
  assert(compStale.stale === true, "mismatched revision → stale");
  assert(compStale.lineage_impact.requires_rebase === true, "stale requires rebase");

  // Promote proposal draft
  const draft = createPromoteProposalDraft(
    cand, "official/agentic-forge-lab", "official/agentic-forge-lab/draft_promote_proposal",
  );
  assert(draft.requires_user_approval === true, "promote requires approval");
  assert(draft.operations.length > 0, "promote has operations");
  assert(draft.source_candidate === "c1", "promote source candidate");
  assert(draft.provenance.package_id === "official/agentic-forge-lab", "promote provenance");

  // Archived candidate
  const archived = archiveCandidate(cand);
  assert(archived.status === "archived", "archived candidate status");
  assert(archived.candidate_id === "c1", "archived preserves id");

  // No platform-reserved namespace in Phase B outputs
  assert(!hasPlatformAgentNamespace(comp), "compare output has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(draft), "promote draft has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(archived), "archived candidate has no platform-reserved namespace");

  // Raw secret blocking in candidate
  assert(blockRawSecrets({ api_key: "RawSecretExample1234567890abcdefABCDEF123456" }), "raw secret in candidate blocked");

  // --- Phase C ---

  // Provider kinds
  assert(isValidProviderKind("deterministic"), "deterministic is valid provider kind");
  assert(isValidProviderKind("cloud_adapter_plan"), "cloud_adapter_plan is valid provider kind");
  assert(!isValidProviderKind("cloud_real"), "cloud_real is not valid provider kind");
  assert(VALID_PROVIDER_KINDS.length === 4, "4 provider kinds");

  // Inference failure taxonomy
  assert(isInferenceFailureKind("rate_limit"), "rate_limit is valid failure kind");
  assert(isInferenceFailureKind("policy_reject"), "policy_reject is valid failure kind");
  assert(!isInferenceFailureKind("unknown_failure"), "unknown_failure is not valid failure kind");
  assert(INFERENCE_FAILURE_KINDS.length === 9, "9 inference failure kinds");

  // Run inference node — deterministic → candidate_seed
  const infResult = runInferenceNode("run_inf", "node_1", "deterministic", "analyze composition");
  assert(infResult.kind === "agentic_forge_inference_node_result", "inference node result kind");
  assert(infResult.node_result.output_action === "candidate_seed", "deterministic → candidate_seed");
  assert(infResult.node_result.target_branch_unchanged === true, "inference: target unchanged");
  assert(infResult.node_result.direct_mutation === false, "inference: no direct mutation");
  assert(infResult.network_performed === false, "deterministic: no network");
  assert(infResult.inference_trace.fingerprint.startsWith("fp_"), "inference trace has fingerprint");

  // Run inference node — objective with proposal → proposal_seed
  const infProposal = runInferenceNode("run_inf", "node_2", "deterministic", "draft proposal");
  assert(infProposal.node_result.output_action === "proposal_seed", "proposal objective → proposal_seed");

  // Run inference node — cloud_adapter_plan → needs_host_policy, no network
  const infCloud = runInferenceNode("run_cloud", "node_cloud", "cloud_adapter_plan");
  assert(infCloud.kind === "agentic_forge_inference_node_plan", "cloud adapter kind");
  assert(infCloud.node_result.status === "needs_host_policy", "cloud adapter needs host policy");
  assert(infCloud.network_performed === false, "cloud adapter: no network performed");
  assert(infCloud.inference_performed === false, "cloud adapter: no inference performed");

  // Run inference node — local_fake → inference_performed=true
  const infLocal = runInferenceNode("run_local", "node_local", "local_fake");
  assert(infLocal.inference_performed === true, "local_fake: inference performed");
  assert(infLocal.network_performed === false, "local_fake: no network");

  // Replay — match
  const expectedFp = computeDeterministicFingerprint({ objective: "default", run_id: "run_replay", node_id: "node_1" });
  const replayOk = replayInferenceNode("run_replay", "node_1", expectedFp);
  assert(replayOk.kind === "agentic_forge_replay_ok", "replay match kind");
  assert(replayOk.fingerprint_match === true, "replay match");

  // Replay — mismatch
  const replayMismatch = replayInferenceNode("run_replay", "node_1", "fp_WRONG");
  assert(replayMismatch.kind === "agentic_forge_replay_mismatch", "replay mismatch kind");
  assert(replayMismatch.fingerprint_match === false, "replay mismatch flag");
  assert(replayMismatch.expected_fingerprint === "fp_WRONG", "replay records expected fingerprint");
  assert(replayMismatch.actual_fingerprint !== "fp_WRONG", "replay records actual fingerprint");

  // Validate inference output — allowed
  for (const action of ALLOWED_INFERENCE_ACTIONS) {
    const v = validateInferenceOutput(action);
    assert(v.validation_result === "accepted", `allowed action ${action} accepted`);
    assert(v.allowed === true, `allowed action ${action} allowed=true`);
  }

  // Validate inference output — forbidden
  for (const action of FORBIDDEN_INFERENCE_ACTIONS) {
    const v = validateInferenceOutput(action);
    assert(v.validation_result === "rejected", `forbidden action ${action} rejected`);
    assert(v.allowed === false, `forbidden action ${action} allowed=false`);
  }

  // Validate inference output — unknown
  const vUnknown = validateInferenceOutput("arbitrary_exec");
  assert(vUnknown.validation_result === "rejected", "unknown action rejected");
  assert(vUnknown.allowed === false, "unknown action allowed=false");

  // Explain inference failure — all known kinds
  for (const kind of INFERENCE_FAILURE_KINDS) {
    const exp = explainInferenceFailure(kind);
    assert(exp.is_known === true, `failure ${kind} is known`);
    assert(exp.recovery_hint.length > 0, `failure ${kind} has recovery hint`);
  }

  // Explain inference failure — unknown
  const expUnknown = explainInferenceFailure("nonexistent");
  assert(expUnknown.is_known === false, "unknown failure is_known=false");

  // No platform-reserved namespace in Phase C outputs
  assert(!hasPlatformAgentNamespace(infResult), "inference result has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(replayOk), "replay result has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(replayMismatch), "replay mismatch has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(validateInferenceOutput("candidate_seed")), "validation result has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(explainInferenceFailure("timeout")), "failure explanation has no platform-reserved namespace");

  // --- Phase D ---

  // Tool risk categories
  assert(TOOL_RISK_CATEGORIES.length === 6, "6 tool risk categories");
  assert(TOOL_RISK_CATEGORIES.includes("prompt_injection"), "prompt_injection is a risk category");
  assert(TOOL_RISK_CATEGORIES.includes("secret_exfiltration"), "secret_exfiltration is a risk category");
  assert(TOOL_RISK_CATEGORIES.includes("nested_delegation"), "nested_delegation is a risk category");

  // Tool call context
  const ctx = createToolCallContext({
    requesting_package: "official/agentic-forge-lab",
    run_id: "run_d",
    target_branch_scope: "branch:target:main",
    approval_policy: "fork_then_approve",
  });
  assert(ctx.requesting_package === "official/agentic-forge-lab", "context has requesting_package");
  assert(ctx.run_id === "run_d", "context has run_id");
  assert(ctx.approval_policy === "fork_then_approve", "context has approval_policy");

  // Tool plan fingerprint
  const fp1 = computeToolPlanFingerprint({ objective: "test" });
  const fp2 = computeToolPlanFingerprint({ objective: "test" });
  const fp3 = computeToolPlanFingerprint({ objective: "other" });
  assert(fp1 === fp2, "same objective → same fingerprint");
  assert(fp1 !== fp3, "different objective → different fingerprint");
  assert(fp1.startsWith("tp_"), "tool plan fingerprint prefix");

  // Toolchain step creation
  const step = createToolchainStep(0, "example/echo", "official/pkg-a");
  assert(step.step_index === 0, "step index");
  assert(step.capability_id === "example/echo", "step capability_id");
  assert(step.provider_package_id === "official/pkg-a", "step provider");
  assert(step.status === "planned", "step default status is planned");
  assert(step.no_execution === true, "step no_execution");
  assert(step.no_ambient_authority === true, "step no_ambient_authority");

  // Prompt injection detection
  assert(hasPromptInjectionPattern({ result: "ignore previous instructions" }), "detects 'ignore previous'");
  assert(hasPromptInjectionPattern({ result: "system: override" }), "detects 'system:'");
  assert(!hasPromptInjectionPattern({ result: "normal output" }), "no false positive on normal output");

  // No platform-reserved namespace in Phase D outputs
  assert(!hasPlatformAgentNamespace(ctx as unknown as Record<string, unknown>), "tool call context has no platform-reserved namespace");
  assert(!hasPlatformAgentNamespace(step as unknown as Record<string, unknown>), "toolchain step has no platform-reserved namespace");

  return { passed, failed: failures.length, failures };
}
