# SPEC-002 - Event Engine

Status: Approved - implementation contract for Codex

Authority order: (1) Architecture Baseline v1.0, (2) AGENTS.md, (3) SPEC-001 Object Runtime, (4) this document.

## Purpose

The Event Engine is the Runtime component responsible for recording immutable business facts into the Business Event Log (Baseline Section 9): "machine stopped," "material received," "document approved," and any equivalent domain fact. It exists so that a fact, once recorded, is permanently and unambiguously part of the record — never edited, never deleted, never reinterpreted after the fact — regardless of what any other Runtime component later does with that fact.

The Event Engine does nothing with a fact beyond recording it durably, in a deterministic append order, and making it readable back in that same order. Every other reaction to a fact — evaluating a rule, moving a process forward, updating an Object, computing a statistic — belongs to a different Runtime component reading from the Event Engine, never to the Event Engine itself.

## Scope

This SPEC defines, for the Event Engine only:

- event identity and the event type registry
- the event metadata and payload model
- event versioning (of Event Type schemas, not of individual events — events do not have revisions)
- timestamp and source handling
- append ordering and immutability guarantees
- validation performed at append time
- the public Runtime API (append and sequential read only)
- error model, security boundary, localization boundary
- performance and determinism requirements
- testing requirements and acceptance criteria

## Out of Scope

The Event Engine must not implement, and Codex must not add under SPEC-002, any of the following:

- **Rule Engine** — no condition evaluation, no WHEN/IF/THEN logic. The Event Engine does not know that rules exist.
- **Process Engine** — no process or lifecycle-state transitions of any kind.
- **Query Engine** — no filtering, searching, sorting, joining, or aggregation over recorded events. The only read operations are by-identity and strictly sequential/streaming (Section "Public Runtime API").
- **Transaction Engine** — the Event Engine does not write Transaction/Audit Log records and does not record rule-evaluation metadata; that is entirely the Transaction Engine's responsibility, consuming the Event Engine's output as one of its inputs.
- **GUI** — no rendering, no View Models.
- **AI** — no adapter, no AI-facing API.
- **Synchronization** — no delta computation, no conflict resolution. The Event Engine exposes append-sequence data that a Synchronization Engine may later use, nothing more.
- **Database providers** — no dependency on SQLite or PostgreSQL specifics; the Event Engine depends on an abstract, append-only Storage Provider contract, mirroring the storage-agnosticism required of the Object Runtime in SPEC-001.
- **Business Packages** — no package format, no capability manifests. Event Type definitions are consumed as inert external configuration, exactly as Object Type definitions are in SPEC-001.
- **Security authorization** — the Event Engine records a caller-supplied source/actor reference on every event but performs no permission check and no capability enforcement.
- **Native plugins** — not referenced anywhere in this SPEC.
- **Object Runtime** — the Event Engine does not call Object Runtime APIs and does not validate that any ObjectId referenced in an event payload actually exists. It may carry `ObjectId` values as opaque foreign keys (shared data-contract type from SPEC-001) but never queries, mutates, or depends on Object Runtime logic. Referential validation, if ever needed, belongs to a component that depends on both engines — never to the Event Engine itself.

## Terminology

- **Event** — a single immutable record of a business fact that has already happened, appended once and never altered.
- **Event Type** — a named, versioned metadata definition describing what payload fields an Event of this type carries. Supplied externally, treated as inert configuration data, exactly as Object Type is in SPEC-001.
- **Payload** — the structured, typed data carried by an Event, validated against its Event Type at append time.
- **Occurred At** — a caller-supplied timestamp describing when the underlying business fact happened.
- **Recorded At** — a caller-supplied timestamp describing when the Runtime accepted the Event for recording.
- **Event Source** — a caller-supplied reference identifying what recorded the event (a user, a device, a plugin, an integration adapter) — stored and returned verbatim, never interpreted.
- **Append Sequence** — a strictly increasing, Event-Engine-assigned integer (or equivalent monotonic marker) recording the order in which events were durably appended within a single Business Event Log. This is the sole ordering mechanism, and it is scoped to one Event Log instance — see "Event Ordering Requirements."
- **Correlation Reference (CorrelationId)** — an optional opaque value grouping related Events together, supplied by the caller.
- **Causation Reference (CausationId)** — an optional opaque value, typically referencing another Event's identity, identifying what caused this Event, supplied by the caller.
- **Business Event Log** — the complete, append-only, ordered store of all Events, as defined in Baseline Section 9.
- **Storage Provider** — the abstract, append-only persistence contract the Event Engine depends on; concrete implementation is out of scope here, matching SPEC-001's treatment of storage.

