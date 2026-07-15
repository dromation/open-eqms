# Object Runtime

This crate implements `architecture/spec/SPEC-001-object-runtime.md`.

The Object Runtime owns Current Object State only. It provides generic object identity, type metadata registration, direct object access by identity, versioned single-object updates, relation maintenance, validation against inert metadata, and a storage-provider boundary.

Shared primitive Runtime contracts such as `ObjectId` and `PropertyValue` are imported from `runtime/runtime-contracts` per ADR-0001. Object Runtime specific data contracts remain in this crate.

It deliberately does not implement the Event Engine, Rule Engine, Process Engine, Query Engine, GUI, AI, Synchronization, database adapters, Content Packages, Transaction Engine, Security, Statistics/KPI Engines, or Package Loader.

The crate is storage-provider agnostic. Production storage providers are outside SPEC-001; tests use an in-memory provider only for conformance.
