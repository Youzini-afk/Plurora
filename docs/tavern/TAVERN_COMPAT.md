# SillyTavern 兼容接入项目

> [English](./TAVERN_COMPAT.en.md) · [中文](./TAVERN_COMPAT.md)

跑在 Plurora 之上、兼容 SillyTavern 资源与扩展的独立项目叫 **YdlTavern**。它在自己的仓库里，不在 Plurora 仓库内。

- 仓库：<https://github.com/Youzini-afk/Plurora-Tavern>
- 定位：跑在 Plurora 之上的接入项目，兼容 SillyTavern 的角色卡、世界书、预设、聊天历史和扩展 API。
- 形态：UI 结构和操作流对 SillyTavern 老用户保持友好，前端全新写一遍；引擎层走 Plurora。

YdlTavern 通过公开协议消费 Plurora。它不读 Plurora 内部，也不依赖私有 API，跟其他第三方项目没有差别。

## 为什么不放在 Plurora 仓库

Plurora 是平台。把一个产品级、跟特定社区直接对接、UI 与扩展兼容范围大的项目塞进 `packages/plurora/`，会立刻撞到章程：「官方包没有特权」。

YdlTavern 体量很大、产品决策很多。它需要自己的仓库节奏、自己的 issue 频道、自己的 release 周期。这些都是产品话题，不是平台话题。两件事分开。

## Plurora 这边继续提供什么

下面这些是 Plurora 已经有、YdlTavern 会直接用上的：

- 公开协议（HTTP `/rpc` + SSE 事件订阅）。
- `secret_ref`、网络声明、外发审计、流式与取消生命周期。
- 模型接入能力包：OpenAI、Anthropic、Gemini、OpenAI-compatible、OpenRouter、DeepSeek、xAI、Fireworks。
- 通用创作能力包：persona-lab、knowledge-lab、context-lab、text-transform-lab、memory-lab。
- 提案与审批生命周期、资产、分支、projection。
- 后续会做「通过 GitHub 地址安装能力包」的能力——YdlTavern 的扩展生态会受益于此。

## 内核绝不会做的事

无论 YdlTavern 多大、多重要：

- 内核不会理解角色卡、世界书、预设、提示词渲染。
- 内核不会硬编码 `{{char}}` / `{{user}}` 替换。
- 内核不会提供 Tavern 专属的钩子或方法。
- 内核不会区别对待 Tavern 形态的能力包和其他能力包。

## TavernHeadless 调研

[`integrations/tavern-headless/`](../../integrations/tavern-headless/) 仍然作为研究记录留在 Plurora 仓库里——它是 Plurora 通用能力包（persona / knowledge / context / model-provider）的灵感来源。这一层是平台话题，跟 YdlTavern 这个接入项目分开。

YdlTavern 的具体兼容路线、扩展桥设计、UI 结构选择，都在 YdlTavern 仓库里讨论。
