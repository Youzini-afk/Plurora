# Performance Baseline

> [English](./BASELINE.en.md) · [中文](./BASELINE.md)

本文档记录 `cargo run -p plurora-cli -- perf baseline` 的用法、测量场景、样本限制、指标定义和比较模式。当前基线只作为开发机参考，不是 CI 预算。

仓库已提交参考基线：[`perf/baseline.json`](../../perf/baseline.json)。它来自一台 Linux 开发机，可作为未来优化的前后对比起点；不要把它当成 CI 合规预算。

性能与代码健康指南见 [`PERFORMANCE_AND_CODE_HEALTH.md`](./PERFORMANCE_AND_CODE_HEALTH.md)。

## 命令

```bash
# 默认 10 次迭代，文本输出
cargo run -p plurora-cli -- perf baseline

# 自定义迭代次数
cargo run -p plurora-cli -- perf baseline --iterations 20

# 30 次迭代 + 3 次预热，并写入 JSON baseline 文件
cargo run -p plurora-cli -- perf baseline --iterations 30 --warmup 3 --baseline-out perf/baseline.json

# 和已提交基线比较；超过 10% wall-clock 回归时退出 2
cargo run -p plurora-cli -- perf baseline --iterations 30 --compare perf/baseline.json --threshold-pct 10

# JSON 输出（stdout 仅 JSON，可程序化处理）
cargo run -p plurora-cli -- perf baseline --format json
```

可用 flags：

- `--iterations <N>`：每个场景记录的迭代次数，必须大于 0。
- `--warmup <N>`：每个场景未记录的预热次数，默认 1。
- `--format text|json`：文本或 JSON 输出。
- `--baseline-out <PATH>`：把完整 JSON envelope 写到指定路径。
- `--compare <PATH>`：读取历史 `perf/baseline.json`，按 `avg_ms` 逐场景比较。
- `--threshold-pct <N>`：比较模式的回归阈值百分比，默认 10.0。

阈值建议：普通 wall-clock 场景先用 10%；端到端或波动较大的场景可用 20%。当前比较结果只作 advisory，不作为 CI gate。

## 测量场景

所有场景都不依赖真实网络或 provider。输入固定，方便在开发机上比较趋势。

| scenario_id | 说明 |
|---|---|
| `inproc_echo_invoke` | Rust inproc 包 echo 能力调用。使用 `examples/packages/echo-rust-inproc/manifest.yaml`。 |
| `first_party_capability_invoke` | 第一方 Package 能力调用。使用无网络、无副作用的 `plurora/asset-lab/preview`。 |
| `event_store_append_list_range` | 内存 event store 批量追加（100 events）、全量 list、range 查询。 |
| `event_store_append_list_range_1k` | 内存 event store 原子追加（1,000 events）、全量 list、kind-prefix 查询。 |
| `event_store_append_list_range_10k` | 内存 event store 原子追加（10,000 events）、全量 list、kind-prefix 查询。 |
| `event_store_append_list_range_100k` | 内存 event store 原子追加（100,000 events）、全量 list、kind-prefix 查询。当 iterations > 1 时自动限制为 1 次迭代。 |
| `work_check` | 安全读取 Work/Assembly source、投影 Package/Component 并运行 authoring resolver。使用 `examples/works/playable-seed-replacement/work.yaml`。 |
| `profile_load` | Profile YAML 解析。使用 `profiles/forge-alpha.yaml`。 |
| `subprocess_echo_invoke` | Subprocess echo 能力调用（需要 Python；不可用时 status=skipped）。 |
| `subprocess_cold_start_ms` | 每次迭代新建 subprocess 包，测量 `load_package` handshake + 首次 invoke。 |
| `subprocess_handshake_ms` | 测量 subprocess spawn + handshake；当前没有单独 spawn-only API。 |
| `subprocess_invoke_steady_1kb` | 已加载 subprocess echo 包的 steady invoke，payload 为 1 KiB。 |
| `subprocess_invoke_steady_10kb` | 已加载 subprocess echo 包的 steady invoke，payload 为 10 KiB。 |
| `subprocess_invoke_steady_100kb` | 已加载 subprocess echo 包的 steady invoke，payload 为 100 KiB。 |
| `outbound_execute_fake_throughput_req_s` | `FakeOutboundExecutor` 上的 1,000 次 `execute_outbound_with_policy` 调用吞吐。 |
| `outbound_stream_fake_ttft_ms` | fake SSE stream 首事件延迟；drain 到完成。 |
| `outbound_stream_fake_steady_events_s` | 计划测量 100 events steady stream；当前 fake executor 缺少 N-frame fixture API，已记录为 `skipped`。 |

