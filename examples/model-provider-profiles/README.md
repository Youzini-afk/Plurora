# Model provider profile examples

> [English](./README.en.md) · [中文](./README.md)

这些示例演示 `official/model-provider-lab` 与 `sdk/typescript/model-provider-adapter` 可接受的 provider profile 形态。它们只包含 `secret_ref`，不包含真实 API key。

示例用途：

- 作为 `validate_profile` / `normalize_request` / `invoke` / `normalize_stream` 的输入参考；
- 作为第三方 provider 包的 profile shape 参考；
- 验证 Plurora 支持多 provider API 差异，而不是只支持 OpenAI-compatible。

当前文件：

- `openai.json`
- `anthropic.json`
- `gemini.json`
- `openai-compatible.json`
- `openrouter.json`
- `deepseek.json`
- `xai.json`
- `fireworks.json`

这些示例不会触发真实网络调用。Alpha 默认 provider invoke path 仍是 fake/local。
