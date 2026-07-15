# Open-EQMS Architecture Baseline v1.0

Status: Normative / Approved / Authoritative for implementation

## 0. Document Control

This document consolidates every architectural decision approved across the Open-EQMS review process into a single normative specification. Where a decision was revised during review, only the final, corrected version is recorded here. No earlier alternative is preserved as a live option.

This document supersedes: the Master Architecture Prompt, the Meta Architecture Specification v1, the Runtime/Content/Plugin architecture reviews, the Repository Structure Proposal v1.0, and the Architecture Freeze review. Those documents remain the historical record of how these decisions were reached; this document is the one to build against.

Supersession note: the technology stack recorded early in this project's history (React/Next.js/Electron frontend, FastAPI/NestJS backend) is superseded by Section 25 below. A native, deterministic Runtime Engine is a fundamentally different technical direction from a web-application stack, and the two are not compatible. Section 25 is authoritative.

## 1. Mission and Philosophy

Open-EQMS is a lightweight, deterministic Business Runtime Platform. It is not an ERP, not a QMS, and not a document management system in the conventional sense. Business applications are implemented as declarative Content Packages, executed by a stable Runtime, in the same relationship a game engine has to the content it loads: the engine is generic and long-lived, the content is domain-specific and replaceable.

The Runtime is designed to remain usable for decades. Business packages evolve independently of it and of each other.

## 2. Fixed Core Principles

- The Runtime contains only universal platform capabilities and no business methods.
- Business methods belong entirely inside Content Packages.
- Declarative packages are preferred over executable code; approximately 90% of business functionality should require no code.
- The Rule DSL is preferred over custom programming, is not a general-purpose language, and must not implement its own query syntax.
- WASM remains an exceptional escape hatch, never the standard development model.
- Native plugins remain isolated processes.
- AI is external to the Runtime.
- Offline-first operation is mandatory.
- Everything user-visible is localizable.
- Facts, once recorded, are never deleted.
- Auditability is mandatory.
- Deterministic behavior is mandatory.
- Optional features must never increase the minimum Runtime footprint.
- Simplicity over completeness. Determinism over cleverness. Stable Runtime over feature-rich Runtime. Business Content over Runtime customization. Long-term maintainability over short-term convenience.

## 3. Design Order

Architecture and content are developed in this order, top to bottom, never starting from technology: applicable standards and regulations -> regulatory and business requirements -> system capabilities -> data objects and relations -> rules and process nodes -> transactions and audit evidence -> KPI and reporting requirements -> GUI views -> integrations -> implementation technologies.

## 4. Runtime Architecture

The Runtime consists of exactly these components: Object Runtime, Event Engine, Rule Engine, Process Engine, Query Engine, Statistics Engine, KPI Engine, Transaction Engine, Package Loader, GUI Engine (View Model abstraction with interchangeable renderers), Synchronization Engine, and Security (Permissions, Capabilities, Identity, Cryptography).

Resource Runtime and State Engine were evaluated and intentionally removed: resources are Object Runtime types, and state transitions belong to the Process Engine. Neither is a separate runtime component.

Boundary rule, enforced by CI, not convention: the Runtime source tree may not import or depend on anything defined inside a Content Package.

## 5. Object Model

Every business entity - employee, machine, supplier, customer, material, product, BOM, document, measurement, task, complaint, audit, requirement, risk, nonconformity, KPI, and any future type - is represented as a generic Object. Business semantics come entirely from Content Packages; the Runtime's Object Runtime has no knowledge of what any given object type means.

Every object instance carries: unique ID, object type, properties, relations, lifecycle state, owner, responsible users, permissions, version, history, comments, attachments, process state, KPI relationships, external references, and retention rules. Object types and their fields are configuration - a typed metadata registry plus a structured properties store - not schema migrations.

## 6. Process and Diagram Engine

A business process is a graph of nodes and permitted transitions, defined as package data and executed by the Process Engine. Each node defines: purpose, problem solved, entry conditions, required data, displayed form, available actions, allowed roles, validation rules, automatic commands, required evidence, KPI effects, approval requirements, exit conditions, next permitted nodes, exception paths, and escalation rules.

The user-facing question at every point is: what must be done now. State transitions are a Process Engine responsibility; there is no separate state machine concept.

## 7. Rule Engine and Rule DSL

