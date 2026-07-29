# SPEC-004 — Query Engine

Revision: B
Gate 0: Complete
Repository status: Committed
Implementation: Not Authorized
Next required stage: Gate 1 repository-grounded technical design

Status: Draft — **Gate 0, Revision B — requirements and initial architecture proposal
only.** This document is not an implementation contract. It does not authorize Codex to
design, scaffold, or write any code. It is submitted for Project Manager review; any
resulting ADR, repository change, or Gate 5 implementation activity requires a separate,
explicit decision after this Gate 0 proposal is reviewed.

**Revision A note.** The original Gate 0 submission received a Project Manager decision
of Conditional Approval: architecture rated sound, but product-vision alignment judged
incomplete against PRS-000's framing of Open-EQMS as an organizational operating system,
not only a Runtime query mechanism. Sections 24–32 below are Revision A's response. Per
the Project Manager's own instruction, they are additive: they extend the Query Engine's
business purpose and the set of inquiries it must make expressible, without changing the
technical architecture, boundaries, or invariants established in Sections 1–23. Every new
section is explicitly composed from primitives already defined there (QuerySource,
Predicate, TraversalSpec, TemporalScope, Aggregation, Computed Value Source,
ResultClassification, ResultProvenance) rather than introducing new Query Engine
capability, to keep the Non-Goals in Section 5 intact.

**Revision B note.** This revision adds one new mandatory section: the **Parallel
Inquiry Fabric** (Section 33), an optional execution strategy for complex organizational
inquiries, plus a formalized Query Execution Pipeline (Section 34) that shows where it
plugs in. Nothing in Sections 1–32 is removed or weakened — the deterministic, read-only,
permission-aware, and storage-agnostic requirements established there apply to every
Parallel Inquiry Fabric branch without exception. The Fabric is explicitly scoped as an
execution strategy inside or adjacent to the Query Engine's execution layer, not a new
engine: Section 33 closes with an explicit confirmation that it does not create a second
Synchronization Engine, Rule Engine, AI-agent framework, or authoritative evidence store.
Codex implementation remains unauthorized.

Authority order: (1) Architecture Baseline v1.0, (2) AGENTS.md, (3) ADR-0001 (shared
runtime-contracts crate), ADR-0002 (shared Unit-of-Work contract), ADR-0003 (shared
`Version` contract), (4) SPEC-001 Object Runtime, SPEC-002 Event Engine, SPEC-003
Transaction Engine — read-only reference, consumed as upstream data sources, never
depended on as code, (5) this document, (6) the mandatory product-requirements input
supplied by the Project Manager for this Gate 0 proposal (reproduced in Section 3).

---

## 1. Purpose

Baseline Section 8 states: "The Query Engine is the single source for querying
structured Runtime data. Business Rules, Statistics, KPIs, Reports, and the AI adapter
layer all read through it; none implement a parallel query mechanism." The Query Engine
exists so that every consumer of Runtime data — a human user, a Rule DSL evaluation, a
Statistics/KPI calculation, a report, an AI adapter, a dashboard widget — reads through
exactly one deterministic, permission-aware, storage-agnostic contract, instead of each
consumer inventing its own filtering, joining, or aggregation logic against the Object
Runtime, Event Engine, or Transaction Engine directly.

Per the Project Manager's mandatory input for this Gate 0 proposal, the Query Engine's
purpose is broader than database search or SQL abstraction: it is the unified,
permission-aware inquiry layer of Open-EQMS, reducing the time from data to context,
understanding, decision, and action across live data, historical events, objects,
relationships, documents, and — where already ingested into the Runtime — externally
sourced information.

**Revision A framing.** The Query Engine is not designed to answer "where is the data?"
but "what does the organization know about this situation?" This is the lens Sections
24–32 apply to the same technical contract defined in Sections 1–23: every organizational
inquiry described there — semantic, graph-shaped, causal, resilience, competency,
policy, maturity, or AI-context-oriented — resolves to an ordinary Query, Predicate,
TraversalSpec, or Computed Value Source already defined in this document, addressed by
business meaning rather than by raw structure.

## 2. Gate Status and Review Boundary

This is a Gate 0 submission: requirements and initial architecture only. It contains no
Rust types, no crate layout, and no implementation checklist, unlike SPEC-001/002/003 at
their Gate-1-and-later stage. It explicitly identifies open architectural questions
(Section 17) that require a Project Manager decision — or a dedicated ADR — before this
document could advance to a Gate-1 implementation-ready SPEC. Codex must not be given
any instruction package derived from this document until that advancement happens
explicitly.

## 3. Mandatory Product Requirements (input, reproduced for traceability)

The following twenty requirement areas were supplied by the Project Manager as mandatory
input to this proposal and are addressed by the sections indicated:

1. Unified inquiry across Runtimes and connected systems — Sections 1, 6, 10.
2. Queries over objects/relationships, Event Engine records, Transaction Engine records,
   documents/revisions, live streams, historical timelines, external ERP/MES/SCADA/PLC/
   CAD/Office/database/API sources — Sections 6, 7, 8, 10.
3. Structured programmatic query contracts — Section 8.
4. Future visual query builder, no database-specific language exposed to users —
   Sections 8, 11, 17.
5. Natural-language inquiry as an upper product layer; deterministic core usable without
   AI — Sections 1, 11, 17.
6. Queries across variable and evolving data models — Sections 7, 12.
7. Temporal and timeline inquiry — Section 7 (TemporalScope).
8. Cross-entity traceability across products, batches, machines, tools, measuring
   equipment, operators, materials, maintenance, calibration, process changes,
   complaints/nonconformities — Section 7 (TraversalSpec).
9. Historical, current-state, continuous, and subscription-based queries — Section 7
   (ExecutionMode).
10. Filtering, sorting, projection, grouping, aggregation, pagination — Section 7.
11. Connection to backend calculators (evaluation matrices, classifiers, categorizers,
    statistics, KPI, risk, flexibility/resilience calculations, approved micro-scripts) —
    Section 10.
12. Explicit distinction between source facts, computed values, statistical
    correlations, AI-generated hypotheses, human-approved conclusions — Section 9.
13. Permission-aware execution, prevention of indirect disclosure via relationships,
    aggregates, or summaries — Section 13.
14. Evidence and explainability metadata — Section 9.
15. Reusable, versioned saved queries for dashboards, reports, audits, validations,
    reviews, alerts, widgets — Section 12.
16. Offline and synchronized-query considerations without assigning synchronization
    responsibility to the Query Engine — Section 15.
17. Clear boundaries with runtime-contracts, Object Runtime, Event Engine, Transaction
    Engine, Synchronization Engine, Security, Statistics Engine, KPI Engine, Rule Engine,
    GUI Engine — Section 6.
18. Execution limits, cancellation, determinism, resource protection, error semantics —
    Sections 14, 16, 18.
19. Extensibility for future graph, document, time-series, data-lake, external-source
    adapters without coupling the public contract to a database technology — Section 10.
20. Testing and verification requirements — Section 20.

**Business-domain inquiry surface (explicit enumeration).** The fuller product
requirements document additionally enumerates the domains the Query Engine must support
inquiry across: products and production batches, machines and equipment, measuring
devices and calibrations, processes and validations, contracts and customer
requirements, complaints and nonconformities, suppliers and materials, competencies and
training, internal policies and external standards, organizational responsibilities, and
risks/actions/business outcomes. None of these are new Query Engine responsibilities —
each is a business Object Type, Event Type, or Transaction domain supplied externally by
a Content Package (Baseline Section 5) and stored by Object Runtime/Event Engine/
Transaction Engine exactly like any other. The Query Engine's obligation is that its
QuerySource/Predicate/TraversalSpec model (Section 7) is fully generic across all of
them without hardcoding any one domain's semantics — the same "no business meaning in
the Runtime" principle already enforced in SPEC-001/002/003. This enumeration is
recorded here for traceability, not because it changes the Query Engine's design.

## 4. Scope

This SPEC, at Gate 0, defines for the Query Engine only:

- the query model (what a query is, structurally)
- the result model, including the evidence/provenance envelope and result classification
- the permission-execution model
- the boundary between the Query Engine and every other Runtime component, Native
  Plugin adapter, and external system
- the extension-point contract for future data-source adapters and computed-value
  sources
- the error model, execution-limit model, and determinism requirements
- proposed (not final) acceptance criteria and testing requirements
- assumptions and unresolved architectural questions requiring a decision before Gate 1

It does not yet define: concrete Rust types, crate layout, a storage/index
implementation, or any Codex-facing implementation checklist. Those belong to a Gate 1
revision of this document, after the open questions in Section 17 are resolved.

## 5. Non-Goals (explicit exclusions)

Per the Project Manager's instruction to avoid an oversized Query Engine, the following
are explicitly **not** Query Engine responsibilities, regardless of how naturally they
might seem to fit alongside "query":

- **Data acquisition or ingestion.** Connecting to ERP, MES, SCADA, PLC, CAD, Office,
  external databases, or external APIs is Native Plugin Architecture's job (Baseline
  Sections 14, 16). The Query Engine never opens a connection to an external system
  itself — see Section 6.
