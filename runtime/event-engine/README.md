# Event Engine

This crate implements `architecture/spec/SPEC-002-event-engine.md`.

The Event Engine owns the Business Event Log only. It records immutable business
facts, validates payloads against inert Event Type metadata, assigns append
sequence order, and exposes read-by-identity plus append-sequence range reads.

It deliberately does not implement Rule Engine, Process Engine, Query Engine,
Transaction Engine, GUI, AI, Synchronization, database adapters, Content
Packages, Security, Statistics/KPI Engines, Package Loader, or Object Runtime
calls.

The crate is storage-provider agnostic. Production storage providers are
outside SPEC-002; tests use an in-memory provider only for conformance.
