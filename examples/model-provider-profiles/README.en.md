# Model provider profile examples

> [English](./README.en.md) · [中文](./README.md)

These examples show provider profile shapes accepted by `official/model-provider-lab` and `sdk/typescript/model-provider-adapter`. They only contain `secret_ref` values and no real API keys.

Use them as:

- input references for `validate_profile`, `normalize_request`, `invoke`, and `normalize_stream`;
- profile-shape references for third-party provider packages;
- proof that Plurora handles multiple provider API shapes rather than only OpenAI-compatible APIs.

Current files:

- `openai.json`
- `anthropic.json`
- `gemini.json`
- `openai-compatible.json`
- `openrouter.json`
- `deepseek.json`
- `xai.json`
- `fireworks.json`

These examples do not trigger real network calls. The Alpha provider invoke path remains fake/local by default.
