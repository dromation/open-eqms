# ADR-0002 — Shared Unit-of-Work Contract for Atomic State + Audit Commits

Status: Approved

Authority: subordinate to Architecture Baseline v1.0; sibling to ADR-0001. Does not restate either.

## Context

Baseline Section 9 requires that a validated state change and its Transaction/Audit record be "written in the same database transaction... so audit trail and state can never drift apart." SPEC-003 (Transaction Engine) identified that achieving this literally, while keeping Object Runtime and the Transaction Engine fully independent of one another (neither may import or call the other), requires a shared, neutral orchestration mechanism that belongs to neither engine. SPEC-003 raised this as two blocking questions rather than deciding it unilaterally. This ADR records the architect's resolution.

## Decision

1. **One opaque Unit-of-Work handle.** A single, neutral data-contract type — the Unit of Work — is defined once and shared by any storage-capable Runtime component that needs to participate in an atomic, multi-write commit boundary. It represents one physical storage-transaction boundary and carries no business meaning of its own.

2. **Location: the shared runtime-contracts layer.** The Unit-of-Work type lives in the same neutral shared contracts layer established by ADR-0001, alongside `ObjectId` and `PropertyValue`. It is not owned by Object Runtime, the Transaction Engine, or any other engine.

3. **Engine independence is preserved.** Object Runtime depends only on the shared contracts layer for this type, exactly as it already depends on it for `ObjectId`/`PropertyValue`. The Transaction Engine depends on the same shared type, the same way. Neither engine imports, calls, or otherwise depends on the other as a result of this decision.

4. **Orchestration happens above both engines.** Opening a Unit of Work, invoking each participating engine's write operation with it, and committing once is the responsibility of a higher-level caller outside both engines (in practice, a future Process Engine action handler or an application-level command/orchestration layer). Neither SPEC-001, SPEC-002, nor SPEC-003 defines that caller; this ADR does not either.

5. **Atomic commit or rollback.** When two or more writes participate in the same Unit of Work, they succeed together or fail together. There is no partial outcome: a failure in any participating write rolls back every write sharing that handle.

6. **No distributed-transaction semantics in the initial implementation.** Two-phase commit, saga/compensating-action patterns, and cross-database coordination are explicitly rejected as the default for the MVP. For the MVP, Object Runtime's and the Transaction Engine's Storage Providers must be configured against the same physical database/transaction manager whenever an atomic state-plus-audit write is required — the Unit of Work is a true, single physical transaction, not an application-level approximation of one.

7. **Storage-provider obligation, stated now, implemented later.** The SQLite and PostgreSQL storage providers (Baseline Section 18) must eventually expose compatible Unit-of-Work participation through the existing storage abstraction. This ADR states the requirement; it does not implement it, and does not block SPEC-003 implementation on the concrete provider work being finished first, as long as a test double / in-memory provider can demonstrate the contract.

8. **SPEC-001 receives a minimal, additive amendment.** Object Runtime's state-mutating write operation(s) may optionally accept this Unit-of-Work handle. This is documented as SPEC-001 Amendment 1, referencing this ADR. It is additive and non-breaking: every existing caller and test that doesn't supply the handle keeps today's independent-commit behavior, unchanged.

## Consequences

- Object Runtime and the Transaction Engine remain fully independent of one another; the only new coupling introduced is a shared dependency on one small, inert type in the neutral contracts layer, the same pattern already established for `ObjectId`/`PropertyValue`.
- The Baseline's literal "same database transaction" guarantee is achievable for MVP deployments where both engines are backed by the same physical store — which matches every deployment shape described in Baseline Section 18 (standalone SQLite, small-company SQLite+sync, enterprise PostgreSQL); none of these imply Object Runtime and the Transaction Engine living in separate physical databases at MVP stage.
- A future deployment that genuinely requires separate physical stores for the two engines cannot reuse this ADR's guarantee as-is. That scenario requires its own, separate ADR that explicitly states it weakens or changes the Baseline Section 9 guarantee — it must never be introduced silently, by Codex or otherwise, as a substitute for this decision.
- This ADR does not define: the concrete Unit-of-Work Rust type's internal representation, the concrete storage-wiring implementation that makes cross-engine physical-transaction sharing work, or the orchestrating caller that uses it. These remain implementation details and future-SPEC concerns.

## Rejected Alternatives

- **Saga / compensating-action pattern** — rejected as the default because it silently weakens the Baseline's literal guarantee (eventual consistency instead of true atomicity) without an explicit decision to accept that trade-off.
- **Two-phase commit across separate databases** — rejected as unnecessary complexity; no MVP deployment shape in Baseline Section 18 requires Object Runtime and the Transaction Engine to sit in different physical databases.
- **Direct dependency between Object Runtime and the Transaction Engine** (either engine calling into or importing the other to coordinate commits internally) — rejected outright; it would violate the engine-independence requirement carried through SPEC-001, SPEC-002, and SPEC-003, and the Baseline's general Runtime boundary rules (Section 4, Repository Rules 1–3).

## References

Architecture Baseline v1.0 Sections 9 and 18. ADR-0001 (shared runtime-contracts crate). SPEC-001 Object Runtime (amended by this ADR — see SPEC-001 Amendment 1). SPEC-003 Transaction Engine (Blocking Questions 1 and 2, resolved by this ADR).
