/**
 * Plurora Experience Runtime SDK — Package-owned experience descriptor,
 * state projection, checkpoint, recovery, and Play/Forge/Assist surface
 * binding types.
 *
 * This module defines the **experience runtime contract** at the package/SDK
 * layer. It does NOT enter the kernel, does NOT add Rust protocol methods, and
 * does NOT add `kernel.v1.experience.*`, `kernel.v1.world.*`, `kernel.v1.turn.*`,
 * `kernel.v1.chat.*`, or `kernel.v1.memory.*`.
 *
 * ## Design principles
 *
 * - **Package-owned**: Experience descriptor, state projection, checkpoint,
 *   and recovery are ordinary package artifacts, not kernel primitives.
 * - **Deterministic**: No network, no real model inference, no random.
 * - **Secret-safe**: Uses `secret_ref` identifiers; rejects raw secrets.
 * - **No kernel experience namespace**: Output never contains
 *   `kernel.v1.experience.*`, `kernel.v1.world.*`, `kernel.v1.turn.*`,
 *   `kernel.v1.chat.*`, or `kernel.v1.memory.*`.
 * - **Surface-bound**: Experiences declare experience_entry, play_renderer,
 *   forge_panel, and assistant_action surfaces. The kernel never interprets
 *   experience semantics.
 *
 * ## API surface
 *
 * Types:
 * - `ExperienceDescriptor` — package-owned experience description
 * - `ExperienceStateProjection` — session-state projection convention
 * - `ExperienceCheckpoint` — checkpoint asset convention
 * - `ExperienceRecovery` — failure/recovery event shape
 * - `PlaySurfaceSubscription` — Play surface state subscription pattern
 * - `ForgeBinding` — Forge surface binding to experience session
 * - `AssistBinding` — Assist surface binding to experience session
 * - `ExperienceLifecycleState` — lifecycle states for an experience run
 * - `CheckpointFormat` — checkpoint format enum
 * - `RecoveryStrategy` — recovery strategy enum
 *
 * Constructors / Validators:
 * - `createExperienceDescriptor()` — build an experience descriptor
 * - `validateExperienceDescriptor()` — validate descriptor shape
 * - `createStateProjection()` — build a state projection
 * - `createCheckpoint()` — build a checkpoint
 * - `inspectCheckpoint()` — inspect checkpoint shape
 * - `createRecovery()` — build a recovery event shape
 * - `draftRecoveryPlan()` — draft a deterministic recovery plan
 * - `createPlaySurfaceSubscription()` — build a play surface subscription
 * - `createForgeBinding()` — build a forge binding
 * - `createAssistBinding()` — build an assist binding
 *
 * Safety helpers:
 * - `blockRawSecrets()` — check for raw-secret-like content
 * - `looksLikeRawSecret()` — heuristic check for raw secret values
 * - `isSecretFieldName()` — check for secret field names
 * - `hasKernelExperienceNamespace()` — check for forbidden kernel namespace
 *
 * Self-test:
 * - `runExperienceRuntimeSelfTest()` — pure-TS self-test
 */

// ---------------------------------------------------------------------------
// Lifecycle states
// ---------------------------------------------------------------------------

/**
 * Lifecycle states for an experience run.
 * Package-owned — the kernel has no experience state machine.
 */
export type ExperienceLifecycleState =
  | "created"
  | "running"
  | "paused"
  | "checkpointed"
  | "recovering"
  | "recovered"
  | "failed"
  | "completed"
  | "archived";

export const EXPERIENCE_LIFECYCLE_STATES: ExperienceLifecycleState[] = [
  "created",
  "running",
  "paused",
  "checkpointed",
  "recovering",
  "recovered",
  "failed",
  "completed",
  "archived",
];

// ---------------------------------------------------------------------------
// Checkpoint formats
// ---------------------------------------------------------------------------

export type CheckpointFormat =
  | "snapshot"
  | "incremental"
  | "delta";

export const CHECKPOINT_FORMATS: CheckpointFormat[] = [
  "snapshot",
  "incremental",
  "delta",
];

// ---------------------------------------------------------------------------
// Recovery strategies
// ---------------------------------------------------------------------------

