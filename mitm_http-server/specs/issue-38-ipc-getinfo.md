---
name: Feature Specification (Spec Kit)
about: Propose a new feature or rule.
title: "Feature: Dynamic Component Versioning via IPC (GetInfo)"
labels: "feature, spec-kit"
assignees: ""
---

## Feature Intent

To support independent versioning of the core components (`mitm_http-server`, `mitm_iam-server`, `mitm_scheduler-server`), the HTTP server must be able to dynamically query the current runtime versions of the other components. This requires extending the Unix Domain Socket (UDS) IPC protocol with a new `GetInfo` request.

## Requirements (EARS Syntax)

1. The `mitm_common` library shall define a new `GetInfo` request and a corresponding `InfoResponse` payload for both `IpcRequest` (IAM) and `SchedulerRequest` (Scheduler) enums.
2. The `mitm_iam-server` shall intercept the `GetInfo` request over UDS and respond with its `CARGO_PKG_VERSION` and component name.
3. The `mitm_scheduler-server` shall intercept the `GetInfo` request over UDS and respond with its `CARGO_PKG_VERSION` and component name.
4. The `mitm_http-server` shall query both the IAM and Scheduler sockets when the `/info` or `/api/system/v1/info` endpoints are invoked, dynamically populating the `core_components` JSON array instead of using hardcoded version strings.

## Scope

- **Component:** `core-layer`
- **Modules affected:** 
  - `mitm_common/src/ipc.rs` (Protocol definition)
  - `mitm_iam-server/src/main.rs` (UDS Server Handler)
  - `mitm_scheduler-server/src/main.rs` (UDS Server Handler)
  - `mitm_http-server/src/handlers/mod.rs` (Info Endpoint)
  - `mitm_http-server/src/ipc_client.rs` (IPC Dispatcher)

## SpecDD Architecture Alignment (Drift Control)

Please confirm that this feature respects the global `mitm-2` constraints defined in `.sdd` files:

- [x] **Architecture:** The layered architecture is maintained (no direct bypass from Collector to Delivery).
- [ ] **Architecture:** Feature affects architecture: SpecKit feature forces update of the SpecDD .sdd
- [x] **Security:** Envelope Encryption (AES-GCM) is NOT bypassed for PII data.
- [x] **Data Model:** Core PostgreSQL schemas remain intact (feature-specific tables are allowed).
- [x] **Standards:** SPDX headers, English documentation, and independent `go.mod` per layer will be maintained.

## Acceptance Criteria

- [ ] `mitm_common` correctly compiles with the new IPC message structures.
- [ ] Both IAM and Scheduler services return their respective `CARGO_PKG_VERSION` when queried over UDS.
- [ ] `GET /info` successfully aggregates and returns the real-time versions from all three running core components.
- [ ] If an IPC socket is down, the HTTP server handles the error gracefully (e.g., reporting `"version": "offline"` or `"unknown"` for that component).
