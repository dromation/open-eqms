# Plugins

This directory is reserved for Open-EQMS native plugins.

Native plugins are external processes that integrate with systems such as SCADA, CAD, Office, PDF, OCR, AI backends, ERP connectors, or hardware drivers through versioned Runtime API contracts.

Plugins must remain outside the Runtime process. Plugin failures must never corrupt Runtime state.

Internal structure under `plugins/` will be created only through approved SPECs and ADRs. Do not add speculative plugin frameworks, adapters, or implementation dependencies during Phase 0.
