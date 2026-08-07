# Yggdrasil Charter

> [English](./CHARTER.en.md) · [中文](./CHARTER.md)

Yggdrasil is an open digital platform for people, AI, and software to create and operate together.

It enables applications, tools, services, worlds, games, agents, creative environments, and forms not yet named to be created, composed, run, inspected, modified, moved, and replaced. The platform supplies reliable common ground without prescribing what those forms must become.

This charter defines the identity, goals, and boundaries that Yggdrasil does not change casually.

## Platform identity

Yggdrasil is not only a kernel, and it is not identical to one official interface. The complete platform includes:

- a deliberately small constitutional substrate that owns only generic mechanisms;
- an evolvable and competitive ecosystem of public protocols and components;
- Host and runtime infrastructure that manages real machines, networks, and data lifecycles;
- a complete and usable official distribution that remains replaceable;
- third-party products, tools, services, and experiences built above it.

Lower layers enable higher layers. A higher layer must not turn its product ontology into the physical law of the whole platform.

## Long-term construction goals

### 1. Be as open as practical

Openness means more than visible source code. Yggdrasil also aims for:

- public first-party implementations, protocols, data formats, and extension points;
- user ability to read, export, back up, migrate, and delete their data;
- no private API, hidden authority, or irreplaceable routing priority for official implementations;
- replaceable clients, models, agents, components, storage, executors, Hosts, and products;
- first-class local, self-hosted, and offline use;
- equal third-party participation without repository-internal knowledge.

### 2. Maximize possibility and plurality

The platform does not prescribe one application form, workflow, interaction model, or content ontology. Products may choose different protocols, shells, components, data models, and execution modes, and radically different systems may coexist in the same platform ecosystem.

Plurality comes from independently replaceable dimensions, not from adding endless switches to one fixed product.

### 3. Pursue advanced technology that creates real value

Yggdrasil deliberately adopts technology that materially increases freedom, security, performance, portability, or maintainability: capability security, content addressing, portable components, local-first operation, multiple Hosts, streaming and cancellation, explicit causality, effect receipts, and protocol negotiation.

“Advanced” does not mean collecting new terminology. Technology that adds complexity without a meaningful capability or long-term return does not belong in the platform core.

### 4. Evolve for the long term

The platform must keep changing without tearing user data and the ecosystem apart:

- keep stable layers small while upper layers compete, fork, and change;
- give public contracts maturity levels, versions, compatibility windows, deprecation, and migration;
- preserve, copy, and transfer unknown fields and unknown artifacts;
- keep old content readable without requiring a surviving official service;
- let failed abstractions retire instead of accumulating forever;
- keep implementation frameworks, languages, and vendors out of the constitution.

### 5. Be genuinely usable

An open platform must not use neutrality as an excuse to hand all complexity to users. The official distribution must provide a coherent, reliable, recoverable default experience: useful after installation, simple for simple tasks, progressively disclosed for advanced work, understandable about authority and risk, clear about data, and fast for creators to begin with.

The official experience may be opinionated, but it must achieve usability through public boundaries rather than platform privilege.

## Non-negotiable principles

### Users own their data and choices

The platform holds user work, history, and configuration; it does not use them as lock-in. Portability, backup, recovery, and deletion are fundamental capabilities, not post-release extras.

### Authority is explicit

Every cross-boundary action must answer who received which power over what resource, under which conditions, from where, and until when. A declaration is an upper bound rather than a grant; a credential is not a substitute for resource scope and policy.

### Official implementations have no privilege

Official packages, shells, Host profiles, clients, and services use the same registration, authorization, invocation, audit, and migration mechanisms as third parties. Maintainer identity may express responsibility, not implicit authority.

### Public contracts outrank internal convenience

In-process calls, HTTP, stdio, WASM, remote services, and clients may use different transports, but they preserve the same identity, authority, and behavioral meaning. Internal implementations must not depend on ecosystem-inaccessible bypasses.

### The substrate owns only mechanisms that cannot safely move upward

Conversation, models, agents, memory, worlds, games, documents, project workflows, editors, and deployment-product semantics are not constitutional substrate. Shared meaning belongs to protocols, machine operations to the Host, and interaction viewpoints to distributions or products.

### Composition and replacement outrank centralization

Components, protocols, clients, and products can be replaced independently. An official default must not silently become the permanent only choice; ambiguity is resolved by explicit selection, profiles, or policy rather than official priority.

### The default product is not platform law

The official distribution is an important part of Yggdrasil and should be an excellent product. Its Home, Project, Play, Forge, Assist, or future workflows constrain only the distribution and profiles that choose them; they do not constrain every Yggdrasil product.

### Evolution preserves migration paths

Stability does not mean additive-only forever. Breaking change requires version boundaries, migration tools, compatibility periods, and a path for reading old data.

### Quality systems serve construction goals

Tests, conformance, fixtures, dogfood, and reference implementations find defects, prevent regressions, and increase trust. They do not decide why the platform exists and must not become a reason to invent features merely to “prove an abstraction.”

## The place of AI

AI is a first-class participant, not the platform's only center. Humans, assistants, agents, ordinary software components, and automation services all work through explicit identities and capabilities. Models, prompts, memory, agent loops, and inference policy belong to replaceable protocols and components rather than the constitutional substrate.

## Non-goals

Yggdrasil does not aim to:

- unify all digital products into one ontology or workflow;
- judge direction by feature count, test count, or technology count;
- make the official distribution the single form third parties must copy;
- tolerate an unusable, incomplete, or unrecoverable experience in the name of openness;
- create private protocols, hidden authority, or data lock-in in the name of usability;
- disguise arbitrary remote shells, unlimited authority, or undeclared effects as flexibility.

## Stability commitment

This charter changes only through explicit revision. When a future feature conflicts with it, redesign the feature and its layer before promoting product convenience into platform principle.
