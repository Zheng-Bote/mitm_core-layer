# Feature Specification: Dynamic Component Versioning via IPC (Issue #38)

## 1. Overview
Currently, the `/api/system/v1/info` endpoint (and the legacy `/info` endpoint) exposed by the `mitm_http-server` hardcodes the version numbers of its sibling services (`mitm_iam-server` and `mitm_scheduler-server`). Because these components are independently versioned and compiled as separate binaries, the HTTP server must dynamically query their versions at runtime. This specification defines the extension of the Unix Domain Socket (UDS) inter-process communication (IPC) protocol to support a `GetInfo` request, enabling robust, decentralized component version reporting.

## 2. Technical Objectives
1. **Protocol Extension:** Expand the `mitm_common::ipc` module to include new request/response variants for querying component metadata.
2. **IAM Server Integration:** Implement a handler in `mitm_iam-server` that listens for the new IPC request and responds with its compile-time `CARGO_PKG_VERSION`.
3. **Scheduler Server Integration:** Implement a handler in `mitm_scheduler-server` that listens for the new IPC request and responds with its compile-time `CARGO_PKG_VERSION`.
4. **HTTP Server Aggregation:** Refactor the `handle_info` function in `mitm_http-server` to asynchronously query both the IAM and Scheduler UDS sockets. Aggregate the results into the final JSON response.
5. **Fault Tolerance:** If a sibling service is offline or the UDS socket is unresponsive, the HTTP server must not crash or fail the entire request. It should gracefully report the component's version as `"offline"`.

## 3. Affected Components
- **Library:** `core-layer/mitm_common/src/ipc.rs`
- **IAM Daemon:** `core-layer/mitm_iam-server/src/main.rs`
- **Scheduler Daemon:** `core-layer/mitm_scheduler-server/src/main.rs`
- **HTTP Daemon:** 
  - `core-layer/mitm_http-server/src/handlers/mod.rs`
  - `core-layer/mitm_http-server/src/ipc_client.rs`

## 4. Implementation Details

### 4.1. IPC Protocol (`mitm_common`)
- Add `GetInfo` to `IpcRequest` (used by IAM).
- Add `GetInfoResult { name: String, version: String }` to `IpcResponse` (used by IAM).
- Add `GetInfo` to `SchedulerRequest` (used by Scheduler).
- Add `GetInfoResult { name: String, version: String }` to `SchedulerResponse` (Wait, does `SchedulerResponse` exist? Let's check or create it, or reuse a generic response if applicable. The scheduler currently might not have a generic response enum, we need to verify).

### 4.2. Daemons
- **IAM Server:** Add a match arm in the UDS accept loop to handle `IpcRequest::GetInfo` by returning `IpcResponse::GetInfoResult`.
- **Scheduler Server:** Add a match arm to handle `SchedulerRequest::GetInfo`. 

### 4.3. HTTP Server
- Create a helper function in `ipc_client.rs` (e.g., `query_iam_info` and `query_scheduler_info`).
- In `handle_info`, use `tokio::join!` or sequential `.await` to query both sockets with a short timeout (e.g., 500ms).
- Map timeouts/errors to an `"offline"` string.

## 5. Drift Control & Architecture Constraints
- **Decoupling:** The HTTP server remains decoupled from the physical implementations of the other servers, relying strictly on the shared UDS contract.
- **Resilience:** The addition of IPC queries on an unauthenticated public endpoint (`/info`) must be fast and heavily bound by timeouts to prevent Denial of Service (DoS) via socket exhaustion.

## 6. Acceptance Criteria
1. The `mitm_common` library successfully compiles with the new IPC structures.
2. The HTTP server's `/info` endpoint returns the correct `CARGO_PKG_VERSION` of the running `iam-server` and `scheduler-server`.
3. If the IAM server is killed, the HTTP server's `/info` endpoint returns `"version": "offline"` for the `mitm_iam-server` component, while still successfully returning a 200 OK response.