## Event Identity

- Every Event has a globally unique identifier, assigned once at append time and never reused, changed, or reassigned.
- Event identity carries no business meaning and no ordering guarantee by itself; ordering is governed exclusively by Append Sequence (see "Event Ordering Requirements"), never by parsing or comparing identifiers.
- An Event's identity is independent of its Event Type; two Events of different types never collide on identity.
- The Event Engine receives its Event ID generator through dependency injection, mirroring the deterministic identifier-generation approach used for Object IDs in SPEC-001, so that replay of a given sequence of append calls is always deterministic.

## Event Metadata

Every Event instance carries, alongside its payload:

- its identity
- its Event Type reference (name + schema version)
- its Append Sequence value
- two caller-supplied timestamps, Occurred At and Recorded At (see "Event Timestamps")
- its Event Source reference (see "Event Source")
- zero or more `ObjectId` references identifying which Object(s), if any, the fact concerns (stored opaquely — see Out of Scope)
- an optional correlation reference (CorrelationId) grouping this Event with related Events, and an optional causation reference (CausationId) identifying what caused it, e.g. linking a corrective Event to the Event it corrects — the Event Engine stores both as plain data and assigns them no special behavior

## Event Payload Model

- A payload is a set of named, typed fields, each field's type drawn from a fixed set of primitive kinds: text, localized-text-key, number, boolean, date/time, enum-value, reference (an `ObjectId`), and binary blob. This mirrors the `PropertyValue` kind set defined in SPEC-001 Section 7, reused here for consistency rather than redefined — see Assumptions.
- Payload structure is entirely determined by the Event's Event Type; the Event Engine assigns no meaning to any field beyond checking it matches the declared type and required/optional status at append time.
- Once appended, a payload's stored values are never modified. A correction to a previously recorded fact is represented by appending a new Event (optionally carrying a correlation reference to the original), never by altering the original Event's payload.

## Event Type Definitions

- Event Types are configuration, held in a typed metadata registry, exactly parallel to Object Types in SPEC-001: a name, a schema version, and a set of payload field definitions (name, kind, required/optional, structural constraints).
- Registering or updating an Event Type is a distinct, explicit administrative operation, separate from appending Event instances.
- An Event Type registration that would change the meaning of already-appended Events' payloads is not an update to the existing schema version — it must be registered as a new schema version, leaving prior Events interpretable exactly as they were recorded (Baseline Section 21: "Every transaction type carries a schema version from day one so historical records remain interpretable").
- Event Type definitions are append-only: an existing schema version, once registered, can never be modified. Only new schema versions may be registered; there is no in-place edit operation on a schema version, by design, not merely by convention.
- A new schema version may add optional fields but must not invalidate, reinterpret, or require re-validation of Events recorded under any prior schema version.

## Event Versioning

- Individual Events are not versioned and have no revision history — they are immutable from the moment of append, so the concept of a "base version" from SPEC-001 does not apply here.
- Versioning in the Event Engine applies only to Event Type *schemas*: each Event Type registration carries a schema version, and every appended Event records which schema version of its Event Type it was validated against, so that historical Events remain interpretable even after later schema versions are registered.

## Event Timestamps

- Every Event carries two timestamps, both supplied by the caller at append time: Occurred At (when the underlying fact happened) and Recorded At (when the Runtime accepted the Event for recording). The Event Engine never generates either timestamp itself and never substitutes its own clock reading for a missing one; a missing required timestamp is a validation failure, not a default.
- Timestamps are metadata for display and downstream interpretation only. They play no role in determining append order or in resolving any ordering question — that is Append Sequence's role exclusively (see next section).

## Event Source

