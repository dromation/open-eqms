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
- **Prior Reference** — an optional, caller-supplied, tagged link from a Transaction to either a preceding Transaction (`PriorReference::Transaction(TransactionId)`) or a triggering Event (`PriorReference::Event(EventId)`), stored opaquely. It is an identity/causation link only — it never carries a hash value.
- **Prior Transaction Hash** — an optional, caller-supplied hash value (Level 2+ only), supplied only when Prior Reference is the Transaction variant, used exclusively to chain this Transaction's own computed hash to the referenced prior Transaction's hash (see "Transaction Hash (Level 2)"). Supplying a Prior Transaction Hash without a Transaction-kind Prior Reference is a validation failure.
- **Canonical Transaction Content** — the complete, unambiguous byte representation of a Transaction's own fields, used as input to hash computation. Includes every field on the record except the transaction_hash field itself — explicitly including the assigned Append Index and, for Level 2+, the assigned Transaction Sequence, both of which are assigned before hash computation within the same atomic allocate-and-append operation (see "Transaction Hash (Level 2)" and "Storage Abstraction"). For Level 3, includes every other Level 3 field (signer identity, signer role, signature meaning, authentication-evidence reference, signed revision, reason, signing timestamp) but explicitly excludes the signature bytes themselves.
- **Cryptographic Provider** — the abstract, pluggable contract the Transaction Engine depends on for computing hashes (Level 2) and verifying signatures (Level 3). It does not produce or hold signatures: a Level 3 signature is always supplied by the caller, already produced elsewhere, outside this SPEC's scope. No concrete algorithm or library is chosen by this SPEC.
- **Storage Provider (Transaction Store)** — the abstract, append-only persistence contract the Transaction Engine depends on; concrete implementation is out of scope here.

## Transaction Identity

- Every Transaction has a globally unique identifier (TransactionId), assigned once at append time and never reused, changed, or reassigned.
- TransactionId carries no business meaning; callers must not depend on it encoding level, object, actor, or time.
- The Transaction Engine receives its TransactionId generator through dependency injection, mirroring the deterministic identifier-generation approach required for `ObjectId` (SPEC-001) and `EventId` (SPEC-002), so replay is always deterministic. Generation is independent of durable-commit timing: a TransactionId is generated and returned immediately by every Append call, whether or not that call is participating in a Unit of Work (see "Public Runtime API").
- Duplicate TransactionIds are rejected deterministically.
- A TransactionId returned by a Unit-of-Work-participating append is provisional until that Unit of Work commits: if the Unit of Work is rolled back, that TransactionId identifies no durable Transaction and is never returned by a subsequent read.

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
- A Transaction may optionally carry a Prior Reference: a tagged link to either a preceding Transaction (for continuity/tamper-evidence within one Object's history) or a triggering Event (from the Event Engine's Business Event Log, e.g., the fact that caused a rule to fire and produce this state change). The Transaction Engine never dereferences, queries, or validates the existence of what a Prior Reference points to — that responsibility, if needed at all, belongs to whichever component supplied the reference. A Prior Reference never itself carries a hash value; see "Transaction Hash (Level 2)" for the separate, optional Prior Transaction Hash field.

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
- Both values are assigned atomically at the moment of durable commit (see "Storage Abstraction"), never earlier — a failed validation, failed signature verification, or rolled-back Unit of Work never consumes a value.

## Rule-Evaluation Metadata Fields

- A Transaction resulting from rule evaluation may optionally carry: the triggering Event ID (a Prior Reference, see above), the rule version that fired, the evaluation result, the set of generated actions, and execution metadata (e.g., execution time, chain depth) — all supplied by the caller and stored as opaque, structural data.
- Per Baseline Section 9, this is not a fourth permanent log: it is a set of fields on the Transaction record itself. The Transaction Engine assigns no special behavior to these fields beyond storing and returning them; it never evaluates a rule, never re-derives an evaluation result, and never validates that a stated rule version actually produced the stated result.

## Transaction Hash (Level 2)

