# ADR-0000: Adopt Architecture Baseline v1.0

Status: Approved

Date: 2026-07-15

## Context

Open-EQMS previously contained a Python/Kivy/Lua prototype at the repository root. The project has since adopted Architecture Baseline v1.0 as the authoritative technical direction for a native, deterministic Business Runtime Platform.

The repository needs a clear separation between historical prototype material and the current-generation implementation area.

## Decision

Open-EQMS adopts `architecture/Open-EQMS_Architecture_Baseline_v1.0.md` as the normative, approved, authoritative reference for implementation.

The existing Python/Kivy/Lua code is recognized as the historical prototype and preserved under `legacy/python-kivy-prototype/`.

The new-generation implementation will be developed separately from the historical prototype and will follow the Runtime / Content Package / Plugin boundary defined by the Baseline.

Repository growth will be incremental. `runtime/`, `packages/`, and `plugins/` begin as README-only top-level areas. Internal structure will be added only through approved SPECs and ADRs.

## Consequences

The historical prototype remains available for reference without being treated as active architecture.

Implementation work must cite the Baseline and any relevant approved ADRs or SPECs.

Architectural changes require a new ADR and explicit approval before implementation.

## References

- `architecture/Open-EQMS_Architecture_Baseline_v1.0.md`
- `legacy/python-kivy-prototype/README.md`
