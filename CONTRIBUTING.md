# Contributing to Open-EQMS

Thank you for considering a contribution to Open-EQMS.

Open-EQMS is currently in pre-alpha architecture and runtime implementation work. The project is not certified, compliant, production-ready, or suitable for regulated use.

## Project Authority

Before contributing, read:

1. `AGENTS.md`
2. `architecture/Open-EQMS_Architecture_Baseline_v1.0.md`
3. Relevant ADRs under `architecture/adr/`
4. Relevant SPECs under `architecture/spec/`

The Architecture Baseline, approved ADRs, approved SPECs, and repository reality govern implementation. Product vision documents guide direction but do not authorize implementation by themselves.

## Contribution Scope

Good contributions are small, reviewable, deterministic, and aligned with existing crate boundaries.

Do:

- Preserve the Runtime / Content Package / Plugin boundary.
- Keep Runtime crates deterministic and local-first.
- Add focused tests with behavior changes.
- Keep public APIs conservative and documented.
- Use existing crate patterns before adding new abstractions.

Do not:

- Add speculative dependencies or placeholder subsystems.
- Add business semantics inside generic Runtime crates.
- Add architecture changes without an approved ADR or SPEC amendment.
- Claim compliance, certification, production readiness, or regulated-use suitability.
- Modernize legacy prototype code into the new Runtime without explicit approval.

## Development Commands

Run these before proposing a code change:

```powershell
cargo fmt --all -- --check
cargo check --workspace --tests
cargo clippy --workspace --tests -- -D warnings
cargo test --workspace
git diff --check
```

On the current external NTFS workspace, Cargo may print hard-link fallback warnings from the incremental compilation cache. Those warnings are environmental and are not Rust diagnostics.

## Pull Requests

A pull request should include:

- A concise description of the change.
- The ADR/SPEC/requirement that authorizes it.
- A summary of tests run.
- Any intentionally deferred work or known limitation.

Keep unrelated formatting, refactoring, and generated-file churn out of the same pull request.

## Issues

When filing an issue, include:

- What you expected.
- What happened.
- The affected crate, document, or workflow.
- Reproduction steps when applicable.

For security-sensitive reports, follow `SECURITY.md` instead of opening a public issue with exploit details.

