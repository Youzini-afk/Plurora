# Documentation Style and Red Lines

> [English](./STYLE.en.md) · [中文](./STYLE.md)

These are the minimum documentation rules for the Plurora repository. They keep platform identity, long-term architecture, official product choices, current implementation, and construction direction distinct, so development history or one product profile cannot redefine the whole platform.

## Documentation truth hierarchy

When documents conflict, interpret them in this order:

1. [`CHARTER.md`](CHARTER.en.md) defines platform identity, long-term goals, and non-negotiable principles;
2. [`architecture/VISION.md`](architecture/VISION.en.md) and long-term architecture documents define layers, ownership, and evolution direction;
3. [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) defines product responsibility of the official distribution; a specific Product Profile constrains only products that adopt it;
4. contracts, specs, and guides define current public behavior and usage;
5. [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) states current implementation facts; code, generated schemas, and CI are final evidence for concrete counts and behavior;
6. [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) describes construction direction and trade-offs, not implemented fact or permanent commitment;
7. historical implementation plans and Git history explain how the repository arrived here but cannot override newer long-term documents or current status.

Changing platform identity requires explicit Charter revision. Changing official product opinion must not silently rewrite the substrate. An implementation change should update related status, contracts, or guides in the same commit.

## Identity and naming

Repository identity is part of the public contract, not decorative wording.

- Product and project name: `Plurora` in prose, `plurora` in machine identifiers.
- Public method IDs use owner-based dot namespaces: `context.*`, `journal.*`, `capability.*`, `authority.*`, `object.*`, `identity.*`, `host.*`, `protocol.*`, `change.*`, `projection.*`, and `shell.*`.
- Platform-owned event kinds use the explicit registry and semantic slash namespaces; they are written by `plurora/runtime`.
- Package capability and event IDs begin with the exact Package ID followed by `/`.
- First-party Packages use publisher namespace `plurora/*`. That identity grants no authority, routing priority, UI privilege, or substrate ownership.
- Generated schemas, OpenAPI, and SDK identities come from the executable registry and generators; do not hand-edit parallel names.
- The pre-release tree keeps one selected identity set. Do not add old-name aliases, fallback environment variables, duplicate CLI entry points, or compatibility routes merely to make a rename appear safer.
- A stable breaking change uses an explicit new contract/profile/version boundary and migration plan, not a hidden alias.

Durable documents name the current identity directly. Historical rename plans are deleted after their conclusions move into architecture, specs, status, and checks.

## Write for readers, not as a development log

Readers care what something is, why it belongs to a layer, how to use it, where its boundary lies, and what happens on failure.

Write about:

- what platform, protocols, components, Host, distributions, and products own;
- how to run, install, invoke, debug, migrate, and recover;
- accurate `implemented`, `partial`, and `deferred` state;
- authority, data, error, cancellation, compatibility, and migration boundaries.

Do not write:

- commit-log narration such as “we recently completed X” or “Round 10A.4 advanced Y”;
- phase names, test counts, or feature counts merely to display effort;
- candidate direction as implemented fact;
- completed temporary plans on the shortest reading path forever.

## Do not substitute phase numbers for meaning

Long-lived documents do not use temporary labels such as `Round X`, `Phase Y`, `T-track`, or `U-track`. Name stable meaning directly: “device authority,” “artifact lifecycle,” or “remote Component.”

Use status terms consistently:

- `implemented`: the public path is operational;
- `partial`: substantial capability exists but a boundary or lifecycle remains incomplete;
- `experimental` / `candidate`: maturity is not Stable;
- `deferred` / `planned`: not implemented or intentionally postponed.

A one-time implementation plan may live in `docs/roadmap/`, but is deleted on completion, with durable conclusions moved into architecture, spec, guide, or status documents.

## Distinguish platform, distribution, and Product Profile

- Platform documents must not make Project, Home, Play, Forge, Assist, Tavern, chat, worlds, or deployment the only center of Plurora;
- the official distribution may be opinionated but identifies its organization as a replaceable product choice;
- a Product Profile may constrain participants that adopt it but is not mandatory ontology for every product;
- when a product need motivates lower-layer work, documentation identifies whether the result belongs to a Protocol, Host, or a substrate mechanism that truly cannot move upward;
- convenience of the current first-party implementation is not a reason to enter the substrate.

