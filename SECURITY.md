# Security policy

## Supported versions

unstave is pre-1.0. Security fixes are made for the latest published release and
backported only when feasible. Upgrade to the most recent version before reporting
or validating a vulnerability.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older pre-1.0 releases | No |

## Reporting a vulnerability

Please report suspected vulnerabilities through
[GitHub private vulnerability reporting](https://github.com/eddiesr93/unstave/security/advisories/new).
Do not open a public issue or discussion, and do not include credentials,
proprietary source code, or other sensitive material in a report.

Include the affected version and surface, a minimal proof of concept, the expected
impact, and any mitigations you have already identified. Reports should contain
enough detail to reproduce the behavior without access to a private repository.

The project aims to acknowledge reports within 72 hours and provide an initial
assessment within seven days. These are response targets rather than guarantees.
Validated issues will be coordinated privately until a fix and release notes are
ready.

## Scope

The following components are in scope:

- The Rust analysis core and report renderers.
- The codemod.
- The Node-API native binding and `@unstave/node`.
- `@unstave/vite-plugin`.
- Release automation and distribution artifacts, including the Homebrew formula.

Report vulnerabilities in third-party dependencies to their maintainers. You may
also report a dependency vulnerability here when unstave is directly affected so
the project can track and update the dependency.
