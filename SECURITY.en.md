# Security policy

> [English](./SECURITY.en.md) · [中文](./SECURITY.md)

We take security issues in the Plurora Host, public contracts, and official clients seriously.

## Report privately

Do not open a public issue, discussion, or pull request for a security vulnerability.

Prefer GitHub [Private vulnerability reporting](https://github.com/Youzini-afk/Plurora/security/advisories/new). If that path is unavailable, contact the repository maintainers privately and mark the message as a security report.

## What to include

- Affected version, commit, or runtime (Host, Web, Desktop, CLI);
- The shortest reproduction you can share, without real secrets;
- Practical impact: who can do what, to which resources;
- Any mitigation you already know.

## Scope

Please report:

- Unauthorized access to a Host, Installation, Run, secret, or device grant;
- Path escape, symlink bypass, or deletion of linked-local sources;
- First-party bypasses, hidden authority, or raw secret leakage;
- Official Web / Desktop issues that let one Installation act as another.

Usually out of scope:

- Capabilities that are unimplemented or documented as deferred;
- Expected effects that occur only after an explicit Realization or outbound approval;
- Vulnerabilities in third-party Packages (report those to the Package maintainers).

## Handling

Maintainers will acknowledge the report, assess impact, and coordinate disclosure. Please allow reasonable time and do not publish exploit details before a fix is available.