## Proper role of tests and conformance

Tests, fixtures, conformance, dogfood, and external integrations are used to:

- find defects;
- constrain public behavior;
- prevent regressions;
- measure performance, compatibility, and reliability;
- support current status statements.

Do not present them as the purpose of the project or invent functionality merely to “prove an abstraction.” First explain what is being built for users, creators, or the ecosystem; then explain how quality systems preserve it.

Use precise terms:

- a concrete Package or repository used by tests → `fixture`, `integration fixture`, `compatibility case`;
- conditions required before stability → `adoption condition`, `compatibility condition`, `fitness condition`;
- do not call a product a “pressure source” or “platform proof.”

## Concept documents and status documents

### Long-term and concept documents

`CHARTER`, `VISION`, `ARCHITECTURE`, `CONSTITUTIONAL_SUBSTRATE`, `CAPABILITY_PACKAGE`, `PLATFORM_PRODUCT_MODEL`, candidate constitutions, and stable protocol documents describe goals, ownership, mechanisms, and long-term boundaries.

They are not polluted by individual commits or phases, but must change when platform goals, architectural ownership, or the contract itself changes.

### Current status documents

`ALPHA_STATUS`, roadmaps, and compatibility or conformance matrices may contain versions, counts, `implemented` / `partial` / `deferred`, and current limitations, but are still not development logs.

### Guides

A guide describes the current path for a reader to complete a task and states whether it belongs to the platform, Host, official distribution, or a Profile. A current UI concept does not become platform constitution merely because a guide uses it.

## Verifiable implementation facts

Check code, generated artifacts, or CI before documenting:

- Web and Desktop frameworks and lifecycle;
- method, event, schema, test, or Package counts;
- supported execution forms, databases, and transports;
- authority enforcement, secrets, deployment, recovery, and migration behavior;
- compatibility or external-integration coverage.

Avoid copying fast-drifting counts into many documents. Exact method / event / schema / conformance counts belong only in [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) and spec tables anchored by `scripts/check-identity.py`. Other documents should point at those sources or `plurora conformance --list`.

## Mechanical checks

Before committing documentation changes, run:

```bash
python3 scripts/check-docs.py
python3 scripts/check-identity.py
```

The script checks repository-local relative links, Chinese counterparts for English documents, and complete cross-links in documents that already use the standard language switch. It does not judge platform direction, architecture opinions, or wording, and does not replace human review.

## Chinese / English synchronization

Maintain primary narrative, navigation, and guides in both languages:

- Chinese defaults to `xxx.md`, English to `xxx.en.md`;
- use `> [English](./xxx.en.md) · [中文](./xxx.md)` at the top;
- update both languages in the same commit; wording may differ, but facts, status, boundaries, and links match;
- generated inventories and ecosystem-standard npm/cargo README files may remain English-only.

## Documentation red lines

- ❌ Do not include raw stderr, API keys, tokens, passwords, or raw secrets; use `secret_ref` examples.
- ❌ Do not include a specific user's absolute Host path in reader-facing guides; use conventions such as `~/.plurora/<area>/`.
- ❌ Do not claim coverage or interoperability without code, fixtures, or compatibility checks.
- ❌ Do not move YdlTavern or another product's chat, character, or prompt semantics into the platform substrate.
- ❌ Do not treat an first-party Package ID, UI slot, or default provider as authority, routing priority, or permanent ontology.
- ❌ Do not use openness to excuse unusable, incomplete, or unrecoverable products; do not use usability to excuse private APIs or data lock-in.
- ❌ Do not retain temporary plans, completed migration checklists, or commit messages as permanent specifications.

## Before adding or substantially changing a document

Ask:

1. Who is the reader, and what must they understand or complete?
2. Is this platform principle, architecture, protocol, Host, distribution, Product Profile, guide, or status?
3. Who owns this meaning and lifecycle?
4. Does it turn a current official choice into a platform-wide requirement?
5. Has current fact been checked against code or generated artifacts?
6. Are error, cancellation, recovery, migration, and deletion paths clear?
7. Are Chinese/English and relative links synchronized?

## One-line summary

**Documentation keeps platform goals, layers, product opinions, and current facts in their proper places; it is stable reference, not a proof document or a development log.**
