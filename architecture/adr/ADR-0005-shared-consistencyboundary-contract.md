# ADR-0005 — Shared `ConsistencyBoundary` Contract

Status: Approved (Gate 0), pending repository commit. Project Manager review
independently verified every SPEC-004 citation (Sections 7, 9, 18) against committed
text, and verified "Document H" against the lettered document scheme fixed in the
Gate 1 instruction's Part 13 (H = ADR Recommendations). One sub-citation could not be
independently re-derived from any file in this workspace: the numeral "I.2" for the
ConsistencyBoundary blocking decision. Document I (Blocking Decisions) was never
committed as a standalone file — it exists only in the original Gate 1 chat
transcript — so "I.2" rests on recollection of that transcript, not a re-checkable
artifact, unlike every other citation in this ADR. The underlying substance is not in
doubt (SPEC-004 itself and the independent Gate 1 technical review both confirm
ConsistencyBoundary is a genuine blocking gap), so this does not change the Approved
status, but the Document I report should be committed to the repository as its own
file so this citation — and any future one — becomes independently verifiable rather
than dependent on memory. This ADR fixes a contract shape and ownership boundary
only; it does not add the upstream public APIs each engine will eventually need to
produce its own component value, and it does not authorize any Query Engine
implementation that consumes this contract.

Authority: subordinate to Architecture Baseline v1.0; sibling to ADR-0001, ADR-0002,
ADR-0003. Does not restate any of them.

## Context

SPEC-004 (Query Engine) Sections 7, 9, and 18 require that "identical accessible data
snapshot" — the condition its determinism guarantees are stated against — be a
checkable contract, not an assumption. The corrected SPEC-004 introduced
`ConsistencyBoundary` for exactly this purpose: a value capable of identifying, at
minimum, an Object Runtime revision/snapshot marker, an Event Engine sequence upper
bound, a Transaction Engine committed-through identifier, a Computed Value Source
version, a Concept Mapping version, and the query-definition version a given
execution was defined against.

No such type exists today. SPEC-004 Gate 1 repository inspection confirmed
`runtime-contracts` currently defines only `ObjectId`, `Version`, `UnitOfWork`, and
`PropertyValue`/`PropertyValueKind` — no consistency-boundary type — and confirmed
that none of Object Runtime, Event Engine, or Transaction Engine currently exposes a
public API returning a stable snapshot/upper-bound marker. This is recorded as
Blocking Decision I.2 in the Gate 1 report. Because a `ConsistencyBoundary` value is,
by definition, composed from state owned by at least three separate engines plus
Query-Engine-local versioning concepts, it is a cross-engine concept in exactly the
sense the Gate 1 review's guardrails describe — it must not be invented inside Query
Engine, and belongs in `runtime-contracts` or requires this ADR. It requires this
ADR.

## Decision

1. **`ConsistencyBoundary` is a shared, business-neutral Runtime data contract**,
   added to `runtime-contracts` alongside `ObjectId`, `Version`, `PropertyValue`, and
   the Unit-of-Work contracts (ADR-0001, ADR-0002, ADR-0003). It follows the same
   test those ADRs already applied: business-neutral, needed by more than one Runtime
   component, and unsafe to duplicate locally without creating drift.

2. **Canonical shape: a composite, versioned value with named, independently
   optional component slots** — one slot per contributing engine or version concept
   (Object Runtime, Event Engine, Transaction Engine, Computed Value Source, Concept
   Mapping, query-definition). A given `ConsistencyBoundary` populates only the
   slots relevant to the query it was built for; an aggregation-only query touching
   solely Event Engine data need not carry a Transaction Engine slot. This is a
   deliberate rejection of a single opaque whole-Runtime hash or version integer —
   see Rejected Alternatives.

   **The query-definition slot identifies a canonical logical query definition, not
   merely a stored identifier or revision number.** A saved query's object identity
   or revision is not, by itself, sufficient for reproducibility: the same stored
   query can legitimately produce a different result if evaluation semantics, unit
   conversion rules, collation, or ordering semantics change, even though the stored
   query definition itself did not. Where such a concern is already covered by
   another slot (Computed Value Source version, Concept Mapping version), this slot
   does not duplicate it; where evaluation/collation/ordering semantics are not yet
   covered by any other slot, this slot's value must incorporate them — e.g., as a
   canonical, versioned representation of the query's logical structure plus the
   semantic version of the evaluation rules applied to it — not just a saved-query
   revision number. The exact encoding of "canonical logical query definition" is
   left to Query Engine's own future implementation work; this ADR fixes only that a
   bare stored-object revision number is not an acceptable substitute.

