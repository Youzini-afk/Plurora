# Memory Package Authoring Guide

> [English](./MEMORY_PACKAGE_AUTHORING.en.md) · [中文](./MEMORY_PACKAGE_AUTHORING.md)

This guide explains how to author, replace, and consume long-term memory and knowledge through ordinary Packages and Components under the current Contract V1. Shared Memory meaning may be defined by an optional Protocol, while concrete records and retrieval belong to Components and Products rather than the constitutional substrate.

## Core Principles

1. Memory stays above the substrate. Adopted Protocols, Components, and Products manage records, retrieval, updates, corrections, and redaction plans; Contract V1 distributes implementations through ordinary Packages rather than `platform.memory.*`.
2. No publisher priority. `plurora/memory-lab` is one implementation. Third-party packages like `thirdparty/memory-lab` are fully interchangeable.
3. Mutations go through proposals. Memory updates, corrections, and forget/redaction produce proposal drafts or plans. They never directly mutate trusted state or delete records. Consumers must approve before application.
4. The reference implementation is locally replayable. It does not require network, embedding APIs, or model inference. Third-party packages may add such capabilities through their own outbound/network permissions.
5. Branch-aware. Memory records are scoped to branches. Retrieval and views can filter by branch reference.
6. Raw-secret blocking. All capability inputs are scanned for raw secrets. Raw API keys, tokens, and passwords are rejected with `redaction_state: unsafe_blocked`. Use `secret_ref` references instead.
7. No forbidden namespaces. Memory packages must not reference `platform.memory.*`, `platform.experience.*`, `platform.world.*`, `platform.scene.*`, `platform.turn.*`, `platform.chat.*`, `platform.agent.*`, `platform.model.*`, `platform.prompt.*`, or `platform.director.*`.

## Memory Lab Capability Contract

`plurora/memory-lab` provides 9 capabilities:

| Capability | Purpose | Output Shape |
|---|---|---|
| `describe_memory_contract` | Describe the package contract | `memory_lab_contract` |
| `record_memory` | Record a memory entry | `memory_record` |
| `retrieve_memory` | Retrieve matching memories | `retrieval_result` |
| `trace_retrieval` | Show how retrieval matched | `retrieval_trace` |
| `draft_memory_update` | Draft a proposal-gated update | `memory_update_draft` |
| `apply_memory_correction` | Produce a correction shape | `memory_correction` |
| `draft_forget_redaction` | Draft a redaction plan | `memory_redaction_plan` |
| `branch_memory_view` | View memories scoped by branch | `memory_branch_view` |
| `explain_memory_provenance` | Explain record provenance | `memory_provenance` |

### Surfaces

- `forge_panel`: Inspect memory records, traces, drafts, corrections, redaction plans, and provenance.
- `assistant_action`: Draft approval-gated updates, corrections, or redaction plans.
- `home_card`: Record and retrieve memories.

## Memory Record

A `memory_record` has:

- `record_id`: Deterministic ID derived from key + content address.
- `record_kind`: One of `fact`, `preference`, `observation`, `correction`, `summary`, `context`.
- `key`: Lookup key for the record.
- `content`: The record content (arbitrary value).
- `content_address`: Stable content-addressed hash (FNV-1a 64-bit).
- `branch_ref`: The branch this record belongs to.
- `disclosure`: AI-generated / live-generated / unspecified metadata.
- `source_refs`: Protocol-visible source references.
- `knowledge_refs`: Optional cross-references to knowledge-lab entries.

## Retrieval

`retrieve_memory` uses deterministic keyword matching (case-insensitive substring). It supports branch-aware filtering: when `branch_ref` is specified, only records from that branch are considered.

`trace_retrieval` produces a detailed trace showing each step of the retrieval algorithm.

## Proposal-Gated Update

`draft_memory_update` produces a `memory_update_draft` with:

- `update_kind`: `add_record`, `modify_record`, `correct_record`, `forget_record`, `merge_records`.
- `requires_user_approval`: always `true`.
- `plan_only`: always `true`; no direct state mutation.
- `content_address`: Stable hash of the draft.

Consumers must approve and apply the draft through the proposal lifecycle.

## Correction

`apply_memory_correction` produces a `memory_correction` shape:

- `original_record_ref`: Reference to the record being corrected.
- `corrected_content`: The corrected content.
- `requires_user_approval`: always `true`.
- `content_address`: Stable hash.

## Forget / Redaction

`draft_forget_redaction` produces a `memory_redaction_plan`:

- `target_record_refs`: Records targeted for redaction.
- `redaction_scope`: `record_only` or broader.
- `status`: `draft` (requires approval before becoming `applied`).
- `plan_only`: always `true`; no direct deletion.
- `requires_user_approval`: always `true`.

The redaction plan is a proposal. The actual deletion/redaction happens only after explicit user approval.

## Branch-Aware View

`branch_memory_view` supports scopes:

- `current_branch`: Records from the specified branch only.
- `all_branches`: Records from all branches.
- `specified_branch`: Same as current_branch (explicit).
- `branch_diff`: Records grouped by branch for comparison.

## Provenance

`explain_memory_provenance` produces a chain where each step has:

- `step`: `record_created`, `record_retrieved`, `update_drafted`, `correction_applied`, `redaction_planned`, `branch_viewed`, `provenance_traced`.
- `ref`: Protocol-visible reference.
- `content_address`: Stable content-addressed hash.
- `description`: Human-readable explanation.

## Cross-Package Integration

The `plurora/playable-creation-board` includes optional `memory_refs` in its `request_change` output:

- `memory_package_id`: The memory package to use (default `plurora/memory-lab`).
- `retrieve_context_plan`: Describes how to retrieve memory context for change planning (optional, the board runs without it).
- `knowledge_refs`: Optional cross-references to knowledge-lab entries.

This is an optional cross-reference. The board does not depend on memory-lab to operate.

## Third-Party Replacement

`thirdparty/memory-lab` proves that `plurora/memory-lab` has no special priority:

- Same 9 capabilities, 3 surfaces.
- Same output shapes (`memory_record`, `retrieval_trace`, `update_draft`, `correction`, `redaction_plan`, `branch_view`, `provenance`).
- Projected into a Work/Assembly node by `examples/works/memory-lab-replacement/work.yaml`.
- First-party `memory-lab` is not a default provider; multiple compatible candidates require an explicit Binding or remain ambiguous.

## What This Is Not

- Not a RAG product. The reference implementation uses replayable keyword matching, not vector search or embedding APIs.
- Not a chat memory system. There are no conversation turns, messages, or prompt semantics.
- Not kernel memory. No `platform.memory.*` methods or namespaces exist.
- Not the only way. Third-party packages can provide different retrieval algorithms, storage backends, or embedding-based matching through ordinary package capabilities.
