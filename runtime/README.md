# Runtime

This directory is reserved for the Open-EQMS Runtime.

The Runtime contains only universal platform capabilities defined by Architecture Baseline v1.0. Business methods do not belong here.

Internal structure under `runtime/` will be created only through approved SPECs and ADRs. Do not add component subdirectories, dependencies, crates, frameworks, or implementation code without approved architecture and specification coverage.

Current approved crates:

- `runtime/runtime-contracts` — shared dependency-neutral Runtime data contracts from ADR-0001.
- `runtime/object-runtime` — SPEC-001 Object Runtime.
- `runtime/event-engine` — SPEC-002 Event Engine.
- `runtime/transaction-engine` — SPEC-003 Transaction Engine.
- `runtime/query-engine` — SPEC-004 Query Engine (finite one-shot execution over read-only `QuerySource` providers, with validation, authorization filtering, consistency-boundary recording, provenance, classification, projection, sorting, aggregation, SavedQuery catalog registration/replay, bounded Context Package assembly, limits, and cancellation; no concrete engine adapters, Security implementation, concrete SavedQuery persistence, richer multi-source Context Package population, cursors, or parallel execution yet).

The Runtime must preserve deterministic behavior, offline-first operation, auditability, immutable facts, localization, and the Runtime / Content Package / Plugin boundary.
