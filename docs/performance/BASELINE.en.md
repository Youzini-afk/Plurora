# Performance Baseline

> [English](./BASELINE.en.md) · [中文](./BASELINE.md)

This document records usage, measurement scenarios, sample limits, metric definitions, and compare mode for `cargo run -p plurora-cli -- perf baseline`. The current baseline is only a developer-machine reference, not a CI budget.

The repository commits a reference baseline at [`perf/baseline.json`](../../perf/baseline.json). It was produced on a Linux developer machine and is useful as a before/after reference for future optimizations; do not treat it as a CI budget.

Performance/code-health guide: [`PERFORMANCE_AND_CODE_HEALTH.en.md`](./PERFORMANCE_AND_CODE_HEALTH.en.md).

## Command

```bash
# Default 10 iterations, text output
cargo run -p plurora-cli -- perf baseline

# Custom iteration count
cargo run -p plurora-cli -- perf baseline --iterations 20

# 30 measured iterations + 3 warmups, writing a JSON baseline file
cargo run -p plurora-cli -- perf baseline --iterations 30 --warmup 3 --baseline-out perf/baseline.json

# Compare against the committed baseline; exit 2 when wall-clock regression exceeds 10%
cargo run -p plurora-cli -- perf baseline --iterations 30 --compare perf/baseline.json --threshold-pct 10

# JSON output (stdout contains JSON only, machine-parseable)
cargo run -p plurora-cli -- perf baseline --format json
```

Available flags:

- `--iterations <N>`: measured iterations per scenario; must be greater than 0.
- `--warmup <N>`: unrecorded warmup iterations per scenario; default is 1.
- `--format text|json`: text or JSON output.
- `--baseline-out <PATH>`: write the full JSON envelope to a file.
- `--compare <PATH>`: read a previous `perf/baseline.json` and compare by scenario `avg_ms`.
- `--threshold-pct <N>`: regression threshold for compare mode; default is 10.0.

Threshold guidance: use 10% for ordinary wall-clock scenarios and 20% for end-to-end or variable scenarios. Compare mode is advisory for now, not a CI gate.

## Measurement scenarios

All scenarios avoid real network or provider dependencies. Inputs are fixed so developer machines can compare trends.

| scenario_id | Description |
|---|---|
| `inproc_echo_invoke` | Rust inproc package echo capability invocation. Uses `examples/packages/echo-rust-inproc/manifest.yaml`. |
| `official_capability_invoke` | Official package capability invocation. Uses `official/composition-lab/describe`. |
| `event_store_append_list_range` | In-memory event store batch append (100 events), full list, range query. |
| `event_store_append_list_range_1k` | In-memory event store atomic append (1,000 events), full list, kind-prefix query. |
| `event_store_append_list_range_10k` | In-memory event store atomic append (10,000 events), full list, kind-prefix query. |
| `event_store_append_list_range_100k` | In-memory event store atomic append (100,000 events), full list, kind-prefix query. Auto-capped to 1 iteration when iterations > 1. |
| `composition_check` | Composition descriptor validation and package loading. Uses `examples/compositions/playable-seed-replacement/`. |
| `profile_load` | Profile YAML parsing. Uses `profiles/forge-alpha.yaml`. |
| `subprocess_echo_invoke` | Subprocess echo capability invocation (requires Python; status=skipped if unavailable). |
| `subprocess_cold_start_ms` | Fresh subprocess package per iteration, measuring `load_package` handshake plus first invoke. |
| `subprocess_handshake_ms` | Subprocess spawn + handshake; there is no separate spawn-only API yet. |
| `subprocess_invoke_steady_1kb` | Steady invoke on an already loaded subprocess echo package with a 1 KiB payload. |
| `subprocess_invoke_steady_10kb` | Steady invoke on an already loaded subprocess echo package with a 10 KiB payload. |
| `subprocess_invoke_steady_100kb` | Steady invoke on an already loaded subprocess echo package with a 100 KiB payload. |
| `outbound_execute_fake_throughput_req_s` | Throughput for 1,000 `execute_outbound_with_policy` calls on `FakeOutboundExecutor`. |
| `outbound_stream_fake_ttft_ms` | Fake SSE stream first-event latency, drained to completion. |
| `outbound_stream_fake_steady_events_s` | Planned 100-event steady stream measurement; currently `skipped` because the fake executor has no public N-frame fixture API. |

## Output fields

JSON output uses an envelope:

| Top-level field | Description |
|---|---|
| `schema` | JSON schema identifier; currently `plurora.bench.v1`. |
| `created_at` | Creation time as Unix seconds. |
| `git` | Commit, branch, and dirty status for the producing checkout. |
| `env` | Environment: OS, target triple, CPU count, rustc / CPU brand when available. |
| `baseline` | `ScenarioResult[]`. |
| `comparisons` | `ComparisonResult[]` in compare mode; emitted only when comparisons exist. |
| `meta` | Iterations, warmup count, tool version, ok/skipped/error counts, and note. |

