# Path B: Self-Contained Packages

> [English](./PATH_B_SELF_CONTAINED.en.md) · [中文](./PATH_B_SELF_CONTAINED.md)

Path B is the opt-out mode for kernel v1. A package sets `entry.contract: "none"` to declare that it does not participate in v1 capability / permission enforcement. The kernel still hosts it, records lifecycle, and exposes operator visibility, but does not inject capability handles.

## What Path B is

A Path B package is a self-contained app or tool. It can run as a process managed by the Plurora host, but it does not gain platform authority through manifest permissions, does not use `kernel.v1.capability.invoke`, and does not depend on v1 bindings.

This is not a second-class path. It is a first-class mode for migration, compatibility layers, existing tools, and prototypes.

## When to use it

- Porting an existing app and first getting it under host lifecycle.
- Running third-party tools that do not need Plurora capabilities.
- Prototyping before the authority boundary is known.
- Needing only host start/stop/observe behavior, not platform permissions.
- Bringing your own network, storage, or UI sandbox and accepting that v1 does not enforce it.

## When not to use it

Do not choose Path B when:

- the package needs to invoke other capability providers;
- the package needs manifest-declared network access;
- the package needs host-injected secrets via `secret_ref`;
- the package needs events.read / events.append authority;
- the package needs declared-vs-used authority audit;
- the package should be a reusable platform capability for other packages.

Use Path A (`entry.contract: "v1"`) for those scenarios.

## Manifest shape

Minimum shape:

```yaml
id: example/self-contained
version: 0.1.0
entry:
  kind: subprocess
  contract: "none"
  command: ["./run-example"]
```

Path B can keep descriptive metadata, surface descriptors, or host startup fields, but permission declarations do not mint v1 capability handles.

## What changes

| Behavior | Path A | Path B |
|---|---|---|
| Manifest permission enforcement | yes | no |
| Capability handles | injected | not injected |
| Reverse kernel calls | handle-scoped | unavailable or denied by host policy |
| Secret resolution | `secret_ref` + host resolver | no v1 secret binding |
| Network outbound | `kernel.v1.outbound.*` + audit | not managed by v1 outbound |
| Lifecycle events | emitted | emitted |
| Package logs | capturable | capturable |
| Conformance kit | authority and invocation checks | self-contained and observable checks |

## Lifecycle and audit visibility

Path B still emits package lifecycle events: loading, starting, ready, stopping, stopped, unloaded, degraded, and log. Event payloads should include or make derivable:

```json
{ "contract_mode": "none" }
```

This lets operators distinguish a package managed by the host from one protected by v1 capability enforcement.

## Security meaning

Path B is an explicit trust boundary. The kernel does not claim to intercept every side effect. The host can still use OS sandboxing, containers, profile policy, filesystem permissions, network isolation, or user prompts, but those are not the v1 capability contract.

If you need auditable, revocable, least-authority capabilities, use Path A.

## Conformance behavior

`plurora conformance package --contract v1 --path <package>` detects `entry.contract: "none"`:

- manifest parse: PASS/FAIL;
- contract mode: PASS and marked Path B;
- entry support: PASS/FAIL;
- bindings, capability, permission: SKIP or WARNING;
- lifecycle visibility: PASS/FAIL;
- fixture invocation: self-contained smoke only.

A Path B package can be 100% compliant because inapplicable checks are not counted.

## Path A vs Path B

| Dimension | Path A (`v1`) | Path B (`none`) |
|---|---|---|
| Main use | platform capability package | self-contained app / migration tool |
| Authority source | kernel handles | package itself / host external policy |
| Manifest permissions | enforced | descriptive; no v1 authority |
| SDK | generated SDK + bindings | SDK optional |
| Audit | declared vs used | lifecycle + mode marker |
| Third-party reuse | yes | usually no |
| Least authority | kernel-enforced | host/OS-managed |

## Migration advice

You can start with Path B and later move to Path A:

1. Collect actual side effects and call needs.
2. Declare network, secret, and capability use in the manifest.
3. Adopt bindings and SDK.
4. Run the conformance kit.
5. Switch `entry.contract` to `"v1"`.

## Operator tips

- Treat Path B as an external-process trust boundary, not a v1 sandbox.
- Display `contract_mode: none` in package lists and dashboards.
- Use more explicit profile approval for Path B packages.
- If a Path B package needs network or filesystem access, prefer OS/container policy.
- Periodically evaluate whether it should migrate to Path A.

## Package-author tips

- Explain in README why Path B was chosen.
- Do not imply in the manifest that v1 permissions will be enforced.
- If secrets or outbound may be needed later, design a migration path early.
- Preserve stdout/stderr conventions so host lifecycle is not broken.
- Provide a health check or minimal smoke so the conformance kit can verify startup.

## Common misunderstanding

Path B is not a synonym for "unsafe mode". It only means the security boundary is not provided by the kernel v1 capability contract. A Path B package can still be protected by OS sandboxing, containers, read-only filesystems, network isolation, and human approval.

Path B is also not an audit bypass. The host should still record lifecycle, logs, exit status, and contract mode; it simply will not produce declared-vs-used authority reports.

## References

- [`../spec/KERNEL_V1_CONTRACT.md`](../spec/KERNEL_V1_CONTRACT.en.md)
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.en.md)
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.en.md)
