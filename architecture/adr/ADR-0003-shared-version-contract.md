# ADR-0003 — Shared `Version` Contract

Status: Approved

Authority: subordinate to Architecture Baseline v1.0; sibling to ADR-0001 and ADR-0002. Does not restate either.

## Context

SPEC-001 defines `Version` (Section 7) as a strictly increasing revision marker used for optimistic-concurrency conflict detection on Objects (Baseline Section 20). It currently lives only inside the object-runtime crate. SPEC-003 (Transaction Engine) needs to carry base/resulting version values on every Level 1+ Transaction, and the Unit-of-Work integration (ADR-0002) requires that the same version value pass cleanly between an `update_object_in_unit_of_work(...)` call and a Transaction Engine append sharing that Unit of Work. A locally defined, transaction-engine-only version type (e.g., `ObjectVersion(u64)`) would be semantically identical to `Version` but Rust-type-incompatible with it — the same duplicated-primitive problem ADR-0001 already resolved for `ObjectId` and `PropertyValue`. This ADR records the promotion of `Version` into the same shared layer, for the same reason.

## Decision

1. **`Version` is a business-neutral shared Runtime data contract**, not something specific to Object Runtime's implementation. Like `ObjectId` and `PropertyValue`, it is Runtime-wide vocabulary: any component that reads or writes a version number for optimistic-concurrency or audit purposes needs the same type, not a lookalike.

2. **`Version` is promoted from `object-runtime` into `runtime-contracts`**, joining `ObjectId`, `PropertyValueKind`, `PropertyValue`, and the Unit-of-Work contracts already there (ADR-0001, ADR-0002). This is a relocation, not a redefinition: its shape and meaning as defined in SPEC-001 Section 7 do not change.

3. **`object-runtime` and the future `transaction-engine` depend on the same shared `Version` type.** Object Runtime depends on `runtime-contracts` for it, exactly as it already does for `ObjectId`/`PropertyValue`, and re-exports it from the same path existing callers already use, so the relocation is invisible to them. The (not-yet-built) Transaction Engine depends on the same shared type for its base/resulting version fields — never on a type it defines itself, and never on `object-runtime` directly.

4. **`runtime-contracts` remains dependency-neutral.** Adding `Version` does not change this: `runtime-contracts` still has zero dependency on `object-runtime`, `event-engine`, or any `transaction-engine` crate. Dependency direction remains one-way, into `runtime-contracts` only.

5. **Existing Object Runtime public behavior and tests remain unchanged.** This is a pure relocation: `Version`'s shape, semantics, and every invariant in SPEC-001 Section 8 stay exactly as approved. Every existing SPEC-001 test must continue to pass unmodified.

6. **No engine-to-engine dependency is introduced.** This decision only adds a shared type to the existing neutral layer; it creates no new dependency between `object-runtime`, `event-engine`, or `transaction-engine`.

7. **Parallel local version types are rejected.** A transaction-engine-local `ObjectVersion` or any similarly named, structurally-identical-but-separately-typed version representation is explicitly not an acceptable design. If a future component genuinely needs a *different* kind of version concept (not the Object Runtime optimistic-concurrency marker), that requires its own ADR explaining why the shared `Version` type doesn't apply — it is not a default any component may reach for on its own.

## Consequences

- SPEC-003's Base-Version and Resulting-Version Metadata section can now reference one real, shared type instead of an ambiguous "the same representation Object Runtime uses," closing the gap identified in Stage A review.
- The relocation touches the already-implemented, tested Object Runtime crate. It must be executed as a pure move (re-exported at the same path), following the same discipline already used once for `ObjectId`/`PropertyValue` under ADR-0001.
- This ADR does not implement the migration itself, and does not implement the Transaction Engine. Both remain separate, explicitly gated implementation tasks.

## Rejected Alternatives

- **Transaction Engine defines its own `ObjectVersion` locally** — rejected: creates a duplicate, driftable primitive for a concept that's already shared Runtime vocabulary, and breaks clean value-passing through the Unit of Work.
- **Transaction Engine depends on `object-runtime` directly for `Version` only** — rejected for the same reason ADR-0002 rejected this pattern for the Unit-of-Work handle: Cargo dependencies aren't fine-grained, so "just the type" isn't enforceable, and it reopens exactly the engine-to-engine coupling this project has consistently avoided.
- **Leave `Version` where it is and have SPEC-003 describe its own compatible-but-separate type** — rejected: "compatible but separate" is precisely the outcome this ADR exists to prevent.

## References

Architecture Baseline v1.0 Section 20. ADR-0001 (shared runtime-contracts crate). ADR-0002 (shared Unit-of-Work contract). SPEC-001 Object Runtime (amended by this ADR — see SPEC-001 Amendment 2). SPEC-003 Transaction Engine (Base-Version and Resulting-Version Metadata; Assumption 7, resolved by this ADR).