- Every Event carries a caller-supplied Event Source reference identifying what recorded it (a user, device, plugin, or adapter identity — represented as an opaque reference, consistent with SPEC-001's `OwnershipInfo`/reference treatment).
- The Event Engine stores and returns the Event Source verbatim. It performs no authentication, no authorization, and no validation that the referenced source is legitimate — that is Security's responsibility, entirely outside this SPEC.
- An Event with a missing or empty Event Source is rejected at append time as a validation failure; the Event Engine does not silently record facts with no attributed source.
- Event Source is treated as an opaque Runtime identifier and must not contain localized display text or user-facing names; any human-readable label for a source belongs to the component that resolves the identifier for display, never to the Event Engine's stored value.

## Event Ordering Requirements

- The Event Engine assigns an Append Sequence value to every Event at the moment it is durably appended. This value strictly increases with each successful append and is never reused, skipped in a way that implies a gap represents a real event, or reassigned.
- Append Sequence is the single source of truth for event ordering. No component may derive or infer ordering from timestamps, identifiers, or arrival time at any layer other than the Event Engine's own append operation.
- Two concurrent append calls are serialized by the Event Engine such that each receives a distinct, correctly ordered Append Sequence value; the resulting order is deterministic with respect to the order the Event Engine actually committed each append, not the order callers issued the calls.
- Append Sequence is scoped to a single Business Event Log instance. It is not a cross-site, multi-deployment, or global ordering guarantee. Cross-site ordering, synchronization ordering, and conflict resolution across deployments are Synchronization Engine concerns, entirely outside this SPEC.

## Event Immutability

- Once an Event is successfully appended, none of its fields — payload, metadata, timestamp, source, or Append Sequence — may ever be modified.
- There is no update operation and no delete operation anywhere in the Event Engine's public API. This is an absolute invariant, not a default that can be overridden by a caller flag.
- A correction, retraction, or amendment to a previously recorded fact is always represented as a new Event (Baseline Section 2: "Facts, once recorded, are never deleted"), optionally referencing the original via the correlation reference described in "Event Metadata." The Event Engine assigns no special interpretation to a correction; recognizing and acting on a correction relationship is a concern for components that read the Event Log, not for the Event Engine itself.

## Validation Rules

At append time, and only at append time, the Event Engine validates:

- the Event Type reference resolves to a registered Event Type and schema version
- every required payload field is present
- every present payload field matches its declared kind and structural constraints
- no payload field is present that isn't declared on the Event Type
- both Occurred At and Recorded At are present and well-formed
- an Event Source is present
- any `ObjectId` reference present in the payload is structurally well-formed (the Event Engine checks that it looks like a valid identifier; it does not check that the referenced Object actually exists — see Out of Scope)

No validation occurs, and no re-validation is ever performed, after an Event has been successfully appended — immutability means there is nothing left to validate.

## Public Runtime API

Described as behavior/contracts, not code. The Event Engine exposes exactly these capability groups:

**Type registry operations**
- Register or update an Event Type definition (a new schema version; never an in-place redefinition of an existing schema version).
- Retrieve an Event Type definition by name and schema version.

**Append operation**
- Append a new Event: given an Event Type reference, payload, Occurred At, Recorded At, Event Source, optional `ObjectId` references, optional correlation reference, and optional causation reference, validates and durably records it, returning its identity and assigned Append Sequence value. Fails entirely (no partial record) on any validation failure or Storage Provider failure.

**Read operations**
- Read a single Event by identity.
- Read a contiguous range of Events by Append Sequence, in strictly ascending order, supporting incremental/streaming consumption rather than requiring the full log to be materialized at once — mirroring the internal bulk-read surface defined for the Query Engine's future use in SPEC-001 Section 6. This is the only mechanism by which any other component (Rule Engine, Query Engine, Statistics Engine, Transaction Engine) observes new Events; it is not a filtering or search API.

No other operations exist. In particular: no update, no delete, no filter-by-payload-field, no search, and no cross-event transactional grouping of any kind.

## Error Model

The Event Engine returns structured, typed errors — never panics across its public API boundary — covering at minimum:

- Event Type not found / schema version not registered
- Validation failure (missing required field, unknown field, type mismatch, constraint violation, missing or malformed Occurred At / Recorded At, missing Event Source, malformed `ObjectId` reference)
- Event not found (on read-by-identity)
- Invalid or out-of-range Append Sequence bounds (on range read)
- Duplicate identity on append (should not occur given identity generation, but must be a defined, handled error rather than an assumption)
- Malformed Event Type metadata on registration
- Storage Provider failure during append

Each error kind is distinct and machine-distinguishable, matching the error-taxonomy approach established in SPEC-001 Section 9. A validation failure error identifies the specific failing field or metadata element and the expected constraint, wherever applicable — not merely a generic validation error. A Storage Provider failure during append never leaves a partial record; the log is left exactly as it was before the failed append.

## Security Boundary

- The Event Engine is not an authorization engine. It records the caller-supplied Event Source verbatim and performs no check of whether that caller was entitled to record the fact — that determination happens upstream, before the append call reaches the Event Engine.
- The Event Engine does not perform encryption. Whether recorded events are encrypted at rest is a Storage Provider concern, exactly as in SPEC-001; the payload data contract must stay opaque enough at this layer that an encrypting Storage Provider can be substituted without a contract change.
- Because every Event is permanently retained (Baseline Section 2), the Event Engine must never record a payload field that a caller did not explicitly supply — it must not backfill, infer, or default sensitive fields, since doing so would permanently and irreversibly attribute inferred data to a recorded fact.

## Localization Boundary

- Event Type names and payload field names are language-neutral tokens; the Event Engine never stores or returns display text for them.
- A payload field of kind `localized-text-key` (see "Event Payload Model") references a translation key rather than embedding language-specific text, keeping the Event Engine itself locale-neutral, exactly as in SPEC-001.
- Any free-text payload value is stored as supplied, optionally tagged with a language identifier; the Event Engine performs no translation, formatting, or locale-aware behavior.

## Performance Requirements

- Append must be a near-constant-time operation with respect to total log size; the Event Engine must not degrade as the log grows, consistent with Baseline Section 23's industrial, weak-hardware, offline deployment target.
- Sequential/streaming range reads must support incremental consumption without requiring the full log, or even the full requested range, to be materialized in memory at once.
- Event Type metadata must be cached in memory after first load, bounded in size relative to the number of distinct registered Event Types, not the number of appended Events.
- The Event Engine must contribute a small, fixed share of the overall Runtime footprint target; it must not embed any indexing, search, or aggregation capability reserved for other components.

## Determinism Requirements

- Given an identical sequence of append calls with identical inputs, the Event Engine always produces the identical resulting log content and Append Sequence assignment — no hidden nondeterminism from concurrency, caching, or iteration order.
- Concurrent append calls are serialized such that the resulting order is well-defined and reproducible from the Event Engine's own commit order, never from wall-clock timestamps or caller-side arrival order.
- The Event Engine never reads its own wall clock to make a correctness or ordering decision; timestamps are caller-supplied data, not an input to any Event Engine decision.
- The iteration order of returned payload fields and Event metadata is itself deterministic — reading the same stored Event twice always returns its fields in the same order.
- Validation errors are deterministic: the same invalid input always produces the same error kind and, where applicable, identifies the same failing field.

## Testing Requirements

- A conformance test suite must run against any Storage Provider implementation (including a simple in-memory one) without modification, proving storage-agnosticism, mirroring SPEC-001's approach.
- Tests proving immutability: no code path exists, under any input, that modifies or removes a previously appended Event.
- Tests proving deterministic ordering under concurrent append calls.
- Negative tests for every error kind in "Error Model."
- A boundary test proving the Event Engine has zero compile-time or runtime dependency on Content-Package-defined code, and zero calls into Object Runtime, Rule Engine, Process Engine, Query Engine, Transaction Engine, or Security logic.
- A test proving a forced Storage Provider failure during append leaves the Business Event Log completely unchanged.
- Tests proving deterministic field/metadata iteration order and deterministic error output for repeated invalid inputs.

## Acceptance Criteria

1. Appending an Event with a missing required payload field is rejected before any record is durably stored.
2. Two concurrent append calls each receive a distinct, correctly ordered Append Sequence value, and replaying the same calls against a fresh instance in the same commit order produces byte-identical resulting log content.
3. No public API method exists that modifies or deletes a previously appended Event.
4. No public API method exists that accepts a filter, search predicate, sort order, or aggregation over payload fields.
5. An Event referencing an `ObjectId` that does not correspond to any real Object is accepted without error (the Event Engine performs no existence check).
6. Swapping the Storage Provider implementation requires no change to any Event Engine caller.
7. Every append call missing Occurred At, Recorded At, or an Event Source is rejected.
8. Reading a range of Events by Append Sequence returns them in strictly ascending order, and supports being consumed incrementally without loading the entire range into memory at once.
9. Registering a new schema version for an existing Event Type does not alter how previously appended Events under the prior schema version are read back.
10. A forced Storage Provider failure during append leaves the Business Event Log completely unchanged — no partial record is ever visible.
11. Repeating the same invalid append call against a fresh instance produces the identical error kind and field reference every time; reading the same Event twice returns its fields in the same order every time.

## Assumptions

1. **Payload kind set reuse** — this SPEC reuses the `PropertyValue` kind set from SPEC-001 Section 7 (text, localized-text-key, number, boolean, date/time, enum, reference, blob) for Event payload fields, rather than defining a new, separate set. The Baseline does not specify this explicitly; sharing the primitive types keeps the two engines consistent without creating a dependency between them (the kinds are a shared data-contract convention, not shared code or a shared registry).
2. **Identifier format** — assumed to be the same time-orderable unique identifier class assumed for `ObjectId` in SPEC-001 (e.g., UUIDv7/ULID-class), for consistency, while ordering itself still depends solely on Append Sequence, never on the identifier.
3. **Correlation and causation references are inert** — assumed that the optional correlation reference (CorrelationId) and causation reference (CausationId, typically pointing at an earlier Event) are both stored as plain opaque data with no Event-Engine-side interpretation or validation that any referenced Event exists. Recognizing and acting on corrections or correlated groups is left entirely to downstream readers.
4. **`ObjectId` reference cardinality** — assumed that an Event may reference zero, one, or many `ObjectId` values in its payload (e.g., a single-machine "machine stopped" fact vs. a multi-object fact), with the exact cardinality per field declared by the Event Type definition, mirroring SPEC-001's relation-cardinality approach.
5. **Append Sequence representation** — assumed to be a simple monotonic integer, matching the `Version` representation assumption in SPEC-001, for the same reasons (simplicity, no Baseline-mandated alternative).
6. **Range-read pagination shape** — assumed that the sequential read API takes a starting Append Sequence and a maximum batch size, returning events in order plus a continuation point, rather than an open-ended stream primitive — chosen for consistency with the incremental-consumption requirement without prescribing a specific streaming abstraction.

## Blocking Questions

None. Every ambiguity encountered was resolvable by staying inside the stated Event Engine boundary (Baseline Section 9, Section 2) and by following the precedent already set in SPEC-001; each is recorded as an assumption above rather than a blocker.

## Implementation Checklist for Codex

1. Define the Event, EventTypeRef, payload field-kind, EventMetadata, EventSource, Occurred At / Recorded At timestamps, and correlation/causation reference data contracts as idiomatic Rust types, reusing the payload-kind set established in SPEC-001 where applicable — no business-specific variants.
2. Define the append-only Storage Provider trait boundary the Event Engine depends on, without implementing a concrete provider (an in-memory test provider is in scope for testing only).
3. Implement Event Type registration/retrieval against the metadata registry, including schema-version tracking.
4. Implement the Append operation exactly as scoped ("Public Runtime API"), including full validation ("Validation Rules") and deterministic Append Sequence assignment under concurrency.
5. Implement Read-by-identity and the incremental/streaming range-read-by-Append-Sequence operation — no additional read operations.
6. Implement the full error taxonomy from "Error Model" as distinct, structured error types.
7. Enforce immutability as a structural guarantee (no code path capable of mutating or removing a stored Event), not merely as an API-level omission.
8. Write the conformance test suite against the in-memory Storage Provider covering "Testing Requirements" and all eleven acceptance criteria, including the forced-storage-failure and determinism tests.
9. Write a boundary/lint check confirming zero references to any Content-Package-defined symbol, and zero calls into Object Runtime, Rule Engine, Process Engine, Query Engine, Transaction Engine, or Security logic, from within the Event Engine source tree.
10. Do not implement anything listed in "Out of Scope," even as a stub, beyond the minimal shared data-contract shapes explicitly named above (e.g., reuse of `ObjectId` and payload-kind types as opaque references).