Business rules follow the event-condition-action pattern (WHEN / IF / THEN), evaluated deterministically by the Rule Engine. The Rule DSL is intentionally small: it evaluates conditions, invokes the Query Engine, creates actions, generates events, and updates state through Runtime APIs. It does not implement its own filtering or aggregation syntax. That always goes through the Query Engine.

The Rule DSL is versioned independently of the Runtime API, carrying the same backward-compatibility guarantee: a package written against Rule DSL version N must keep working on every later Runtime that still declares support for version N.

Runtime-enforced execution limits are mandatory:

- Maximum execution time.
- Maximum rule depth.
- Maximum chain length.
- Recursion detection.
- Cycle detection.
- Maximum generated actions.
- Maximum generated events.
- Maximum iterations.

## 8. Query Engine

The Query Engine is the single source for querying structured Runtime data. Business Rules, Statistics, KPIs, Reports, and the AI adapter layer all read through it; none implement a parallel query mechanism.

## 9. Event and Transaction Model

The operational data model has three parts: Current Object State (the authoritative, queryable live data), Business Event Log (immutable recorded facts - machine stopped, material received, document approved - never deleted), and Transaction/Audit Log (validated state changes, written in the same database transaction as the state change they describe, so audit trail and state can never drift apart).

Rule evaluation is not a separate permanent log. Rule-evaluation metadata - triggering event ID, rule version, evaluation result, generated actions, execution metadata - is recorded as fields on the resulting Transaction/Audit Log entry. This preserves full traceability of why a rule fired without a fourth persistent store.

Three transaction levels apply, distinguished by regulatory weight:

- Level 1 (ordinary edits): transaction ID, object, operation, old/new value, user, device, time, base version, no signature required.
- Level 2 (process events): adds sequence number, server receipt time, transaction hash, link to the prior event.
- Level 3 (confirmed business milestones - approvals, releases, e-signatures): requires signer identity, role, meaning of signature, authentication evidence, signed revision, timestamp, reason, and a cryptographic signature built on an established library.

## 10. Statistics and KPI Engines

The Statistics Engine performs continuous aggregation over live data: averages, counts, trends, rates, distributions. The KPI Engine evaluates those statistics against configured business targets: threshold exceeded, SLA violated, target achieved, escalation required. Statistics calculate; KPIs evaluate. These are distinct responsibilities, not duplicated ones.

A KPI definition carries: goal, calculation, source data, target, warning threshold, critical threshold, owner, review frequency, escalation rule, applicable scope, and historical trend.

## 11. Content Package Model

A Content Package defines: objects, relations, events, rules, processes, forms, menus, views, dashboards, reports, statistics, KPI definitions, translations, document templates, and a Capability Manifest. Packages are signed and versioned (semantic version plus a declared Runtime compatibility range); dependencies form a resolvable, acyclic graph, resolved from a local package cache so resolution works offline.

Packages carry an explicit trust/complexity tier based on what they use: pure-declarative, declarative plus Rule DSL, or declarative plus an exceptional WASM module. This tier determines the level of review a package requires before it is trusted in a regulated deployment.

## 12. GUI Architecture

The Runtime is GUI-agnostic. Business logic never depends on a graphical framework. The Runtime exposes View Models - structured descriptions of what to render, resolved from package-defined forms, menus, tables, graphs, process diagrams, dashboards, and document layouts - and renderers consume View Models to produce an actual interface.

Renderers are interchangeable by design. Version 1 targets a Native Desktop renderer only. Web and mobile renderers are recognized future adapters, explicitly deferred, not part of the v1 scope. No browser runtime and no JavaScript runtime are required for the v1 Native Desktop experience.

## 13. Security Architecture

Security has four responsibilities: Permissions (role-based access control scoped by organization, site, and team, with delegation and separation of duties), Capabilities (per-package Capability Manifests, fine-grained, enforced on every privileged API call at runtime - not only at install time - with least privilege as the default), Identity (users, devices, authentication), and Cryptography (integrity, signing, package and transaction verification, using established libraries only - never a custom algorithm).

Permission changes are themselves audited. Offline devices cache only data the authenticated user is authorized to access, encrypted at rest.

## 14. Native Plugin Architecture

Native integrations - SCADA, CAD, Office, PDF, OCR, AI backends, ERP connectors, hardware drivers - run as isolated processes, never in-process with the Runtime. Communication occurs exclusively through versioned RPC contracts against controlled Runtime APIs. A plugin failure must never corrupt Runtime state; this requires an explicit lifecycle contract per plugin (restart policy, health checks, versioned schema) so the Runtime and a plugin can be upgraded independently.