- Every Level 2+ Transaction carries a transaction hash: a content-integrity hash computed by the Transaction Engine, via the abstract Cryptographic Provider (never a custom algorithm), over the Transaction's own Canonical Transaction Content — every field on the record except the transaction_hash field itself and, for Level 3, except the signature bytes themselves (see "Signature Metadata Contract (Level 3)"). The signature bytes are deliberately excluded from the hash input: the signature and the hash are kept fully disjoint, each covering a different part of the record.
- Within the single atomic allocate-and-append operation (see "Storage Abstraction"), the Append Index and, for Level 2+, the Transaction Sequence are assigned first; the transaction hash is computed immediately afterward, over the now-complete Canonical Transaction Content, which therefore includes the just-assigned Append Index and Transaction Sequence. This ordering is required because the hash's own input is defined to include these fields — a hash computed before they exist could not cover them. None of this is externally observable as separate steps: the caller sees one atomic operation succeed or fail as a whole.
- A Prior Transaction Hash may optionally be supplied by the caller, only alongside a Transaction-kind Prior Reference. When supplied, the Transaction Engine incorporates it into this Transaction's hash computation as additional canonical input, producing a genuine hash chain (Baseline Roadmap Phase 3: "hash-chained audit for critical events") without ever reading or looking up the prior record itself — chaining is achieved purely from caller-supplied input, keeping the Transaction Engine's own logic a pure, deterministic function of its inputs.
- The Transaction Engine never invents, defaults, or infers a transaction hash, and never accepts one as caller input: it is always computed by the Cryptographic Provider from the Transaction's own canonical content (and, when supplied, the Prior Transaction Hash) at append time, after validation, any Level 3 signature verification, and Append Index/Transaction Sequence assignment all succeed, and never recomputed or checked again afterward — immutability means there's nothing left to reverify once appended.

## Signature Metadata Contract (Level 3)

- Every Level 3 Transaction carries: signer identity (an opaque reference, structurally similar to actor reference but distinct — a Level 3 signer is not assumed to be the same as the Level 1/2 actor), signer role, signature meaning (a language-neutral token or enum — see "Localization Boundary"), an authentication-evidence reference (opaque, e.g. pointing at how the signer proved their identity — not interpreted by the Transaction Engine), the signed revision (identifying exactly what version/state was signed — see Assumption 6), a reason (free text, taggable), a signing timestamp, and a cryptographic signature.
- The signature is supplied by the caller, already produced elsewhere — the Transaction Engine never produces or holds signing key material, and the Cryptographic Provider exposes no signing capability. Verification is performed through the abstract Cryptographic Provider, using an established library it wraps (Baseline Section 13). The Transaction Engine never implements verification logic itself, and it never implements or requires signing logic.
- The signature covers the signed revision only — never the Transaction record as a whole, never the transaction_hash field, and the signature bytes themselves are excluded from the transaction_hash's own input entirely. Keeping the signature and the hash fully disjoint — the signature signs the signed revision; the hash covers the record's other canonical content, but never the signature bytes — removes any need to reason about ordering or circularity between them.
- At append time, the Transaction Engine requires the supplied signature to verify successfully against the supplied signed revision and signer reference, via the Cryptographic Provider's verify capability, before computing the transaction hash. This is a structural check only: it confirms the signature is mathematically valid for the stated inputs. It does not confirm the signer was authorized to sign, or that the reason given is truthful — those are Security's and the business process's concerns, entirely outside this SPEC.
- The Transaction Engine additionally requires the supplied signed revision to correspond to this same Transaction's own resulting version (or base version, per how signed revision is ultimately defined — see Assumption 6): a structural equality check between two already-supplied fields, never a call to Object Runtime. A Level 3 append whose signed revision does not match is rejected as a validation failure, independent of whether the signature itself verifies.
- A Level 3 Transaction whose signature fails verification, or whose signed revision does not correspond to this Transaction's own version, is rejected at append time; no partial record is ever stored.

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
- for Level 2+: server receipt time is present and well-formed; any supplied Prior Reference is structurally well-formed; a Prior Transaction Hash, if supplied, is only valid alongside a Transaction-kind Prior Reference (supplying one without the other is a validation failure) — see "Transaction Hash (Level 2)." Transaction Sequence, Append Index, and transaction hash are never supplied by the caller: they are assigned or computed by the Transaction Engine itself as part of a successful append, so there is nothing to validate as caller input for these three fields.
- for Level 3: signer identity, signer role, signature meaning, authentication-evidence reference, signed revision, reason, signing timestamp, and a cryptographic signature are all present; the signed revision corresponds to this Transaction's own resulting version (or base version, per Assumption 6); and the signature verifies successfully via the Cryptographic Provider — in that order, per "Signature Metadata Contract (Level 3)."