export type RecoveryStrategy =
  | "restore_last_checkpoint"
  | "replay_from_checkpoint"
  | "restart_session"
  | "manual_intervention"
  | "discard_and_reset";

export const RECOVERY_STRATEGIES: RecoveryStrategy[] = [
  "restore_last_checkpoint",
  "replay_from_checkpoint",
  "restart_session",
  "manual_intervention",
  "discard_and_reset",
];

// ---------------------------------------------------------------------------
// Experience Descriptor
// ---------------------------------------------------------------------------

export interface ExperienceDescriptor {
  kind: "experience_runtime_descriptor";
  package_id: string;
  version: string;
  display_name?: string;
  description?: string;
  surfaces: {
    experience_entry: string;
    play_renderer: string;
    forge_panel: string;
    assistant_action: string;
  };
  capabilities: {
    describe_contract: string;
    create_checkpoint: string;
    inspect_checkpoint: string;
    draft_recovery: string;
    bind_agent_run: string;
  };
  lifecycle_states: ExperienceLifecycleState[];
  checkpoint_formats: CheckpointFormat[];
  recovery_strategies: RecoveryStrategy[];
  package_kind: "ordinary";
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createExperienceDescriptor(options: {
  package_id: string;
  version?: string;
  display_name?: string;
  description?: string;
  surfaces: {
    experience_entry: string;
    play_renderer: string;
    forge_panel: string;
    assistant_action: string;
  };
  capabilities: {
    describe_contract: string;
    create_checkpoint: string;
    inspect_checkpoint: string;
    draft_recovery: string;
    bind_agent_run: string;
  };
}): ExperienceDescriptor {
  return {
    kind: "experience_runtime_descriptor",
    package_id: options.package_id,
    version: options.version ?? "0.1.0",
    display_name: options.display_name,
    description: options.description,
    surfaces: options.surfaces,
    capabilities: options.capabilities,
    lifecycle_states: [...EXPERIENCE_LIFECYCLE_STATES],
    checkpoint_formats: [...CHECKPOINT_FORMATS],
    recovery_strategies: [...RECOVERY_STRATEGIES],
    package_kind: "ordinary",
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capabilities.describe_contract,
    },
  };
}

export function validateExperienceDescriptor(d: unknown): string[] {
  const errors: string[] = [];
  if (typeof d !== "object" || d === null) {
    errors.push("descriptor must be an object");
    return errors;
  }
  const obj = d as Record<string, unknown>;
  if (obj.kind !== "experience_runtime_descriptor") errors.push("kind must be experience_runtime_descriptor");
  if (typeof obj.package_id !== "string" || !obj.package_id) errors.push("package_id must be a non-empty string");
  if (typeof obj.version !== "string") errors.push("version must be a string");
  if (obj.package_kind !== "ordinary") errors.push("package_kind must be ordinary");
  if (obj.inference_performed !== false) errors.push("inference_performed must be false");
  if (obj.network_performed !== false) errors.push("network_performed must be false");

  const surfaces = obj.surfaces as Record<string, unknown> | undefined;
  if (!surfaces) {
    errors.push("surfaces is required");
  } else {
    for (const slot of ["experience_entry", "play_renderer", "forge_panel", "assistant_action"]) {
      if (typeof surfaces[slot] !== "string" || !surfaces[slot]) {
        errors.push(`surfaces.${slot} must be a non-empty string`);
      }
    }
  }

  const caps = obj.capabilities as Record<string, unknown> | undefined;
  if (!caps) {
    errors.push("capabilities is required");
  } else {
    for (const cap of ["describe_contract", "create_checkpoint", "inspect_checkpoint", "draft_recovery", "bind_agent_run"]) {
      if (typeof caps[cap] !== "string" || !caps[cap]) {
        errors.push(`capabilities.${cap} must be a non-empty string`);
      }
    }
  }

  // Check for forbidden kernel namespace
  const str = JSON.stringify(d);
  for (const ns of ["kernel.v1.experience.", "kernel.v1.world.", "kernel.v1.turn.", "kernel.v1.chat.", "kernel.v1.memory."]) {
    if (str.includes(ns)) errors.push(`must not contain ${ns}`);
  }

  return errors;
}

