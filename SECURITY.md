# Security Policy

## Reporting a vulnerability

Please **do not** file a public issue for a security vulnerability. Instead,
report it privately so it can be addressed before disclosure.

The best way to reach the maintainers is through GitHub's private vulnerability
reporting on this repository:

- [https://github.com/eddiesr93/unstave/security/advisories/new](https://github.com/eddiesr93/unstave/security/advisories/new)

Alternatively, email the maintainers and include as much detail as possible:

- A description of the vulnerability and its impact.
- Steps to reproduce (or a minimal proof of concept).
- Affected versions and components.
- Any suggested fix, if known.

You should receive an acknowledgement, and we will work with you to understand
and address the issue. Please allow time for a fix to be prepared and released
before public disclosure.

## Scope

The following components are in scope for security reports:

- The Rust analysis core (`unstave-core`) and renderers (`unstave-report`).
- The codemod (`unstave-codemod`).
- The Node-API native binding (`unstave-napi` / `@unstave/node`).
- The Vite plugin (`@unstave/vite-plugin`).
- The release pipeline and distribution artifacts, including the Homebrew
  formula (`Formula/unstave.rb`).

Out of scope are third-party dependencies themselves; report issues in
upstream projects to their maintainers. A vulnerability that is the direct
result of a third-party dependency may still be reported here so we can track
and update the affected version.

## Supported versions

This project is **pre-1.0**. Only the latest released version is actively
supported with security fixes. When a vulnerability is fixed, the fix is
released in the next version and backported to older releases only when
feasible and practical.

| Version | Supported |
| --- | --- |
| latest | ✅ |
| older (pre-1.0) | ❌ |