No validation occurs, and no re-validation is ever performed, after a Transaction has been successfully appended.

## Public Runtime API

Described as behavior/contracts, not code. The Transaction Engine exposes exactly these capability groups:

**Append operation**
- Append a new Transaction at a declared level, given the fields required for that level (see "Transaction Levels" and "Validation Rules") — for Level 2+, this may include an optional Prior Transaction Hash as chaining input, valid only alongside a Transaction-kind Prior Reference, never the final transaction hash itself — optionally participating in a caller-supplied Unit of Work (see "Atomic State-Change and Audit-Record Boundary"). Processing order: structural validation; then, for Level 3, signature verification and signed-revision correspondence; then, as a single atomic Transaction Store operation, Append Index (and, for Level 2+, Transaction Sequence) assignment, transaction hash computation over the now-complete record, and durable persistence or Unit-of-Work staging (see "Storage Abstraction" and "Transaction Hash (Level 2)").
  - For an independent append (no Unit of Work supplied), this call is itself the durable commit point: it returns the assigned TransactionId, Append Index, and, for Level 2+, the assigned Transaction Sequence value and computed transaction hash.
  - For a Unit-of-Work-participating append, durable commit happens later, at that Unit of Work's own commit — Append Index, Transaction Sequence, and transaction hash do not exist yet when this call returns. This call returns only the assigned TransactionId and an explicit staged (not-yet-durable) result; no placeholder numbering or provisional hash is ever returned. After the Unit of Work commits successfully, the caller retrieves the finalized Transaction — including its assigned Append Index, Transaction Sequence, and transaction hash — via the Read-by-identity operation (see "Read operations"), using the TransactionId already returned. If the Unit of Work is instead rolled back, no Transaction is ever readable under that TransactionId — it identifies no durable record.
  - Fails entirely (no partial record, and no value ever allocated) on any validation failure, signature-verification failure, or Storage Provider failure.

**Read operations**
- Read a single Transaction by TransactionId.
- Read a contiguous range of Transactions by Append Index, in strictly ascending order, supporting incremental/streaming consumption — mirroring the Event Engine's range-read design (SPEC-002). This is the only mechanism by which other components observe the Transaction/Audit Log, and it covers every Transaction Level, including Level 1; it is not a filtering or search API.

No other operations exist: no update, no delete, no filter-by-field, no search, no cross-transaction grouping beyond the opaque Prior Reference already described.

## Storage Abstraction

- The Transaction Engine depends on an abstract, append-only Transaction Store contract (its own Storage Provider), independent of and structurally parallel to Object Runtime's and the Event Engine's storage abstractions. It must not assume SQLite or PostgreSQL specifics. Per ADR-0002, for the MVP, the concrete Transaction Store implementation and Object Runtime's concrete Storage Provider implementation must be configured against the same physical database/transaction manager whenever atomic state+audit writes are required; the SQLite and PostgreSQL providers must eventually expose compatible Unit-of-Work participation.
- The Transaction Store contract exposes exactly one atomic allocate-and-append operation — not separate pre-allocation calls. Given a fully validated, and — for Level 3 — signature-verified Transaction ready to record (not yet hash-computed), the atomic operation: (1) assigns the next Append Index (every level) and, for Level 2+, the next Transaction Sequence value; (2) computes the transaction hash via the Cryptographic Provider over the now-complete Canonical Transaction Content, which includes the values just assigned in step 1; and (3) durably persists the completed record — either independently, or staged as part of a supplied Unit of Work and finalized only at that Unit of Work's commit. All three steps are internal to this single atomic operation and are never exposed as separate public calls; a design exposing `next_append_index()` / `next_transaction_sequence()`, or a separate hash-computation step, as calls independent from this one atomic operation does not satisfy this contract: it risks a value being allocated and never associated with any durably stored record if a later step fails.
- Append Index and Transaction Sequence values are never consumed by a failed validation, a failed signature verification, or a rolled-back Unit of Work: allocation happens only at the moment of durable commit, never at staging or validation time. This SPEC's guarantee covers the durable log's content — strict monotonic order and a one-to-one correspondence between an assigned value and a durably committed Transaction — and does not extend to policing a Storage Provider's raw internal counter primitive; a compliant provider must use a transactional counter, not a non-transactional sequence generator, to satisfy this guarantee in practice.
- The Transaction Store contract must also support: reading one Transaction by TransactionId, streaming Transactions in Append Index order, and detecting duplicate TransactionId.
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

