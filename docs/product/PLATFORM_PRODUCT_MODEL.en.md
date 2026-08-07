# Platform Product Model

> [English](./PLATFORM_PRODUCT_MODEL.en.md) · [中文](./PLATFORM_PRODUCT_MODEL.md)

This document defines how the official Plurora distribution becomes a complete, usable product without promoting its product choices into mandatory ontology for the entire platform.

## Product responsibility

The lower platform expands possibility; the official distribution makes a coherent, explicit, and changeable set of defaults for users. It is not a protocol debugger usable only by developers, and it does not exist merely to demonstrate substrate capability.

The official distribution is responsible for:

- making first installation, launch, and local use direct;
- managing content, components, projects, data, identities, authority, and Hosts;
- providing reliable everyday operation, diagnostics, updates, backup, recovery, and migration;
- letting creators begin from templates and grow into composition, debugging, extension, and publication;
- exposing real state and control to advanced users without forcing everyone to learn internal architecture first;
- using public boundaries throughout so third-party distributions can reorganize the same platform capabilities.

## Four primary participants

### Users

They want to discover, install, and use applications, tools, and experiences without first learning packages, protocols, or Hosts. The default path must be safe, clear, and recoverable.

### Creators

They may begin by modifying existing content, composing components, or using AI assistance, and later move into custom protocols, data, surfaces, and execution. Creation should not be divided artificially into a closed “player mode” and a completely different developer platform.

### Component and product developers

They need stable SDKs, clear contracts, a debuggable runtime, compatibility and migration tools, publishable artifacts, and integration without official repository internals.

### Host operators

They need installation, updates, identity, authority, resources, networking, logs, health, backup, recovery, and device management, with an understandable account of what the platform actually executed.

One person may hold several roles. The product adapts through progressive disclosure rather than creating disconnected platforms for each role.

## Complete user lifecycle

The official distribution covers at least:

```text
obtain Plurora
→ start locally or connect to a Host
→ discover / import content
→ inspect source, authority, and resource requirements
→ install
→ use
→ update
→ diagnose and recover
→ back up / export / migrate
→ stop / archive / delete
```

Every step needs explicit state, cancellation where meaningful, failure explanation, and a next action. A successful-install happy path alone is not completeness.

## Complete creator lifecycle

```text
create or import
→ run locally
→ observe state and events
→ modify content, composition, or components
→ use human or AI assistance
→ debug and test
→ package
→ share or deploy
→ publish updates and migrations
```

Creator tools may be opinionated but receive no private kernel authority. AI assistance and direct human editing use the same explicit identity, scope, change, and effect boundaries.

## Current organization of the official shell

The current Web/Desktop distribution organizes itself around Home, Settings, Project frames, Project Console, and contributed surfaces. This is an evolving official product structure, not a mandatory structure for all Plurora clients.

- **Home / Library:** discover, install, launch, and resume;
- **Settings / Control:** Hosts, identities, authority, storage, connections, and package management;
- **Project / App frame:** host the primary interface of an installed instance;
- **Workbench / Console:** diagnostics, creation, changes, deployment, and recovery;
- **Contextual assistance:** explanations, suggestions, and controlled operations within explicit scope.

A third-party distribution may have no Home, no Project, or a completely different root object and navigation model. It needs only to obey the public contracts and authority boundaries it adopts.

## Product boundary of Project

Project is a practical model used by the current official Host and distribution to organize installable, runnable instances. It fits applications, workspaces, and some experiences, but it is not the permanent root of all Plurora data and interaction.

Other products may organize around a World, Document, Service, Workspace, Collection, Simulation, or their own protocol objects. The generic substrate must not force those objects to pretend to be Projects, and the Host does not gain their content semantics merely by managing a Project.

## Usability principles

### Make the simple path genuinely simple

Local defaults, discovery, sensible authority suggestions, and clear recovery reduce concepts that a user must learn. Advanced configuration remains available without dominating first use.

### Disclose depth progressively

A user moves from use, light modification, and composition into component and protocol authoring within one product. Depth comes from expandable capability rather than a separate hidden system.

### Make authority understandable

Authority UI explains the operation, target resource, duration, risk, and revocation path rather than showing only internal scope names.

### Prioritize state and recovery

Long-running work, remote connections, installation, updates, deployment, and migration are observable, cancellable where possible, and recoverable. Failure must not require manual database editing.

### Treat data management as first-class product work

Storage use, backup, export, import, retention, deletion, and migration receive the same product attention as installation and running.

### Accessibility, internationalization, and performance are not polish

Keyboard use, screen readers, low-power devices, mobile, multiple languages, and slow networks belong in the design from the beginning.

## Balancing official opinion and platform openness

The official distribution may choose default protocols, components, layouts, and workflows, and it should polish those choices aggressively. The boundary is:

- defaults are visible and replaceable;
- data survives client replacement;
- official components use public interfaces only;
- third parties can provide similar or radically different products;
- official product needs may motivate platform improvements but do not automatically become substrate responsibilities;
- a capability serving only the official interface stays in the distribution or its profile.

## Measuring product completeness

A product is not better merely because it has more features. Ask whether:

- users can complete full lifecycles;
- new users succeed with reasonable defaults;
- advanced users can understand and control real state;
- failure is recoverable;
- creators can grow from simple edits to independent publication;
- work survives replacement of components, clients, or Hosts;
- new capability preserves public boundaries and third-party space.

Current implementation is recorded in [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md), and construction direction in [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.en.md).
