# ADR-0004 — Security Authorization Boundary and Neutral Permission Contract

Status: Approved (Gate 0), pending repository commit. Project Manager review verified
every substantive claim against committed source and prior review records; one
citation defect was found and corrected (an AGENTS.md rule 8 misattribution — rule 8
concerns the Query Engine, not Security ownership, and has been replaced with the
correct citation, Baseline Section 13 alone). This ADR fixes a contract shape and
ownership boundary only; it does not implement a Security Engine and does not
authorize any Query Engine permission-filtering implementation.

Authority: subordinate to Architecture Baseline v1.0; sibling to ADR-0001, ADR-0002,
ADR-0003. Does not restate any of them.

## Context

SPEC-004 (Query Engine) Section 13 makes per-record permission enforcement an
absolute invariant: no candidate record, projection, aggregate, or piece of evidence
may influence a result unless it has been individually authorized. This is not a
Query Engine policy choice — Baseline Section 13 already assigns authorization
decisions to Security, and every engine SPEC written so far (SPEC-001 Section 6,
SPEC-002, SPEC-003) treats Security as an external decision authority the engine
enforces against, never as something an engine implements itself.

No Security crate or API currently exists in the repository. The workspace has five
members — `runtime-contracts`, `object-runtime`, `event-engine`, `transaction-engine`,
`query-engine` — and none of them defines or depends on an authorization type. This
was confirmed directly during SPEC-004 Gate 1 repository inspection and is recorded
there as Blocking Decision I.1: any data-returning Query Engine execution is blocked
on this decision, and the Gate 1 instruction explicitly forbade defining a local
placeholder (e.g., a stubbed `PermissionProvider` trait) to work around the gap.
Codex correctly did not do so in the Slices 1–6 partial implementation; this ADR is
the follow-up architectural decision that implementation was deliberately left
waiting on.

Three questions must be answered before any engine can consume authorization
decisions without inventing its own version of this contract: where authorization is
enforced, what identity and permission context reaches a consuming engine, and which
layer owns the denial decision itself.

## Decision

1. **Security remains the sole owner of authorization policy and denial decisions.**
   No Runtime engine — Query Engine included — evaluates roles, capabilities,
   delegation rules, or any other policy logic itself. This restates, rather than
   changes, Baseline Section 13; it is recorded here because every subsequent point
   in this ADR depends on it holding exactly.

2. **A neutral authorization decision contract is added to `runtime-contracts`**,
   not to Query Engine, Security, or any other single engine. This follows the same
   test ADR-0001 established: the contract is business-neutral, needed by more than
   one Runtime component (Query Engine today; GUI Engine, Rule Engine, Statistics/KPI
   Engine plausibly later), and safe to expose without creating an engine-to-engine
   dependency. The contract consists of:
   - An opaque **caller/permission context** type (a Runtime-wide equivalent of the
     opaque string-newtype pattern already used for `ActorRef`, `SignerRef`, etc. in
     Transaction Engine) — every consuming engine treats it as an opaque token to
     pass through, never something it inspects, parses, or constructs itself.
   - An opaque **action** and a **target reference** shape sufficient to name "this
     caller, this operation, this specific record from this specific source."
     `ObjectId` alone is not sufficient for this, since identifiers are not
     guaranteed globally unique across every source a target reference might need to
     name. The target reference must therefore carry, at minimum: a source/namespace
     component (which QuerySource or engine owns the record), an object-type
     component, an object-identifier component, and an optional subresource
     component. The subresource slot is not used by anything in this ADR's scope,
     but is reserved now because field-level, attachment-level, and evidence-level
     access decisions are foreseeable future needs (SPEC-004's Evidence and Context
     Package model) that would otherwise force a breaking change to the target shape
     later.
   - A **decision** result: `Allow`, `Deny`, or `HiddenDeny` (a denial that must be
     indistinguishable from "record does not exist" to the caller) — directly
     satisfying SPEC-004 Section 13's requirement that a permission denial can be
     returned "in a form that does not itself leak the existence of the denied
     data."
   - No policy logic accompanies these types. `runtime-contracts` defines the
     *shape* of a decision, never how one is reached.

