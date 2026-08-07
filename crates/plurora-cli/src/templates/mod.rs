pub(crate) const PYTHON_SUBPROCESS_TEMPLATE: &str = r#"#!/usr/bin/env python3
import json
import sys


def respond(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "package.handshake":
        respond({"jsonrpc": "2.0", "id": request.get("id"), "result": {"ready": True, "package_protocol_version": "0.1.0"}})
    elif method == "capability.invoke":
        params = request.get("params", {})
        respond({"jsonrpc": "2.0", "id": request.get("id"), "result": {"output": params.get("input")}})
    else:
        respond({"jsonrpc": "2.0", "id": request.get("id"), "error": {"code": "unknown_method", "message": method}})
"#;

pub(crate) fn typescript_subprocess_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    if (capability_id !== "{id}/echo") {{
      throw new Error(`unsupported capability: ${{capability_id}}`);
    }}
    return input ?? null;
  }},
}});
"#
    )
}

pub(crate) fn typescript_package_json(id: &str) -> String {
    format!(
        r#"{{
  "name": "{}",
  "version": "0.1.0",
  "type": "module",
  "private": true,
  "scripts": {{
    "check": "tsc --noEmit"
  }},
  "devDependencies": {{}}
}}
"#,
        id.replace('/', "-")
    )
}

pub(crate) const TYPESCRIPT_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "skipLibCheck": true,
    "types": ["node"]
  },
  "include": ["package.ts"]
}
"#;

pub(crate) const TYPESCRIPT_SUBPROCESS_RUNTIME_TEMPLATE: &str = r#"import readline from "node:readline";

function respond(id, payload) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, ...payload }) + "\n");
}

export function serveSubprocessPackage(options) {
  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", async (line) => {
    let request;
    try {
      request = JSON.parse(line);
    } catch (error) {
      respond(null, { error: { code: "invalid_json", message: String(error) } });
      return;
    }
    try {
      if (request.method === "package.handshake") {
        const result = options.onHandshake
          ? await options.onHandshake(request.params ?? {})
          : { ready: true, package_protocol_version: "0.1.0" };
        respond(request.id, { result });
      } else if (request.method === "capability.invoke") {
        const output = await options.onInvoke(request.params ?? {});
        respond(request.id, { result: { output } });
      } else {
        respond(request.id, { error: { code: "unknown_method", message: request.method ?? "<missing>" } });
      }
    } catch (error) {
      respond(request.id, { error: { code: "package_error", message: String(error) } });
    }
  });
}

serveSubprocessPackage({
  onInvoke: ({ input }) => input ?? null,
});
"#;

/// TypeScript subprocess template for a networked capability package.
/// Demonstrates: network declarations, secret_ref usage, outbound audit.
pub(crate) fn typescript_networked_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";
import {{ secretRef, isValidSecretRef, NetworkDeclaration, OutboundAuditHelper }} from "../../sdk/typescript/secure-execution/index.js";

// Example network declaration — package declares which hosts/methods it needs.
const networkDeclarations = [
  new NetworkDeclaration({{
    host: "api.example.com",
    methods: ["GET", "POST"],
    purpose: "model inference",
  }}),
];