- **Canonical hash-input construction**: there is exactly one well-defined, unambiguous canonical byte representation of a Transaction's hashable content (every field except the transaction_hash field itself — see "Canonical Transaction Content"), so any component independently recomputing the hash from the same recorded fields constructs identical input bytes.
- **Stable hash output**: given identical canonical input bytes and the same Cryptographic Provider algorithm, the computed hash is always identical. A Cryptographic Provider whose hash output varies for identical input (e.g., due to an embedded nonce or salt) does not satisfy this SPEC.
- **Stable assignment under controlled generator/store state**: given a fresh store and a deterministic, injected TransactionId generator, replaying the same script of append calls produces identical resulting Transaction records, Append Index/Transaction Sequence assignments, and hashes. This is a testing-only guarantee for reproducible conformance fixtures under controlled conditions — it does not mean repeated production appends of logically similar operations return identical TransactionId, Append Index, or Transaction Sequence values; those are always freshly assigned and unique per successful append (see "Transaction Identity").
- **Deterministic range ordering**: reading the same committed range of Transactions always returns them in the same Append Index order, regardless of how many times or from where the read happens.
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
- Tests proving the computed transaction hash is sensitive to the assigned Append Index and, for Level 2+, Transaction Sequence values: two otherwise-identical Transactions that receive different assigned numbering produce different hashes, confirming the hash genuinely covers the complete Canonical Transaction Content as defined.
- Tests proving a Unit-of-Work-participating append returns only a TransactionId and a staged result (no Append Index, Transaction Sequence, or transaction hash); that after the shared Unit of Work commits, reading that TransactionId returns the finalized Transaction with correct assigned values; and that after a rollback, reading that TransactionId returns nothing.
- Tests proving the Unit of Work contract: an Object Runtime write and a Transaction Engine append sharing a Unit of Work either both commit or both roll back, using a test double for the shared storage-wiring layer (since real cross-engine physical-transaction wiring is a deployment concern outside this SPEC).
- Negative tests for every error kind in "Error Model," including Level 3 signature-verification failure.
- A boundary test proving the Transaction Engine has zero compile-time or runtime dependency on Content-Package-defined code, and zero calls into Object Runtime, Event Engine, Rule Engine, Process Engine, Query Engine, or Security logic.

## Acceptance Criteria

