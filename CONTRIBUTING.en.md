# Contributing to Plurora

> [English](./CONTRIBUTING.en.md) · [中文](./CONTRIBUTING.md)

Thank you for helping build the platform. This document explains how to make a reviewable change in this repository. See [`docs/CHARTER.md`](docs/CHARTER.en.md) for principles and [`docs/STYLE.md`](docs/STYLE.en.md) for writing rules.

## Get it running

Follow [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.en.md) to start the Host and Web shell. Desktop and release builds are in [`BUILDING.md`](BUILDING.md).

## Which layer are you changing

Decide which kind of fact you are changing before you edit:

| You are changing | Read first | Do not casually change |
|---|---|---|
| Platform principles | [`docs/CHARTER.md`](docs/CHARTER.en.md) | Official UI or a product profile |
| Layering and ownership | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.en.md) | One-time roadmaps |
| Public methods / events | [`docs/spec/PUBLIC_CONTRACT.md`](docs/spec/PUBLIC_CONTRACT.en.md) | Generated artifacts themselves |
| Official product experience | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.en.md) | The constitutional substrate |
| Current implementation facts | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) | Turning status into permanent promises |

Keep one pull request inside one product or subsystem boundary.

## Hard rules

- First-party Packages, shells, and clients have no private API, hidden authority, or bypass.
- Do not hand-edit `docs/spec/v1/schemas/`, `sdk/openapi.yaml`, `sdk/rust/plurora-contract-sdk/`, or `sdk/typescript/contract-sdk/`. Change the registry or type source, then run `scripts/regen-sdks.sh`.
- Linked-local sources are never deleted. Keep path containment and symlink defenses.
- Do not put raw secrets, tokens, or user absolute paths in docs or tests.
- Do not modify YdlTavern, marketplace / payment / DRM systems, or unrelated products.
- Keep Chinese and English documentation factually synchronized in the same commit.

## Changing a contract

1. Edit `PlatformMethod` in `crates/plurora-runtime/src/protocol.rs`, event constants in `crates/plurora-core/src/event.rs`, and the matching domain types.
2. Update `crates/plurora-cli/src/schema_export/` if the export mapping changes.
3. Run `scripts/regen-sdks.sh`.
4. Commit generated schemas and SDKs with the source change.
5. Prefer a named case in `crates/plurora-cli/src/conformance/` over a scattered unit test for behavior regressions.

## Writing documentation

- Chinese default files are `xxx.md`; English files are `xxx.en.md`. Use the standard language switch at the top.
- Write for readers: what it is, how to use it, and where the boundary is. Do not write development logs or phase numbers.
- Exact counts belong only in [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) and spec files anchored by the identity checker.
- Run `python scripts/check-docs.py` and `python scripts/check-identity.py` before you submit documentation.

## Suggested local checks

Choose checks in proportion to the change. You do not need the full suite every time:

```bash
cargo fmt --check
python scripts/check-docs.py
python scripts/check-identity.py
cargo test -p <affected-crate>
npm run check --prefix clients/web
cargo run -p plurora-cli -- conformance --case <pattern>
git diff --check
```

Full workspace tests, complete conformance, Docker, Desktop sidecar smoke, and Host operations acceptance normally belong in CI.

## Pull requests

1. Branch from current `main`.
2. Keep commits reviewable. Do not rewrite published history.
3. Say what changed, why, which checks you ran, and any environment limits.
4. Keep documentation and code fact changes in the same pull request.
5. Do not file public issues for security problems; see [`SECURITY.md`](SECURITY.en.md).

Look for issues labeled `good first issue` when you want a starting task. If you are unsure which layer a change belongs to, start a discussion before changing the substrate.
