# Transaction Engine

This crate implements `architecture/spec/SPEC-003-transaction-engine.md`.

The Transaction Engine owns the Transaction/Audit Log only. It records immutable
validated state-change facts, assigns append ordering, computes Level 2+
transaction hashes through an abstract cryptographic provider, verifies Level 3
signatures through that same provider boundary, and exposes read-by-identity plus
append-index range reads.

It deliberately does not implement Object Runtime behavior, Event Engine
behavior, Rule Engine, Process Engine, Query Engine, GUI, AI, Synchronization,
database adapters, Content Packages, Security authorization, Statistics/KPI
Engines, Package Loader, cryptographic algorithms, signing, or any mutation of
previously appended Transactions.

The crate is storage-provider agnostic. Production storage providers are outside
SPEC-003; tests use in-memory providers only for conformance.