1. Appending a Level 1 Transaction missing any required Level 1 field is rejected before any record is durably stored.
2. Appending a Level 2 Transaction missing server receipt time, or any other caller-supplied mandatory Level 2 field, is rejected before any record is durably stored. Transaction Sequence and transaction hash are never supplied by the caller, so their absence from caller input is never a basis for rejection: the Transaction Engine assigns Transaction Sequence and computes the transaction hash itself, only as part of a successful atomic append, after all validation (and, for Level 3, signature verification) succeeds.
3. Appending a Level 3 Transaction whose signature does not verify against the supplied signed revision and signer reference is rejected, and no partial record is stored.
4. Two concurrent Level 2+ appends each receive a distinct, correctly ordered Transaction Sequence value.
5. No public API method exists that modifies or deletes a previously appended Transaction.
6. No public API method exists that accepts a filter, search predicate, sort order, or aggregation.
7. Given a shared Unit of Work, a failure in the Object Runtime write leaves no Transaction record appended, and a failure in the Transaction append leaves the Object Runtime write uncommitted.
8. Omitting the Unit of Work parameter on both an Object Runtime write and a Transaction append results in each committing independently, exactly as SPEC-001 and SPEC-002 already specify on their own.
9. A resulting version that is not exactly one greater than the supplied base version is rejected.
10. Under a deterministic test identity/sequence generator and a fresh store, replaying the same script of append calls produces byte-identical Transaction records, including identical hashes and signature-verification results — a testing-only guarantee under controlled generator/store state, not a claim that repeated production appends of logically similar operations yield identical TransactionId, Append Index, or Transaction Sequence values.
11. Swapping the Transaction Store implementation requires no change to any Transaction Engine caller.
12. Level 1 Transactions are retrievable through the Append Index range-read operation, in the same ordered stream as Level 2 and Level 3 Transactions.
13. A failed validation, failed signature verification, or a rolled-back Unit of Work never results in an allocated Append Index or Transaction Sequence value appearing anywhere in the durable log.
14. Appending a Level 3 Transaction whose signed revision does not match this Transaction's own resulting version (or base version, per Assumption 6) is rejected as a validation failure, independent of whether the signature itself verifies.
15. Supplying a Prior Transaction Hash without a corresponding Transaction-kind Prior Reference is rejected as a validation failure.
16. Computing the transaction hash twice over identical Canonical Transaction Content via the same Cryptographic Provider always yields the same hash output.
17. The transaction hash is computed after the Append Index and, for Level 2+, the Transaction Sequence are assigned, and covers those assigned values as part of the Canonical Transaction Content; two Transactions differing only in assigned numbering never produce the same hash.
18. Appending within a Unit of Work returns only a TransactionId and a staged result, never a placeholder Append Index, Transaction Sequence, or transaction hash; after that Unit of Work commits, reading the same TransactionId returns a Transaction whose Append Index, Transaction Sequence, and transaction hash are present and correct, and after a rollback, reading that TransactionId returns nothing.

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
2. Define TransactionId, transaction schema version, Transaction Level, Append Index, Transaction Sequence, PriorReference (a tagged Transaction/Event link), PriorTransactionHash, ActorRef/DeviceRef/SiteRef (local to this crate), the Level 1/2/3 field-set data contracts, and the two-mode Append result type (a committed result carrying Append Index, Transaction Sequence, and transaction hash for an independent append; a staged result carrying only TransactionId for a Unit-of-Work-participating append — see "Public Runtime API") as idiomatic Rust types, reusing `ObjectId`, the shared `Version` type, `PropertyValue`, and the Unit of Work contracts from the shared contracts layer (runtime-contracts).
3. Define the abstract, append-only Transaction Store trait boundary, without implementing a concrete provider (an in-memory test provider is in scope for testing only).
4. Define the abstract Cryptographic Provider trait boundary (hash + verify only — no signing capability), without implementing or selecting a concrete algorithm or library; provide a deterministic test double for conformance testing.
5. Implement the Append operation in this order: structural validation; then (Level 3 only) signature verification and signed-revision correspondence; then, as a single atomic Transaction Store operation, (a) allocation of Append Index (and, for Level 2+, Transaction Sequence), (b) transaction hash computation via the Cryptographic Provider over the now-complete Canonical Transaction Content (including the values just allocated), and (c) durable persistence or Unit-of-Work staging — never as separate pre-allocation, hash, and append calls (see "Storage Abstraction").
6. Implement Read-by-identity and the incremental/streaming range-read-by-Append-Index operation — no additional read operations.
7. Implement the full error taxonomy from "Error Model" as distinct, structured error types.
8. Enforce immutability as a structural guarantee: no code path capable of mutating or removing a stored Transaction.
9. Write the conformance test suite covering "Testing Requirements" and all eighteen acceptance criteria, including the Unit-of-Work commit/rollback tests using a test double for shared storage wiring.
10. Write a boundary/lint check confirming zero references to Content-Package-defined symbols and zero calls into Object Runtime, Event Engine, Rule Engine, Process Engine, Query Engine, or Security logic, from within the Transaction Engine source tree.
11. Do not implement anything listed in "Out of Scope," even as a stub, beyond the minimal shared data-contract shapes explicitly named above.