`ScenarioResult` fields:

| Field | Description |
|---|---|
| `scenario_id` | Scenario identifier |
| `iterations` | Number of iterations |
| `total_ms` | Total wall time (ms) |
| `avg_ms` | Average per iteration |
| `min_ms` | Minimum iteration time |
| `p50_ms` | 50th percentile iteration time |
| `p95_ms` | 95th percentile iteration time |
| `p99_ms` | 99th percentile iteration time |
| `max_ms` | Maximum iteration time |
| `memory_rss_mb_delta` | RSS change before/after the scenario in MiB; omitted when unavailable |
| `iterations_capped` | `true` when a scenario lowers the requested iteration count to avoid excessive runtime |
| `status` | `ok` / `skipped` / `error` |
| `notes` | Additional context |

`ComparisonResult` fields:

| Field | Description |
|---|---|
| `scenario_id` | Scenario identifier |
| `baseline_avg_ms` | Historical baseline `avg_ms` |
| `current_avg_ms` | Current run `avg_ms` |
| `delta_pct` | `(current - baseline) / baseline * 100` |
| `regression` | `delta_pct > --threshold-pct` |

## Sample limitations

- Default 10 iterations. Adjustable via `--iterations`.
- `--iterations 0` is rejected; every scenario must run at least once.
- Each iteration is independently timed; warmups are controlled by `--warmup` and are excluded from samples.
- Measurement uses `std::time::Instant`; precision depends on OS (typically 1 µs or better).
- The in-memory event-store scenario appends 100 events per iteration. Scale scenarios cover 1k/10k/100k atomic append. Each iteration uses an independent store and session so accumulated events do not distort fixed-size metrics.
- `event_store_append_list_range_100k` auto-caps to 1 iteration when `--iterations > 1` to avoid excessive runtime, and sets `iterations_capped=true`.
- `EventStore::append_with_sequence` provides atomic append and prevents duplicate sequences under concurrent same-session access.
- `EventStore::list_kind_prefix` and `list_session_kind_prefix` provide query pushdown. Audit and range queries no longer routinely call `list_all()` and then filter the full result.
- No criterion or statistical framework is used. The goal is a developer-machine reference, not a CI compliance budget.

## Red lines

- No official-package fast path. Official and third-party packages share the same routing and permission boundaries.
- No bypass of permissions, hooks, schema validation, redaction, or audit.
- No real network or provider required.
- No runtime boundary or public protocol changes.

## Metrics for future optimization tracking

Use these metrics for before/after comparisons during later optimization:

1. In-process invoke latency — Watch this if a resolve cache or handler table is introduced.
2. Event-store batch throughput — Compare append/list/range/kind-prefix latency for 100 events, 1k, 10k, and 100k.
3. Event-store scale trend — Use the 1k/10k/100k scenarios to compare growth across versions.
4. Composition check latency — Set/index-based diagnostics should improve this.
5. Profile load latency — Use it as the YAML parsing baseline; re-measure when profiles grow.
6. Subprocess invoke latency — Re-measure with a stable subprocess environment.
7. Fake outbound executor and fake streams — Use them as the no-network reference for outbound audit/policy paths.
8. Future UI optimization should use frontend diagnostics to compare HTML bytes and elapsed_ms.

The current pre-human-testing baseline should also pay attention to install/profile/surface/security-bridge paths so project install, profile loading, static bundle serving, and bridge safety boundaries do not regress noticeably.

## Sample reference output

```
scenario                           iters     total       avg       min       p50       p95       p99      max     rssΔ  status
----------------------------------------------------------------------------------------------------------------------------------
inproc_echo_invoke                    30      0.31     0.010     0.009     0.010     0.014     0.021    0.021     0.19  ok
event_store_append_list_range_10k     30   1270.76    42.359    38.292    40.491    52.422    54.724   54.724     0.01  ok [10000 events per iteration]
event_store_append_list_range_100k     1    416.26   416.265   416.265   416.265   416.265   416.265  416.265   -25.70  ok capped [100000 events per iteration (capped to 1 iteration from 30)]
subprocess_invoke_steady_100kb        30     97.21     3.240     2.919     3.202     3.765     3.945    3.945     0.00  ok [echo payload data field is 102400 bytes]
outbound_stream_fake_steady_events_s  30     -0.00     0.000     0.000     0.000     0.000     0.000    0.000      n/a  skipped [FakeOutboundExecutor currently emits a fixed 3-event stream]

baseline: 16 ok, 1 skipped, 0 error (17 scenarios)
```

Values above are from a specific developer machine. They are a reference, not a CI compliance budget.
