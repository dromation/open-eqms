# ADR-0001 - Shared Runtime Contracts Crate

Status: Approved

Date: 2026-07-15

## Context

SPEC-001 implemented the Object Runtime first, so several primitive data-contract
types currently originate there. SPEC-002 Event Engine needs to carry the same
opaque Object identifiers and primitive payload value shapes, but the Event
Engine must not depend on Object Runtime behavior, validation, registries,
storage logic, or any other engine.

Keeping these shared primitive contracts inside an engine crate would force
either an engine-to-engine dependency or duplicate type definitions. Both
options would weaken the Runtime boundary established by Architecture Baseline
v1.0 and AGENTS.md.

## Decision

Create a new minimal shared crate:

`runtime/runtime-contracts`

This crate contains dependency-neutral Runtime data contracts only. It begins
with the genuinely shared, business-neutral types:

- `ObjectId`
- `PropertyValueKind`
- `PropertyValue`

`object-runtime` may depend on `runtime-contracts`.

`event-engine` may depend on `runtime-contracts`.

`runtime-contracts` must not depend on either engine and must not contain Object
Runtime behavior, Event Engine behavior, validation services, registries,
storage logic, business semantics, package logic, or plugin logic.

Future additions to `runtime-contracts` require the same test: the type must be
business-neutral, shared by more than one Runtime component, and safe to expose
without creating an engine dependency.

## Consequences

- Shared primitive Runtime contracts have one canonical definition.
- Event Engine can reuse Object identifiers and primitive payload values without
  depending on Object Runtime.
- Object Runtime keeps its existing public behavior through import updates and
  compatibility re-exports where needed.
- Engine-specific types remain in their owning engine crates.
- This decision prevents duplicate primitive type definitions and prevents
  Object Runtime from becoming an accidental shared dependency for other
  Runtime engines.