- **Synchronization.** Delta computation, conflict resolution, offline queueing, and
  peer-assisted transfer belong entirely to the Synchronization Engine (Baseline Section
  19). The Query Engine's only relationship to synchronization is that it must remain
  usable against whatever data a given node currently has, online or offline — it does
  not implement or coordinate the sync process itself.
- **Statistical calculation.** Computing averages, trends, distributions, and rates is
  the Statistics Engine's job (Baseline Section 10). The Query Engine surfaces already
  -computed statistical values as a queryable result type; it does not compute them.
- **KPI evaluation.** Evaluating a statistic against a threshold, target, or escalation
  rule is the KPI Engine's job (Baseline Section 10). Same relationship as Statistics.
- **Business rule evaluation.** The Rule DSL invokes the Query Engine (Baseline Section
  7); the direction never reverses. The Query Engine never evaluates a WHEN/IF/THEN rule.
- **AI interpretation.** Natural-language inquiry, summarization, explanation, and
  hypothesis generation are AI-adapter responsibilities, external to the Runtime
  (Baseline Section 15). The Query Engine is the deterministic layer AI reads through; it
  must remain fully usable and testable with zero AI component present or running.
- **GUI rendering and the visual query builder's UI.** The Query Engine emits result
  models and accepts a structured query definition; rendering a query builder or a result
  view is GUI Engine territory (Baseline Section 12).
- **Authentication and permission-policy authorship.** Security (Baseline Section 13)
  decides what a caller may see. The Query Engine enforces that decision on every
  candidate record; it does not define roles, capabilities, or delegation policy.
- **Any write, mutation, or deletion of any Runtime data.** The Query Engine is read-only
  without exception. This is an absolute invariant, parallel to Object Runtime's
  no-delete invariant and Event Engine's immutability invariant.
- **Owning the authoritative store of any data it queries.** The Query Engine builds
  indexes/projections over data owned by Object Runtime, Event Engine, Transaction
  Engine, and document storage; it is never the source of truth for any record.
- **Defining business concepts, ontologies, or organizational meaning.** Revision A adds
  Semantic Inquiry Layer, Organizational Knowledge Graph, and Business Inquiry Pattern
  capabilities (Sections 24–30), all of which resolve business vocabulary (e.g.
  "Complaint," "Supplier," "resilience," "maturity") to structural queries via
  Concept Mappings and Business Inquiry Patterns supplied externally by Content
  Packages, consumed as inert, versioned configuration — exactly as Object Type and
  Event Type metadata already are (SPEC-001 Section 5.5, SPEC-002 "Event Type
  Definitions"). The Query Engine never authors, infers, or hardcodes a business concept
  or organizational-maturity model itself; doing so would be a scope violation of this
  SPEC, identical in kind to the prohibition on business semantics already established
  for Object Runtime (SPEC-001 Invariant 1) and Event Engine.
- **A second Synchronization Engine, Rule Engine, AI-agent framework, or authoritative
  evidence store.** Revision B's Parallel Inquiry Fabric (Section 33) decomposes complex
  inquiries into concurrent branches that exchange verified intermediate evidence, but
  this exchange is inquiry-scoped and temporary, never a replication of authoritative
  Runtime stores, a synchronization-delta mechanism, a rule-evaluation engine, or an
  autonomous agent framework. See Section 33's Scope Boundary subsection for the full
  statement of what the Fabric must never become.

## 6. Engine Boundaries

- **Object Runtime, Event Engine, Transaction Engine.** Each already exposes (or, for the
  Transaction Engine, already implements) a non-filtering internal bulk/streaming read
  surface explicitly reserved for the Query Engine's use (SPEC-001 Section 6, SPEC-002
  "Public Runtime API," SPEC-003's Read-by-identity/range operations). The Query Engine
  is the sole consumer of these surfaces for building its own indexes/projections. None
  of the three engines gain any new dependency on the Query Engine as a result — the
  dependency runs one way, Query Engine depending on each engine's neutral read surface,
  never the reverse.
- **External systems (ERP, MES, SCADA, PLC, CAD, Office, external databases, external
  APIs).** Per Baseline Sections 8, 14, and 16 read together: the Query Engine queries
  "structured Runtime data," industrial and external systems are connected only through
  Native Plugin adapters, and Open-EQMS "stores references, selected relevant
  measurements, calculated values, event windows, and verified evidence snapshots — not
  a full duplicate of source system history." The Query Engine therefore **never**
  connects to, authenticates against, or issues a query directly to any external system.
  It only ever queries structured data that a Native Plugin adapter has already written
  into the Runtime as Objects, Events, or Transactions. This is proposed as a locked
  boundary rather than left open — see Section 17, Question 1, for the one remaining
  point needing explicit confirmation (whether any narrow exception is ever justified).
- **Documents and document revisions.** Per Baseline Section 17, generated documents
  (DOCX/XLSX/PDF/CSV/ODT/ODS) are generated views of Runtime data, carrying metadata
  (record ID, revision, status, generation time, approval state, approvers, verification
  code, template version). The Query Engine queries this metadata and the underlying
  structured Runtime records; it does not parse, render, or generate document content
  itself — that remains a Native Plugin / document-generation concern.
- **Synchronization Engine.** No dependency in either direction beyond the Query Engine
  needing to function correctly against whatever local data currently exists — it never
  calls into Synchronization Engine logic and Synchronization Engine never calls into the
  Query Engine's evaluation logic (see Section 15). Where Parallel Inquiry Fabric branches
  execute across multiple nodes (Section 33, a future extension), transport and node
  synchronization remain entirely external, governed by the Synchronization Engine and
  approved communication adapters — the Fabric's inquiry-scoped Evidence Exchange is not
  a second Synchronization Engine (Section 33's Evidence-Exchange-Is-Not-Synchronization
  subsection).
- **Security.** The Query Engine depends on Security's authorization decision for every
  candidate record (Section 13); it does not depend on Security's internal
  role/capability representation beyond that decision boundary.
- **Statistics Engine, KPI Engine, Rule Engine.** The Query Engine exposes a minimal,
  read-only "Computed Value Source" extension point (Section 10) so these engines'
  already-computed outputs are queryable through the same contract as any other source.
  The Query Engine never invokes their calculation or evaluation logic itself.
- **GUI Engine.** Consumes Query Engine result models to build View Models; the Query
  Engine has no knowledge of rendering.
- **Content Packages (Semantic Layer source).** Per Baseline Section 5 and Repository
  Rule 1, the Query Engine never imports, calls, or depends on Content-Package-defined
  code. Content Packages may register Concept Mappings and Business Inquiry Patterns
  (Sections 24, 26) as declarative data, read the same way Object Type/Event Type
  metadata is read — never as executable logic. This is the only relationship between
  the Query Engine and Content Packages.
- **runtime-contracts.** The Query Engine depends on the shared neutral types already
  established there (`ObjectId`, `PropertyValue`/`PropertyValueKind`, `Version`, the
  Unit-of-Work handle type where relevant to read-consistency, not writes) — the same
  pattern already used by Object Runtime, Event Engine, and Transaction Engine. It
  introduces no new engine-to-engine dependency by doing so.

## 7. Terminology and Query Model

- **Query** — a structured, declarative description of what to retrieve, never a raw
  database-specific string (no embedded SQL, no vendor query language) at the public
  contract layer.
- **QuerySource** — a reference to one addressable Runtime data domain a query reads
  from: an Object Type, an Event Type, a Transaction level/type, a document/revision
  metadata set, a file/attachment reference, or a registered Computed Value Source
  (Section 10). Future graph, time-series, or data-lake-backed sources register as
  additional QuerySource kinds through the same extension point (Section 6, Section 17
  Question 7) — an application may extend the data model it exposes without breaking the
  common inquiry layer, as long as it registers as a conforming QuerySource. Extensible
  by registration, never by hardcoding a new source kind into the Query Engine's core
  logic.
- **Predicate** — a composable boolean expression over fields exposed by a QuerySource,
  built only from a fixed, structural set of operators (equality, range, containment,
  existence, relationship-traversal, temporal-window). Predicates are data, not code;
  there is no mechanism for a caller to supply an executable expression.
- **Projection** — the set of fields (direct or computed) a query returns per result
  item.
- **GroupBy / Aggregation** — grouping keys plus a fixed set of deterministic aggregation
  functions (count, sum, average, min, max, distinct-count). Any aggregation beyond this
  fixed set is a Statistics Engine concern, exposed to the Query Engine only as a
  Computed Value Source (Section 10), never implemented ad hoc inside the Query Engine.
- **TemporalScope** — an explicit, first-class part of a query: a point-in-time
  ("as of"), a bounded time range, or a full-history/all-revisions traversal. Baseline
  Section 21's four data classes (current operational projection, regulated history,
  archived evidence, disposable sync state) inform which underlying store a given
  TemporalScope resolves against; the Query Engine does not invent a fifth.