3. **Each slot is explicitly three-state, not two-state.** "Not requested" and
   "unavailable" are not the only two possibilities; a slot that is requested and
   genuinely producible carries a value. Every slot must therefore be capable of
   representing exactly one of: **not requested** (the query never needed this
   component), **unavailable** (needed, but the owning source could not supply a
   stable value, carrying a structured reason — point 4), or **available** (carrying
   the component's actual value). Collapsing "unavailable" and "not requested" into a
   single absent-field representation would make it impossible for a consumer to
   distinguish "this query didn't touch that engine" from "this query needed that
   engine's boundary and didn't get one" — precisely the ambiguity this contract
   exists to remove.

4. **"Unavailable" requires a structured reason, not a bare marker.** A consuming
   engine cannot make a sound decision (reject for deterministic execution, mark
   weak-consistency, produce an explicitly non-reproducible result, or defer the
   capability — SPEC-004 Section 17's original framing) from an undifferentiated
   "unavailable" flag alone. The reason must distinguish, at minimum: the upstream
   public API for this component does not yet exist (a Gate 1/ADR-level gap, not a
   runtime condition); the source is transiently unreachable; the source does not
   support this concept at all (a permanent, not transient, limitation); the caller
   lacks access to the information needed to produce the value; the previously used
   marker has been lost or is no longer verifiable; or the source produced the value
   only under a weaker consistency guarantee than this boundary normally requires.
   Capability-not-implemented, transient failure, permanent unsupported source,
   authorization restriction, and degraded execution are not interchangeable, and a
   consumer's correct response differs for each.

5. **Equality is a strict, narrow comparison; compatibility is a separate concept
   this contract does not resolve implicitly.** Two `ConsistencyBoundary` values are
   equal if and only if they have an identical set of populated slots, identical
   state (not-requested / unavailable / available) for every slot, and identical
   value for every slot in the available state. A boundary with only an Object
   Runtime slot populated is never equal to one with an Object Runtime slot and an
   Event Engine slot populated, even if the Object Runtime slots match exactly —
   matching on a shared subset of populated slots is a **compatibility** question,
   not equality, and this contract does not answer it implicitly. A consumer that
   needs subset/superset or partial-overlap reasoning must request it through an
   explicit, separately named comparison procedure it defines and justifies itself;
   the default the shared contract provides is strict equality only, so that two
   different consumers cannot silently arrive at two different interpretations of
   what "the same boundary" means.

6. **Every populated slot value must meet a minimum stability standard, not merely
   exist.** For Object Runtime's revision/snapshot marker, Event Engine's sequence
   upper bound, Transaction Engine's committed-through identifier, and any future
   component slot alike, the value must: be stable for the same logical snapshot
   (repeated reads against unchanged state return the same value); not depend on any
   single process's in-memory state; remain interpretable after a process restart;
   never be derived from an unstable or non-deterministic hash; carry a defined
   scope or namespace so it cannot be confused with a superficially similar marker
   from a different source or deployment; never produce a false-positive match on
   reuse (e.g., a wrapped or reset counter must not silently compare equal to an
   earlier, different snapshot); and remain interpretable long-term, not just within
   a single running process's lifetime. A component whose owning engine cannot meet
   this standard must report itself as **unavailable** (point 3) with the
   appropriate structured reason (point 4) rather than supply a marker that merely
   happens to exist without actually being stable.

7. **Canonical, deterministic serialization is required, and `runtime-contracts` is
   its normative owner — not merely the type's owner.** `ConsistencyBoundary` must
   support a stable, deterministic encoding (explicit field order, no hash-map
   iteration order, no `Debug`-formatting-as-encoding) so it can be recorded on a
   completed result and compared reproducibly, mirroring the canonical-encoding
   discipline Query Engine's own Slice 4 already established for its private
   ordering module. Unlike that internal, test-only encoding, this one is a
   cross-crate contract: if more than one consumer independently implemented its own
   encoding of the same logical value, two different byte representations of "the
   same" boundary could exist, silently defeating the equality rule in point 5.
   `runtime-contracts` is therefore the single normative specification of the
   canonical format or the canonical encoding rules — a concrete helper
   implementation of that specification may live in another crate, but there must be
   exactly one normative specification, and any additional implementation must be
   verified against a set of mandatory, shared cross-crate test vectors rather than
   trusted to match by construction. This ADR still does not choose a concrete
   format (CBOR, JSON, protobuf, or a hand-written binary layout) — it only fixes
   that there must be one normative specification and a test-vector mechanism to
   keep every implementation of it honest.

8. **Ownership is split cleanly between the shared type and each contributing
   engine.** `runtime-contracts` owns the `ConsistencyBoundary` type definition,
   its slot shape, and its equality/compatibility rules. Each contributing engine
   (Object Runtime, Event Engine, Transaction Engine) owns producing its own
   component value through its own future public API — this ADR does not add those
   APIs. A consumer (Query Engine or any future one) only ever composes a
   `ConsistencyBoundary` by reading each populated slot from its owning engine; it
   never fabricates, infers, or estimates a component value itself.

9. **Adding the upstream component-producing APIs to Object Runtime, Event Engine,
   and Transaction Engine is explicitly out of scope for this ADR.** Each is its own
   small, separately gated SPEC amendment (e.g., a "current revision marker" query
   for Object Runtime, a "current maximum Append Sequence" query for Event Engine, a
   "committed-through Append Index" query for Transaction Engine), to be scoped and
   approved individually once this ADR fixes what shape each must ultimately supply.

## Consequences

- SPEC-004's determinism requirements (Sections 7, 9, 18) gain one real, canonical,
  cross-engine type to reference instead of an assumed "identical snapshot" concept.
- Query Engine's Gate 1 Blocking Decision I.2 becomes resolvable once (a) this ADR
  is approved, (b) `ConsistencyBoundary` exists in `runtime-contracts`, and (c) at
  least the component APIs a given query actually needs exist upstream. None of
  those steps is completed by this ADR alone.
- Object Runtime, Event Engine, and Transaction Engine each acquire a small future
  SPEC amendment obligation (point 9) — none is authorized or scoped by this ADR
  itself.
- Any consumer needing subset/superset or partial-overlap reasoning between two
  boundaries must define and justify its own explicit comparison procedure (point
  5) — this ADR does not supply one by default, deliberately.
- `runtime-contracts` acquires an obligation to publish and maintain the
  cross-crate test vectors required by point 7, not just the type definition —
  without them, the "one normative specification" guarantee has no enforcement
  mechanism.
- This ADR does not implement Query Engine's consumption of `ConsistencyBoundary` —
  attaching it to query execution, results, and Context Packages remains a separate,
  later, explicitly authorized implementation slice, matching the same discipline
  already applied to Slices 1–6.
- Until every slot a given query needs is genuinely producible, that query cannot
  honestly claim full deterministic reproducibility — this ADR makes that limitation
  visible and checkable rather than papering over it.

## Rejected Alternatives

- **Query Engine defines its own local `ConsistencyBoundary`/`ConsistencyToken`** —
  rejected: this is the exact cross-engine-type-invention case the Gate 1 review
  guardrails exist to prevent, already flagged twice (Gate 1 report Document D,
  Project Manager review round one).
- **Treat "identical accessible data snapshot" as an informal, assumed guarantee
  with no checkable type** — rejected: this is precisely the gap the SPEC-004
  correction round already found unacceptable — an unfalsifiable determinism claim
  is not a determinism guarantee.
- **A single opaque hash or monotonic counter covering the whole Runtime** —
  rejected: cannot express "this query only touched Event Engine and a Computed
  Value Source," cannot represent partial or per-component unavailability, and does
  not map onto the fact that each engine independently owns and advances its own
  revision concept at its own rate. A composite, slotted value is the only shape
  that lets a query's actual footprint determine which components matter.
- **Fix the concrete serialization format now, inside this ADR** — rejected: the
  exact format (CBOR, JSON, protobuf, hand-written binary) is an implementation
  detail properly resolved once a consuming Gate 1 design knows exactly how the
  value will be constructed and compared; fixing only that `runtime-contracts` is
  the normative owner and that cross-crate test vectors are mandatory (point 7)
  avoids over-specifying a decision this ADR does not need to make, while still
  preventing the multiple-incompatible-encodings failure mode a fully open-ended
  "encoding is someone else's problem" stance would risk.
- **A two-state slot model (present vs. absent) instead of three states** —
  rejected: conflates "this query never needed that engine" with "this query needed
  that engine and didn't get a stable value," which is exactly the ambiguity
  SPEC-004's determinism correction round already found unacceptable for the
  boundary concept as a whole; the three-state model (point 3) is required for the
  same reason the composite slotted shape itself is required.
- **An undifferentiated "unavailable" flag instead of a structured reason** —
  rejected: a consuming engine's correct response to "the upstream API doesn't
  exist yet" (a Gate 1/ADR-level gap) is not the same as its correct response to
  "the source is temporarily down" or "the caller isn't authorized to see this" —
  collapsing these into one flag would force every consumer to either over-react
  (treat a transient failure as permanent) or under-react (treat a permanent
  limitation as retryable).
- **Defining equality to also cover subset/superset compatibility by default** —
  rejected: this was the ADR's own original ambiguity, identified in review — two
  independent consumers could reasonably implement two different subset-matching
  rules while both believing they implemented "the" compatibility rule the contract
  defines. Keeping equality strict and pushing compatibility to an explicit,
  separately justified procedure (point 5) removes that ambiguity entirely rather
  than trying to specify one universal compatibility rule that fits every future
  consumer's needs.

## References

SPEC-004 Sections 7 (ConsistencyBoundary terminology), 9 (evidence immutability and
completed-result requirements), 18 (Determinism Requirements, parallelism invariant).
SPEC-004 Gate 1 repository-grounded technical design report, Blocking Decision I.2,
Document H (ADR Recommendations). ADR-0001 (shared runtime-contracts crate,
precedent for the neutral-type test applied in Decision 1). ADR-0003 (precedent for
promoting a single-engine-originated type into a shared, canonical Runtime contract).
