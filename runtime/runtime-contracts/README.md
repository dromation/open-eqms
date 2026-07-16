# Open-EQMS Runtime Contracts

Dependency-neutral Runtime data contracts shared by Runtime engines.

This crate contains contracts only. It must not contain engine behavior,
validation services, registries, storage logic, transaction-manager logic,
business semantics, package logic, plugin logic, global mutable state, or
dependencies on Runtime engine crates.

`UnitOfWork` is an opaque handle and the Unit-of-Work traits define provider
participation shape only. Concrete storage providers own begin, commit,
rollback, staging, and any physical transaction wiring.
