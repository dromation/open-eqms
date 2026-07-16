# SPEC-001 — Object Runtime

Status: Draft — implementation contract for Codex
Authority: Architecture Baseline v1.0 (normative). This SPEC introduces no new architectural concepts; it operationalizes Baseline Sections 2, 4, 5, 6 (boundary only), 8 (boundary only), 9 (partial), 13 (boundary only), 18 (boundary only), 20, 21, 22, 23, and 28.

---

## 1. Purpose

The Object Runtime is the Runtime component responsible for representing every business entity — regardless of domain — as a generic Object, and for managing that Object's identity, current state, versioning, relationships, and metadata. It is the single authoritative store of **Current Object State** as defined in Baseline Section 9. It has no knowledge of what any object type *means*; all business semantics are supplied externally by Content Packages (Baseline Section 5).

The Object Runtime exists so that every other Runtime component — Event Engine, Rule Engine, Process Engine, Query Engine, Statistics/KPI Engines, Transaction Engine, GUI Engine — can read and mutate business data through one consistent, deterministic, storage-agnostic contract, without any of them needing to know how objects are stored or what "object" means for a given domain.

## 2. Scope

This SPEC defines, for the Object Runtime only:

- object identity and the object type registry
- the object instance data model (properties, relations, lifecycle-state reference, ownership, permissions reference, version, metadata)
- the public API surface for creating, reading, updating, and relating objects
- data contracts for all of the above
- invariants the Object Runtime must uphold unconditionally
- error handling, security boundary, localization boundary, performance and determinism requirements
- testing requirements and acceptance criteria

