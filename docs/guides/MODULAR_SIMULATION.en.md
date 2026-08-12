# Modular Simulation Work Kit

> [English](./MODULAR_SIMULATION.en.md) · [中文](./MODULAR_SIMULATION.md)

`modular-simulation` is the first Work kit intended for continued creation rather than a one-off demo. It uses a rule-driven, state-rich 5×5 colony-management scenario to prove that Work, Assembly, Ports, portable state, Surfaces, Runs, Powerbox, Realization, and replacement form one coherent path without introducing a general 3D engine or a first-party private interface.

## Composition

| Part | Implementation | Boundary |
|---|---|---|
| Simulation Component | `plurora/modular-simulation` Rust `rust_inproc` Package | all state crosses the call boundary as input/output; no process-global game state, implicit network, or secret |
| Input Port | Assembly export `input` → `apply_input` | `build`, `assign`, and `advance` are deterministic reducer operations |
| Portable save | Assembly export `save` → `export_save`, Installation-scoped `save` StateSlot | canonical JSON plus SHA-256; backup required; schema `modular-simulation.save.v1` |
| Optional AI | Runtime import `ai-advisor` plus a separate `example/ai-advisor` provider Work | invoked only through an explicit Powerbox Binding; even a loaded provider yields `binding_unavailable` without a Binding and is never discovered ambiently |
| Web Surfaces | static `surface_bundle` in `plurora/modular-simulation-renderer` | `play_renderer` and `asset_editor`; only allowlisted capabilities cross the Surface bridge |
| Local Run | `host.run.*` plus `AssemblyRuntimeDriver` | exact AssemblyLock/Component pins, a Run session, and Package leases; closing a page does not stop the Run |
| Remote-server fork | `example/modular-simulation-server` Work | its `OperationalIntent` produces an effect-free Realization plan first; no implicit build or apply |
| Community replacement | `community/modular-simulation` | the same manifest, Component, Port, and provider-routing rules as the first-party implementation; two available providers require explicit selection |

“Isolation” here first means the Component, state, and authority boundaries. The reference Rust implementation currently uses the Host catalog's `rust_inproc` backend, so it is not an OS process sandbox. It gets no extra authority from being first party. A distribution that requires process isolation can replace it with a `subprocess` implementation while preserving the same Component, Port, and state contracts.

## Run and inspect

```bash
plurora work check examples/works/modular-simulation --json
plurora work check examples/works/modular-simulation-community --json
plurora work check examples/works/modular-simulation-server --json
plurora work check examples/works/modular-simulation-ai-advisor --json

plurora work promote-component examples/works/modular-simulation \
  --node simulation \
  --assembly-id example/modular-simulation-core \
  --json

plurora conformance --tag modular_simulation
```

Forge profiles autoload the simulation, renderer, and local adviser Packages. To use the adviser, install/run `example/modular-simulation-ai-advisor` as an ordinary Work, create its Exposure, and explicitly select a Binding for the consumer's `ai-advisor` Port. After the base Work is installed, Library Play still performs Run preflight first. Missing local artifacts, exact Component pins, or required Bindings produce structured gaps and never trigger Realization apply. Even when a compatible capability is loaded, `request_ai_move` can use only the Binding selected and revalidated for the current Run and Port.

## State, forks, and replacement

Simulation state is a versioned portable document rather than a Package singleton:

- `create_state` creates v1 state;
- `apply_input` returns the next revision without mutating hidden state;
- `export_save` computes a stable digest over canonical bytes;
- `migrate_save` explicitly converts v0 to v1 and is idempotent for existing v1 state;
- `inspect_state` returns structured statistics only;
- `server_tick` uses the same reducer for the server entrypoint.

`examples/works/modular-simulation-community` replaces the simulation node with an ordinary third-party Package while retaining compatible Ports and the save schema. Durable-state replacement still follows StateSlot rules: an incompatible schema, owner, or portability claim requires a migration Port or an explicit user-approved reset. `examples/works/modular-simulation-server` is a separate WorkRevision fork; its remote-server intent neither mutates the base Work nor creates a Realization automatically.

## Promote to reusable component

`plurora work promote-component` accepts one or more root Assembly nodes and deterministically computes:

- bindings retained inside the selected subgraph;
- cut external imports and exports;
- existing root exposures;
- StateSlots owned by selected nodes;
- a new nested Assembly plus structured diagnostics.

The result is a content-addressed `AssemblyPromotionCandidate` and nested `AssemblyRevision`. The command explicitly reports `persisted:false` and `published:false`: it writes no ObjectStore object, changes no Work, installs no Package, and does not let an agent publish. A creator can review the candidate and persist or publish it only through a separate authorized change.

## Layout

- base Work: `examples/works/modular-simulation/`
- server fork: `examples/works/modular-simulation-server/`
- community replacement: `examples/works/modular-simulation-community/`
- optional adviser provider: `examples/works/modular-simulation-ai-advisor/`
- Rust Package: `packages/plurora/modular-simulation/`
- static renderer: `packages/plurora/modular-simulation-renderer/`
- community Package: `examples/packages/community-modular-simulation/`
- adviser Package: `examples/packages/modular-simulation-ai-advisor/`

These directories are inspectable source inputs. Runtime identity still comes from generated WorkRevision, AssemblyRevision, AssemblyLock, Package-envelope, and digest values; a path is never authority.