## 15. AI Architecture

AI is external to the Runtime, reached only through interchangeable adapters (OpenAI, Claude, Gemini, local/enterprise models, and future providers). AI has no direct database access - only Query Engine access to data the requesting user is already authorized to see.

AI may query, summarize, explain, compare, recommend, predict, and generate drafts. AI must never replace deterministic business rules, modify official records directly, approve regulated actions, bypass permissions, delete audit history, or invent evidence. Every AI output is tagged with its data sources, whether it is fact, calculation, or recommendation, its uncertainty, missing data, and the time and context of the analysis.

## 16. SCADA and External Integration

SCADA and other industrial systems are external live data providers, connected through standard adapters (OPC UA, MQTT, REST, message brokers, file import, database connectors). Open-EQMS stores references, selected relevant measurements, calculated values, event windows, and verified evidence snapshots - not a full duplicate of source system history. All integrations are isolated behind adapters; no vendor-specific code lives inside core business logic.

## 17. Document Generation

Structured Runtime data is always the source of truth. DOCX, XLSX, PDF, CSV, ODT, and ODS documents are generated views of that data, carrying metadata: record ID, revision, status, generation time, approval state, approvers, verification code, and applicable template version. Office format support is modular and delivered through plugins; the Runtime does not embed an office suite.

## 18. Storage Architecture

The Runtime does not depend on a specific database implementation. Storage providers are deployment-specific behind a common internal abstraction: standalone deployments use SQLite; small-company deployments use SQLite with synchronization; enterprise deployments use PostgreSQL. Business logic, the Object Runtime, and the Query Engine are unchanged across providers.

## 19. Synchronization Architecture

Synchronization transfers deltas, not full records: small verified change packages (transaction ID, object type, object ID, operation, field, old/new value, user, device, base version, timestamp), delivered idempotently and resumably. Offline queues, conflict detection, and selective synchronization are core v1 functionality.

Controlled peer-assisted transfer remains optional, compiles as a separate module so it does not affect the minimum Runtime footprint, and is deferred until an actual multi-site deployment requires it. It is not a public peer-to-peer network, has no blockchain consensus, and any use requires authenticated nodes, encrypted transfer, and revocable, auditable participation.

## 20. Conflict Resolution

Different-field concurrent edits merge automatically. Same-field concurrent edits on non-critical fields resolve to the server-authoritative or last-approved value; critical or approved fields require explicit human conflict review. No value is ever silently discarded. Ordering relies on transaction ID (idempotency) and base version or server sequence number, never on wall-clock timestamp alone.

## 21. Data Retention and Versioning

Four data classes are handled distinctly: the current operational projection (mutable), the complete regulated history (append-only, retained per policy, never deleted to save space), archived evidence (object storage, subject to retention and legal hold), and disposable technical synchronization state (safely purgeable). Every transaction type carries a schema version from day one so historical records remain interpretable, or are migrated through a documented, controlled process, as the data model evolves.

## 22. Localization

Localization is a Runtime capability; packages supply translations. Business identifiers remain language-neutral. Unicode is mandatory; right-to-left languages, fallback languages, locale-aware formatting, and localized reports and document generation are all supported by the Runtime, not reimplemented per package.

## 23. Performance and Footprint

Target Runtime binary footprint: approximately 20-40 MB. No embedded browser, no embedded office suite, no embedded AI model. Office support, AI support, and synchronization (beyond core delta sync) are modular and must not grow the minimum footprint when unused. The Runtime is designed to run on weak hardware, low-bandwidth connections, and in offline, industrial environments.

## 24. Regulatory Architecture and Traceability

Every applicable standard clause is represented as a Requirement Record: ID, source standard, clause, requirement text or authorized summary, interpretation, required evidence, required process, responsible role, required data, required approval, retention period, audit trail, KPI or monitoring requirement, applicable scope, implementation status, verification status, and linked system controls.

Traceability is a real, queryable relational chain, not documentation:

Requirement -> System rule -> Process node -> Data object -> User action -> Generated evidence -> KPI -> Audit result.

## 25. Technology Stack

Runtime core: Rust - memory safety without garbage-collection pauses, a mature WASM toolchain, and a small resource footprint consistent with Section 23.

Sandboxed package scripting: WASM (wasmtime/wasmer), capability-scoped per the package's Capability Manifest - the exceptional escape-hatch layer described in Section 11, never the default.

