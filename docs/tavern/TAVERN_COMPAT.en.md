# SillyTavern-compatible integration project

> [English](./TAVERN_COMPAT.en.md) · [中文](./TAVERN_COMPAT.md)

The independent project that's compatible with SillyTavern's resources and extensions and runs on Plurora is called **YdlTavern**. It lives in its own repository, not inside Plurora.

- Repo: <https://github.com/Youzini-afk/Plurora-Tavern>
- Position: an integration project on top of Plurora, compatible with SillyTavern's character cards, world books, presets, chat history, and extension API.
- Shape: the UI structure and interaction flow stay familiar to longtime SillyTavern users; the frontend is freshly written; the engine layer runs on Plurora.

YdlTavern consumes Plurora through the public protocol. It doesn't read Plurora internals or rely on private APIs — same standing as any other third-party project.

## Why it doesn't live in this repo

Plurora is the platform. Putting a product-grade project — one that talks directly to a specific community and covers a wide compatibility surface — into `packages/plurora/` would immediately violate the charter: "first-party Packages have no privileges."

YdlTavern is large, full of product decisions, and needs its own repo cadence, issue channels, and release cycle. Those are product concerns, not platform concerns. The two stay separate.

## What Plurora keeps providing

These are already in Plurora and YdlTavern will use them directly:

- The public protocol (HTTP `/rpc` plus SSE event subscription).
- `secret_ref`, network declarations, outbound audit, streaming and cancel lifecycle.
- Model integration packages: OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, Fireworks.
- Generic creative capability packages: persona-lab, knowledge-lab, context-lab, text-transform-lab, memory-lab.
- The proposal/approval lifecycle, assets, branches, projections.
- Coming soon: installing capability packages from a GitHub address — YdlTavern's extension ecosystem will benefit.

## What the kernel will never do

No matter how big or important YdlTavern gets:

- The kernel will not understand character cards, world books, presets, or prompt rendering.
- The kernel will not hardcode `{{char}}` / `{{user}}` substitution.
- The kernel will not offer Tavern-specific hooks or methods.
- The kernel will not treat Tavern-shaped packages differently from any other package.

## TavernHeadless research

[`integrations/tavern-headless/`](../../integrations/tavern-headless/) stays in the Plurora repo as research notes. It informs Plurora's generic capability packages (persona / knowledge / context / model-provider). That layer is a platform concern, kept separate from YdlTavern as an integration project.

YdlTavern's actual compatibility roadmap, extension bridge design, and UI structure live in the YdlTavern repo.
