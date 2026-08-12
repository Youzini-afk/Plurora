# Foreign Work, Rights, and opaque state

> [English](./FOREIGN_WORK_RIGHTS.en.md) · [中文](./FOREIGN_WORK_RIGHTS.md)

This guide explains how Plurora brings external URIs, local executables, managed binaries, OCI images, remote services, and entitlement adapters into the Work / Installation / Run model without inventing a special “closed-source project” type or a privileged DRM path.

## Separate identity from location

`ForeignCapsuleDescriptor` is portable, content-addressed Work content. It declares only:

- a stable capsule identity;
- abstract launch requirements;
- optional ordinary `PortDescriptor`s;
- optional `StateSlotDescriptor`s;
- Rights and Transparency artifact references.

It cannot contain an executable path, working directory, URI, endpoint, or image coordinate from a user's machine. Validation recursively inspects annotations and rejects such fields with `RawPath`. Concrete coordinates belong to one Installation and are stored as `plurora.foreign-launch-binding.v1` in the Installation-scoped secret store. Installation public views, events, receipts, and diagnostics never echo that value.

Supported Host-local targets are:

- `external_uri`;
- `local_executable`;
- `managed_artifact`;
- `oci_image`;
- `remote_service`;
- `entitlement_adapter`.

The portable requirement fixes the target kind and launch ID. An Installation binding must match both exactly and cannot use local configuration to change the Work entrypoint's meaning.

## Rights are declarations, not authority

`RightsDeclaration` gives an independent `allowed`, `denied`, `requires_entitlement`, or `unspecified` disposition for ten operations: install, execute, backup, export state, copy across Hosts, redistribute artifacts, modify, derive, modding, and dedicated server.

The current Host policy is evaluated per effect:

- Installation create/update rejects an explicit `install: denied`; other install dispositions remain declarations, while execution is checked again at the Run boundary;
- `execute: denied` or `execute: unspecified` blocks a Run, while `requires_entitlement` can proceed only through an ordinary entitlement capability adapter;
- backup, state export, cross-Host copy, and dedicated server each require an explicit `allowed` disposition;
- to preserve existing install/run behavior for Work without a Rights artifact, a completely absent declaration treats install and execute as allowed; other operations remain `unspecified` and are never performed automatically.

Rights do not mint capabilities, widen Host grants, or constitute a legal ruling. Library presents publisher/source declarations, referenced evidence, and Host-enforced boundaries in separate columns. `TransparencyDeclaration` likewise records source visibility, reproducible-build claims, SBOM/provenance/signature references, telemetry disclosures, and state portability as auditable facts rather than a trust shortcut.

## Run and entitlement

A foreign launch is an ordinary Work entrypoint. Run preflight reads the exact Work revision, launch requirement, Installation revision, and current Rights. A missing binding, mismatched target kind, denied Right, or failed entitlement produces a structured gap/error and never an implicit Realization apply.

Entitlement is an ordinary `capability.invoke`: the binding names a provider Package, capability, and optional version, and the Host accepts only the adapter's allow decision plus an optional launch target. There is no `drm.*` API, copied ownership database, or hidden first-party path. Adapter input, credentials, local coordinates, child stderr, and command details do not enter Debug output, public Installation payloads, or errors.

A `plurora.foreign/dedicated-server` entrypoint also requires `dedicated_server: allowed`. A Realization plan that would copy material across Hosts rechecks `copy_across_hosts: allowed`; an already-present binary does not bypass Rights.

## Protocol Ports and the Binding boundary

Closed source and composability are separate facts. A ForeignCapsule may declare public save, health, mod, lobby, or other ordinary Ports. Compatibility uses the same protocol/interface/version/profile/interaction/effect/transport rules, with no priority from publisher or source visibility.

The declaration alone does not teach the Host how to connect an arbitrary process to a runtime handle. Executable Binding requires an ordinary Component/Host adapter that supplies an explicit transport and provider endpoint. It uses the public Exposure/Powerbox/Binding flow, has no foreign-only bridge, and performs neither ambient discovery nor automatic provider selection. An open-source program with no protocol may likewise remain only a Capsule.

## Opaque state: backup and export are separate

`InstallationStateAction::Backup` snapshots the current state tree at an exact Installation revision and returns a content-addressed `urn:plurora:installation-state-snapshot:v1` receipt. It does not replace, migrate, or clear live state. Restore remains an explicit `replace` update.

Two separate Rights are enforced:

- creating the snapshot requires `backup: allowed`;
- downloading it through `object.get` also requires the current Work to declare `export_state: allowed`.

A public read carries both the exact Installation ID and a descriptor actually issued by the Host journal. Knowing a digest, forging a descriptor, or presenting another Installation's receipt is insufficient. Snapshot paths must be sorted, unique, relative, and free of traversal or platform prefixes. Existing containment and symlink defenses remain in force for state effects.

## CLI

Read the current revision first, then use a stable idempotency key:

```bash
plurora installation backup <installation-id> \
  --expected-revision <revision> \
  --idempotency-key <stable-key>

plurora installation bind-foreign <installation-id> <launch-id> \
  --binding binding.json \
  --expected-revision <revision> \
  --idempotency-key <stable-key>
```

`bind-foreign` first admits the exact secret reference into Installation policy, then writes the local binding through the ordinary `plurora/secret-store-lab/put_installation_secret` capability. The CLI never prints binding content. If the outcome is uncertain, reread the Installation revision and Run state before deciding whether to retry with the same key.

## Web / PWA

Home and Installation Frame read Rights/Transparency only from the public `host.installation.*` summary. Foreign Work cards show origin, trust, declarations, evidence, and Host enforcement. Local launch bindings are written through controlled input and never read back from the public summary. Backup and Download are separate affordances; closing a page neither stops a Run nor implicitly backs up, exports, or starts anything.

## Verifiable boundary

Foreign Work conformance covers a Capsule without protocols, rejection of portable coordinates, closed-source Ports using the ordinary compatibility contract, conservative Rights, redacted entitlement with no DRM method, and dedicated entrypoint plus public backup schemas. Deeper state/launch lifecycles, Realization Rights gates, Installation journaling, and Web behavior are covered by their crate, service, and TypeScript tests.