3. **The actual policy evaluation is owned by an injected dependency, not a
   concrete crate reference.** Query Engine (and any future consumer) depends on a
   neutral trait — analogous in kind to `object-runtime`'s `StorageProvider` or
   `transaction-engine`'s `CryptographicProvider` — implemented by whatever Security
   Engine crate is eventually built, or by a test double in tests. Query Engine never
   depends on a concrete Security Engine crate directly; it depends on the trait
   shape defined alongside the decision contract.

   **The trait's declaration location is constrained, not left fully open.** The
   trait must be declared in the same business-neutral contract layer as the
   decision types it operates on (`runtime-contracts`, or a sibling
   dependency-neutral crate at the same layer) — a layer that both the eventual
   Security provider and every consumer can depend on without any consumer becoming
   dependent on a concrete Security Engine crate. A future Security Engine SPEC may
   choose the exact crate name and may add its own implementation-specific types
   around it, but it may not relocate the trait itself into a concrete Security
   Engine crate that consumers would then need to depend on directly — doing so
   would silently recreate the exact engine-to-engine coupling this ADR exists to
   prevent. This constraint is deliberately about dependency direction, not naming.

4. **Authorization decisions and observable query behavior are two different
   things, and this ADR fixes both, not just the first.** Security returns exactly
   one of `Allow`, `Deny`, or `HiddenDeny` for a given caller/action/target — that is
   the full extent of what Security decides. What a consuming engine *does* with
   that decision depends on where the evaluated record sits in the shape of the
   request, and is fixed here so every consumer applies the same mapping instead of
   improvising one:
   - `HiddenDeny` on a directly requested single record → the operation behaves as
     if the record does not exist (a "not found" outcome, never a distinguishable
     "hidden" outcome).
   - `HiddenDeny` on a candidate considered while building a set, projection,
     traversal, or aggregate → that candidate is silently excluded from the
     candidate set before any further processing touches it; its exclusion must not
     itself be observable (e.g., as a gap, a placeholder, or a count discrepancy a
     caller could use to infer it existed).
   - `Deny` on a directly requested single record → the operation returns an
     explicit permission-denied outcome for that record.
   - `Deny` on a candidate considered while building an aggregate, projection, or
     traversal → that candidate must not influence the result (matching SPEC-004
     Section 13's per-record-before-aggregation requirement), but a single denied
     candidate among many authorized ones does not, by itself, fail the entire
     operation — the operation proceeds over the remaining authorized candidates
     unless the consuming engine's own contract says otherwise.

   The earlier framing — that a consumer "returns exactly what the decision result
   says" — is corrected by this point: a consumer returns exactly what Security
   decided *for each individual record*, but how that per-record decision surfaces
   in the overall operation's result depends on the record's role in the request,
   per the mapping above.

5. **Per-record enforcement is a call-site obligation, not a one-time check.** Every
   candidate record a consuming engine considers — whether returned directly,
   projected, or folded into an aggregate — requires its own decision call before it
   is permitted to influence any part of a result. This restates SPEC-004 Section 13
   rather than changing it; it is recorded here so the authorization contract's shape
   (point 2) is understood to be called per-record, not per-query.

6. **Denial representation (`Deny` vs. `HiddenDeny`) is a Security policy decision**,
   attached to the decision response itself, never a Query Engine (or other
   consumer) choice. A consuming engine never decides independently whether a given
   denial should be visible or hidden; it applies the observable-behavior mapping in
   point 4 to whichever of the two Security actually returned.

7. **Every authorization decision must be minimally traceable, even though this ADR
   does not require that traceability to appear in any public Query Engine
   response.** For a regulated, diagnostic-capable system, it must be possible to
   later demonstrate which policy version produced a given decision. The decision
   contract (point 2) is therefore extended with a minimal audit envelope carried
   alongside — not instead of — the `Allow`/`Deny`/`HiddenDeny` result: a
   policy/ruleset revision reference, a decision or evaluation identifier, a decision
   timestamp, the identity of the Security provider that produced the decision, an
   internal reason code (not necessarily disclosed to the caller), and the decision
   itself. Consumers are not required to surface any of this in their own public
   results; they are required to receive it from Security and to be able to pass it
   through to an audit/Transaction record where the consuming engine's own SPEC
   requires one. This ADR fixes only that the envelope must exist and travel with
   the decision — its persistence and audit-record integration is each consuming
   engine's own concern.

   **The audit envelope is operational metadata and must not influence
   authorization semantics or equality of authorization decisions.** Two decisions
   are the same decision if and only if their `Allow`/`Deny`/`HiddenDeny` result and
   the caller/action/target they were evaluated against match — never because their
   timestamps, decision IDs, or evaluation IDs happen to match or differ. A consumer
   must never compare, deduplicate, or reason about decisions by envelope fields
   instead of by their actual authorization content.

8. **Small-group aggregate suppression (SPEC-004 Section 17, Question 3) remains
   explicitly out of scope for this ADR.** Per-record enforcement (point 5) does not
   by itself prevent inference from very small aggregate groups; that is a future
   Security policy addition, not a gap in this contract.

9. **No Security Engine implementation is authorized by this ADR.** Building the
   concrete crate that implements the injected trait — the actual Security Engine,
   per Baseline Section 13 — is a separate, future, explicitly gated SPEC and
   implementation track, following the same Gate 0 → Gate 1 → implementation
   discipline already used for SPEC-001 through SPEC-004. This ADR fixes the
   contract shape and ownership so that future work, and Query Engine's own
   permission-filtering implementation, has one canonical target to build against
   instead of each arriving at a slightly different guess.

## Consequences

- Query Engine's Gate 1 Blocking Decision I.1 becomes resolvable once (a) this ADR
  is approved, (b) the neutral decision contract exists in `runtime-contracts`, and
  (c) a concrete Security Engine implementation (even a minimal first version) exists
  to inject. None of those three steps is completed by this ADR alone.
- Future Runtime components that need authorization decisions (GUI Engine, Rule
  Engine, Statistics/KPI Engine) share the same contract instead of each engine
  inventing a compatible-but-separate permission-context type — the same class of
  problem ADR-0001 and ADR-0003 already solved for `ObjectId`/`PropertyValue` and
  `Version`.
- This ADR does not implement Query Engine permission filtering. That remains a
  separate, later, explicitly authorized implementation slice, gated on the neutral
  contract and a Security implementation both existing.
- A future Security Engine SPEC must define the concrete policy model and the
  trait's exact method signatures, within the dependency-layer constraint this ADR
  already fixes (point 3) — it may choose where the implementing crate lives, but
  not relocate the trait itself into that crate.
- Every consuming engine takes on an obligation to apply the observable-behavior
  mapping (point 4) consistently, and to thread the audit envelope (point 7) through
  to its own audit/Transaction records wherever its own SPEC already requires one —
  neither obligation is optional once this ADR is approved.

## Rejected Alternatives

- **Query Engine defines its own local permission-context and decision type** —
  rejected: this is exactly the cross-engine-type-invention pattern the Gate 1 review
  guardrails were written to prevent, and it would produce a Query-Engine-flavored
  permission concept incompatible with whatever every other engine eventually needs.
- **Query Engine depends directly on a concrete Security Engine crate** — rejected
  for the same reason ADR-0002 and ADR-0003 rejected direct engine-to-engine
  dependencies for the Unit-of-Work handle and `Version`: Cargo dependencies are not
  fine-grained, so "just the decision call" is not enforceable, and it reopens the
  exact engine-to-engine coupling this project has consistently avoided. An injected
  trait, in the same style as `StorageProvider`/`CryptographicProvider`, preserves
  testability and keeps the dependency direction one-way.
- **Leave the identity/permission-context shape undefined until the Security Engine
  is fully built** — rejected: this is precisely the situation that produced the
  Gate 1 blocking decision and the placeholder-interface risk both review rounds
  flagged. Fixing the neutral shape now, ahead of the full policy engine, lets Query
  Engine's next implementation slice build against a real, approved contract instead
  of guessing or stalling indefinitely.
- **A single combined "security context" opaque blob covering identity, action, and
  target together** — rejected: collapsing these into one opaque value would prevent
  a consuming engine from doing its own per-record enforcement loop (point 5) without
  reconstructing structure the contract deliberately keeps opaque; keeping
  caller/action/target as separate, individually opaque pieces preserves per-record
  call-site flexibility without exposing policy internals.
- **Treating `Deny`/`HiddenDeny` as a single uniform "stop" signal the consumer
  reacts to identically everywhere** — rejected: a permission-denied result on a
  directly requested record and a silently excluded candidate inside an aggregate
  are observably different outcomes required by SPEC-004 itself (Section 13); a
  uniform reaction would either leak existence through an unnecessary hard failure
  or incorrectly fail an entire query over one denied candidate among many.
- **Leaving decision traceability entirely to whichever Security Engine SPEC comes
  later** — rejected: without fixing now that an audit envelope must accompany every
  decision, a first Security implementation could reasonably ship a bare
  `Allow`/`Deny`/`HiddenDeny` with no way to later prove which policy version
  produced it, which would be a regulated-system gap discovered too late to fix
  cheaply.

## References

Architecture Baseline v1.0 Section 13. AGENTS.md rules 18, 19. SPEC-004 Section 13
(Permission Model), Section 16 (Error Model — permission-denied representation),
Section 17 (Question 3, small-group suppression — explicitly out of scope here),
Section 21 (Acceptance Criteria 4, 14). SPEC-004 Gate 1 repository-grounded technical
design report, Blocking Decision I.1. ADR-0001 (shared runtime-contracts crate,
precedent for the neutral-type test applied in Decision 2). ADR-0002 and ADR-0003
(precedent for rejecting direct engine-to-engine dependency in favor of a shared
contract).
