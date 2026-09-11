# Security Policy

Open-EQMS is pre-alpha software. It is not certified, compliant, production-ready, or suitable for regulated use.

## Supported Versions

No released version is currently supported for production security response.

Security reports are still welcome for the active `main` branch because they help improve the project before release.

| Version | Supported |
| --- | --- |
| `main` pre-alpha | Best-effort review |
| Any release build | Not currently supported |

## Reporting a Vulnerability

Do not open a public issue containing exploit details, secrets, private data, or step-by-step attack instructions.

Preferred reporting path:

1. Use GitHub private vulnerability reporting if it is enabled for this repository.
2. If private reporting is not available, open a minimal public issue asking for a private security contact. Do not include sensitive details in that issue.

Please include, privately:

- Affected commit, crate, document, or workflow.
- Reproduction steps.
- Expected impact.
- Whether the issue requires local access, repository access, or a networked deployment.
- Any proposed minimal fix, if known.

## Scope

In scope:

- Runtime crate vulnerabilities.
- Unsafe handling of authorization, audit, transaction, or synchronization boundaries.
- Dependency or build-chain issues.
- Documentation that could cause unsafe deployment assumptions.

Out of scope:

- Claims that require a production deployment, hosted service, or cloud environment that this repository does not provide.
- Vulnerabilities in the preserved legacy prototype unless they affect active Runtime work.
- Social engineering, spam, denial-of-service against maintainers, or requests to test third-party systems.

## Disclosure

The maintainers will review reports on a best-effort basis. Because the project is pre-alpha, fixes may be handled as ordinary repository changes unless a private advisory process is available and appropriate.