// ---------------------------------------------------------------------------
// State Projection
// ---------------------------------------------------------------------------

export interface ExperienceStateProjection {
  kind: "experience_state_projection";
  package_id: string;
  session_id: string;
  state: Record<string, unknown>;
  projection_id: string;
  branch_ref?: string;
  checkpoint_ref?: string;
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createStateProjection(options: {
  package_id: string;
  session_id: string;
  state: Record<string, unknown>;
  projection_id?: string;
  branch_ref?: string;
  checkpoint_ref?: string;
  capability_id: string;
}): ExperienceStateProjection {
  return {
    kind: "experience_state_projection",
    package_id: options.package_id,
    session_id: options.session_id,
    state: options.state,
    projection_id: options.projection_id ?? `projection:${options.package_id}:${options.session_id}`,
    branch_ref: options.branch_ref,
    checkpoint_ref: options.checkpoint_ref,
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

// ---------------------------------------------------------------------------
// Checkpoint
// ---------------------------------------------------------------------------

export interface ExperienceCheckpoint {
  kind: "experience_checkpoint";
  package_id: string;
  session_id: string;
  checkpoint_id: string;
  format: CheckpointFormat;
  state_snapshot: Record<string, unknown>;
  asset_refs: string[];
  branch_ref: string;
  sequence: number;
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createCheckpoint(options: {
  package_id: string;
  session_id: string;
  checkpoint_id?: string;
  format?: CheckpointFormat;
  state_snapshot: Record<string, unknown>;
  asset_refs?: string[];
  branch_ref?: string;
  sequence?: number;
  capability_id: string;
}): ExperienceCheckpoint {
  return {
    kind: "experience_checkpoint",
    package_id: options.package_id,
    session_id: options.session_id,
    checkpoint_id: options.checkpoint_id ?? `checkpoint:${options.package_id}:${Date.now()}`,
    format: options.format ?? "snapshot",
    state_snapshot: options.state_snapshot,
    asset_refs: options.asset_refs ?? [],
    branch_ref: options.branch_ref ?? `branch:${options.package_id}:default`,
    sequence: options.sequence ?? 1,
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

export function inspectCheckpoint(checkpoint: ExperienceCheckpoint): {
  valid: boolean;
  errors: string[];
  summary: string;
} {
  const errors: string[] = [];
  if (checkpoint.kind !== "experience_checkpoint") errors.push("kind must be experience_checkpoint");
  if (!checkpoint.checkpoint_id) errors.push("checkpoint_id must be non-empty");
  if (!checkpoint.session_id) errors.push("session_id must be non-empty");
  if (!checkpoint.state_snapshot || typeof checkpoint.state_snapshot !== "object") {
    errors.push("state_snapshot must be an object");
  }
  if (!CHECKPOINT_FORMATS.includes(checkpoint.format)) errors.push(`invalid checkpoint format: ${checkpoint.format}`);
  if (typeof checkpoint.sequence !== "number" || checkpoint.sequence < 1) {
    errors.push("sequence must be a positive number");
  }
  return {
    valid: errors.length === 0,
    errors,
    summary: errors.length === 0
      ? `Checkpoint ${checkpoint.checkpoint_id} valid (format=${checkpoint.format}, sequence=${checkpoint.sequence}, assets=${checkpoint.asset_refs.length})`
      : `Checkpoint ${checkpoint.checkpoint_id} invalid: ${errors.join("; ")}`,
  };
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

export interface ExperienceRecovery {
  kind: "experience_recovery";
  package_id: string;
  session_id: string;
  failure_kind: string;
  failure_detail: string;
  last_checkpoint_ref: string | null;
  recovery_strategy: RecoveryStrategy;
  recovery_plan: {
    steps: string[];
    requires_user_approval: boolean;
    affected_asset_refs: string[];
  };
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export const EXPERIENCE_FAILURE_KINDS = [
  "state_corruption",
  "checkpoint_missing",
  "checkpoint_corrupt",
  "capability_failure",
  "session_expired",
  "resource_exhausted",
  "package_error",
  "unknown",
] as const;

export function createRecovery(options: {
  package_id: string;
  session_id: string;
  failure_kind: string;
  failure_detail?: string;
  last_checkpoint_ref?: string | null;
  recovery_strategy?: RecoveryStrategy;
  capability_id: string;
}): ExperienceRecovery {
  const strategy = options.recovery_strategy ?? "restore_last_checkpoint";
  const steps: string[] = [];
  let requiresApproval = false;

  switch (strategy) {
    case "restore_last_checkpoint":
      steps.push("locate last checkpoint asset", "restore state from checkpoint", "resume from checkpoint sequence");
      requiresApproval = false;
      break;
    case "replay_from_checkpoint":
      steps.push("locate last checkpoint", "restore state", "replay events after checkpoint", "verify replay consistency");
      requiresApproval = true;
      break;
    case "restart_session":
      steps.push("create new session", "re-initialize state from descriptor", "notify user of restart");
      requiresApproval = true;
      break;
    case "manual_intervention":
      steps.push("pause experience", "present failure breadcrumbs to user", "await user action");
      requiresApproval = true;
      break;
    case "discard_and_reset":
      steps.push("discard current state", "reset to initial descriptor state", "archive failed session");
      requiresApproval = true;
      break;
  }

  return {
    kind: "experience_recovery",
    package_id: options.package_id,
    session_id: options.session_id,
    failure_kind: options.failure_kind,
    failure_detail: options.failure_detail ?? "",
    last_checkpoint_ref: options.last_checkpoint_ref ?? null,
    recovery_strategy: strategy,
    recovery_plan: {
      steps,
      requires_user_approval: requiresApproval,
      affected_asset_refs: [],
    },
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

export function draftRecoveryPlan(options: {
  package_id: string;
  session_id: string;
  failure_kind: string;
  last_checkpoint_ref?: string | null;
  capability_id: string;
}): {
  kind: "experience_recovery_plan";
  recommended_strategy: RecoveryStrategy;
  available_strategies: RecoveryStrategy[];
  plan: {
    steps: string[];
    requires_user_approval: boolean;
    checkpoint_available: boolean;
  };
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
} {
  const checkpointAvailable = options.last_checkpoint_ref != null;
  const recommended: RecoveryStrategy = checkpointAvailable
    ? "restore_last_checkpoint"
    : "restart_session";

  const recovery = createRecovery({
    package_id: options.package_id,
    session_id: options.session_id,
    failure_kind: options.failure_kind,
    last_checkpoint_ref: options.last_checkpoint_ref,
    recovery_strategy: recommended,
    capability_id: options.capability_id,
  });

  return {
    kind: "experience_recovery_plan",
    recommended_strategy: recommended,
    available_strategies: [...RECOVERY_STRATEGIES],
    plan: {
      steps: recovery.recovery_plan.steps,
      requires_user_approval: recovery.recovery_plan.requires_user_approval,
      checkpoint_available: checkpointAvailable,
    },
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

// ---------------------------------------------------------------------------
// Play Surface Subscription
// ---------------------------------------------------------------------------

export interface PlaySurfaceSubscription {
  kind: "experience_play_surface_subscription";
  package_id: string;
  session_id: string;
  surface_id: string;
  subscription_type: "state_change" | "checkpoint" | "lifecycle";
  filter?: Record<string, unknown>;
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createPlaySurfaceSubscription(options: {
  package_id: string;
  session_id: string;
  surface_id: string;
  subscription_type?: "state_change" | "checkpoint" | "lifecycle";
  filter?: Record<string, unknown>;
  capability_id: string;
}): PlaySurfaceSubscription {
  return {
    kind: "experience_play_surface_subscription",
    package_id: options.package_id,
    session_id: options.session_id,
    surface_id: options.surface_id,
    subscription_type: options.subscription_type ?? "state_change",
    filter: options.filter,
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

// ---------------------------------------------------------------------------
// Forge Binding
// ---------------------------------------------------------------------------

export interface ForgeBinding {
  kind: "experience_forge_binding";
  package_id: string;
  session_id: string;
  surface_id: string;
  inspect_capabilities: string[];
  proposal_capabilities: string[];
  branch_aware: true;
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createForgeBinding(options: {
  package_id: string;
  session_id: string;
  surface_id: string;
  inspect_capabilities?: string[];
  proposal_capabilities?: string[];
  capability_id: string;
}): ForgeBinding {
  return {
    kind: "experience_forge_binding",
    package_id: options.package_id,
    session_id: options.session_id,
    surface_id: options.surface_id,
    inspect_capabilities: options.inspect_capabilities ?? [],
    proposal_capabilities: options.proposal_capabilities ?? [],
    branch_aware: true,
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

// ---------------------------------------------------------------------------
// Assist Binding
// ---------------------------------------------------------------------------

export interface AssistBinding {
  kind: "experience_assist_binding";
  package_id: string;
  session_id: string;
  surface_id: string;
  action_capabilities: string[];
  approval_policy: "fork_then_approve";
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

export function createAssistBinding(options: {
  package_id: string;
  session_id: string;
  surface_id: string;
  action_capabilities?: string[];
  capability_id: string;
}): AssistBinding {
  return {
    kind: "experience_assist_binding",
    package_id: options.package_id,
    session_id: options.session_id,
    surface_id: options.surface_id,
    action_capabilities: options.action_capabilities ?? [],
    approval_policy: "fork_then_approve",
    inference_performed: false,
    network_performed: false,
    provenance: {
      package_id: options.package_id,
      capability_id: options.capability_id,
    },
  };
}

// ---------------------------------------------------------------------------
// Agent Run Binding
// ---------------------------------------------------------------------------

export interface AgentRunBinding {
  kind: "experience_agent_run_binding";
  package_id: string;
  session_id: string;
  agent_package_id: string;
  run_capabilities: string[];
  scoped_to_branch: true;
  target_branch_ref: string;
  scratch_branch_ref: string;
  inference_performed: false;
  network_performed: false;
  provenance: {
    package_id: string;
    capability_id: string;
  };
}

// ---------------------------------------------------------------------------
// Secret safety helpers
// ---------------------------------------------------------------------------

const SECRET_FIELD_NAMES = [
  "api_key", "secret", "token", "password", "private_key",
  "access_token", "refresh_token", "auth_token",
];

const SECRET_VALUE_PREFIXES = ["sk-", "Bearer ", "bearer "];

export function isSecretFieldName(name: string): boolean {
  const lower = name.toLowerCase();
  return SECRET_FIELD_NAMES.some((s) => lower === s || lower.includes(s));
}

export function looksLikeRawSecret(value: string): boolean {
  for (const prefix of SECRET_VALUE_PREFIXES) {
    if (value.startsWith(prefix)) return true;
  }
  if (value.length >= 40) {
    const hasUpper = /[A-Z]/.test(value);
    const hasLower = /[a-z]/.test(value);
    const hasDigit = /[0-9]/.test(value);
    if (hasUpper && hasLower && hasDigit) return true;
  }
  return false;
}

function isSecretRefValue(value: string): boolean {
  return (
    value.startsWith("secret_ref:") ||
    value.startsWith("secretRef:") ||
    value.startsWith("secret-ref:") ||
    value.startsWith("host:")
  );
}

function containsRawSecret(value: unknown): boolean {
  if (typeof value === "string") {
    if (isSecretRefValue(value)) return false;
    return looksLikeRawSecret(value);
  }
  if (Array.isArray(value)) {
    return value.some(containsRawSecret);
  }
  if (typeof value === "object" && value !== null) {
    for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
      if (isSecretFieldName(key)) {
        if (typeof val === "string" && !isSecretRefValue(val)) return true;
        if (val !== null && val !== undefined && typeof val !== "string") return true;
      }
      if (typeof val === "string" && looksLikeRawSecret(val)) return true;
      if (containsRawSecret(val)) return true;
    }
  }
  return false;
}

export function blockRawSecrets(input: unknown): {
  clean: boolean;
  redaction_state: "clean" | "unsafe_blocked";
  reason?: string;
} {
  if (containsRawSecret(input)) {
    return {
      clean: false,
      redaction_state: "unsafe_blocked",
      reason: "input contains raw-secret-like content; use secret_ref references instead",
    };
  }
  return { clean: true, redaction_state: "clean" };
}

// ---------------------------------------------------------------------------
// Kernel namespace safety
// ---------------------------------------------------------------------------

const FORBIDDEN_NAMESPACES = [
  "kernel.v1.experience.",
  "kernel.v1.world.",
  "kernel.v1.turn.",
  "kernel.v1.chat.",
  "kernel.v1.memory.",
];

export function hasKernelExperienceNamespace(value: unknown): boolean {
  const str = JSON.stringify(value);
  return FORBIDDEN_NAMESPACES.some((ns) => str.includes(ns));
}

// ---------------------------------------------------------------------------
// Self-test
// ---------------------------------------------------------------------------

export function runExperienceRuntimeSelfTest(): {
  passed: number;
  failed: number;
  results: { label: string; ok: boolean; detail?: string }[];
} {
  const results: { label: string; ok: boolean; detail?: string }[] = [];
  let passed = 0;
  let failed = 0;

  function assert(label: string, condition: boolean, detail?: string) {
    if (condition) {
      passed++;
      results.push({ label, ok: true });
    } else {
      failed++;
      results.push({ label, ok: false, detail });
    }
  }

  // 1. Descriptor creation
  const desc = createExperienceDescriptor({
    package_id: "official/experience-runtime-lab",
    surfaces: {
      experience_entry: "official/experience-runtime-lab/entry",
      play_renderer: "official/experience-runtime-lab/play",
      forge_panel: "official/experience-runtime-lab/forge",
      assistant_action: "official/experience-runtime-lab/assist",
    },
    capabilities: {
      describe_contract: "official/experience-runtime-lab/describe_contract",
      create_checkpoint: "official/experience-runtime-lab/create_checkpoint",
      inspect_checkpoint: "official/experience-runtime-lab/inspect_checkpoint",
      draft_recovery: "official/experience-runtime-lab/draft_recovery",
      bind_agent_run: "official/experience-runtime-lab/bind_agent_run",
    },
  });
  assert("descriptor kind", desc.kind === "experience_runtime_descriptor");
  assert("descriptor package_kind", desc.package_kind === "ordinary");
  assert("descriptor has 4 surfaces", Object.keys(desc.surfaces).length === 4);
  assert("descriptor has 5 capabilities", Object.keys(desc.capabilities).length === 5);
  assert("descriptor inference_performed=false", desc.inference_performed === false);
  assert("descriptor network_performed=false", desc.network_performed === false);

  // 2. Descriptor validation
  const validationErrors = validateExperienceDescriptor(desc);
  assert("descriptor valid", validationErrors.length === 0, validationErrors.join("; "));

  const badDesc = { kind: "wrong", package_id: "", surfaces: {} };
  const badErrors = validateExperienceDescriptor(badDesc);
  assert("bad descriptor invalid", badErrors.length > 0);

  // 3. State projection
  const proj = createStateProjection({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    state: { health: 100, step_index: 1 },
    capability_id: "official/experience-runtime-lab/describe_contract",
  });
  assert("projection kind", proj.kind === "experience_state_projection");
  assert("projection has state", proj.state.health === 100);
  assert("projection inference_performed=false", proj.inference_performed === false);

  // 4. Checkpoint
  const cp = createCheckpoint({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    state_snapshot: { health: 100, step_index: 5 },
    asset_refs: ["asset:module:seed"],
    capability_id: "official/experience-runtime-lab/create_checkpoint",
  });
  assert("checkpoint kind", cp.kind === "experience_checkpoint");
  assert("checkpoint has state", cp.state_snapshot.health === 100);
  assert("checkpoint has assets", cp.asset_refs.length === 1);
  assert("checkpoint inference_performed=false", cp.inference_performed === false);

  // 5. Checkpoint inspection
  const cpInspect = inspectCheckpoint(cp);
  assert("checkpoint inspect valid", cpInspect.valid);
  assert("checkpoint inspect no errors", cpInspect.errors.length === 0);

  // 6. Recovery
  const rec = createRecovery({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    failure_kind: "state_corruption",
    last_checkpoint_ref: "checkpoint:123",
    capability_id: "official/experience-runtime-lab/draft_recovery",
  });
  assert("recovery kind", rec.kind === "experience_recovery");
  assert("recovery has steps", rec.recovery_plan.steps.length > 0);
  assert("recovery inference_performed=false", rec.inference_performed === false);
  assert("recovery last_checkpoint_ref set", rec.last_checkpoint_ref === "checkpoint:123");

  // 7. Recovery plan
  const plan = draftRecoveryPlan({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    failure_kind: "checkpoint_missing",
    last_checkpoint_ref: null,
    capability_id: "official/experience-runtime-lab/draft_recovery",
  });
  assert("recovery plan kind", plan.kind === "experience_recovery_plan");
  assert("recovery plan recommended", plan.recommended_strategy === "restart_session");
  assert("recovery plan checkpoint_available=false", plan.plan.checkpoint_available === false);
  assert("recovery plan inference_performed=false", plan.inference_performed === false);

  // 8. Play surface subscription
  const sub = createPlaySurfaceSubscription({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    surface_id: "official/experience-runtime-lab/play",
    capability_id: "official/experience-runtime-lab/describe_contract",
  });
  assert("subscription kind", sub.kind === "experience_play_surface_subscription");
  assert("subscription default type", sub.subscription_type === "state_change");
  assert("subscription inference_performed=false", sub.inference_performed === false);

  // 9. Forge binding
  const forge = createForgeBinding({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    surface_id: "official/experience-runtime-lab/forge",
    capability_id: "official/experience-runtime-lab/describe_contract",
  });
  assert("forge binding kind", forge.kind === "experience_forge_binding");
  assert("forge binding branch_aware", forge.branch_aware === true);
  assert("forge inference_performed=false", forge.inference_performed === false);

  // 10. Assist binding
  const assist = createAssistBinding({
    package_id: "official/experience-runtime-lab",
    session_id: "session_test",
    surface_id: "official/experience-runtime-lab/assist",
    capability_id: "official/experience-runtime-lab/describe_contract",
  });
  assert("assist binding kind", assist.kind === "experience_assist_binding");
  assert("assist approval_policy", assist.approval_policy === "fork_then_approve");
  assert("assist inference_performed=false", assist.inference_performed === false);

  // 11. Secret safety
  const safeInput = blockRawSecrets({ objective: "safe" });
  assert("safe input clean", safeInput.clean);

  const unsafeInput = blockRawSecrets({ api_key: "RawSecretExample1234567890abcdefABCDEF123456" });
  assert("raw secret blocked", !unsafeInput.clean);
  assert("raw secret unsafe_blocked", unsafeInput.redaction_state === "unsafe_blocked");

  const secretRefInput = blockRawSecrets({ api_key: "secret_ref:env:MY_KEY" });
  assert("secret_ref allowed", secretRefInput.clean);

  // 12. Kernel namespace safety
  const safeOutput = hasKernelExperienceNamespace(desc);
  assert("descriptor no kernel namespace", !safeOutput);

  const badOutput = hasKernelExperienceNamespace({ ref: "kernel.v1.experience.run" });
  assert("detects kernel.v1.experience namespace", badOutput);

  // 13. Lifecycle states
  assert("9 lifecycle states", EXPERIENCE_LIFECYCLE_STATES.length === 9);
  assert("created is first", EXPERIENCE_LIFECYCLE_STATES[0] === "created");
  assert("archived is last", EXPERIENCE_LIFECYCLE_STATES[8] === "archived");

  // 14. Checkpoint formats
  assert("3 checkpoint formats", CHECKPOINT_FORMATS.length === 3);

  // 15. Recovery strategies
  assert("5 recovery strategies", RECOVERY_STRATEGIES.length === 5);

  // 16. No kernel namespace in any output
  const allOutputs = [desc, proj, cp, rec, plan, sub, forge, assist];
  for (const output of allOutputs) {
    const str = JSON.stringify(output);
    for (const ns of FORBIDDEN_NAMESPACES) {
      assert(`no ${ns} in output`, !str.includes(ns), `found ${ns}`);
    }
  }

  return { passed, failed, results };
}
