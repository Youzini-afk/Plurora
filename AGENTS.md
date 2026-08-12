# Plurora Agent Instructions

## Repository model

- Read the architecture, specification, guide, product, and status documents that own the area being changed. One-time roadmap documents are not durable contracts.
- Source registries and generators define machine-readable contracts; architecture and specification documents define durable ownership and invariants.
- Keep changes within the requested product or subsystem boundary.

## Non-negotiable boundaries

- Contract changes do not permit destructive user-file behavior. Preserve path containment, symlink defenses, and the rule that linked-local sources are never deleted.
- Do not modify YdlTavern, release configuration, marketplace/payment/DRM systems, or unrelated products.
- Work, Assembly, Game, Library, and Realization are not Constitutional Substrate concepts.
- First-party Packages, Shells, and agents have no private API, publisher priority, hidden authority, ambient root shell, or secret bypass.
- Plans and agent output do not grant execution authority. External effects require persisted plans or ChangeSets, current authority, explicit policy decisions, and receipts.
- Do not hand-edit generated schemas, OpenAPI, Rust SDK, or TypeScript SDK. Change the registry/generator source and regenerate.
- Keep Chinese and English documentation factually synchronized in the same commit.

## Validation discipline

Run checks proportional to the affected area, including where relevant:

```text
cargo fmt --check
cargo metadata --no-deps
target crate tests
affected TypeScript tests and typecheck
python scripts/check-docs.py
python scripts/check-identity.py
git diff --check
generated-output cleanliness checks
```

Full workspace Rust tests, complete conformance, Docker, Windows backup/restore, Desktop sidecar smoke, and Host operations acceptance normally belong in CI unless the task explicitly requires local execution.

## Git workflow

- Preserve unrelated user changes and do not rewrite published history.
- Keep commits reviewable and report the checks actually run, any environment block, and remaining risk.
- Do not merge or push unless the user request or active repository workflow authorizes it.