- **TraversalSpec** — a bounded relationship-graph traversal specification (direction,
  depth, relation-type filter) across Object relations (SPEC-001), Event
  correlation/causation references (SPEC-002), and Transaction PriorReference chains
  (SPEC-003), used to answer cross-entity traceability questions (e.g., batch → machine
  → operator → calibration record) and to realize Baseline Section 24's requirement that
  traceability be "a real, queryable relational chain, not documentation."
- **ExecutionMode** — one-shot (against current state or a TemporalScope), continuous
  (long-lived, evaluated repeatedly against changing data), or subscription (caller is
  notified of new/changed matching results). Threshold-based and event-triggered queries
  (requirement/Section 8.6 of the fuller product requirements) are proposed as a
  refinement of subscription mode — a subscription whose Predicate includes a threshold
  or event condition — not a fourth mode, so the query model stays closed. Anomaly
  detection is explicitly **not** proposed as a Query Engine capability of its own: it
  requires a statistical model deciding what "anomalous" means, which is Statistics
  Engine territory (Section 10, Computed Value Source), surfaced to the Query Engine only
  as an already-computed flag/score to filter or subscribe on. See Section 17, Questions
  5 and 8, for the delivery-mechanism and notification-action-boundary questions this
  raises.

  **First-implementation query lifetime (correction).** The first Query Engine
  implementation supports finite, one-shot query executions only. Continuous queries,
  standing queries, subscriptions, streaming result updates, and long-lived change feeds
  are outside the first implementation's scope and require a later specification or
  revision (Section 17, Question 5 — now explicitly deferred). This is independent of
  execution strategy: a query may use Linear Execution, Parallel Independent Execution,
  or local Cooperative Parallel Inquiry Fabric execution (Section 33) while still being
  a finite, one-shot inquiry. The Parallel Inquiry Fabric decomposes one inquiry into
  bounded concurrent branches that all complete and converge to one normalized result —
  it does not introduce continuous or subscription behavior, and nothing in Section 33
  should be read as doing so.
- **ConsistencyBoundary (correction).** A named, explicit value that identifies the
  fixed data snapshot a given query execution is defined against, so "identical
  accessible data snapshot" (Section 18) is a checkable contract rather than an
  assumption. A ConsistencyBoundary is capable of identifying, at minimum: an Object
  Runtime revision or snapshot marker, an Event Engine sequence upper bound, a
  Transaction Engine committed-through identifier, a registered Computed Value Source
  version, a Concept Mapping version (Section 24), and the query definition's own
  version. Every query execution — one-shot, Linear, Parallel Independent, or Cooperative
  Parallel — is defined against exactly one ConsistencyBoundary. The Query Engine reads
  each component of a ConsistencyBoundary from its owning engine (e.g., Object Runtime
  supplies its own revision marker); it does not compute or own any of them itself. Gate
  1 must specify the concrete representation; this document fixes only the requirement
  that one must exist and be recorded against every result (Section 9).
- **Pagination** — a cursor/continuation-token plus maximum-batch-size mechanism, mirroring
  the incremental-consumption pattern already established for Event Engine range reads
  (SPEC-002) and the Object Runtime's internal bulk-read surface (SPEC-001). No
  offset-based pagination is proposed, for the same reasons those SPECs avoided it under
  concurrent mutation.

## 8. Public Contract Shape (programmatic query only, at Gate 0)

Described as behavior/contracts, not code, and limited to the structured programmatic
layer — the visual query builder and natural-language layer are explicitly upper product
layers built on top of this contract, not part of it (Section 11):

- **Execute a query**: given a QuerySource (or a bounded join/traversal across several),
  a Predicate, a Projection, optional GroupBy/Aggregation, an optional TemporalScope, an
  ExecutionMode, and pagination parameters, returns a QueryResult (Section 9) or a
  structured error (Section 16).
- **Register / retrieve a SavedQuery** (Section 12).
- **Cancel a running or subscribed query** (Section 14).
- **Register a QuerySource provider or Computed Value Source** (Section 10) — an
  administrative operation, distinct from executing a query, mirroring the Type
  registry/Event Type registry administrative operations already established in
  SPEC-001/SPEC-002.
- **Build Context Package** (Revision A, Section 32) — given a record or small record
  set, returns a bounded, permission-filtered, classified/provenance-tagged bundle of
  directly related information for AI-adapter consumption. Composes existing query
  operations; introduces no new data or interpretive behavior.

No operation accepts a raw query-language string, a caller-supplied executable
expression, or direct access to any storage-provider-specific feature. This is the
mechanism by which requirement 4 (no database-specific language exposed to users) is
satisfied at the contract level — the visual query builder, whenever built, targets this
same structural contract rather than generating SQL.

## 9. Result Model — Evidence, Provenance, and Classification

Every QueryResult carries, per Baseline Section 15 (AI output tagging) and Section 24
(traceability chain), not only the requested data but a mandatory evidence envelope:

- **ResultProvenance** (per result item, or per result set where a set-level guarantee is
  sufficient — see assumptions): source engine/QuerySource identity, source record
  identity, relevant timestamps, document/revision version where applicable, the
  Predicate/Projection/Aggregation actually applied, interpretation origin, and any
  access restriction that caused data to be filtered, suppressed, or withheld from the
  result (so a caller can distinguish "nothing exists" from "something exists but is
  restricted," wherever Security's disclosure policy permits revealing that distinction
  at all).
- **ResultClassification** — every result item is tagged with exactly one of five kinds:
  **source fact** (a direct Object/Event/Transaction/document record), **computed value**
  (an already-computed Statistics/KPI/rule-engine output, surfaced via a Computed Value
  Source), **statistical correlation** (a Statistics Engine derived relationship, not a
  recorded fact), **AI-generated hypothesis** (only ever present when an AI adapter is the
  caller producing that hypothesis and asking the Query Engine to store/surface it as
  such — the Query Engine never generates this classification itself), or **human
  -approved conclusion** (a fact or hypothesis that has passed an explicit human approval
  step, e.g. a Level 3 Transaction). This directly operationalizes requirement 12 and
  Baseline Section 15's tagging requirement for AI output, generalized to every result
  the Query Engine returns, not only AI-originated ones.
- **Correlation is never causation.** A result tagged **statistical correlation** must
  never be relabeled, displayed, or passed downstream as a **source fact** or a
  **human-approved conclusion** by any Query Engine operation. The Query Engine enforces
  this as a structural invariant on its own classification tagging; it does not rely on a
  consuming layer (GUI, AI adapter, report) to apply this distinction correctly.
