# Conformance 反馈环使用指南

> [English](./CONFORMANCE_FEEDBACK.en.md) · [中文](./CONFORMANCE_FEEDBACK.md)

`plurora conformance` 支持过滤、计时和诊断。它用于快速定位失败项和慢项。

## 基本用法

```bash
# 运行全部 conformance cases（默认行为不变）
cargo run -p plurora-cli -- conformance
```

## 列出 case

```bash
# 列出所有 case id 和 tag，不执行
cargo run -p plurora-cli -- conformance --list
```

输出格式：`<case_id>  [<tag1>, <tag2>, ...]`

## 按 case id 过滤

```bash
# 按子串过滤（匹配 case id 中包含指定子串的 case）
cargo run -p plurora-cli -- conformance --case sharing_lab.contract_shape

# 过滤所有 sharing_lab case
cargo run -p plurora-cli -- conformance --case sharing_lab
```

## 按 tag 过滤

```bash
# 按 tag 过滤（case 只需包含任一指定 tag 即被选中）
cargo run -p plurora-cli -- conformance --tag sharing

# 组合多个 tag（OR 语义）
cargo run -p plurora-cli -- conformance --tag network --tag secret
```

## Fail-fast

```bash
# 第一个失败后立即停止
cargo run -p plurora-cli -- conformance --fail-fast
```

## Slowest report

```bash
# 末尾显示最慢 10 个 case（默认）
cargo run -p plurora-cli -- conformance

# 自定义最慢 N
cargo run -p plurora-cli -- conformance --slowest 3
```

## 组合使用

```bash
# 只跑 sharing 相关 case，fail-fast，显示最慢 3
cargo run -p plurora-cli -- conformance --tag sharing --fail-fast --slowest 3

# 按 case id 和 tag 同时过滤（AND 语义：必须同时满足两个条件）
cargo run -p plurora-cli -- conformance --case sharing_lab --tag secret
```

## 可用 tags

| Tag | 说明 |
|---|---|
| runtime | 内核 runtime 行为（会话、事件、能力、钩子等） |
| session | 会话生命周期 |
| event | 事件追加与读取 |
| capability | 能力发现与调用 |
| package | 包加载、卸载、重启 |
| official | 官方包 conformance |
| schema | JSON Schema 验证 |
| protocol | 公开协议分发 |
| permission | 权限与身份 |
| hook | 钩子 fabric 切片 |
| subprocess | 子进程包执行（通常较慢） |
| host | host 诊断与 profile |
| surface | surface contribution |
| proposal | 提案生命周期 |
| asset | 资产注册表 |
| projection | projection 注册表 |
| substrate | SQLite 底座 |
| composition | composition descriptor |
| replacement | 第三方替换证明 |
| generated | 生成的包模板 conformance（通常较慢） |
| secret | Secret reference 与 raw-secret blocking |
| network | 网络权限与出站 |
| outbound | 出站 executor 边界 |
| live | Live model 调用 |
| stream | 流式生命周期 |
| agentic | Agentic Forge |
| experience | experience runtime 与 playable board |
| memory | Memory lab |
| sharing | Sharing lab |
| slow | 已知较慢的 case（子进程启动、生成包模板等） |

## 输出格式

每个 case 输出一行：

```
<case_id>  PASS|FAIL  <duration>
```

末尾汇总最慢 N 项和总体结果。
