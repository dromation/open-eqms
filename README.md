# Open-EQMS

Open-EQMS is an open-source project for enterprise quality management and related business processes.

## Current Project

Open-EQMS Next Generation is the deterministic Business Runtime Platform defined by Architecture Baseline v1.0.

The active implementation target is a native Runtime that executes declarative Content Packages while preserving deterministic behavior, offline-first operation, auditability, localization, and a clear Runtime / Content Package / Plugin boundary.

The authoritative architecture is maintained in `architecture/Open-EQMS_Architecture_Baseline_v1.0.md`.

## Historical Prototype

The previous Python/Kivy/Lua-era prototype is preserved under `legacy/python-kivy-prototype/` for history and reference.

The prototype is not the active implementation and must not be used as the architectural foundation for the new Runtime. No compatibility between the prototype and the new Runtime is currently guaranteed.

## Status

The repository is in architecture and governance transition. The new Runtime is not yet implemented, certified, compliant, or production-ready.

## Pre-Alpha Demo

`apps/open-eqms-demo` is a local, single-process reference application for the VS-001 traceable asset registration slice. It is user-facing demo code built on top of the Runtime crates, not an `examples/` API snippet.

Run the full deterministic scenario with:

```powershell
cargo run -p open-eqms-demo -- run-demo
```

Available commands:

- `register-asset [--field=value]`
- `record-calibration [--field=value]`
- `show-asset <asset-id>`
- `show-timeline <asset-id>`
- `run-demo`

The scripted path registers one traceable asset, appends `asset.registered`, records an accepted calibration through `asset.calibration_performed` and `asset.calibration_accepted`, commits the accepted calibration Object update together with its Level2 Transaction, then prints the current asset and timeline.

Important boundaries and limitations:

- The demo is pre-alpha, local, in-memory, and non-production. It is not certified, compliant, or suitable for regulated use.
- Each CLI invocation starts a fresh in-process store. Use `run-demo` for the complete end-to-end path in one process.
- The demo composes `open-eqms-object-runtime`, `open-eqms-event-engine`, and `open-eqms-transaction-engine`; it does not depend on `open-eqms-query-engine`.
- Registration writes a Level1 Transaction with no prior reference. Accepted calibration writes a Level2 Transaction with `prior_reference` pointing to the registration Transaction and no caller-supplied prior hash. No Level3 transaction fields are populated.
- Event appends are independent and are not staged in a Unit of Work. If a later Object/Transaction step fails, the structured outcome reports the immutable Event records already created and recovery guidance.
- `show-timeline` uses direct Event Engine append-sequence reads and Transaction Engine append-index reads, then filters by asset in the application layer. This is temporary orchestration pending Query Engine execution; it is not Query Engine execution.
- The demo cryptographic provider is deterministic and non-cryptographic. It exists only to exercise the Transaction Engine hash boundary.
- The demo implements no delete, undo, Security Engine, authorization contract, or `ConsistencyBoundary`.

## License

Open-EQMS remains licensed under GPL-2.0. See `LICENSE`.
