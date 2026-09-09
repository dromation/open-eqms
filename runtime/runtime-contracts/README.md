# Open-EQMS Runtime Contracts

Dependency-neutral Runtime data contracts shared by Runtime engines.

This crate contains contracts only. It must not contain engine behavior,
validation services, registries, storage logic, transaction-manager logic,
business semantics, package logic, plugin logic, global mutable state, or
dependencies on Runtime engine crates.

`UnitOfWork` is an opaque handle and the Unit-of-Work traits define provider
participation shape only. Concrete storage providers own begin, commit,
rollback, staging, and any physical transaction wiring.

The authorization contract contains opaque caller/action/target request shapes,
an injected provider trait, the `Allow` / `Deny` / `HiddenDeny` result vocabulary,
and the minimal trace envelope required by ADR-0004. It contains no role,
capability, delegation, policy, or denial-disclosure implementation.

`ConsistencyBoundary` is the shared composite read-boundary contract required by
ADR-0005. It records named optional component slots and provides deterministic
canonical encoding rules. Owning engines remain responsible for producing their
own stable component markers through future engine APIs.
