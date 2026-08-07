# License boundaries

> [English](./LICENSING.en.md) · [中文](./LICENSING.md)

First-party source code in the Yggdrasil repository is licensed under version 3.0 only of the GNU Affero General Public License, identified by the SPDX expression `AGPL-3.0-only`. The complete legal text is in the repository-root [`LICENSE`](../LICENSE).

## First-party code

The following parts consistently use `AGPL-3.0-only`:

- the Rust workspace, CLI, Host, service, and runtime;
- the Web shell and Desktop wrapper;
- repository-maintained Rust and TypeScript SDKs;
- official capability packages under `packages/official/`;
- repository-maintained build, verification, and release scripts.

Unless separately agreed in writing, contributions to these first-party parts enter the repository under the same license.

## Content that is not relicensed

The repository also contains compatibility, test, or integration material that may retain its own license:

- examples or fixtures under `examples/` that explicitly declare another license;
- upstream projects and license metadata recorded under `integrations/`;
- third-party dependencies listed in lockfiles.

Those records do not change the license of Yggdrasil first-party code and do not relicense third-party content as AGPL. A capability package's manifest `license` field describes that package itself; third-party packages should declare their actual license.

## Distribution and network use

The AGPL contains specific source-availability obligations for modification, distribution, and offering a modified version to users over a network. Actual use and distribution are governed by the complete [`LICENSE`](../LICENSE); this document explains repository boundaries and is not a substitute for legal advice.

## Consistency check

CI runs:

```bash
bash scripts/check-license-metadata.sh
```

The check keeps first-party Cargo/npm metadata, root-package lockfile records, and official capability-package manifests on `AGPL-3.0-only`, preventing future license drift.
