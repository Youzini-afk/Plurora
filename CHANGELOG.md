# Changelog

All notable changes to Plurora will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### User-visible

- Official Web/PWA shell with Library, Settings, Installation frames, and Host pairing
- Tauri Desktop wrapper that starts a loopback-only managed Host
- Local Host you can start from source, plus remote Host access after HTTPS pairing
- Work packing, Installation create/update/remove, explicit Run start/stop
- Powerbox for connecting capabilities across Installations
- Realization plans that apply to Docker or a Target Agent only after approval
- Encrypted secret store with `secret_ref`; the UI never reads raw secret values
- `modular-simulation` Work kit as a runnable authoring example
- Documentation entry path: five-minute getting started, contributing, and security policy

### Contributor details

#### Changed
- Reframed Plurora as an open digital creation and runtime platform guided
  by openness, plurality, valuable advanced technology, long-term evolution,
  and usability
- Separated constitutional substrate, Protocol Commons, Components/Content,
  Host control planes, distributions, and products in the architecture docs
- Defined the official distribution as a complete but replaceable product,
  and play-creation as an optional Product Profile rather than platform law
- Reclassified Package as a distribution envelope, Component as an execution
  unit, Project as a current Host/distribution model, and Contract V1 names as
  compatibility surfaces with explicit long-term owners
- Replaced completed temporary roadmap plans with durable architecture, guide,
  status, and construction-direction documentation

#### Added
- Vite bundling and iframe-based SurfaceHost in clients/web
- Tauri 2.x desktop wrapper at clients/desktop
- Loopback-only managed Desktop Host sidecar with a durable SQLite profile,
  random-port readiness handshake, and one-time cookie bootstrap
- Installable Web PWA shell with responsive mobile navigation
- Durable, expiring, revocable Host device grants with action scopes and
  one-time HTTPS pairing
- Mobile control of installations and controlled development through
  the same Host API/RPC boundary
- Durable controlled-development ChangeSet journal, Host lease, scratch
  verification, managed promotion, and recovery
- Durable Realization jobs and deployment revisions with explicit recovery
  and rollback
- Explicit proxy-route exposure: Host-authenticated by default, public vhost
  only after an explicit user choice
- GitHub Actions CI and release workflows
- `scripts/release-version.sh` for version stamping
- `BUILDING.md` with cross-platform build instructions
- This changelog

#### Security
- Query-string Host credentials are accepted only by the two browser SSE
  endpoints; ordinary RPC and Host API requests require Bearer or cookie auth
- Cookie-authenticated mutations enforce same-origin `Origin` when present
- Pairing and access journals persist only domain-separated credential digests

#### Outbound
- `host.outbound.execute` for unary HTTPS
- `host.outbound.stream` for SSE/NDJSON streaming
- `host.outbound.websocket.*` for bidirectional WebSocket
- Outbound completion audit events
- Manifest `permissions.secret_refs` declarations
- Subprocess SDK reverse public-contract dispatch plus WebSocket helper

## [0.1.0] — TBD (initial release)

Initial public release. The user-visible list above is the intended 0.1.0 story;
exact contents will be frozen when the tag is cut.
