# Conformance Feedback Loop Guide

> [English](./CONFORMANCE_FEEDBACK.en.md) · [中文](./CONFORMANCE_FEEDBACK.md)

`plurora conformance` supports filtering, timing, and diagnostics. Use it to find failures and slow cases quickly.

## Basic usage

```bash
# Run all conformance cases (default behavior unchanged)
cargo run -p plurora-cli -- conformance
```

## Listing cases

```bash
# List all case ids and tags without executing
cargo run -p plurora-cli -- conformance --list
```

Output format: `<case_id>  [<tag1>, <tag2>, ...]`

## Filtering by case id

```bash
# Substring filter (matches cases whose id contains the given substring)
cargo run -p plurora-cli -- conformance --case sharing_lab.contract_shape

# Filter all sharing_lab cases
cargo run -p plurora-cli -- conformance --case sharing_lab
```

## Filtering by tag

```bash
# Tag filter (case matches if it has ANY of the specified tags)
cargo run -p plurora-cli -- conformance --tag sharing

# Combine multiple tags (OR semantics)
cargo run -p plurora-cli -- conformance --tag network --tag secret
```

## Fail-fast

```bash
# Stop after the first failure
cargo run -p plurora-cli -- conformance --fail-fast
```

## Slowest report

```bash
# Show the 10 slowest cases at the end (default)
cargo run -p plurora-cli -- conformance

# Custom slowest N
cargo run -p plurora-cli -- conformance --slowest 3
```

## Combining options

```bash
# Run only sharing-tagged cases, fail-fast, show slowest 3
cargo run -p plurora-cli -- conformance --tag sharing --fail-fast --slowest 3

# Filter by both case id and tag (AND semantics: both conditions must match)
cargo run -p plurora-cli -- conformance --case sharing_lab --tag secret
```

## Available tags

| Tag | Description |
|---|---|
| runtime | Kernel runtime behavior (session, event, capability, hook, etc.) |
| session | Session lifecycle |
| event | Event append and read |
| capability | Capability discovery and invocation |
| package | Package load, unload, restart |
| official | Official package conformance |
| schema | JSON Schema validation |
| protocol | Public protocol dispatch |
| permission | Permissions / principals |
| hook | Hook fabric slice |
| subprocess | Subprocess package execution (typically slow) |
| host | Host diagnostics and profile |
| surface | Surface contribution |
| proposal | Proposal lifecycle |
| asset | Asset registry |
| projection | Projection registry |
| substrate | SQLite substrate |
| composition | Composition descriptor |
| replacement | Third-party replacement proof |
| generated | Generated package template conformance (typically slow) |
| secret | Secret reference / raw-secret blocking |
| network | Network permission and outbound |
| outbound | Outbound executor boundary |
| live | Live model calls |
| stream | Streaming lifecycle |
| agentic | Agentic Forge |
| experience | Experience runtime / playable board |
| memory | Memory lab |
| sharing | Sharing lab |
| slow | Known-slow cases (subprocess startup, generated package templates, etc.) |

## Output format

Each case produces one line:

```
<case_id>  PASS|FAIL  <duration>
```

A slowest-N summary and overall result are printed at the end.
