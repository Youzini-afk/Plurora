# Play-Creation Product Profile

> [English](./PLAY_CREATION_MODEL.en.md) · [中文](./PLAY_CREATION_MODEL.md)

Play-creation is one opinionated product form that Plurora can support: a person can experience a work and also inspect, modify, fork, compose, and continue creating it.

This document defines the product language and surface collaboration of the current official play-creation profile. It is not the only product stance of the whole Plurora platform, and it does not require other applications, services, tools, or distributions to adopt Project, Play, Forge, or Assist.

The general product model is in [`PLATFORM_PRODUCT_MODEL.md`](PLATFORM_PRODUCT_MODEL.en.md).

## Premise

Many creation tools divide people rigidly into consumers of finished work and developers of tools. The play-creation profile makes those behaviors a continuum:

- users can launch work, observe state, save, fork, replace components, and request changes;
- creators debug, compose, extend, and publish through the same public contracts;
- AI assistance and direct editing use explicit identity, authority, change, and approval boundaries;
- deeper creation does not require switching to a developer edition with private platform authority.

## Surfaces in the current profile

These slots belong to the current Shell Profile and Contract V1 compatibility surface, not to the constitutional substrate. Future shells may adopt different slots and interaction structures.

### Home / Play

Home discovers and launches works or projects managed by the current distribution. Play hosts the work's primary surface. The platform owns navigation, authority, and Host connection; the work owns its visual and domain semantics.

### Forge / Workbench

Forge is a workspace for deeper creation and inspection. Events, components, objects, branches, changes, authority, and diagnostics may be observed there. Domain editors come from components or surfaces rather than kernel hard-coding.

### Assist

Assist is contextual assistance. It may explain state, suggest work, draft controlled changes, or call already-authorized capabilities, but it is not a privileged mutation path. Third-party assistants, direct human tools, or automation components may replace it.

## Typical play-creation loop

```text
discover a work
→ launch and use it
→ inspect state and history
→ request a change or fork
→ human or AI tooling produces a candidate
→ review and authorize
→ apply to a new branch or the current work
→ compare, retain, share, or continue creating
```

A product may omit, extend, or reorder these steps. The platform supplies generic identity, authority, objects, history, invocation, and Host capability; adopted protocols and components own the work's semantics.

## Default opinions of the profile

The play-creation profile favors:

- inspectable history and traceable important changes;
- a branch or recoverable point before modification;
- replaceable components without loss of the work;
- third-party surfaces and creation tools;
- AI output as candidate work rather than unlimited authority;
- sharing that includes content, dependencies, compatibility, and migration information rather than only a screenshot or live link.

These opinions constrain products that choose the profile, not other Plurora products.

## Coexistence with other product forms

A document tool, ordinary Web service, IDE, automation system, multiplayer world, or headless agent runtime may ignore the play-creation profile completely. It can still share Plurora identity, authority, objects, invocation, components, Hosts, and protocol evolution.

Likewise, YdlTavern, world simulations, external game engines, and other experiences may adopt all, part, or none of this profile. Official implementations receive no implicit priority.

## Construction boundary

Play-creation needs may reveal missing generic platform capability, but they do not directly move these concepts into the constitutional substrate:

- player, character, message, turn, world, scene, or game-rule semantics;
- Home, Play, Forge, Assist, or a concrete editor;
- one fixed proposal, branch, or publishing workflow;
- official component IDs, default models, or UI state.

A mechanism moves downward only after it is generalized, assigned to the correct layer, and independently useful to other product forms.

## Current implementation

The current official distribution, packages, and templates provide part of a play-creation experience. They are product capability rather than the only measure of platform completeness. Current status is recorded in [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md).
