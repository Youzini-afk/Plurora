# Plurora Agent Instructions

## Active implementation program

The repository is currently executing the destructive Work / Assembly / Installation / Run / Realization program.

Before changing any file, read the complete execution brief:

- Chinese: [`docs/roadmap/WORK_ASSEMBLY_REALIZATION.md`](docs/roadmap/WORK_ASSEMBLY_REALIZATION.md)
- English: [`docs/roadmap/WORK_ASSEMBLY_REALIZATION.en.md`](docs/roadmap/WORK_ASSEMBLY_REALIZATION.en.md)

Then read every prerequisite listed in section 0 of that brief. The brief defines the target boundary; source code and generated artifacts define the current implementation state.

Do not stop after summarizing or rewriting the plan. Begin the next incomplete Phase and carry it to a complete Phase boundary.

## Git workflow

- The intended implementation branch is `feature/work-assembly-realization`.
- When the environment allows branch creation, create it from the latest clean `main` before Phase 1.
- When the agent environment forbids creating branches, the task must be started with that branch already selected; do not silently implement the program on `main`.
- Each Phase ends with a clean worktree, one reviewable commit, and a push before the next Phase begins.
- Do not amend or rewrite completed Phase commits. Fix a failed Phase with an explicit follow-up commit.
- Observe GitHub CI for the pushed commit. Do not begin the next Phase while the current Phase CI is failing.

## Non-negotiable boundaries

- This is a pre-release destructive refactor. Do not retain compatibility aliases, readers, fallback directories, duplicate RPCs, or migration code for Project, old Composition, or Deployment identities.
- Destructive contracts do not permit destructive user-file behavior. Preserve path containment, symlink defenses, and the rule that linked-local sources are never deleted.
- Do not modify YdlTavern, release configuration, marketplace/payment/DRM systems, or unrelated products.
- Work, Assembly, Game, Library, and Realization are not Constitutional Substrate concepts.
- First-party Packages, Shells, and agents have no private API, publisher priority, hidden authority, ambient root shell, or secret bypass.
- Plans and agent output do not grant execution authority. External effects require persisted plans or ChangeSets, current authority, explicit policy decisions, and receipts.
- Do not hand-edit generated schemas, OpenAPI, Rust SDK, or TypeScript SDK. Change the registry/generator source and regenerate.
- Keep Chinese and English documentation factually synchronized in the same commit.

## Validation discipline

Run bounded checks locally as specified by the active Phase, including where relevant:

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

Full workspace Rust tests, complete conformance, Docker, Windows backup/restore, Desktop sidecar smoke, and Host operations acceptance belong in GitHub CI unless the task explicitly changes that policy.

## Phase report

At each Phase boundary report:

```text
Phase and commit SHA
User capability and architectural boundary completed
Retired identity or debt removed
Bounded local checks
GitHub CI run and conclusion
Any deviation from the execution brief and its reason
Input preconditions for the next Phase
```

## Completion

Phase 9 removes the temporary execution brief and this active-program instruction. Durable semantics move into architecture, specification, guide, product, and status documents. Replace this file with general repository instructions only when they remain useful after the program; otherwise delete it.