Storage: SQLite (standalone/small deployments) and PostgreSQL (enterprise deployments) behind the internal storage abstraction described in Section 18.

GUI: a native rendering toolchain for the v1 Native Desktop renderer described in Section 12; no browser or JavaScript runtime dependency for v1.

Native plugins (Section 14) may use whichever language suits the integration; C++ is acceptable only where required by a legacy CAD parser or an industrial driver with no practical alternative - never as a default choice.

This supersedes any earlier reference to a browser-application stack (React, Next.js, Electron, FastAPI, NestJS) from before the Runtime/Content architecture was adopted. That stack described a web application; Open-EQMS is now a native runtime engine, a different technical category, and the earlier stack no longer applies.

## 26. MVP Boundary

Each area is classified using the same categories used throughout this review: core MVP, second phase, regulated extension, optional future feature, or reject.

| Area | Classification |
| --- | --- |
| Regulatory requirement traceability | Core MVP |
| Object / Process / Rule / Query engines | Core MVP |
| Transaction model (Level 1-2) | Core MVP |
| Statistics & KPI engines | Core MVP |
| Native Desktop GUI (View Model + renderer) | Core MVP |
| Offline cache + delta synchronization | Second phase |
| Conflict resolution (non-critical fields) | Second phase |
| Level 3 e-signatures / hash-chained audit | Regulated extension |
| AI analytical layer | Optional future feature |
| SCADA / industrial adapters | Optional future feature |
| Controlled peer-assisted transfer | Optional future feature |
| Web / mobile GUI renderers | Optional future feature |
| Custom cryptographic protocol design | Reject |
| Full event sourcing for all object types | Reject |
| Separate Resource Runtime / State Engine | Reject (merged into Object Runtime / Process Engine) |

## 27. Implementation Roadmap

- Phase 0 - Core loop: object model, process/diagram engine, forms and reports, single storage provider, single site, no synchronization, no cryptography beyond basic integrity.
- Phase 1 - Trust foundation: Transaction/Audit Log (Levels 1-2), Statistics and KPI engines reading live data.
- Phase 2 - Offline: local cache, queued writes, storage-provider abstraction finalized (SQLite/PostgreSQL), delta synchronization, conflict resolution for non-critical fields.
- Phase 3 - Regulated approvals: Level 3 e-signatures, hash-chained audit for critical events, human conflict review workflow.
- Phase 4 - AI layer: controlled read-only API, adapter-based provider integration.
- Phase 5 - Industrial integration: SCADA/PLC/MES adapters.
- Phase 6 - Multi-site: peer-assisted transfer, local factory servers - only if an actual deployment requires it.

## 28. Repository Organization and Governance

Architecture is the single source of truth for the project; the `architecture/` directory holds baseline documents, specifications, decisions (ADR-driven), diagrams, and standards. Runtime, packages, plugins, the SDK, and the package builder are kept in clearly separated top-level directories, mirroring the Runtime/Content/Plugin boundary described in Sections 4, 11, and 14.

Schemas and the Rule DSL are versioned as first-class, independently tracked artifacts. The package builder validates every package's manifest, capability declarations, and schema compatibility, and runs it against a regression/conformance suite before allowing it to be signed.

- Repository Rule 1: Runtime must never depend on business packages.
- Repository Rule 2: Packages must not modify the Runtime.
- Repository Rule 3: Plugins communicate only through public Runtime APIs.
- Repository Rule 4: All business methods belong inside packages.
- Repository Rule 5: The Runtime contains only universal platform functionality.
- Repository Rule 6: Every architectural decision is documented (ADR) before implementation.
- Repository Rule 7: Backward compatibility is preferred over short-term convenience.
- Repository Rule 8: Performance and deterministic behavior take priority over feature count.
- Repository Rule 9: Optional functionality must never increase the minimum Runtime footprint.
- Repository Rule 10: The repository remains understandable by a new contributor within one day.

### Enforcement

- Rules 1, 2, and 3 are enforced by a CI dependency-graph check, not left to convention.
- Rule 6 is enforced by a CI check requiring an ADR for any change under `runtime/`.

### Approval

Architecture Baseline v1.0 is approved for implementation. This document is the authoritative reference for all Open-EQMS development going forward. Changes to any principle or component described here require a new Architecture Decision Record and explicit re-approval. They are not made silently in the course of implementation.