提交的历史 `perf/baseline.json` 已删除旧 descriptor-check 样本，因为它与 `work_check` 的安全文件读取、artifact projection 和递归解析成本不可比较；`first_party_capability_invoke` 也已从退役 Package 切换到 `plurora/asset-lab/preview`，旧耗时不能沿用。下一次在基准 Linux 环境采样时再为这两个场景建立首个可比较参考值。

## 输出字段

JSON 输出使用 envelope：

| 顶层字段 | 说明 |
|---|---|
| `schema` | JSON schema 标识；当前为 `plurora.bench.v1`。 |
| `created_at` | 生成时间，Unix 秒。 |
| `git` | 生成时的 commit、branch 和 dirty 状态。 |
| `env` | 生成环境：OS、target triple、CPU 数、rustc / CPU brand（可用时）。 |
| `baseline` | `ScenarioResult[]`。 |
| `comparisons` | 比较模式下的 `ComparisonResult[]`，仅在有比较结果时输出。 |
| `meta` | 迭代、预热、工具版本、ok/skipped/error 计数与说明。 |

`ScenarioResult` 字段：

| 字段 | 说明 |
|---|---|
| `scenario_id` | 场景标识符 |
| `iterations` | 迭代次数 |
| `total_ms` | 总耗时（毫秒） |
| `avg_ms` | 平均耗时 |
| `min_ms` | 最小耗时 |
| `p50_ms` | 50 分位耗时 |
| `p95_ms` | 95 分位耗时 |
| `p99_ms` | 99 分位耗时 |
| `max_ms` | 最大耗时 |
| `memory_rss_mb_delta` | 场景运行前后 RSS 变化（MiB）；不可用时省略 |
| `iterations_capped` | 场景为了避免过慢而自动降低迭代次数时为 `true` |
| `status` | `ok` / `skipped` / `error` |
| `notes` | 附加说明 |

`ComparisonResult` 字段：

| 字段 | 说明 |
|---|---|
| `scenario_id` | 场景标识符 |
| `baseline_avg_ms` | 历史 baseline 的 `avg_ms` |
| `current_avg_ms` | 当前运行的 `avg_ms` |
| `delta_pct` | `(current - baseline) / baseline * 100` |
| `regression` | `delta_pct > --threshold-pct` |

## 样本限制

- 默认 10 次迭代。可通过 `--iterations` 调整。
- `--iterations 0` 会被拒绝；所有场景必须至少运行一次。
- 每次迭代独立计时；预热由 `--warmup` 控制，不计入样本。
- 测量使用 `std::time::Instant`，精度取决于 OS（通常 1 µs 或更优）。
- 内存事件存储场景每次迭代追加 100 个事件。扩展规模场景覆盖 1k/10k/100k 原子追加。每次迭代使用独立 store 和会话，避免累计事件影响固定规模指标。
- `event_store_append_list_range_100k` 当 `--iterations > 1` 时自动限制为 1 次迭代以避免过慢，并设置 `iterations_capped=true`。
- `EventStore::append_with_sequence` 提供原子追加，保证同一会话并发时不会产生重复 sequence。
- `EventStore::list_kind_prefix` 和 `list_session_kind_prefix` 提供查询下推。审计和范围查询不再常规执行 `list_all()` 后全量过滤。
- 不使用 criterion 或统计框架；当前目标是建立开发机参考，不是 CI 合规预算。

## 红线

- 不做第一方 Package fast path。第一方 Package 和第三方包走同一路由与权限边界。
- 不绕过权限、钩子、schema、脱敏或审计。
- 不需要真实网络或 provider。
- 不修改 runtime 边界或公开协议。

## 后续优化追踪指标

后续优化可以用这些指标做前后对比：

1. in-process 调用延迟 — 如果引入 resolve cache 或 handler table，应观察该指标变化。
2. 事件存储批量吞吐 — 100 个事件、1k、10k、100k 的 append/list/range/kind-prefix 延迟都可比较。
3. 事件存储规模趋势 — 用 1k/10k/100k 场景观察跨版本增长曲线。
4. Work check 延迟 — source containment、Package projection 与递归 resolver 的变化应由该场景反映。
5. profile load 延迟 — 作为 YAML 解析基线；profile 变大后应重新测量。
6. 子进程调用延迟 — 需要稳定的子进程环境再做比较。
7. 出站 fake executor 与 fake stream — 作为出站审计/策略路径的无网络参考。
8. 后续 UI 优化应另用前端诊断比较 HTML bytes 和 elapsed_ms。

当前人测前基线还应特别关注 install/profile/surface/security bridge 路径，确保项目安装、profile 加载、静态 bundle 暴露和 bridge 安全边界没有明显回归。

## 样本参考输出

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

以上数值来自特定开发机，仅作参考。不作为 CI 合规预算。
