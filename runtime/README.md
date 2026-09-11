# Runtime

This directory is reserved for the Open-EQMS Runtime.

The Runtime contains only universal platform capabilities defined by Architecture Baseline v1.0. Business methods do not belong here.

Internal structure under `runtime/` will be created only through approved SPECs and ADRs. Do not add component subdirectories, dependencies, crates, frameworks, or implementation code without approved architecture and specification coverage.

Current approved crates:

- `runtime/runtime-contracts` — shared dependency-neutral Runtime data contracts from ADR-0001.
- `runtime/object-runtime` — SPEC-001 Object Runtime.
- `runtime/event-engine` — SPEC-002 Event Engine.
- `runtime/transaction-engine` — SPEC-003 Transaction Engine.
- `runtime/query-engine` — SPEC-004 Query Engine (finite one-shot execution over read-only `QuerySource` providers, with validation, authorization filtering, consistency-boundary recording, provenance, classification, projection, sorting, aggregation, deterministic continuation tokens, SavedQuery catalog registration/replay, bounded Context Package assembly, local bounded Parallel Inquiry Fabric execution, limits, and cancellation; no concrete engine adapters, Security implementation, concrete SavedQuery persistence, richer multi-source Context Package population, or distributed inquiry execution yet).
- `runtime/process-engine` — SPEC-006 Process Engine V1 (bounded process-definition registry, graph-legal Object transition orchestration, paired Object/Transaction Unit-of-Work commit, and independent transition Event append; no GUI forms, role enforcement, Rule Engine evaluation, KPI effects, package loading, or process-definition persistence yet).
- `runtime/security` — SPEC-007 Security minimal authorization provider (deterministic in-memory exact-match authorization over the shared ADR-0004 contract only; no authentication, enterprise IAM, cryptography, signing, databases, networking, package loading, GUI filtering, or regulated deployment claims).

The Runtime must preserve deterministic behavior, offline-first operation, auditability, immutable facts, localization, and the Runtime / Content Package / Plugin boundary.