The Object Runtime, per Baseline Section 4, absorbs what would otherwise be a separate Resource Runtime (resources are simply Object types) and holds no state-transition logic (that is the Process Engine's responsibility — see Out of Scope).

## 3. Out of Scope

This SPEC does **not** define, and Codex must not implement as part of SPEC-001:

- **Event Engine** — the Business Event Log is a separate, append-only store; Object Runtime does not emit, persist, or interpret events.
- **Rule Engine** — no condition/action evaluation of any kind lives here.
- **Process Engine** — Object Runtime stores a lifecycle-state value on each object but does not define, validate, or transition between states. State transitions and the meaning of any given state are entirely the Process Engine's concern (Baseline Section 6).
- **Query Engine** — no filtering, searching, sorting, joining, or aggregation logic. Object Runtime exposes only direct, by-identity access plus a narrow internal read surface that the Query Engine (built separately) uses to build indexes. This exclusion is an invariant, not a phasing note — see Section 12.
- **GUI** — no rendering, no View Models.
- **AI** — no adapter, no AI-facing API.
- **Synchronization** — no delta computation, no conflict resolution policy. Object Runtime exposes the version/base-version data the Synchronization Engine needs, nothing more.
- **Database implementation** — Object Runtime depends on an abstract Storage Provider contract (Baseline Section 18); it must not embed or assume SQLite or PostgreSQL specifics.
- **Business Packages** — no package format, no capability manifests, no package-supplied validation logic beyond consuming externally supplied type/field metadata as inert configuration.
- **SCADA / industrial adapters, Office integration** — not referenced anywhere in this SPEC.
- **Transaction Engine, Security, Statistics/KPI Engines, Package Loader** — not implemented here. Where Object Runtime's data must be visible to them (version numbers for the Transaction Engine, permission-scope references for Security), this SPEC describes the shape of that data only, not the consuming component's logic.

## 4. Terminology

- **Object** — a single generic business entity instance managed by the Object Runtime.
- **Object Type** — a named, versioned metadata definition describing what properties and relations an Object of this type may carry. Supplied externally (by a Content Package or an equivalent configuration source); the Object Runtime treats it as data, not code.
- **Property** — a single named, typed value on an Object instance, defined by the Object's type.
- **Relation** — a directed, typed reference from one Object to another (or to a set of others).
- **Lifecycle State** — an opaque, externally defined token stored on an Object, owned and interpreted by the Process Engine. The Object Runtime stores and returns it verbatim; it never validates transitions.
- **Version / Base Version** — a monotonically increasing revision marker on an Object, used for optimistic-concurrency conflict detection (Baseline Section 20).
- **Current Object State** — the mutable, queryable live projection of an Object, as defined in Baseline Section 9. This is what the Object Runtime owns.
- **Storage Provider** — the abstract persistence contract the Object Runtime depends on (Baseline Section 18); its concrete implementation (SQLite/PostgreSQL) is out of scope here.
- **Capability Manifest reference** — an opaque identifier the Object Runtime passes through on privileged calls; it does not interpret or enforce it (that is Security's responsibility, Baseline Section 13).

## 5. Functional Requirements

### 5.1 Object identity

- Every Object has a globally unique identifier, assigned once at creation and never reused, changed, or reassigned — including after logical deletion or archival.
- Identity is independent of Object Type: two Objects of different types never collide on ID, and an Object's type is immutable after creation (retyping an Object is not a supported operation; it must be represented as a new Object plus an explicit relation if a domain ever needs that, which is a package-level concern).
- The Object Runtime does not interpret the identifier's internal structure; callers must not depend on it encoding business meaning (Baseline: "Business identifiers remain language-neutral" and Repository Rule 5 — the Runtime carries no business meaning).

### 5.2 Object lifecycle

- Every Object instance carries exactly one Lifecycle State value at a time, stored as an opaque token supplied and interpreted by the Process Engine.
- The Object Runtime accepts a new Lifecycle State value on an update call, stores it, and returns it unchanged on read. It performs no validation of whether a given transition is legal — that check happens upstream, in the Process Engine, before the update call reaches the Object Runtime.
- The Object Runtime never deletes an Object outright. There is no delete operation in the public API. What a domain calls "deletion" is represented as a Lifecycle State value (e.g., an archived/retired state) supplied by the Process Engine, consistent with Baseline Section 2 ("Facts, once recorded, are never deleted").

### 5.3 Object versioning

- Every Object carries a version number that increments by exactly one on every successful update.
- Every update call must supply the base version the caller last read. If the current stored version does not match the supplied base version, the update is rejected with a version-conflict error and no state change occurs (Baseline Section 20's ordering principle: base version, never wall-clock time).
- The Object Runtime does not resolve conflicts; it only detects and reports them. Conflict resolution policy (auto-merge on different fields, human review on critical fields) is implemented by components that sit above the Object Runtime.
- The Object Runtime does not itself retain full historical revisions of an Object; retaining and reconstructing history is the Transaction/Audit Log's responsibility (Baseline Section 9). The Object Runtime's obligation is limited to ensuring the version number and base-version check are always available and correct for whichever component builds that history.

### 5.4 Object relationships

- A Relation is a directed, typed edge from one Object to one or more other Objects, identified by (source Object ID, relation type, target Object ID).
- Relation types are part of Object Type metadata (externally supplied), not hardcoded in the Object Runtime.
- The Object Runtime enforces referential existence at write time (a relation may not be created pointing at an Object ID that does not exist) but enforces no business meaning about cardinality, required relations, or cascading behavior beyond what the supplied Object Type metadata declares as structural constraints (e.g., "at most one," "at least one," "many").
- Removing a relation is itself a versioned update to the source Object (or a dedicated relation-removal call — see Section 8) and is never a silent side effect of any other operation.
- No relation may be silently dropped when a referenced Object's Lifecycle State changes; that decision belongs to the Process Engine.

### 5.5 Object metadata

- Object Types and their Property/Relation definitions are configuration, held in a typed metadata registry, not schema migrations (Baseline Section 5). Registering or updating an Object Type is a distinct, explicit administrative operation from creating or updating Object instances.
- The Object Runtime validates every instance write against the currently registered definition for that Object's type: unknown properties are rejected, required properties must be present, and each property's value must match its declared type.
- Object Type metadata itself carries a version. The Object Runtime records which Object Type version an instance was last validated against, so that a later, controlled metadata migration (Baseline Section 21: "migrated through a documented, controlled process") has a starting point — the Object Runtime does not perform migrations itself.

## 6. Public API

Described as behavior/contracts, not code. The Object Runtime exposes exactly these capability groups:

**Type registry operations**
- Register or update an Object Type definition (metadata only; rejected if it would invalidate existing instances without an accompanying migration marker).
- Retrieve an Object Type definition by name and version.

**Instance operations**
- Create an Object: given a type, initial properties, initial relations, owner, responsible users, and a permission-scope reference, returns the new Object's identity and version. Fails if the type is unknown or validation fails.
- Read an Object by identity: returns the full current state (properties, relations, lifecycle state, owner, responsible users, permission-scope reference, version, metadata, retention rule reference) or a not-found error. No filtering parameters exist on this call.
- Update an Object: given an identity, a base version, and a set of property/relation/lifecycle-state/ownership changes, applies them atomically or rejects the whole update with a version-conflict or validation error. Partial application is never permitted. (See "Amendment 1 (ADR-0002)" for an optional, additive Unit-of-Work parameter on this operation.)
- Add / remove a Relation on an existing Object: a narrower, explicit form of update scoped to relations only, for callers that don't need to touch properties.

**Internal read surface (for the Query Engine only)**
- A bulk/streaming read interface that other Runtime components (principally the Query Engine) use to build their own indexes over Current Object State. This is explicitly not a filtering API — it is a full-scan or change-feed style interface; any selection logic on top of it lives entirely in the Query Engine.

No other operations exist. In particular, there is no search, no delete, no bulk-update-by-criteria, and no cross-object transaction spanning multiple Objects in a single call (multi-object consistency, if ever required, is a Transaction Engine concern layered on top of single-Object atomic updates).

## 7. Data Contracts

Described structurally, not as code:

- **ObjectId** — an opaque, globally unique value. Equality-comparable. No business meaning.
- **ObjectTypeRef** — a (name, version) pair identifying a registered Object Type.
- **PropertyValue** — a tagged value of one of a fixed set of primitive kinds: text, localized-text-key, number, boolean, date/time, enum-value, reference (an ObjectId), attachment-reference, binary blob. The exact enumeration is an assumption pending confirmation — see Section 15.
- **PropertyDefinition** — name, PropertyValue kind, required/optional, and structural constraints (min/max, pattern, allowed enum values), owned by Object Type metadata.
- **Relation** — (relationType: string, targetId: ObjectId), with cardinality constraints defined on the owning Object Type.
- **LifecycleState** — an opaque string token; no fixed enumeration exists at the Object Runtime layer.
- **Version** — a strictly increasing integer (or equivalent monotonic marker) plus the base-version value supplied on every update call.
- **OwnershipInfo** — owner (an ObjectId referencing an Identity-bearing Object, e.g., an Employee) and a set of responsible-user references, same kind.
- **PermissionScopeRef** — an opaque reference the Security component resolves; the Object Runtime stores and returns it, never evaluates it.
- **RetentionRuleRef** — an opaque reference to a retention policy (Baseline Section 21); stored and returned, not interpreted.
- **ExternalReference** — an opaque (system, key) pair for cross-referencing external systems (e.g., a SCADA point); stored and returned, never interpreted.
- **ObjectRecord** — the full assembled read result: ObjectId, ObjectTypeRef, properties, relations, LifecycleState, OwnershipInfo, PermissionScopeRef, Version, RetentionRuleRef, ExternalReferences, and a pointer to comments/attachments (whose own storage contract is not defined by this SPEC — assumption, see Section 15).

## 8. Invariants

These hold unconditionally and are the basis for the acceptance criteria in Section 14:

1. The Object Runtime contains no business semantics; identical mechanisms apply to every Object Type without exception.
2. No object is ever physically deleted by the Object Runtime; there is no delete operation, only Lifecycle State changes supplied externally.
3. An Object's identity and type, once assigned, never change.
4. Every successful update strictly increments the version by one; every update carries a base version; a mismatch always rejects the update, with no partial effect.
5. The Object Runtime performs no filtering, sorting, searching, or aggregation; any such capability found in an implementation is a scope violation of this SPEC.
6. The Object Runtime never imports, calls, or depends on anything defined inside a Content Package (Baseline Repository Rule 1); it only consumes externally supplied Object Type metadata as inert configuration data.
7. The Object Runtime is storage-provider agnostic: swapping the underlying Storage Provider must not require any change to Object Runtime logic or its public API.
8. All relation targets must exist at the time a relation is written; dangling relations are never silently created.
9. Every write operation is atomic: it fully succeeds or has no effect, even under concurrent access or process interruption.
10. The Object Runtime never generates or depends on wall-clock time to decide correctness of an operation; ordering and conflict detection rely solely on version numbers.

## 9. Error Handling

The Object Runtime returns structured, typed errors — never panics across its public API boundary — covering at minimum:

- Object not found
- Object Type not found / not registered
- Version conflict (base version mismatch)
- Validation failure (unknown property, missing required property, type mismatch, constraint violation)
- Invalid relation (target does not exist, cardinality violated)
- Duplicate identity on create (should not occur given identity generation, but must be a defined, handled error rather than an assumption)
- Malformed metadata on type registration

Each error kind is distinct and machine-distinguishable (not a single generic error string), so that callers (Process Engine, Transaction Engine, GUI Engine) can react deterministically — e.g., a version conflict is retried or escalated to conflict-review; a validation failure is surfaced to the user; a not-found is handled differently from a conflict.

## 10. Security Considerations

- The Object Runtime is not an authorization engine. It stores and returns `PermissionScopeRef` and `OwnershipInfo` verbatim; the Security component (Baseline Section 13) is solely responsible for evaluating whether a given caller may perform a given operation.
- Every Create call must be supplied a non-empty owner and a permission-scope reference; the Object Runtime rejects creates that omit them rather than silently defaulting to an open/unscoped value. This preserves least-privilege by construction rather than relying on the caller to remember.
- Permission-scope changes are themselves ordinary versioned updates and therefore automatically subject to the same version-conflict and audit-visible mechanics as any other change — consistent with Baseline Section 13's requirement that permission changes be audited (the actual audit recording is the Transaction Engine's job; the Object Runtime's obligation is simply to never let a permission-scope change bypass the normal update path).
- The Object Runtime does not perform encryption; whether data at rest is encrypted is a Storage Provider concern (Baseline Section 13: "Offline devices cache only data the authenticated user is authorized to access, encrypted at rest"). The data contracts in Section 7 must not assume plaintext-only storage — property values must be opaque enough at the Object Runtime's layer that an encrypting Storage Provider can be substituted without a contract change.

## 11. Localization Considerations

- Business identifiers (Object Type names, Property names, relation-type names) are language-neutral tokens; the Object Runtime never stores or returns display text for them.
- Any user-authored free text (e.g., a text property, a comment) is stored as-is, optionally tagged with a language identifier, but the Object Runtime performs no translation, formatting, or locale-aware behavior itself — that is a Runtime-wide Localization capability (Baseline Section 22) layered above, and package-supplied translations remain entirely outside this SPEC.
- The `localized-text-key` PropertyValue kind (Section 7) exists specifically so a Property can reference a translation key rather than embedding a specific language's text directly, keeping the Object Runtime itself locale-neutral.

## 12. Performance Requirements

- Read-by-identity and write-by-identity must be near-constant-time operations with respect to total object count; the Object Runtime must not degrade linearly (or worse) as the number of stored Objects grows, since Baseline Section 23 targets industrial-scale, weak-hardware, offline deployments.
- Object Type metadata must be cached in memory after first load, bounded in size (proportional to the number of distinct registered types, not instance count).
- The Object Runtime itself must contribute a small, fixed share of the overall 20–40MB Runtime footprint target (Baseline Section 23); it must not embed a query planner, full-text index, or any capability reserved for the Query Engine.
- The internal bulk/streaming read surface (Section 6) must support incremental consumption (not requiring the full object set to be materialized in memory at once) so that the Query Engine can build indexes without imposing its memory cost onto the Object Runtime.

## 13. Determinism Requirements

- Given an identical sequence of API calls with identical inputs (including base versions), the Object Runtime always produces the identical resulting state and the identical sequence of errors — no hidden nondeterminism from concurrency, caching, or iteration order.
- Concurrent updates to the same Object are serialized (directly or via the version-conflict mechanism) such that the outcome never depends on timing; concurrent updates to different Objects never interfere with each other.
- The Object Runtime accepts timestamps as caller-supplied input where a timestamp is part of an Object's data (e.g., a measurement's recorded time); it never derives correctness or ordering decisions from its own wall-clock reads.

## 14. Testing Requirements and Acceptance Criteria

**Testing requirements**
- A conformance test suite must run against any Storage Provider implementation (including a simple in-memory one) without modification, proving storage-agnosticism.
- Property-based / invariant-style tests for each invariant in Section 8, particularly: version-conflict detection under concurrent updates, atomicity under interruption, and rejection of dangling relations.
- Negative tests for every error kind in Section 9.
- A boundary test proving the Object Runtime has zero compile-time or runtime dependency on any Content-Package-defined code.

**Acceptance criteria** (each must be demonstrably true before SPEC-001 is considered implemented)

1. Two concurrent updates to the same Object with the same base version: exactly one succeeds; the other returns a version-conflict error; the Object's final state reflects only the successful update.
2. An attempt to create a relation targeting a nonexistent Object ID is rejected before any state change occurs.
3. An attempt to write a property not defined on the Object's registered type is rejected with a validation error.
4. No public API method exists that accepts a filter, query predicate, or sort order.
5. No public API method exists that permanently removes an Object or its version history.
6. Swapping the Storage Provider implementation (e.g., in-memory vs. a real backing store) requires no change to any Object Runtime caller.
7. Every Create call without an owner or permission-scope reference is rejected.
8. Replaying the same sequence of calls against a fresh instance produces byte-identical resulting Object state.

## 15. Assumptions

These are not settled by Architecture Baseline v1.0 and are assumed here for the sake of a concrete, implementable contract. Each should be confirmed or corrected before or during implementation:

1. **Identifier format** — assumed to be a time-orderable unique identifier (e.g., UUIDv7/ULID-class), not a simple sequential integer, to keep offline-generated IDs collision-free (Baseline: offline-first is mandatory). The Baseline does not specify a format.
2. **PropertyValue kind enumeration** — the set listed in Section 7 (text, localized-text-key, number, boolean, date/time, enum, reference, attachment-reference, blob) is inferred from the object examples in Baseline Section 5 (documents, measurements, materials, etc.); the Baseline does not give an exhaustive list.
3. **Comments and attachments** — Baseline Section 5 lists "comments" and "attachments" as things every object instance carries, but does not specify whether they are Properties, a distinct sub-structure, or references to Objects of their own type. This SPEC assumes they are referenced via a pointer/relation-like mechanism rather than embedded inline, to keep Object records bounded in size; this should be confirmed.
4. **Version representation** — assumed to be a simple monotonic integer per Object. The Baseline only requires "base version" comparisons (Section 20), not a specific representation.
5. **Relation cardinality enforcement** — assumed to be declared per relation type in Object Type metadata and enforced by the Object Runtime at write time; the Baseline states relations exist but does not specify where cardinality is enforced.
6. **Single-Object atomicity is sufficient** — assumed that cross-object multi-write transactions are not a Object Runtime concern and are composed above it (by the Transaction Engine) rather than needing multi-object atomic writes here.

## 16. Blocking Questions

None. Every ambiguity encountered was resolvable by staying inside the stated Object Runtime boundary (Section 4/5/9) and is recorded as an assumption above rather than a blocker.

---

# Implementation Checklist for Codex

1. Define the ObjectId, ObjectTypeRef, PropertyValue, PropertyDefinition, Relation, LifecycleState, Version, OwnershipInfo, PermissionScopeRef, RetentionRuleRef, ExternalReference, and ObjectRecord data contracts (Section 7) as idiomatic Rust types — no business-specific variants.
2. Define the Storage Provider trait boundary the Object Runtime depends on, without implementing a concrete provider (an in-memory test provider is in scope for testing only).
3. Implement Object Type registration/retrieval against the metadata registry, including type-version tracking (Section 5.5).
4. Implement Create, Read-by-ID, Update (whole-record, versioned), and Add/Remove-Relation operations exactly as scoped in Section 6 — no additional operations.
5. Implement the internal bulk/streaming read surface for the Query Engine's future use, as a read-only, non-filtering interface.
6. Implement validation against registered Object Type metadata on every write (Section 5.5, 5.4).
7. Implement optimistic-concurrency version checking on every update (Section 5.3), with atomic all-or-nothing application.
8. Implement the full error taxonomy in Section 9 as distinct, structured error types.
9. Enforce Section 10's create-time requirement for owner + permission-scope reference.
10. Write the conformance test suite against the in-memory Storage Provider covering Section 14's testing requirements and all eight acceptance criteria.
11. Write a boundary/lint check (or note for the CI dependency-graph check already planned repo-wide) confirming zero references to any Content-Package-defined symbol from within the Object Runtime source tree.
12. Do not implement anything listed in Section 3 (Out of Scope) even as a stub beyond the minimal data-contract shapes explicitly named in Section 7.

---

## Amendment 1 (ADR-0002) — Optional Unit-of-Work Participation

Status: Approved addendum. Additive only; nothing in Sections 1–16 above is changed by this amendment. Where anything below appears to conflict with the sections above, it does not — this amendment only adds a new, optional capability to one existing operation.

**Context.** SPEC-003 (Transaction Engine) requires that an Object Runtime state-mutating write and a Transaction/Audit record be committed atomically, in the same physical storage transaction (Baseline Section 9). Achieving this without Object Runtime depending on the Transaction Engine, or vice versa, requires Object Runtime's Update operation to optionally accept a shared, neutral Unit-of-Work handle. This amendment, approved via ADR-0002, adds that capability additively.

**Amendment.**

- Object Runtime's Update operation (Section 6) may optionally accept a Unit-of-Work handle, of the type defined in the shared runtime-contracts layer (ADR-0001 / ADR-0002).
- When the parameter is omitted, Update behaves exactly as specified in Sections 1–16 above, with no change whatsoever: it commits independently, on its own, exactly as every existing caller and test already expects.
- When supplied, Update stages its write against the given Unit-of-Work handle instead of committing independently; the actual commit or rollback is driven by whichever orchestrating caller holds the handle, per ADR-0002 — not by Object Runtime itself.
- This amendment adds no new Object Runtime business logic, changes no data contract in Section 7, and does not alter any invariant in Section 8. It only adds an optional participation point to one existing operation.
- Object Runtime does not depend on the Transaction Engine, or on any other component, as a result of this amendment. It depends only on the shared Unit-of-Work handle type in the neutral contracts layer, exactly as it already depends on that layer for `ObjectId`/`PropertyValue`.

**Compatibility requirement.** Every existing SPEC-001 test must continue to pass unmodified after this amendment is implemented. If any existing test requires modification to keep passing, that indicates the amendment was not implemented additively, and this must be reported rather than silently resolved.

**Testing addition.** A new test must demonstrate that Update, when given a Unit-of-Work handle alongside a Transaction Engine append sharing the same handle, either both commit or both roll back together (using a test double for the shared storage-wiring layer, consistent with SPEC-003's own testing requirements).
