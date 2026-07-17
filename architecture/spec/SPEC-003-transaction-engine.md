# SPEC-003 - Transaction Engine

Status: Draft - implementation contract for Codex

Authority order: (1) Architecture Baseline v1.0, (2) AGENTS.md, (3) Approved ADRs (ADR-0000, ADR-0001, ADR-0002), (4) SPEC-001 Object Runtime (as amended by ADR-0002's Amendment 1), (5) SPEC-002 Event Engine, (6) this document.

## Purpose

The Transaction Engine is the Runtime component responsible for the Transaction/Audit Log: the record of every validated state change, written so that the state change and its audit record can never drift apart (Baseline Section 9). Where the Object Runtime owns *what the data currently is* and the Event Engine owns *what facts occurred*, the Transaction Engine owns *proof of what changed, when, under whose authority, and why* — at the level of regulatory weight the change requires (Baseline Section 9's three transaction levels).

The Transaction Engine does not decide whether a change is correct, does not evaluate rules, does not run processes, and does not query business data. It records validated state-change facts, exactly as supplied, permanently and immutably, and exposes narrow, deterministic access to them.

## Scope

This SPEC defines, for the Transaction Engine only:

- transaction identity and transaction schema versioning
- the three transaction levels and their distinct field sets
- how atomic co-commit between an Object Runtime state change and its Transaction/Audit record is achieved without introducing an engine-to-engine dependency
- how a Transaction record links to Object IDs and Event IDs
- old/new value, base-version/resulting-version, actor/device/site/source, and sequence-number metadata
- rule-evaluation metadata fields carried on the Transaction record (never a separate log)
- the transaction-hash contract for Level 2 and the signature contract for Level 3
- transaction immutability
- validation rules, the public Runtime API, storage abstraction, error model
- determinism, security boundary, localization boundary, performance requirements
- testing requirements, acceptance criteria, assumptions, and blocking questions

## Out of Scope

The Transaction Engine must not implement, and Codex must not add under SPEC-003, any of the following:

- **Rule Engine** — no condition/action evaluation. The Transaction Engine only stores rule-evaluation *metadata* supplied by a caller (Baseline Section 9: "Rule evaluation is not a separate permanent log"); it never evaluates a rule itself.
- **Process Engine** — no process nodes, state-transition validation, or workflow routing.
- **Query Engine** — no filtering, searching, sorting, joining, or aggregation over Transaction records. The only read operations are by-identity and strictly sequential/streaming, mirroring the pattern established in SPEC-001 and SPEC-002.
- **Object Runtime business logic** — the Transaction Engine never validates Object Type metadata, never checks that a base/resulting version actually matches Object Runtime's live state, and never mutates Current Object State. It records the caller-supplied version facts; enforcing that they're true is Object Runtime's own responsibility at the moment of its own write (see "Atomic State-Change and Audit-Record Boundary").
- **Event Engine business logic** — the Transaction Engine never appends, reads, or validates Business Events. It may carry an opaque reference to an Event ID, exactly as the Event Engine carries opaque `ObjectId` references.
- **Synchronization** — no delta computation, no conflict resolution. The Transaction Engine exposes the sequence-number and version data a Synchronization Engine may later use, nothing more.
- **Security authorization** — no permission evaluation, and no decision about whether a signer was entitled to sign. The Transaction Engine verifies that a Level 3 signature is cryptographically valid for the stated signer and content (a structural check, described in "Signature Metadata Contract (Level 3)"); it never decides whether that signer was authorized to perform the signed action.
- **Cryptographic algorithm design** — the Transaction Engine depends on an abstract Cryptographic Provider contract for hashing and signature verification (Baseline Section 13: "using established libraries only — never a custom algorithm"). It never implements a hashing or verification algorithm itself, and it never produces signatures.
- **Database providers** — no dependency on SQLite or PostgreSQL specifics; the Transaction Engine depends on an abstract, append-only Storage Provider contract, exactly as SPEC-001 and SPEC-002 require of their own storage.
- **Business Packages, GUI, AI, SCADA, Office integration, native plugins** — not referenced anywhere in this SPEC.
- **Statistics/KPI Engines** — no aggregation, trend calculation, or threshold evaluation.

## Terminology

- **Transaction** — a single immutable record of a validated state change, at one of three regulatory levels, appended once and never altered.
- **Transaction/Audit Log** — the complete, append-only, ordered store of all Transactions (Baseline Section 9).
- **Transaction Level** — one of Level 1 (ordinary edit), Level 2 (process/significant event), or Level 3 (confirmed regulated milestone) — see "Transaction Levels."
- **Append Index** — a strictly increasing, Transaction-Engine-assigned marker recording append order within the Transaction/Audit Log, assigned to every Transaction regardless of level. Purely technical; carries no regulatory meaning; governs the log's sequential read order.
- **Transaction Sequence** — the Baseline-mandated regulatory sequence field (Baseline Section 9: Level 2 "adds sequence number"), carried only by Level 2 and Level 3 Transactions. Distinct from Append Index: Transaction Sequence is a regulatory field, Append Index is the technical read-ordering key every Transaction has regardless of level.
- **Unit of Work** — an opaque handle representing one physical storage-transaction boundary, used to co-commit an Object Runtime state change and its Transaction record atomically. Defined and owned by neither engine — see "Atomic State-Change and Audit-Record Boundary."
- **Prior Reference** — an optional caller-supplied link from a Transaction to either a preceding Transaction or a triggering Event, stored opaquely.
- **Cryptographic Provider** — the abstract, pluggable contract the Transaction Engine depends on for computing hashes (Level 2) and verifying signatures (Level 3). It does not produce or hold signatures: a Level 3 signature is always supplied by the caller, already produced elsewhere, outside this SPEC's scope. No concrete algorithm or library is chosen by this SPEC.
- **Storage Provider (Transaction Store)** — the abstract, append-only persistence contract the Transaction Engine depends on; concrete implementation is out of scope here.

## Transaction Identity

- Every Transaction has a globally unique identifier (TransactionId), assigned once at append time and never reused, changed, or reassigned.
- TransactionId carries no business meaning; callers must not depend on it encoding level, object, actor, or time.
- The Transaction Engine receives its TransactionId generator through dependency injection, mirroring the deterministic identifier-generation approach required for `ObjectId` (SPEC-001) and `EventId` (SPEC-002), so replay is always deterministic.
- Duplicate TransactionIds are rejected deterministically.

## Transaction Schema Version

- Every Transaction record carries a schema version describing the shape of the record format itself, independent of and in addition to its Transaction Level (Baseline Section 21: "Every transaction type carries a schema version from day one so historical records remain interpretable").
- Schema versions are append-only in the same sense as SPEC-002's Event Type versioning: an existing schema version is never redefined in place; only a new version may be introduced, and previously recorded Transactions remain interpretable exactly as they were recorded, under their original schema version.

## Transaction Levels

Every Transaction declares exactly one level at append time. The Transaction Engine validates that all fields required for the declared level are present; it never infers or upgrades a level on the caller's behalf.

**Level 1 — ordinary edits.** Required fields: TransactionId, ObjectId, operation (an opaque, caller-supplied operation descriptor — e.g., create/update/relation-change — validated only for presence and well-formedness, never interpreted for business meaning), old value, new value, actor reference, device reference, timestamp, base version, resulting version. No signature required.

**Level 2 — process or significant events.** All Level 1 fields, plus: Transaction Sequence, server receipt time, transaction hash, and an optional Prior Reference (to a preceding Transaction or a triggering Event).

**Level 3 — confirmed regulated milestones (approvals, releases, e-signatures).** All Level 2 fields, plus: signer identity, signer role, signature meaning, authentication-evidence reference, signed revision, reason, timestamp (of signing, distinct from the Level 1 edit timestamp), and a cryptographic signature.

A Transaction's level, once appended, is immutable exactly like every other field — a Level 1 record is never later "upgraded" to Level 2 or 3; a new, separate Transaction is appended if a higher level of assurance is subsequently required for the same fact, consistent with "corrective facts are new records, not mutations" already established in SPEC-002.

## Atomic State-Change and Audit-Record Boundary

This is the central design question for the Transaction Engine: Baseline Section 9 requires that a state change and its Transaction/Audit record be "written in the same database transaction," so the two can never drift apart — yet Object Runtime must not depend on the Transaction Engine, the Transaction Engine must not contain Object Runtime business logic, and the Event Engine must not depend on the Transaction Engine either. The resolution is an explicit, shared **Unit of Work** port, owned by neither engine:

- A Unit of Work is an opaque handle representing one physical storage-transaction boundary. Per ADR-0002, it is defined as a shared, neutral data-contract type living in the shared runtime-contracts layer established by ADR-0001, alongside `ObjectId`/`PropertyValue`.
- Object Runtime's Update operation, as extended additively by SPEC-001 Amendment 1 (ADR-0002), and the Transaction Engine's Append operation (see "Public Runtime API") each accept an **optional** Unit of Work parameter. When omitted, each engine commits its own write independently, exactly as SPEC-001 and SPEC-002 already specify — nothing about their existing, already-implemented behavior changes. When supplied, the write participates in the shared transaction boundary instead of committing on its own.
- Neither engine is aware of the other's existence through this mechanism. Object Runtime depends only on the Unit of Work port shape (already alongside the shared types it depends on); the Transaction Engine depends only on the same shape. Neither imports or calls the other's API.
- An **orchestrating caller** — not defined by this SPEC, and not either engine (in practice, a future Process Engine action handler or an application-level command handler) — is responsible for: opening a Unit of Work, invoking Object Runtime's Update with it, invoking the Transaction Engine's Append with the same handle, and committing exactly once. If either write fails while staged, the whole Unit of Work is aborted and rolled back: neither the Object state change nor the Transaction record is applied.
- Making a shared Unit of Work handle result in one true physical database transaction (so the guarantee is real, not just an application-level convention) is a **storage-wiring concern**. Per ADR-0002, the MVP deployment constraint is explicit: Object Runtime's and the Transaction Engine's Storage Providers must be configured against the same physical database/transaction manager whenever atomic state+audit writes are required, and the SQLite and PostgreSQL storage providers must eventually expose compatible Unit-of-Work participation through the existing storage abstraction (Baseline Section 18). This SPEC defines the contract shape both engines must honor; the concrete storage wiring itself remains a deployment-level concern, not either engine's own logic.
- If the two engines are ever deployed against genuinely separate physical storage systems, a single-physical-transaction guarantee is not achievable. Per ADR-0002, saga, compensating-action, two-phase-commit, and other cross-database coordination patterns are explicitly rejected as the default and must not be introduced by Codex under this SPEC. Any future deployment requiring genuinely separate physical stores requires its own, separate ADR that explicitly documents that it weakens or changes the Baseline Section 9 guarantee — it is not a decision this SPEC or its implementation may make silently.

## Links to Object IDs and Event IDs

- Every Transaction (all levels) references exactly one `ObjectId` describing which Object the recorded state change concerns. This is stored as an opaque reference, structurally validated (well-formed identifier) but never resolved or queried against Object Runtime.
- A Transaction may optionally carry a Prior Reference pointing to either a preceding Transaction (for continuity/tamper-evidence within one Object's history) or a triggering Event (from the Event Engine's Business Event Log, e.g., the fact that caused a rule to fire and produce this state change). Both are stored as opaque references of the same shape; the Transaction Engine never dereferences, queries, or validates the existence of what a Prior Reference points to — that responsibility, if needed at all, belongs to whichever component supplied the reference.

## Old/New Values

- Every Level 1+ Transaction carries an old value and a new value for the field(s) changed, using the same `PropertyValue` shape established in SPEC-001 and shared via ADR-0001, so a Transaction's recorded change is directly comparable to what Object Runtime itself stores.
- The Transaction Engine performs no semantic interpretation of old/new values beyond structural validation (they must be well-formed `PropertyValue` instances); it never checks that the new value actually matches what Object Runtime currently holds — that check, if it happens at all, happens inside Object Runtime's own Update operation, before the Unit of Work commits.

## Base-Version and Resulting-Version Metadata

- Every Level 1+ Transaction carries the base version (the Object's version immediately before the change) and the resulting version (immediately after), using the shared `Version` type promoted into the runtime-contracts layer alongside `ObjectId`/`PropertyValue`/the Unit-of-Work contracts (ADR-0003). The Transaction Engine must not define its own, separately-typed version representation (e.g., a locally declared `ObjectVersion`): a semantically identical but Rust-type-incompatible duplicate would defeat the point of sharing it, particularly since base/resulting versions must pass cleanly between Object Runtime and the Transaction Engine through the same Unit of Work.
- The Transaction Engine validates only internal, structural consistency: the resulting version must be exactly one greater than the base version. It does not call Object Runtime to confirm these values actually match live Object state — that guarantee comes from Object Runtime's own optimistic-concurrency check at the moment of its Update call, inside the same Unit of Work described above, not from any check performed by the Transaction Engine itself.

## Actor, Device, Site, and Source Metadata

- Every Transaction carries an actor reference, a device reference, and an optional site/deployment reference — opaque, caller-supplied identifiers, structurally similar in role to the Event Engine's `ActorRef`/`DeviceRef`/`SiteRef`-style fields but defined independently within the Transaction Engine, so as not to create a dependency on the Event Engine crate for what is, at this stage, a small, simple reference shape (see Assumptions).
- The Transaction Engine stores and returns these references verbatim. It performs no authentication or authorization; that is Security's responsibility, entirely outside this SPEC (Baseline Section 13).
- A Transaction with a missing actor or device reference is rejected at append time as a validation failure — the Transaction Engine never silently records a change with no attributed actor.

## Sequence Numbers

- Every Transaction, at every level, carries an Append Index, assigned by the Transaction Engine at append time: strictly increasing, never reused, never derived from wall-clock time — the same ordering discipline established for the Event Engine's Append Sequence (SPEC-002). Append Index is what the sequential range-read operation orders by (see "Public Runtime API"), so every Transaction, including Level 1, is reachable through the complete ordered log stream.
- Level 2 and Level 3 Transactions additionally carry a Transaction Sequence value — the Baseline-mandated regulatory sequence number — also assigned by the Transaction Engine at append time, independent of Append Index. Level 1 Transactions do not carry a Transaction Sequence value, consistent with "Transaction Levels"; this does not affect their reachability, since Append Index, not Transaction Sequence, governs read order.
- Both Append Index and Transaction Sequence are scoped to a single Transaction/Audit Log instance. Neither is a cross-site or global ordering guarantee; cross-site ordering is a Synchronization Engine concern, outside this SPEC.

## Rule-Evaluation Metadata Fields

- A Transaction resulting from rule evaluation may optionally carry: the triggering Event ID (a Prior Reference, see above), the rule version that fired, the evaluation result, the set of generated actions, and execution metadata (e.g., execution time, chain depth) — all supplied by the caller and stored as opaque, structural data.
- Per Baseline Section 9, this is not a fourth permanent log: it is a set of fields on the Transaction record itself. The Transaction Engine assigns no special behavior to these fields beyond storing and returning them; it never evaluates a rule, never re-derives an evaluation result, and never validates that a stated rule version actually produced the stated result.

## Transaction Hash (Level 2)

- Every Level 2+ Transaction carries a transaction hash: a content-integrity hash covering the Transaction's own recorded fields, computed via the abstract Cryptographic Provider (never a custom algorithm).
- If a Prior Reference to a preceding Transaction is supplied and the caller additionally supplies that preceding Transaction's hash value as opaque input, the Transaction Engine incorporates it into this Transaction's hash computation, producing a genuine hash chain (Baseline Roadmap Phase 3: "hash-chained audit for critical events") without the Transaction Engine ever reading or looking up the prior record itself — chaining is achieved purely from caller-supplied inputs, keeping the Transaction Engine's own logic a pure, deterministic function of its inputs.
- The Transaction Engine never invents, defaults, or infers a transaction hash; it is always computed by the Cryptographic Provider from the Transaction's own content (and, when supplied, the prior hash) at append time, and never recomputed or checked again afterward — immutability means there's nothing left to reverify once appended.

## Signature Metadata Contract (Level 3)

- Every Level 3 Transaction carries: signer identity (an opaque reference, structurally similar to actor reference but distinct — a Level 3 signer is not assumed to be the same as the Level 1/2 actor), signer role, signature meaning (a language-neutral token or enum — see "Localization Boundary"), an authentication-evidence reference (opaque, e.g. pointing at how the signer proved their identity — not interpreted by the Transaction Engine), the signed revision (identifying exactly what version/state was signed), a reason (free text, taggable), a signing timestamp, and a cryptographic signature.
- The signature is supplied by the caller, already produced elsewhere — the Transaction Engine never produces or holds signing key material, and the Cryptographic Provider exposes no signing capability. Verification is performed through the abstract Cryptographic Provider, using an established library it wraps (Baseline Section 13). The Transaction Engine never implements verification logic itself, and it never implements or requires signing logic.
- At append time, the Transaction Engine requires the supplied signature to verify successfully against the supplied signed content and signer reference, via the Cryptographic Provider's verify capability. This is a structural check only: it confirms the signature is mathematically valid for the stated inputs. It does not confirm the signer was authorized to sign, or that the reason given is truthful — those are Security's and the business process's concerns, entirely outside this SPEC.
- A Level 3 Transaction whose signature fails verification is rejected at append time; no partial record is ever stored.

## Transaction Immutability

- Once a Transaction is successfully appended, none of its fields — at any level — may ever be modified. There is no update operation and no delete operation anywhere in the Transaction Engine's public API. This is an absolute invariant, not a default that can be overridden by a caller flag, matching the immutability guarantee already established for Events in SPEC-002.
- A correction to a previously recorded Transaction is always represented as a new Transaction (optionally referencing the original via a Prior Reference), never as a modification of the original.

## Validation Rules

At append time, and only at append time, the Transaction Engine validates:

- the declared Transaction Level resolves to a known level (1, 2, or 3)
- all fields required for that level (per "Transaction Levels") are present and structurally well-formed
- old value and new value are well-formed `PropertyValue` instances
- resulting version is exactly one greater than base version
- actor and device references are present (site reference is optional)
- for Level 2+: server receipt time is present and well-formed, and any supplied Prior Reference — including an optional prior-record hash value supplied for chaining (see "Transaction Hash (Level 2)") — is structurally well-formed. Transaction Sequence, Append Index, and transaction hash are never supplied by the caller: they are assigned or computed by the Transaction Engine itself as part of a successful append, so there is nothing to validate as caller input for these three fields.
- for Level 3: signer identity, signer role, signature meaning, authentication-evidence reference, signed revision, reason, signing timestamp, and a cryptographic signature are all present, and the signature verifies successfully via the Cryptographic Provider

No validation occurs, and no re-validation is ever performed, after a Transaction has been successfully appended.

## Public Runtime API

Described as behavior/contracts, not code. The Transaction Engine exposes exactly these capability groups:

**Append operation**
- Append a new Transaction at a declared level, given the fields required for that level (see "Transaction Levels" and "Validation Rules") — for Level 2+, this may include an optional prior-record hash as chaining input, never the final transaction hash itself — optionally participating in a caller-supplied Unit of Work (see "Atomic State-Change and Audit-Record Boundary"). Returns the assigned TransactionId, Append Index, and, for Level 2+, the assigned Transaction Sequence value and computed transaction hash. Fails entirely (no partial record) on any validation failure, signature-verification failure, or Storage Provider failure.

**Read operations**
- Read a single Transaction by TransactionId.
- Read a contiguous range of Transactions by Append Index, in strictly ascending order, supporting incremental/streaming consumption — mirroring the Event Engine's range-read design (SPEC-002). This is the only mechanism by which other components observe the Transaction/Audit Log, and it covers every Transaction Level, including Level 1; it is not a filtering or search API.

No other operations exist: no update, no delete, no filter-by-field, no search, no cross-transaction grouping beyond the opaque Prior Reference already described.

## Storage Abstraction

- The Transaction Engine depends on an abstract, append-only Transaction Store contract (its own Storage Provider), independent of and structurally parallel to Object Runtime's and the Event Engine's storage abstractions. It must not assume SQLite or PostgreSQL specifics. Per ADR-0002, for the MVP, the concrete Transaction Store implementation and Object Runtime's concrete Storage Provider implementation must be configured against the same physical database/transaction manager whenever atomic state+audit writes are required; the SQLite and PostgreSQL providers must eventually expose compatible Unit-of-Work participation.
- The Transaction Store contract must support: appending one Transaction atomically (optionally as part of an externally supplied Unit of Work), reading one Transaction by TransactionId, streaming Transactions in Append Index order, detecting duplicate TransactionId, and atomically allocating both the next Append Index (every level) and, for Level 2+, the next Transaction Sequence value.
- Swapping the concrete Transaction Store implementation must require no change to any Transaction Engine caller.

## Error Model

The Transaction Engine returns structured, typed errors — never panics across its public API boundary — covering at minimum:

- Transaction not found
- Unknown or unsupported Transaction Level
- Validation failure (missing required field for the declared level, malformed value, resulting-version/base-version mismatch, malformed Prior Reference)
- Duplicate TransactionId on append
- Transaction Sequence allocation failure
- Signature verification failure (Level 3)
- Cryptographic Provider failure (hash or signature computation/verification unavailable)
- Storage Provider failure during append (never leaves a partial record)
- Invalid or out-of-range Append Index bounds (on range read)

Each error kind is distinct and machine-distinguishable. A validation failure identifies the specific failing field and, where applicable, the expected constraint and actual value — consistent with the error-detail requirement established in SPEC-002.

## Determinism Requirements

- Given an identical sequence of append calls with identical inputs and deterministic identity/sequence/cryptographic providers, the Transaction Engine always produces identical resulting log content, identical Transaction Sequence assignments, and identical hashes/signatures.
- Concurrent append calls are serialized such that the resulting order is well-defined and reproducible from the Transaction Engine's own commit order, never from wall-clock timestamps.
- The Transaction Engine never reads its own wall clock to make a correctness or ordering decision; all timestamps (edit time, server receipt time, signing time) are caller-supplied or supplied by the Runtime boundary layer as input, not generated internally.
- Iteration order of returned fields is itself deterministic, and validation/verification errors are deterministic for the same invalid input — matching the determinism bar set in SPEC-002's latest revision.

## Security Boundary

- The Transaction Engine is not an authorization engine. It records actor, device, and signer references verbatim and performs no check of whether a caller was entitled to record a given change or signature — that determination happens upstream.
- The Transaction Engine must never silently default a missing actor, device, or signer reference to a privileged or empty value; missing required identity fields are always a validation failure.
- The Transaction Engine does not perform encryption. Whether Transaction records are encrypted at rest is a Storage Provider concern, exactly as in SPEC-001 and SPEC-002.
- Level 3 signature verification (see "Signature Metadata Contract") is the one place this SPEC calls into a security-adjacent capability — but only for mathematical validity, never for authorization. This distinction must be preserved exactly; it is not Codex's judgment call to expand it.

## Localization Boundary

- Field names, the operation descriptor vocabulary, and signature-meaning tokens are language-neutral; the Transaction Engine never stores or returns display text for them.
- The `reason` field (Level 3) and any other free-text field may optionally carry a language tag but are otherwise stored as supplied; the Transaction Engine performs no translation, formatting, or locale-aware behavior, consistent with SPEC-001 and SPEC-002.

## Performance Requirements

- Append must remain a near-constant-time operation with respect to total log size, including hash computation, which must not require scanning prior records (chaining relies on caller-supplied prior-hash input, not a lookup — see "Transaction Hash (Level 2)").
- Sequential/streaming range reads must support incremental consumption without materializing the full log or the full requested range in memory.
- The Transaction Engine must contribute a small, fixed share of the overall Runtime footprint target (Baseline Section 23); it must not embed any indexing, search, or aggregation capability.

## Testing Requirements

- A conformance test suite must run against any Transaction Store implementation (including an in-memory one) without modification.
- Tests proving immutability: no code path modifies or removes a previously appended Transaction.
- Tests proving deterministic Append Index and (for Level 2+) Transaction Sequence assignment under concurrent appends, and deterministic hash/signature-verification output for identical inputs via a deterministic test Cryptographic Provider.
- Tests proving the Unit of Work contract: an Object Runtime write and a Transaction Engine append sharing a Unit of Work either both commit or both roll back, using a test double for the shared storage-wiring layer (since real cross-engine physical-transaction wiring is a deployment concern outside this SPEC).
- Negative tests for every error kind in "Error Model," including Level 3 signature-verification failure.
- A boundary test proving the Transaction Engine has zero compile-time or runtime dependency on Content-Package-defined code, and zero calls into Object Runtime, Event Engine, Rule Engine, Process Engine, Query Engine, or Security logic.

## Acceptance Criteria

1. Appending a Level 1 Transaction missing any required Level 1 field is rejected before any record is durably stored.
2. Appending a Level 2 Transaction without a Transaction Sequence, server receipt time, or transaction hash is rejected.
3. Appending a Level 3 Transaction whose signature does not verify against the supplied signed content and signer reference is rejected, and no partial record is stored.
4. Two concurrent Level 2+ appends each receive a distinct, correctly ordered Transaction Sequence value.
5. No public API method exists that modifies or deletes a previously appended Transaction.
6. No public API method exists that accepts a filter, search predicate, sort order, or aggregation.
7. Given a shared Unit of Work, a failure in the Object Runtime write leaves no Transaction record appended, and a failure in the Transaction append leaves the Object Runtime write uncommitted.
8. Omitting the Unit of Work parameter on both an Object Runtime write and a Transaction append results in each committing independently, exactly as SPEC-001 and SPEC-002 already specify on their own.
9. A resulting version that is not exactly one greater than the supplied base version is rejected.
10. Replaying the same sequence of append calls with deterministic providers produces byte-identical Transaction records, including identical hashes and signatures.
11. Swapping the Transaction Store implementation requires no change to any Transaction Engine caller.
12. Level 1 Transactions are retrievable through the Append Index range-read operation, in the same ordered stream as Level 2 and Level 3 Transactions.

## Assumptions

1. **Unit of Work crate location** — resolved by ADR-0002: it lives in the shared runtime-contracts layer established by ADR-0001, alongside `ObjectId`/`PropertyValue`. No longer an open assumption.
2. **Actor/device/site references are defined locally, not shared** — assumed that Transaction Engine defines its own opaque `ActorRef`/`DeviceRef`/`SiteRef`-equivalent types rather than depending on the Event Engine crate for its structurally similar `EventSource` fields, to avoid a third engine depending on a second engine's crate. Promoting these into the shared contracts layer, if a further engine needs the identical shape, is a future decision, not made here.
3. **Prior Reference is a single, dual-purpose field** — assumed that one Prior Reference field can point at either a preceding Transaction or a triggering Event, since Baseline Section 9's wording ("link to the prior relevant transaction or event," as given in this SPEC's brief) does not clearly choose one over the other. The Transaction Engine treats both as opaquely as each other.
4. **Hash chaining is caller-driven, not Transaction-Engine-driven** — assumed that genuine hash-chaining (Baseline Roadmap Phase 3) is achieved by the caller supplying the prior record's hash as input, not by the Transaction Engine looking up or resolving a Prior Reference itself, to preserve the no-querying invariant.
5. **Operation descriptor vocabulary** — assumed to be an opaque, caller-supplied token (not a fixed enum defined by this SPEC), mirroring how Object Runtime and Event Engine treat externally supplied vocabularies as inert configuration rather than Runtime-defined business meaning.
6. **Signed revision representation** — assumed to be an opaque reference (e.g., an Object version or a content hash) rather than a full embedded copy of the signed state, to keep Transaction records bounded in size; the exact shape is left open pending the Security SPEC.
7. **`Version` promotion to runtime-contracts** — resolved by ADR-0003: `Version` is promoted into the shared runtime-contracts layer established by ADR-0001, alongside `ObjectId`/`PropertyValue`/the Unit-of-Work contracts. No longer an open assumption.

## Blocking Questions

Both blocking questions below were resolved by the architect's decision recorded in ADR-0002. Retained here for traceability; implementation may proceed.

1. **SPEC-001 additive extension for the Unit of Work parameter — RESOLVED.** Approved as a minimal, additive, non-breaking extension to SPEC-001's Update operation (SPEC-001 Amendment 1, ADR-0002). Existing callers and tests are unaffected when the new parameter is omitted.
2. **Storage-wiring feasibility — RESOLVED for MVP.** Per ADR-0002, the MVP requires Object Runtime and the Transaction Engine to be configured against the same physical database/transaction manager whenever atomic state+audit writes are required. Saga, two-phase-commit, or cross-database coordination are explicitly rejected as defaults; any future move to separate physical stores requires its own ADR.

## Implementation Checklist for Codex

1. Confirm ADR-0002 and SPEC-001 Amendment 1 are present in the repository, and confirm the full existing SPEC-001 test suite still passes unmodified after the amendment, before writing any Transaction Engine implementation code. If either is missing, or the SPEC-001 suite doesn't pass unmodified, stop and report rather than proceeding.
2. Define TransactionId, transaction schema version, Transaction Level, Transaction Sequence, Prior Reference, ActorRef/DeviceRef/SiteRef (local to this crate), and the Level 1/2/3 field-set data contracts as idiomatic Rust types, reusing `ObjectId`, `PropertyValue`, and the Unit of Work type from the shared contracts layer.
3. Define the abstract, append-only Transaction Store trait boundary, without implementing a concrete provider (an in-memory test provider is in scope for testing only).
4. Define the abstract Cryptographic Provider trait boundary (hash + verify only — no signing capability), without implementing or selecting a concrete algorithm or library; provide a deterministic test double for conformance testing.
5. Implement the Append operation with full level-aware validation, optional Unit of Work participation, deterministic TransactionId/Transaction Sequence assignment, and hash/signature computation via the Cryptographic Provider.
6. Implement Read-by-identity and the incremental/streaming range-read-by-Transaction-Sequence operation — no additional read operations.
7. Implement the full error taxonomy from "Error Model" as distinct, structured error types.
8. Enforce immutability as a structural guarantee: no code path capable of mutating or removing a stored Transaction.
9. Write the conformance test suite covering "Testing Requirements" and all eleven acceptance criteria, including the Unit-of-Work commit/rollback tests using a test double for shared storage wiring.
10. Write a boundary/lint check confirming zero references to Content-Package-defined symbols and zero calls into Object Runtime, Event Engine, Rule Engine, Process Engine, Query Engine, or Security logic, from within the Transaction Engine source tree.
11. Do not implement anything listed in "Out of Scope," even as a stub, beyond the minimal shared data-contract shapes explicitly named above.
