#!/usr/bin/env python3
"""
Faux Agent Readiness Package — No-Network Proof

This package demonstrates the secure execution substrate shape for
agent-like capability packages without connecting to any pi runtime
or performing real model inference. It:

- Produces proposals/traces/plans only (no real agent loop)
- Uses secret_ref for credential references (never raw secrets)
- Emphasizes public protocol/capability/proposal patterns
- Does NOT add agent/model semantics to the kernel
- Uses streaming faux frames to prove substrate shape
"""

import json
import sys
import time


def respond(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "package.handshake":
        respond({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "result": {
                "ready": True,
                "package_protocol_version": "0.1.0",
            },
        })
    elif method == "capability.invoke":
        params = request.get("params", {})
        capability_id = params.get("capability_id", "")
        input_data = params.get("input", {})

        if capability_id == "example/faux-agent-readiness/propose":
            # Return a faux agent proposal/trace — no real agent loop
            respond({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "output": {
                        "plan_type": "agent_proposal",
                        "proposal_pattern": "change.proposal.create",
                        "trace": [
                            {
                                "step": "receive_task",
                                "description": "Agent receives a task from the caller",
                                "protocol_method": "capability.invoke",
                            },
                            {
                                "step": "plan_actions",
                                "description": "Agent plans which capabilities to invoke",
                                "protocol_method": "change.proposal.create",
                                "secret_ref": "secret_ref:env:AGENT_CREDENTIAL",
                                "redaction_state": "redacted",
                            },
                            {
                                "step": "propose_operations",
                                "description": "Agent proposes asset/projection operations",
                                "operations": [
                                    {"op": "asset.put", "payload": {"secret": "secret_ref:env:AGENT_CREDENTIAL"}},
                                ],
                            },
                            {
                                "step": "await_approval",
                                "description": "Proposal awaits user approval via fork_then_approve",
                            },
                        ],
                        "public_protocol_only": True,
                        "note": "No real agent loop. This proves the proposal/trace shape.",
                    }
                },
            })
        elif capability_id == "example/faux-agent-readiness/stream-trace":
            # Return faux streaming trace frames — no real agent turn
            invocation_id = f"inv_agent_{int(time.time() * 1000)}"
            stream_id = f"str_agent_{int(time.time() * 1000)}"
            respond({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "output": {
                        "plan_type": "streaming_agent_trace",
                        "invocation_id": invocation_id,
                        "stream_id": stream_id,
                        "faux_frames": [
                            {
                                "frame_type": "start",
                                "sequence": 0,
                                "redaction_state": "not_captured",
                                "payload": {"trace": "agent thinking started"},
                            },
                            {
                                "frame_type": "chunk",
                                "sequence": 1,
                                "redaction_state": "redacted",
                                "payload": {"step": "tool_invocation", "capability": "example/some-cap"},
                            },
                            {
                                "frame_type": "chunk",
                                "sequence": 2,
                                "redaction_state": "redacted",
                                "payload": {"step": "response_synthesis"},
                            },
                            {
                                "frame_type": "progress",
                                "sequence": 3,
                                "redaction_state": "not_captured",
                                "payload": None,
                                "metadata": {"percent": 75},
                            },
                            {
                                "frame_type": "end",
                                "sequence": 4,
                                "redaction_state": "not_captured",
                                "payload": None,
                            },
                        ],
                        "secret_ref_example": "secret_ref:env:AGENT_CREDENTIAL",
                        "note": "No real agent turn or model output. Frames prove substrate shape.",
                    }
                },
            })
        else:
            respond({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "error": {
                    "code": "unknown_capability",
                    "message": f"unsupported capability: {capability_id}",
                },
            })
    else:
        respond({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "error": {"code": "unknown_method", "message": method},
        })