- **Result Presentation Type.** Independently of ResultClassification, every QueryResult
  carries a presentation-type tag describing its semantic shape — proposed set: record
  set, linked-object view, timeline, chart-ready series, matrix, map/spatial set,
  document reference set, report, alert, model-annotation set (e.g. 3D), recommended
  -action set, or evidence package. This tag lets GUI Engine choose an appropriate
  renderer (Baseline Section 12's View Model pattern) without the Query Engine performing
  any rendering itself; it is data describing the result's shape, not a rendered view.
  The Query Engine assigns a default presentation type per QuerySource/query shape (e.g.
  a TraversalSpec query defaults to timeline or linked-object-view), and a caller may
  request a different presentation-type tag for the same underlying result, since the
  underlying data is unaffected by how it will be displayed.
- **Evidence immutability, correctly scoped (correction).** The Query Engine is
  read-only and does not own any authoritative store (Section 5); it therefore cannot
  and does not guarantee that source data is globally immutable — the owning engine
  (Object Runtime, Event Engine, Transaction Engine, or a registered source) may
  legitimately amend, correct, version, or supersede a record under its own rules. The
  narrower, actually-enforceable guarantee is: evidence references and evidence
  representations contained in a completed Query Execution Result or Context Package are
  immutable within that result package — once a result is produced, nothing in the Query
  Engine ever rewrites what that result said it saw. To make this guarantee meaningful, a
  completed QueryResult and a completed Context Package must each carry: an identity, a
  version, the ConsistencyBoundary (Section 7) it was produced against, and an integrity
  hash where the underlying source supports one (e.g., a Transaction Engine hash,
  Section 11's SPEC-003 reference). Explanations built from a result may be revised by a
  later execution; the frozen result package itself is never silently rewritten.

## 10. Computed Value Sources and Backend Calculators

To satisfy requirement 11 without absorbing calculation logic into the Query Engine, a
**Computed Value Source** is proposed as a minimal, read-only extension point: any engine
that has already computed a value (Statistics Engine averages/trends, KPI Engine
evaluations, Rule Engine outputs, evaluation matrices, classifiers, categorizers, risk
calculations, flexibility/resilience calculations, approved micro-scripts run under the
package trust-tier model of Baseline Section 11) may register itself as a QuerySource
that the Query Engine reads from exactly like an Object Type or Event Type — through a
narrow read interface, never by the Query Engine invoking the calculation itself inline.
Whether these values are always pre-computed and stored as read-only data before being
queryable, or may be computed synchronously on demand during query execution, is flagged
as Section 17, Question 4 — this Gate 0 proposal recommends the former (pre-computed,
Query Engine only reads) to keep Query Engine execution cost and failure modes fully
decoupled from each calculator's own performance and correctness.

## 11. Relationship to the Visual Query Builder and Natural-Language Layer

Per requirements 4 and 5, both the future visual query builder and natural-language
inquiry are explicitly upper product layers, not part of the Query Engine's core
contract:

- The **visual query builder** translates a user's visual construction into the
  structured Query contract (Section 8); it never generates or exposes a
  database-specific query language to the end user, and it is not designed or scoped by
  this document.
- **Natural-language inquiry** is realized by an AI adapter (Baseline Section 15) that
  translates a natural-language request into the same structured Query contract, then
  reads the result — including its ResultClassification and ResultProvenance — back
  through the Query Engine. The Query Engine's core must remain fully deterministic and
  independently testable and usable with zero AI adapter present, per requirement 5.

Neither layer is designed in this Gate 0 proposal; each is expected to become its own
future SPEC once SPEC-004's deterministic core is stable.

## 12. Saved and Reusable Queries

A **SavedQuery** is a named, versioned, stored query definition (the structural Query
model of Section 7, serialized), reusable by dashboards, reports, audit checks,
validation checks, management reviews, alerts, and application widgets (requirement 15).
Proposed treatment: a SavedQuery is represented as an ordinary Object Runtime object
(reusing SPEC-001's existing versioning, identity, and permission-scope mechanics rather
than the Query Engine building a second, parallel storage/versioning mechanism). This is
flagged as Section 17, Question 2, since it introduces a Query Engine dependency on
Object Runtime for exactly one purpose (storing SavedQuery definitions) and needs
explicit confirmation that this is acceptable rather than assumed.

Every SavedQuery must carry, as mandatory metadata, not optional convenience fields: an
owner, a version, its own access-permission scope (distinct from, and enforced in
addition to, the per-record permission checks in Section 13 — a caller may be authorized
to read the underlying data yet not be authorized to run or modify a given saved
inquiry), a validation status (e.g. draft, reviewed, approved-for-regulated-use), and a
full change history. This is required because SavedQueries are explicitly organizational
knowledge assets reused for audit checks, validation checks, risk monitors, and
management reviews — the same governance rigor already applied to Object Type and Event
Type metadata (SPEC-001 Section 5.5, SPEC-002 "Event Type Definitions") applies here.

A SavedQuery's version is distinct from the underlying data model's evolution
(requirement 6): replaying an older SavedQuery version against a since-evolved Object
Type or Event Type schema must either produce a well-defined result under that
SavedQuery's original field set, or fail with an explicit schema-mismatch error — it must
never silently reinterpret itself against the new schema (mirroring SPEC-002's Event Type
schema-version immutability principle).

## 13. Permission Model

- The Query Engine performs no authorization policy-authoring or identity/role
  evaluation itself; Security (Baseline Section 13) is the sole authority on whether a
  given caller may see a given piece of data.
- **Per-record enforcement before aggregation.** Every candidate record — whether it
  will be returned directly, contribute to a Projection, or contribute to a
  GroupBy/Aggregation bucket — must be individually checked against Security's decision
  before it is permitted to influence any part of a result. A caller must never receive
  a count, sum, average, or any other aggregate value that was computed over records the
  caller could not read individually. This directly satisfies requirement 13's
  prevention of indirect disclosure through aggregates or summaries.
- **Relationship-traversal disclosure.** A TraversalSpec (Section 7) must apply the same
  per-record check at every hop; a traversal must not reveal the existence, count, or any
  attribute of a node the caller is not authorized to read, even indirectly (e.g., via a
  non-empty "related records" count).
- **AI-summary disclosure.** Where an AI adapter reads Query Engine results to produce a
  summary, explanation, or natural-language answer, the same per-record permission
  boundary applies to whatever the AI adapter is given to read — the Query Engine must
  never hand an AI adapter unfiltered access "for context" on the assumption that the
  summary alone will be filtered afterward. A summary generated from data the caller
  could not individually read is exactly the indirect-disclosure case this section
  exists to prevent, whether the intermediate step is an aggregate, a linked record, or
  an AI-generated paraphrase.
- **Small-group aggregate suppression.** Per-record filtering resolves the direct
  disclosure case, but does not by itself resolve inference risk from very small
  aggregate groups (e.g., a "count = 1" bucket effectively revealing a single otherwise
  -hidden record's attribute). This narrower policy question is flagged as unresolved —
  Section 17, Question 3 — since it is a Security-Architecture-level policy decision, not
  a Query Engine design choice, and the Baseline does not currently specify a threshold
  or suppression rule.

## 14. Execution Limits, Cancellation, and Resource Protection

- Every query execution is bounded by explicit, configurable limits: maximum result
  size, maximum execution time, and maximum memory/working-set consumption, consistent
  with Baseline Section 23's weak-hardware, industrial, offline deployment target.
- A caller may cancel a running one-shot query or an active continuous/subscription
  query at any time. Cancellation is cooperative and cannot leave any partial write or
  side effect, because the Query Engine performs no writes in the first place — it can
  only stop producing further results.
- Cancelling or exceeding limits on one query must never affect any other concurrently
  running query or any Runtime state.

## 15. Offline and Synchronization Considerations

The Query Engine must remain fully functional against whatever data a given node
currently holds, whether that node is online, offline, or partially synchronized — it
never blocks on, waits for, or triggers a synchronization cycle. It exposes no
synchronization-relevant operation of its own; if a future capability needs to know
"how stale is this node's data," that is answered by the Synchronization Engine's own
data (e.g., last-sync markers), read by the Query Engine as ordinary QuerySource data,
not computed or owned by the Query Engine (requirement 16).

## 16. Error Model

The Query Engine returns structured, typed errors — never panics across its public
contract boundary — covering at minimum:

- QuerySource not found / not registered
- Unknown or invalid field reference in a Predicate, Projection, or GroupBy clause
- Permission denied (returned in a form that does not itself leak the existence of the
  denied data — e.g., indistinguishable from "not found" where Security requires that)
- Malformed or expired continuation/cursor token
- Query execution timeout or resource-limit exceeded
- Query cancelled (distinct from timeout or error)
- Computed Value Source or QuerySource provider unavailable
- SavedQuery schema-version mismatch on replay
- Malformed query definition (structurally invalid Predicate/Projection/Aggregation
  combination)

Each error kind is distinct and machine-distinguishable, following the taxonomy
established in SPEC-001 Section 9 and SPEC-002's Error Model.

## 17. Architectural Questions and Resolution Status

Every question raised during this Gate 0 process now carries an explicit resolution
status — **Resolved decision**, **Recommended Gate 1 decision**, **Explicitly deferred
capability**, or **Blocking unresolved decision** — so that none remains ambiguous while
Gate 1 planning and repository integration proceed. "Recommended Gate 1 decision" means
the stated resolution is this document's working assumption, to be confirmed (not
re-litigated from scratch) when Gate 1 technical design happens; it is not the same as
leaving the question open.

1. **External-system query boundary.** **Resolved decision.** The Query Engine never
   queries ERP/MES/SCADA/PLC/CAD/Office/external-database/API sources directly, only
   Runtime data already ingested by a Native Plugin adapter. This is locked by Baseline
   Sections 8, 14, and 16 read together, not merely a Query Engine preference; no
   exception is carried forward.
2. **SavedQuery storage ownership.** **Recommended Gate 1 decision.** SavedQuery
   definitions are stored as Object Runtime objects (Section 12), reusing its existing
   versioning, identity, and permission-scope mechanics rather than a second, parallel
   store. Confirm at Gate 1 rather than re-opening from scratch.
3. **Small-group aggregate suppression policy.** **Explicitly deferred capability.**
   Per-record permission filtering (Section 13) is enforced from the first
   implementation; minimum-group-size / k-anonymity-style suppression against inference
   from very small aggregate groups is a Security-Architecture policy decision the
   Baseline does not yet specify, and is deferred to a future Security ADR rather than
   invented here.
4. **Computed Value Source execution timing.** **Recommended Gate 1 decision.** Backend
   calculators (Statistics/KPI/Rule Engine outputs, evaluation matrices, classifiers,
   risk/resilience calculations, micro-scripts) are always pre-computed and stored,
   queried like any other read-only source — never invoked synchronously inline during
   query execution. Confirm at Gate 1; this affects those engines' own SPECs too.
5. **Continuous/subscription delivery mechanism.** **Explicitly deferred capability.**
   Resolved by the first-implementation scope correction (Section 7): the first Query
   Engine implementation is one-shot only. Continuous and subscription ExecutionMode,
   and any push/notification delivery mechanism, are deferred to a later specification
   or revision in full, not merely the transport detail.
6. **Visual query builder and NL layer sequencing.** **Explicitly deferred capability.**
   Both become separate future SPECs built on top of SPEC-004's deterministic core; they
   are not part of SPEC-004 at any Gate.
7. **Future adapter shape (graph, document, time-series, data-lake, external-source).**
   **Recommended Gate 1 decision** for the extension-point contract shape only (Sections
   6, 10 fix a minimal, registration-based QuerySource-provider interface now).
   **Explicitly deferred capability** for any specific future adapter — none is designed
   at Gate 0 or Gate 1.
8. **What-if / hypothetical simulation queries.** **Explicitly deferred capability.**
   Out of the Query Engine's scope entirely. A hypothetical-state simulation (e.g.,
   "what if this supplier, machine, and two employees became unavailable") is a
   Statistics/KPI/Rule-Engine "what-if" capability; the Query Engine, at most, supplies
   the current real data such an engine would simulate from, never a hypothetical
   result of its own.
9. **Notification/action boundary for event-triggered queries.** **Explicitly deferred
   capability.** The condition itself is an ordinary subscription Predicate (Section 7),
   but that mode is itself deferred per Question 5; the "notify" action was, and
   remains, out of the Query Engine's scope regardless — it belongs to the Rule Engine
   or a future dedicated notification capability. The Query Engine only ever
   returns/surfaces matching results; it never sends a notification, assigns a task, or
   takes any other action.
10. **Concept Mapping / Business Inquiry Pattern lifecycle (Revision A, Sections 24,
    26).** **Recommended Gate 1 decision.** Default proposal: this lifecycle mirrors
    Object Type / Event Type schema governance (registration, versioning, no in-place
    edits to a published version) rather than inventing a separate review tier, subject
    to Gate 1 confirmation given these patterns encode organizational, not merely
    structural, meaning.
11. **AI Context Package bundle composition (Revision A, Section 32).** **Recommended
    Gate 1 decision.** Default proposal: the record(s), their ResultProvenance, and
    permission-scope/ConsistencyBoundary metadata are mandatory in every Context
    Package; timeline, related objects, standards/policy references, known risks, and
    prior decisions are populated only where a registered source exists and the caller
    is authorized for it — optional-per-call, not mandatory-per-call. Confirm at Gate 1.
12. **Parallel Inquiry Fabric placement and release sequencing (Revision B, Section
    33).** **Resolved decision — retained.** Gate 1 defines the common branch,
    evidence-exchange, and deterministic-fusion contracts. The first implementation is
    local and in-process only. Multi-process and multi-node execution are deferred.
    Synchronization, node discovery, transport, and remote trust remain outside
    SPEC-004.

Summary: zero Blocking-unresolved-decision questions; five Recommended-Gate-1 decisions
(2, 4, 7's contract shape, 10, 11); five Explicitly-deferred capabilities (3, 5, 6, 8, 9,
plus 7's specific future adapters); two Resolved decisions (1, 12). No question is left
ambiguous, and no question's ambiguity is paired with an authorization to implement the
affected capability — Gate 5 implementation remains unauthorized in full (Gate 0
Decision, below).

## 18. Determinism Requirements

- Given an identical Runtime state and an identical query definition, the Query Engine
  always produces identical results, identical ResultProvenance metadata, and identical
  ResultClassification tags — no hidden nondeterminism from concurrency, caching, or
  iteration order.
- Pagination/continuation tokens are deterministic and reproducible against an unchanged
  Runtime state; behavior when the underlying data changes between pages must be
  explicit (e.g., a defined "snapshot as of first page" semantic or an explicit
  "may reflect intervening changes" semantic) rather than left as an accident of
  implementation — this choice is deferred to Gate 1, not decided here.
- Validation and permission-denial errors are deterministic: the same invalid or
  unauthorized query against the same state always produces the same error kind.
- **Parallelism invariant (Revision B).** Parallelism must not alter query semantics.
  Execution order, scheduling order, and evidence-arrival order may affect performance
  but must never affect the normalized result produced against the same
  ConsistencyBoundary (Section 7 — correction: this is what "the same accessible data
  snapshot" means, made checkable rather than assumed). This applies to every Parallel
  Inquiry Fabric execution mode (Section 33) without exception — see Section 33's
  Deterministic Convergence subsection.
- **ConsistencyBoundary requirement (correction).** The specification does not claim
  deterministic reproducibility without defining how stable source boundaries are
  represented: every query execution is defined against exactly one ConsistencyBoundary
  (Section 7), and every completed QueryResult and Context Package records the
  ConsistencyBoundary it was produced against (Section 9). "Identical Runtime state" and
  "identical accessible data snapshot" throughout this document mean, precisely,
  "identical ConsistencyBoundary."

## 19. Performance Requirements

- The Query Engine must contribute a bounded, explicit share of the overall 20–40MB
  Runtime footprint target (Baseline Section 23); any index or projection it builds over
  Object Runtime/Event Engine/Transaction Engine data must be sized proportionally to
  actual usage, not embed a general-purpose database engine.
- Reads from each upstream engine's bulk/streaming surface must be incremental, matching
  the non-materialize-everything requirement already established in SPEC-001 and
  SPEC-002.
- The Query Engine is designed to run on the same weak-hardware, low-bandwidth, offline
  -capable targets as every other Runtime component (Baseline Section 23).

## 20. Testing Requirements (proposed)

- A conformance test suite proving every QuerySource type enforces per-record permission
  checks before any data (direct, projected, or aggregated) becomes visible.
- Tests proving no error path or successful path ever performs a write to Object
  Runtime, Event Engine, Transaction Engine, or any other Runtime store.
- Determinism tests: identical query + identical state → byte-identical result and
  provenance metadata, across repeated runs.
- Negative tests for every error kind in Section 16.
- A boundary/dependency test proving zero compile-time dependency from the Query Engine
  onto Content-Package-defined code, Synchronization Engine logic, AI adapter code, or
  GUI rendering code.
- SavedQuery replay tests across a schema evolution, proving either a well-defined
  result or an explicit schema-mismatch error, never silent reinterpretation.
- Cancellation tests proving a cancelled continuous/subscription query stops delivering
  results without affecting any other in-flight query.
- Aggregate-disclosure tests proving a caller cannot infer a hidden record's existence
  or value through a count, sum, or other aggregate computed partly over unauthorized
  records.
- Tests proving a statistical-correlation-classified result can never surface as a
  source-fact or human-approved-conclusion classification through any Query Engine
  operation.
- Tests proving SavedQuery registration is rejected when owner, version, access
  -permission scope, validation status, or change history is missing.
- Tests proving an AI-adapter-initiated read is subject to the same per-record
  permission check as any other caller, with no broader "context" access granted.
- Tests proving a Build Context Package call respects its configured bundle size/depth
  limit and returns no item the caller could not obtain via an ordinary query.
- **(Revision B, Parallel Inquiry Fabric)** Execute the same cooperative inquiry
  repeatedly with randomized branch scheduling and evidence-arrival order. The
  normalized result, evidence set, classification set, and Context Package must remain
  identical.
- Verify that evidence exchanged between branches always retains provenance,
  classification, permission scope, and producing-branch identity.
- Verify that a branch cannot infer or expose evidence inaccessible to its permission
  context through another branch.
- Verify that duplicate intermediate evidence is deterministically deduplicated without
  losing provenance.
- Verify that contradictory evidence is preserved and explicitly reported rather than
  silently resolved.
- Verify enforcement of maximum branch count, branch depth, execution time, and resource
  limits.
- Verify that failed, unavailable, cancelled, and timed-out branches produce an
  explicitly incomplete result rather than a falsely complete result.
- Verify that cooperative evidence exchange performs no authoritative Runtime mutation
  and does not implement Synchronization Engine responsibilities.

## 21. Proposed Acceptance Criteria

These are proposed, not final — final acceptance criteria are set at Gate 1 once the
Section 17 questions are resolved.

1. No public Query Engine operation accepts a raw database-specific query string.
2. Every result item can be traced to a ResultProvenance entry identifying its source
   engine, source record identity, and the query definition applied to produce it.
3. A query joining or traversing across Object, Event, and Transaction data performs no
   write to any of those engines.
4. No aggregate or grouped result value is ever influenced by a record the calling
   caller could not individually read.
5. Two identical queries run against identical Runtime state produce byte-identical
   results and provenance metadata.
6. A versioned SavedQuery, replayed after the underlying schema evolves, either produces
   a well-defined result or fails with an explicit schema-mismatch error.
7. No Query Engine operation writes, mutates, or deletes any Object, Event, Transaction,
   or document record.
8. Cancelling a continuous or subscription query stops further result delivery without
   affecting any other query or Runtime state.
9. Every result item carries exactly one ResultClassification tag (source fact, computed
   value, statistical correlation, AI-generated hypothesis, human-approved conclusion).
10. No compile-time or runtime dependency exists from the Query Engine onto any
    Content-Package-defined code, Synchronization Engine logic, AI adapter code, or GUI
    rendering code.
11. No Query Engine operation ever returns a result tagged as **statistical correlation**
    re-tagged or displayed as **source fact** or **human-approved conclusion**.
12. Every QueryResult carries exactly one Result Presentation Type tag, and changing the
    requested presentation type for the same query never changes the underlying data or
    its ResultClassification/ResultProvenance.
13. Every SavedQuery carries a non-empty owner, version, access-permission scope,
    validation status, and change history; a SavedQuery missing any of these fields is
    rejected at registration time.
14. An AI adapter reading Query Engine data to produce a summary receives only records
    the requesting caller is individually authorized to read — never broader "context"
    later filtered after the fact.
15. A Build Context Package call never returns an item the requesting caller could not
    obtain through an ordinary query, and never exceeds its configured bundle size/depth
    limit regardless of how richly connected the requested record is.
16. **(Revision B)** A complex inquiry can be decomposed into bounded structured branches
    without introducing raw executable expressions or database-specific query languages.
17. Verified evidence discovered by one branch can refine another branch while retaining
    complete provenance and permission scope.
18. Identical inquiries against an identical accessible data snapshot produce identical
    normalized results regardless of branch execution or message-arrival order.
19. The final Context Package identifies all completed, failed, unavailable, and
    dynamically created branches.
20. **(Amended by correction)** The Parallel Inquiry Fabric can be disabled. Where the
    inquiry is semantically executable through Linear or Parallel Independent execution,
    disabling the Fabric causes it to use that deterministic mode instead. Where the
    inquiry requires cooperative semantics those modes cannot reproduce, execution fails
    explicitly with an "execution strategy unavailable" error rather than silently
    changing the inquiry's meaning.
21. No cooperative inquiry operation assumes Synchronization Engine ownership or
    mutates authoritative Runtime data.

## 22. Assumptions

1. ResultProvenance is required per result item by default; a coarser per-result-set
   provenance envelope may be acceptable for very large result sets, but this
   optimization is not assumed settled — flagged for Gate 1.
2. QuerySource, Predicate, Projection, and Aggregation reuse the `PropertyValue`/
   `PropertyValueKind` primitive kind set already shared by SPEC-001/SPEC-002, rather
   than defining a new type system, for consistency and to avoid a fourth parallel type
   enumeration in the workspace.
3. Continuation/cursor tokens follow the same opaque-token pattern already assumed for
   Event Engine range reads (SPEC-002 Assumption 6).
4. TraversalSpec depth/direction bounds are always explicit and finite in any single
   query; unbounded graph traversal is not a supported query shape, consistent with the
   execution-limit requirements in Section 14.

## 23. Blocking Questions

None. Section 17's twelve questions each now carry an explicit resolution status
(Resolved decision, Recommended Gate 1 decision, or Explicitly deferred capability);
zero are Blocking unresolved decisions. None are implementation blockers within the
Query Engine boundary itself, and none authorize implementation of the capability they
concern.

---

# Revision A — Organizational Inquiry Layer

Sections 24–32 respond to the Project Manager's Conditional Approval decision. Each
section states the business inquiry it must make expressible, the existing Section 1–23
primitive(s) it composes, what (if anything) is new to the public contract, and what
remains explicitly out of the Query Engine's scope. None of these sections authorize
implementation; they extend this Gate 0 proposal's requirements only.

## 24. Semantic Inquiry Layer

The Semantic Inquiry Layer bridges business questions ("why do we have more
complaints?") to the structural Query model (Section 7) without embedding business
meaning into the Query Engine itself.

- **Concept Mapping** — a named, versioned mapping of a business concept (e.g.
  "Complaint," "Supplier," "Calibration") to one or more QuerySource/Predicate/
  TraversalSpec combinations. Supplied externally by a Content Package as declarative
  configuration, never as code — consumed by the Query Engine exactly as Object Type and
  Event Type metadata already are.
- A concept chain such as Complaint → Supplier → Material → Machine → Operator →
  Calibration → Maintenance → Customer Requirement → Risk → CAPA is not new Query Engine
  logic. It is a named TraversalSpec pattern (Section 7) whose hop sequence and
  relation-type filters are supplied by a Concept Mapping. Traversal execution itself is
  unchanged; only the pattern's business name and hop definition are new, and both are
  external configuration.
- The Semantic Layer is additive on top of Section 8's public contract: a caller may
  address a query by concept name (resolved via Concept Mapping) or by the underlying
  structural Query directly. Both paths produce the same QueryResult, ResultProvenance,
  and ResultClassification — the Semantic Layer changes only how a query is authored,
  never how it is executed, permission-checked, or classified.
- Open item: the concrete format and registration/review lifecycle of Concept Mappings
  (versioning, ownership, approval) is not designed at Gate 0 — see Section 17,
  Question 10.

## 25. Organizational Knowledge Graph

- The "graph" is not a new authoritative store. It is the Query Engine's unified
  traversal view over relationship-bearing data already owned elsewhere: Object
  relations (SPEC-001), Event correlation/causation references (SPEC-002), Transaction
  PriorReference chains (SPEC-003), and Semantic Layer concept chains (Section 24). No
  new engine, crate, or persistent structure is introduced; TraversalSpec (Section 7) is
  the mechanism, unchanged.
- People, competencies, processes, machines, materials, standards, documents,
  validations, contracts, customers, suppliers, KPIs, and risks are each ordinary Object
  Types (or Computed Value Sources, for KPIs) supplied by Content Packages. The "graph"
  is simply that these Object Types are richly interrelated; the Query Engine traverses
  across them without knowing what any of them mean, exactly as it already does for any
  other Object relation.
- Because it is a view, not a store, the graph is always exactly as current, as
  permission-filtered, and as auditable as the underlying Object/Event/Transaction data —
  there is no separate synchronization, consistency, or staleness concern for "the
  graph" beyond what Sections 13 (Permission Model) and 18 (Determinism) already
  guarantee for any traversal query.
- Open item: whether a dedicated graph-shaped QuerySource (Section 17, Question 7's
  future graph adapter) is needed for performance at organizational scale, or whether
  traversal over existing per-engine read surfaces suffices for MVP, is not decided at
  Gate 0.

## 26. Business Inquiry Patterns ("Why," "What Changed," "What Is Affected")

Proposed as a fixed taxonomy of pattern categories, each composed entirely from
primitives already defined in Sections 1–23 — not new query primitives:

- **"What changed"** = a query with two TemporalScope values (Section 7) and a
  diff-projection over the same QuerySource.
- **"What is affected"** = a bounded TraversalSpec (Section 7) outward from a given
  record, permission-filtered exactly as any other traversal (Section 13).
- **"How"** = a TraversalSpec plus TemporalScope reconstructing the sequence of
  Events/Transactions leading to a state — ResultProvenance (Section 9) already carries
  this trail.
- **"Why" / "what caused"** = the causal/correlation inquiry pattern already defined in
  Section 9: candidate causes are surfaced strictly as **statistical correlation** or
  **computed value** results, never as **source fact**, per the correlation-never
  -causation invariant already stated there. A "why" query never returns an assertion;
  it returns classified candidates plus their provenance, for a human — or an AI
  adapter, itself bound by the same rule — to interpret.

Business Inquiry Patterns are registered the same way Concept Mappings are (Section 24):
as named, externally supplied compositions of QuerySource/Predicate/TraversalSpec/
TemporalScope/Aggregation. This chapter adds no new capability to the Query Engine's
core — only a registry of named, reusable compositions of capabilities it already has.

## 27. Organizational Intelligence Queries: Flexibility & Resilience

- Examples ("which processes depend on only one qualified employee," "which products
  have only one supplier," "which machines have no backup," "where is knowledge
  concentrated") are each a relation-cardinality Predicate (Section 7) over an
  Organizational Knowledge Graph traversal (Section 25): count the relations of a given
  type from a node and compare against a threshold.
- This requires no new Query Engine capability beyond Predicate/Aggregation (Section 7)
  and graph traversal (Section 25). The specific thresholds and relation types that
  define "single point of failure" for a given domain are supplied by a Content Package
  as a registered Business Inquiry Pattern (Section 26), not hardcoded.
- Composite resilience *scores* (a single index combining multiple single-point-of
  -failure signals) are Computed Value Source outputs (Section 10) — a Statistics/KPI/
  dedicated-calculator responsibility, surfaced to the Query Engine like any other
  computed value, never calculated by the Query Engine itself.
- The hypothetical/what-if variant of this inquiry (removing a supplier, a machine, and
  two employees at once and asking what breaks) remains the open question already
  flagged in Section 17, Question 8. This chapter covers only inquiry over the actual
  current organizational structure, not simulation over a hypothetical one.

## 28. Learning & Competency Queries

- Examples ("who has not completed training," "which work instructions are outdated,"
  "which procedures have no visual guidance," "which machine has no interactive
  training") are ordinary Predicate/Aggregation queries over Object Types (Competency,
  Training Record, Work Instruction, Document/Revision) and Event Types (training
  -completion events) that a Content Package defines — no different in kind from any
  other domain query.
- Media type (image, video, 3D, interactive) is proposed as a structural attribute of a
  document/attachment reference (an opaque `PropertyValue` kind already assumed in
  SPEC-001/002), so "which procedures have no visual guidance" is a predicate over that
  attribute, not a new Query Engine media-handling capability. The Query Engine never
  processes, renders, or interprets media content itself.

## 29. Policy & Compliance Queries

- Examples ("which policies are violated," "which departments repeatedly deviate,"
  "which contracts are not fully covered," "which customer requirements are uncovered")
  compose directly onto Baseline Section 24's traceability chain (Requirement → System
  rule → Process node → Data object → User action → Generated evidence → KPI → Audit
  result), already stated there to be "a real, queryable relational chain." The Query
  Engine's TraversalSpec over Requirement Record objects (Section 25) is the mechanism;
  Baseline Section 24 already anticipated this capability, it was simply not yet named
  as a Query Engine chapter.
- "Violation" and "deviation" determinations are Rule Engine outputs (WHEN/IF/THEN
  evaluation) or KPI Engine evaluations, surfaced to the Query Engine as Computed Value
  Sources or as Transaction/Event records of a rule firing — the Query Engine itself
  never evaluates whether a policy was violated.

## 30. Organizational Maturity Queries

- Examples ("what is the maturity of this process," "which department has the lowest
  resilience," "which site has the biggest documentation gap") are Computed Value Source
  outputs (Section 10) — a maturity model is a calculator (an "evaluation matrix" or
  "classifier" in Section 10's language) computing a score from underlying Query Engine
  data. The Query Engine's role is limited to (a) supplying that calculator's input data
  via ordinary queries, and (b) surfacing its output as another queryable, classified
  (**computed value**), provenance-tagged result — never defining or computing the
  maturity model itself.
- This keeps organizational-maturity modeling entirely swappable and owned by whichever
  engine/Content Package defines a given maturity framework, without the Query Engine
  taking a position on what "maturity" means for any given domain.

## 31. Live Organizational Knowledge Stream

This chapter clarifies, rather than changes, Section 7's continuous/subscription
ExecutionMode: a live Event/Transaction stream, once resolved through a Concept Mapping
(Section 24) and tagged with ResultClassification/ResultProvenance (Section 9), is what
constitutes an organizational "knowledge" stream rather than a raw data stream — the
distinction is in what the Semantic Layer and classification/provenance model attach to
each streamed item, not in a new streaming mechanism. No new public operation is
introduced by this chapter.

## 32. AI Context Package

- A new, narrowly scoped public operation is proposed: **Build Context Package** —
  given a record or a small set of records, returns a bounded bundle combining: the
  record(s) themselves, their ResultProvenance, a bounded TraversalSpec of directly
  related objects, applicable policy/standard Requirement Records (Section 29), relevant
  document/procedure references, known risk records, and prior human-approved
  conclusions (Section 9) referencing the same record(s) — each item individually
  permission-filtered and classification-tagged exactly as in any other query
  (Sections 9, 13).
- This operation composes existing primitives into one convenience call for AI-adapter
  callers; it introduces no new data, no new classification kind, and no interpretive
  behavior. The Query Engine assembles and returns the bundle; it never summarizes,
  explains, or interprets it — that remains the AI adapter's job (Section 5's AI
  Non-Goal is unchanged).
- Bundle size/depth is bounded by the same execution limits as any other query
  (Section 14), so a Context Package request cannot become an unbounded
  whole-organization dump.
- Open item: the exact bundle composition (which categories are mandatory vs. optional
  per call) is not finalized at Gate 0 — see Section 17, Question 11.

---

# Revision B — Parallel Inquiry Fabric

Section 33 responds to the Project Manager's instruction to add cooperative parallel
inquiry execution as a mandatory section. It is an optional execution strategy layered
onto the Query Engine's existing execution layer — it introduces no new engine, no new
authoritative store, and no weakening of any Section 1–32 requirement. Section 34
formalizes the Query Execution Pipeline to show where it plugs in.

## 33. Parallel Inquiry Fabric

The Query Engine may execute complex inquiries through a **Parallel Inquiry Fabric**.
The fabric enables a structured inquiry to be decomposed into multiple bounded and
independently executable inquiry branches. Typical branches may investigate different
organizational dimensions, including: material and supplier changes; machines and
tooling; maintenance and calibration; operators and competencies; process and document
revisions; environmental conditions; complaints and nonconformities; risks, policies,
and customer requirements; statistics, KPIs, and approved computed values.

The purpose of parallel execution is not merely performance optimization. It enables
Open-EQMS to investigate several candidate explanations, traceability paths, or impact
paths at the same time while preserving evidence, permissions, reproducibility, and
deterministic result semantics.

### Inquiry Decomposition

A complex inquiry may be decomposed into: independent source queries; relationship
-traversal branches; temporal comparison branches; impact-analysis branches; candidate
-explanation branches; evidence-validation branches; computed-value retrieval branches.

Every branch must be represented as a structured Query Engine operation (an ordinary
Query, Section 7) — a branch must not contain hidden executable logic, raw
database-specific query text, AI-only reasoning, or unbounded graph traversal. Every
branch must have explicit: identity; parent inquiry identity; purpose; query definition;
source scope; temporal scope; traversal limits; execution limits; permission context;
evidence requirements; completion state.

### Cooperative Evidence Exchange

Inquiry branches may publish verified intermediate evidence into an inquiry-scoped
**Evidence Exchange**. Other branches may consume this evidence when it is relevant to
their own bounded investigation.

Example: a material-analysis branch identifies that a material batch changed at a
specific time. A complaint-analysis branch may then use that verified timestamp and
batch identity to refine its own structured query.

Intermediate evidence must include: source identity; source record identity; timestamp;
classification (ResultClassification, Section 9); provenance (ResultProvenance,
Section 9); producing branch identity; the query definition that produced it;
permission scope; an integrity identifier.

Intermediate evidence must never be exchanged as an untraceable free-text statement.
AI-generated hypotheses may be present only when explicitly classified as
**AI-generated hypothesis** (Section 9) — they must not be treated as source facts.

### Dynamic Sub-Inquiries

A branch may request creation of an additional bounded sub-inquiry when newly
discovered evidence justifies further investigation. Dynamic sub-inquiries must remain
subject to: maximum branch count; maximum branch depth; maximum total execution time;
maximum resource consumption; explicit source and traversal limits (Section 7's
TraversalSpec bounds); cancellation (Section 14); permission checks (Section 13);
duplicate-work detection.

The fabric must prevent uncontrolled recursive inquiry expansion.

### Evidence Exchange Is Not Synchronization

The inquiry-scoped Evidence Exchange must not become a second Synchronization Engine.
It exchanges temporary, inquiry-scoped verified evidence between active inquiry
branches. It does not: replicate authoritative Runtime stores; resolve long-term
synchronization conflicts; calculate synchronization deltas; manage offline queues;
transfer ownership of records; replace the Synchronization Engine.

Where branches execute on different Open-EQMS nodes, transport and node synchronization
remain external concerns governed by the Synchronization Engine and approved
communication adapters (Section 6). The Query Engine operates only on data currently
accessible under the execution contract.

### Evidence Fusion

After branches complete, their outputs are processed through deterministic **Evidence
Fusion**. Evidence Fusion must: validate provenance; remove exact duplicates; preserve
conflicting evidence; identify corroborating evidence; maintain source classifications;
preserve temporal ordering; preserve permission restrictions; distinguish facts from
computed values and hypotheses (Section 9's ResultClassification); record incomplete or
unavailable branches; produce the evidence set used by the Context Package Builder
(Section 32).

Conflicting evidence must not be silently overwritten or averaged into a false single
conclusion. The result must explicitly preserve unresolved conflicts.

### Candidate Explanations

The Parallel Inquiry Fabric may investigate several candidate explanations
simultaneously. The core Query Engine must not declare causation merely because records
are correlated (the correlation-never-causation invariant, Section 9, applies here
without exception).

Candidate explanations must remain classified as one of: unsupported candidate; weakly
supported candidate; statistically correlated candidate; evidence-supported candidate;
human-approved conclusion.

**Relationship to ResultClassification (Section 9).** This candidate-strength taxonomy
is additive to, not a replacement for, ResultClassification: every candidate explanation
still carries exactly one of the five ResultClassification tags (source fact, computed
value, statistical correlation, AI-generated hypothesis, human-approved conclusion) on
each piece of evidence it cites, and separately carries one of the five candidate
-strength tags above describing how well-supported the *explanation as a whole* is. The
two taxonomies answer different questions — what kind of thing is this evidence, versus
how strong is this candidate explanation — and neither may be inferred from the other.

The exact scoring or evaluation of candidate explanations belongs to approved
Statistics, Rule, Risk, Classification, or other analytical engines (Computed Value
Sources, Section 10), or to an explicitly defined deterministic policy registered the
same way (Section 24's Concept Mapping / Section 26's Business Inquiry Pattern
registration pattern) — never to logic the Query Engine invents itself. The Query Engine
only retrieves, exchanges, and assembles their outputs with provenance; it never assigns
a candidate-strength value on its own initiative. **A `human-approved conclusion` may
only ever be created from a traceable human approval action** (e.g., a Level 3
Transaction, SPEC-003) — the Query Engine must never construct this classification
automatically, regardless of how strong the underlying evidence appears.

### Deterministic Convergence

Parallel execution must not change the semantic result. Given: an identical query
definition; an identical accessible data snapshot; identical registered QuerySources;
identical permissions; identical analytical outputs; identical execution limits — the
Parallel Inquiry Fabric must produce the same normalized result, evidence set,
classification set, and Context Package regardless of: branch execution order; thread
scheduling; node response order; message arrival order; temporary branch completion
order.

Concurrency may affect execution time, but it must not affect the normalized result.
This is stated as a formal invariant in Section 18.

### Partial Results

When one or more branches cannot complete, the system may return a partial result only
when the query contract permits it. A partial result must identify: completed branches;
failed branches; unavailable branches; cancelled branches; timed-out branches; missing
evidence domains; resulting confidence or completeness limitations.

A partial result must never be represented as a complete investigation.

### Execution Modes

The Query Engine must support at least:

- **Linear Execution** for simple and low-cost queries.
- **Parallel Independent Execution** for queries whose branches do not exchange
  evidence.
- **Cooperative Parallel Execution** for branches that exchange verified intermediate
  evidence.
- **Distributed Cooperative Execution** as a future extension across approved
  Open-EQMS nodes.

The planner may select an execution mode according to the query definition, available
resources, and configured policy. Callers must be able to explicitly prohibit
distributed or cooperative execution where required by regulatory, performance,
security, or deployment constraints.

**Execution fallback (correction).** Disabling the Parallel Inquiry Fabric does not
require a silent fallback for every inquiry — some cooperative inquiries dynamically
create sub-inquiries from intermediate evidence (Section 33's Dynamic Sub-Inquiries) in
a way a Linear or Parallel Independent executor cannot reproduce without changing the
inquiry's meaning. The rule: where an inquiry is semantically executable through Linear
or Parallel Independent execution, the planner may use that mode. Where the inquiry
requires cooperative semantics that mode doesn't support, execution must fail explicitly
— e.g., an "execution strategy unavailable" error (Section 16) — rather than silently
falling back and changing what the query means. This corrects Acceptance Criterion 20
(Section 21), which is amended accordingly.

### Permission Isolation

Every branch executes under the initiating caller's permission context or a strictly
narrower delegated context. An inquiry branch must not gain access to data solely
because another branch can access it. Evidence transferred between branches must retain
its permission scope.

Evidence Fusion and Context Package generation must prevent unauthorized evidence from
influencing: visible records; aggregates; classifications; counts; candidate
explanations; summaries; branch-selection decisions visible to the caller. This is the
Parallel Inquiry Fabric's specialization of Section 13's permission model — no new
permission mechanism is introduced.

### Auditability

The execution record of a Parallel Inquiry Fabric inquiry must identify: initial query;
decomposition plan; branch identities; branch query definitions; evidence exchanged;
dynamically created sub-inquiries; branch completion statuses; evidence-fusion
decisions; analytical outputs consumed; final normalized result; final Context Package;
data snapshot or consistency boundary; execution timestamps; cancellation and limit
events.

This execution record supports reproducibility, audits, CAPA investigations, and
regulated evidence packages.

### Scope Boundary

The Parallel Inquiry Fabric is an execution strategy inside or immediately adjacent to
the Query Engine execution layer. It must not become: an autonomous AI-agent framework;
a general distributed-computing platform; a replacement for the Synchronization Engine;
a Process Engine (Baseline Section 6/Section 4 — the authoritative Baseline term; this
corrects an earlier, non-authoritative reference to "Workflow Engine"); a business-rule
engine; a statistical or causal-analysis engine; an authoritative evidence store. The
Query Engine must not become a Process Engine: it never defines or executes process
steps, workflow states, or state transitions — those remain the Process Engine's
exclusive responsibility (Baseline Section 6).

The deterministic structured Query contract (Sections 1–23) remains the foundation of
every branch. **Confirmation:** the Parallel Inquiry Fabric, as specified above, does
not create a second Synchronization Engine (its Evidence Exchange is inquiry-scoped and
temporary, never authoritative — see the Evidence-Exchange-Is-Not-Synchronization
subsection), a second Rule Engine (it never evaluates WHEN/IF/THEN logic or scores
candidate explanations itself — that remains Rule/Statistics/Risk Engine territory), an
AI-agent framework (branches are structured Query Engine operations, never open-ended
AI-driven reasoning loops), or an authoritative evidence store (Evidence Exchange and
Evidence Fusion outputs are transient, inquiry-scoped, and derived entirely from data
already owned by Object Runtime, Event Engine, Transaction Engine, and registered
Computed Value Sources).

## 34. Query Execution Pipeline

To show where the Parallel Inquiry Fabric plugs into query execution, the Query
Engine's execution pipeline is formalized as follows (first stated in this document at
Revision B; earlier sections implied these stages individually without naming the
sequence):

1. Query Validation (Section 16's malformed-query error kind)
2. Permission Resolution (Section 13)
3. Concept Resolution (Section 24 — resolves a concept-named query to its structural
   Query, when addressed via the Semantic Layer; a no-op for queries already expressed
   structurally)
4. Source Resolution (Section 7's QuerySource, Section 6's engine boundaries)
5. Execution Strategy Selection (Section 33's Execution Modes: Linear, Parallel
   Independent, Cooperative Parallel, or — future — Distributed Cooperative)
6. Inquiry Decomposition, when required (Section 33)
7. Parallel Branch Execution (Section 33)
8. Cooperative Evidence Exchange, when enabled (Section 33)
9. Predicate and Traversal Evaluation (Section 7)
10. Computed Value Retrieval (Section 10)
11. Deterministic Evidence Fusion (Section 33)
12. Aggregation (Section 7)
13. Evidence Collection (Section 9's ResultProvenance)
14. Context Package Builder (Section 32)
15. Result Classification (Section 9's ResultClassification)
16. Normalized Result Delivery (Section 16 for errors; Section 18 for the determinism
    guarantee that stages 6–14 must not affect this final stage's output)

Stages 6–8 and 11 are the only stages the Parallel Inquiry Fabric adds; a Linear
Execution query passes through the remaining stages exactly as it did before Revision B.

**Semantic order vs. physical execution plan (correction).** This sixteen-stage
sequence is a semantic ordering, not a mandated physical execution order. Permission
Resolution (stage 2), provenance preservation (stage 13's obligations, carried
throughout), ConsistencyBoundary handling (Section 7, fixed at the start of execution),
and Result Classification / Normalized Result Delivery (stages 15–16) are mandatory
semantic responsibilities: they may never be bypassed, reordered past their dependents,
or weakened, regardless of how execution is physically implemented. By contrast, several
stages may be reordered or interleaved for performance as long as the result is
semantically equivalent to running the normative sequence: Predicate and Traversal
Evaluation (stage 9) may be pushed into a registered QuerySource instead of evaluated
centrally; Evidence Collection (stage 13) may happen incrementally throughout execution
rather than as one discrete step; Aggregation (stage 12) may be performed by a
registered source rather than centrally. The rule: physical execution order may be
optimized; the outcome must always be indistinguishable, in result, evidence,
classification, and Context Package, from running the normative pipeline stage by
stage — this is the same guarantee already stated as the Parallelism invariant
(Section 18), generalized here to physical reordering in general, not only concurrency.

---

## Gate 0 Decision (Revision B, corrected)

```
SPEC-004 — Query Engine
Revision: B
Gate 0: Complete
Repository status: Committed
Implementation: Not Authorized
Next required stage: Gate 1 repository-grounded technical design
```

No ADR change, repository modification beyond adding this file, or Codex implementation
activity is authorized by this document. Repository integration (committing this file
under `architecture/spec/`) is the action this correction pass and its accompanying
instruction package authorize — production implementation is a separate, later, explicit
decision.

Revision A added the organizational-inquiry vision (Sections 24–32) as an extension of,
not a replacement for, the Section 1–23 architecture. Revision B added the Parallel
Inquiry Fabric (Section 33) and the formalized Query Execution Pipeline (Section 34) on
the same terms. This correction pass resolves every open question's status (Section 17:
zero blocking, five recommended-Gate-1, five explicitly-deferred, two resolved), fixes
the "Workflow Engine" reference to the authoritative Baseline term "Process Engine"
(Section 33), states the first implementation's one-shot-only query lifetime
independent of execution strategy (Section 7), narrows the evidence-immutability claim
to completed result packages rather than a global guarantee (Section 9), introduces the
ConsistencyBoundary concept so "identical snapshot" is a checkable contract (Sections 7,
18), separates mandatory pipeline semantics from optimizable physical execution order
(Section 34), and corrects the execution-fallback rule so disabling the Parallel Inquiry
Fabric fails explicitly rather than silently changing an unsupported inquiry's meaning
(Section 33, Acceptance Criterion 20). The technical contract, engine boundaries,
Non-Goals, and every deterministic/read-only/permission-aware/storage-agnostic
requirement established in Sections 1–32 remain unchanged and unweakened by these
corrections.

Next required stage: Gate 1 repository-grounded technical design — reading this
committed document against the actual Object Runtime/Event Engine/Transaction Engine
public APIs, producing a gap/contradiction report, and producing the concrete
implementation design (data contracts, the QuerySource/adapter capability interface,
the implementation slice plan, and the commit plan). That is a separate instruction,
not authorized here, and it produces documents, not code.