// Example outbound audit helper
const auditHelper = new OutboundAuditHelper({{
  packageId: "{id}",
  capabilityId: "{id}/fetch",
}});

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    if (capability_id === "{id}/fetch") {{
      // Build an audit-safe request payload — no raw secrets
      const payload = auditHelper.buildRequestPayload({{
        destinationHost: "api.example.com",
        method: "POST",
        secretRefsUsed: [secretRef("env", "MY_API_KEY")],
        purpose: "model inference",
      }});
      // Return the plan — no real network call
      return {{
        plan: "would request api.example.com",
        network_declarations: networkDeclarations.map(d => d.toManifestEntry()),
        audit_payload: payload,
        // NOTE: This package does NOT make real network calls.
        // It returns a plan/discovery result only.
      }};
    }}
    if (capability_id === "{id}/echo") {{
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}

/// TypeScript subprocess template for a streaming capability package.
/// Demonstrates: streaming lifecycle, faux frame sequence, no real inference.
pub(crate) fn typescript_streaming_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";
import {{ StreamFrameClient, secretRef }} from "../../sdk/typescript/secure-execution/index.js";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    if (capability_id === "{id}/stream-plan") {{
      // Faux streaming lifecycle — no real model inference
      const client = new StreamFrameClient();
      const startFrame = client.start("{id}/stream-plan", {{ prompt_plan: true }});
      const chunk1 = client.chunk({{ token: "faux_1" }});
      const chunk2 = client.chunk({{ token: "faux_2" }});
      const endFrame = client.end();

      return {{
        plan: "streaming capability readiness proof",
        frames: [startFrame, chunk1, chunk2, endFrame],
        secret_ref_example: secretRef("env", "MY_KEY"),
        // NOTE: No real model inference. Frames are faux/demonstration only.
        // This proves the substrate shape (invocation lifecycle, redaction_state,
        // sequence ordering) without coupling to pi runtime or model APIs.
      }};
    }}
    if (capability_id === "{id}/echo") {{
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}

/// TypeScript subprocess template for an agent-runtime package.
/// Demonstrates: deterministic/no-network agent-like package with streaming run,
/// proposal draft, trace summary, and echo capabilities. Uses agent adapter SDK.
/// No real model inference, no real network calls, no raw secrets.
pub(crate) fn typescript_agent_runtime_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";
import {{ StreamFrameClient }} from "../../sdk/typescript/secure-execution/index.js";
import {{ createTraceEvent, createProposalDraft, blockRawSecrets }} from "../../sdk/typescript/agent-adapter/index.js";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    // Deterministic / no-network: no real model, no real network, no raw secrets.
    if (capability_id === "{id}/run") {{
      // Streaming run — returns faux stream frames and trace-shaped data.
      const client = new StreamFrameClient();
      const startFrame = client.start("{id}/run", {{ agent_run: true }});
      const chunk1 = client.chunk({{ trace_step: "plan", detail: "faux planning step" }});
      const chunk2 = client.chunk({{ trace_step: "execute", detail: "faux execution step" }});
      const endFrame = client.end();
      const traceEvent = createTraceEvent({{
        trace_id: "faux-trace-001",
        step: "run_complete",
        capability_id: "{id}/run",
        detail: "deterministic faux agent run — no real model or network",
      }});
      return {{
        plan: "agent runtime faux run",
        frames: [startFrame, chunk1, chunk2, endFrame],
        trace: traceEvent,
      }};
    }}
    if (capability_id === "{id}/explain-run") {{
      // Non-streaming trace summary.
      const traceEvent = createTraceEvent({{
        trace_id: "faux-trace-001",
        step: "explain",
        capability_id: "{id}/explain-run",
        detail: "faux trace summary — deterministic, no real model",
      }});
      return {{
        summary: "agent run trace summary",
        steps: ["plan", "execute", "review"],
        trace: traceEvent,
      }};
    }}
    if (capability_id === "{id}/draft-proposal") {{
      // Non-streaming proposal draft — approval-gated shape.
      const draft = createProposalDraft({{
        title: "Agent-proposed change",
        description: "A deterministic faux proposal generated by the agent-runtime template. No real model inference.",
        expected_effects: [
          {{ kind: "asset.put", description: "Faux asset update from agent proposal" }},
        ],
        approval_required: true,
      }});
      blockRawSecrets(draft);
      return draft;
    }}
    if (capability_id === "{id}/echo") {{
      // Echo — package conformance compatible.
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}

/// TypeScript subprocess template for an experience-runtime package.
/// Demonstrates: deterministic/no-network experience package with experience
/// descriptor, state projection, checkpoint, recovery, and Play/Forge/Assist
/// surface bindings. No real model inference, no real network calls, no raw secrets.
pub(crate) fn typescript_experience_runtime_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";
import {{
  createExperienceDescriptor,
  createStateProjection,
  createCheckpoint,
  inspectCheckpoint,
  draftRecoveryPlan,
  createPlaySurfaceSubscription,
  createForgeBinding,
  createAssistBinding,
  blockRawSecrets,
}} from "../../sdk/typescript/experience-runtime/index.js";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    // Deterministic / no-network: no real model, no real network, no raw secrets.
    // No forbidden platform-reserved namespaces (experience, world, turn, chat, memory)

    // Raw-secret check
    const secretCheck = blockRawSecrets(input);
    if (!secretCheck.clean) {{
      return {{
        kind: "experience_runtime_rejected",
        redaction_state: "unsafe_blocked",
        reason: secretCheck.reason,
        inference_performed: false,
        network_performed: false,
      }};
    }}

    if (capability_id === "{id}/describe-contract") {{
      const desc = createExperienceDescriptor({{
        package_id: "{id}",
        surfaces: {{
          experience_entry: "{id}/entry",
          play_renderer: "{id}/play",
          forge_panel: "{id}/forge",
          assistant_action: "{id}/assist",
        }},
        capabilities: {{
          describe_contract: "{id}/describe-contract",
          create_checkpoint: "{id}/create-checkpoint",
          inspect_checkpoint: "{id}/inspect-checkpoint",
          draft_recovery: "{id}/draft-recovery",
          bind_agent_run: "{id}/bind-agent-run",
        }},
      }});
      return desc;
    }}
    if (capability_id === "{id}/create-checkpoint") {{
      const cp = createCheckpoint({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        state_snapshot: input.state_snapshot ?? {{}},
        asset_refs: input.asset_refs ?? [],
        branch_ref: input.branch_ref,
        sequence: input.sequence,
        capability_id: "{id}/create-checkpoint",
      }});
      return cp;
    }}
    if (capability_id === "{id}/inspect-checkpoint") {{
      const cp = createCheckpoint({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        state_snapshot: input.state_snapshot ?? {{}},
        checkpoint_id: input.checkpoint_id,
        format: input.format,
        sequence: input.sequence,
        capability_id: "{id}/inspect-checkpoint",
      }});
      return inspectCheckpoint(cp);
    }}
    if (capability_id === "{id}/draft-recovery") {{
      const plan = draftRecoveryPlan({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        failure_kind: input.failure_kind ?? "unknown",
        last_checkpoint_ref: input.last_checkpoint_ref ?? null,
        capability_id: "{id}/draft-recovery",
      }});
      return plan;
    }}
    if (capability_id === "{id}/bind-agent-run") {{
      const forge = createForgeBinding({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        surface_id: "{id}/forge",
        inspect_capabilities: ["{id}/describe-contract", "{id}/inspect-checkpoint"],
        proposal_capabilities: ["{id}/draft-recovery"],
        capability_id: "{id}/bind-agent-run",
      }});
      const assist = createAssistBinding({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        surface_id: "{id}/assist",
        action_capabilities: ["{id}/draft-recovery", "{id}/bind-agent-run"],
        capability_id: "{id}/bind-agent-run",
      }});
      const play = createPlaySurfaceSubscription({{
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        surface_id: "{id}/play",
        subscription_type: "state_change",
        capability_id: "{id}/bind-agent-run",
      }});
      return {{
        kind: "experience_agent_run_binding",
        package_id: "{id}",
        session_id: input.session_id ?? "session_default",
        agent_package_id: input.agent_package_id ?? "plurora/agentic-forge-lab",
        scoped_to_branch: true,
        target_branch_ref: input.target_branch_ref ?? "branch:target:default",
        scratch_branch_ref: input.scratch_branch_ref ?? "branch:scratch:default",
        forge_panel_binding: forge,
        assist_binding: assist,
        play_subscription: play,
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/echo") {{
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}

/// TypeScript subprocess template for a playable-board package.
/// Demonstrates: deterministic/no-network playable board with launch,
/// project_state, render_payload, record_player_action, request_change,
/// create_checkpoint capabilities and 4 experience surfaces.
/// No real model inference, no real network calls, no raw secrets.
pub(crate) fn typescript_playable_board_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    // Deterministic / no-network: no real model, no real network, no raw secrets.

    if (capability_id === "{id}/launch") {{
      return {{
        kind: "playable_board_launched",
        board_id: input.board_id ?? "board:default",
        title: input.title ?? "Generated Playable Board",
        lifecycle_state: "created",
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/project_state") {{
      return {{
        kind: "playable_board_state_projection",
        board_id: input.board_id ?? "board:default",
        state_snapshot: input.state_snapshot ?? {{}},
        markers: input.markers ?? [],
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/render_payload") {{
      return {{
        kind: "playable_board_render_payload",
        board_id: input.board_id ?? "board:default",
        render_hint: "grid",
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/record_player_action") {{
      const seq = input.sequence ?? 1;
      return {{
        kind: "playable_board_action_recorded",
        board_id: input.board_id ?? "board:default",
        action_kind: input.action_kind ?? "place_marker",
        sequence: seq,
        state_delta_asset_ref: "asset:state_delta:" + (input.board_id ?? "board:default") + ":" + seq,
        projection_ref: "projection:" + (input.board_id ?? "board:default"),
        provenance: {{ package_id: "{id}" }},
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/request_change") {{
      return {{
        kind: "playable_board_change_request",
        board_id: input.board_id ?? "board:default",
        objective: input.objective ?? "modify board",
        allowed_change_kinds: ["add_module", "remove_module", "add_constraint", "add_marker"],
        risk: "low",
        budget: 1,
        bindable_refs: {{ forge_panel: "{id}/forge-panel", assistant_action: "{id}/assistant-action" }},
        requires_user_approval: true,
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/create_checkpoint") {{
      return {{
        kind: "playable_board_checkpoint",
        board_id: input.board_id ?? "board:default",
        checkpoint_id: "cp:" + Date.now(),
        format: "snapshot",
        sequence: input.sequence ?? 1,
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/echo") {{
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}

/// TypeScript subprocess template for a playable-experience package.
/// Demonstrates: deterministic/no-network playable experience with launch,
/// project_state, render_payload, record_player_action, request_change,
/// create_checkpoint, inspect_checkpoint, draft_recovery capabilities and
/// 4 experience surfaces. Full checkpoint/recovery lifecycle.
/// No real model inference, no real network calls, no raw secrets.
pub(crate) fn typescript_playable_experience_template(id: &str) -> String {
    format!(
        r#"import {{ serveSubprocessPackage }} from "./package.mjs";

serveSubprocessPackage({{
  onHandshake: () => ({{ ready: true, package_protocol_version: "0.1.0" }}),
  onInvoke: ({{ capability_id, input }}) => {{
    // Deterministic / no-network: no real model, no real network, no raw secrets.

    if (capability_id === "{id}/launch") {{
      return {{
        kind: "playable_experience_launched",
        experience_id: input.experience_id ?? "experience:default",
        title: input.title ?? "Generated Playable Experience",
        lifecycle_state: "created",
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/project_state") {{
      return {{
        kind: "playable_experience_state_projection",
        experience_id: input.experience_id ?? "experience:default",
        state_snapshot: input.state_snapshot ?? {{}},
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/render_payload") {{
      return {{
        kind: "playable_experience_render_payload",
        experience_id: input.experience_id ?? "experience:default",
        render_hint: "scene",
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/record_player_action") {{
      const seq = input.sequence ?? 1;
      return {{
        kind: "playable_experience_action_recorded",
        experience_id: input.experience_id ?? "experience:default",
        action_kind: input.action_kind ?? "interact",
        sequence: seq,
        state_delta_asset_ref: "asset:state_delta:" + (input.experience_id ?? "experience:default") + ":" + seq,
        projection_ref: "projection:" + (input.experience_id ?? "experience:default"),
        provenance: {{ package_id: "{id}" }},
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/request_change") {{
      return {{
        kind: "playable_experience_change_request",
        experience_id: input.experience_id ?? "experience:default",
        objective: input.objective ?? "modify experience",
        allowed_change_kinds: ["add_scene", "modify_rule", "add_event"],
        risk: "low",
        budget: 1,
        bindable_refs: {{ forge_panel: "{id}/forge-panel", assistant_action: "{id}/assistant-action" }},
        requires_user_approval: true,
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/create_checkpoint") {{
      return {{
        kind: "playable_experience_checkpoint",
        experience_id: input.experience_id ?? "experience:default",
        checkpoint_id: "cp:" + Date.now(),
        format: "snapshot",
        sequence: input.sequence ?? 1,
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/inspect_checkpoint") {{
      return {{
        kind: "playable_experience_checkpoint_inspection",
        experience_id: input.experience_id ?? "experience:default",
        checkpoint_id: input.checkpoint_id ?? "cp:0",
        valid: true,
        format: input.format ?? "snapshot",
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/draft_recovery") {{
      const hasCheckpoint = !!input.last_checkpoint_ref;
      return {{
        kind: "playable_experience_recovery_plan",
        experience_id: input.experience_id ?? "experience:default",
        recommended_strategy: hasCheckpoint ? "restore_last_checkpoint" : "restart_session",
        plan: {{
          checkpoint_available: hasCheckpoint,
          steps: hasCheckpoint
            ? ["validate_checkpoint", "restore_state", "resume_from_checkpoint"]
            : ["restart_session", "reinitialize_state"],
        }},
        inference_performed: false,
        network_performed: false,
      }};
    }}
    if (capability_id === "{id}/echo") {{
      return input ?? null;
    }}
    throw new Error(`unsupported capability: ${{capability_id}}`);
  }},
}});
"#
    )
}
