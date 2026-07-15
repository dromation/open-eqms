# Open-EQMS Agent Instructions

These instructions apply to Codex and future AI agents working in this repository.

The source of truth for technical direction is:

`architecture/Open-EQMS_Architecture_Baseline_v1.0.md`

Do not duplicate the architecture here. Read the source document before changing implementation.

1. Read the Architecture Baseline before changing implementation.
2. Treat the Baseline as the highest technical authority.
3. Read relevant approved ADRs and SPECs.
4. Preserve the Runtime / Content Package / Plugin boundary.
5. Keep all business methods out of the Runtime.
6. Never create a separate Resource Runtime.
7. Never create a separate State Engine.
8. Use the Query Engine as the only structured-query mechanism.
9. Keep rule-evaluation metadata inside Transaction/Audit records.
10. Keep AI external to the Runtime.
11. Keep native plugins outside the Runtime process.
12. Preserve offline-first operation.
13. Preserve deterministic behavior.
14. Preserve auditability and immutable facts.
15. Keep all user-visible content localizable.
16. Avoid speculative dependencies and speculative directories.
17. Keep changes small, reviewable, and reversible.
18. Stop and report when instructions conflict with the Baseline.
19. Require an ADR for architectural changes.
20. Never silently modernize legacy prototype code into the new architecture.
