# Runtime

This directory is reserved for the Open-EQMS Runtime.

The Runtime contains only universal platform capabilities defined by Architecture Baseline v1.0. Business methods do not belong here.

Internal structure under `runtime/` will be created only through approved SPECs and ADRs. Do not add component subdirectories, dependencies, crates, frameworks, or implementation code without approved architecture and specification coverage.

The Runtime must preserve deterministic behavior, offline-first operation, auditability, immutable facts, localization, and the Runtime / Content Package / Plugin boundary.
