# Task Plan: Dynamic Component Versioning via IPC (Issue #38)

## Phase 1: IPC Protocol Extension
- [ ] **Task 1: Extend IPC Interfaces**
  - **File:** `core-layer/mitm_common/src/ipc.rs`
  - **Action:** 
    - Define a new shared struct `InfoResponse { pub name: String, pub version: String }`.
    - Add `#[serde(rename = "get_info")] GetInfo` to `IpcRequest`.
    - Add `#[serde(rename = "get_info_result")] GetInfoResult(InfoResponse)` to `IpcResponse`.
    - Add `#[serde(rename = "get_info")] GetInfo` to `SchedulerRequest`.

## Phase 2: Daemon Integrations
- [ ] **Task 2: Implement IAM Server Info Handler**
  - **File:** `core-layer/mitm_iam-server/src/main.rs`
  - **Action:** Intercept `IpcRequest::GetInfo` in the UDS accept loop.
  - **Details:** Return `IpcResponse::GetInfoResult(InfoResponse { name: "mitm_iam-server".to_string(), version: env!("CARGO_PKG_VERSION").to_string() })`.

- [ ] **Task 3: Implement Scheduler Server Info Handler**
  - **File:** `core-layer/mitm_scheduler-server/src/main.rs`
  - **Action:** Intercept `SchedulerRequest::GetInfo` in the UDS accept loop.
  - **Details:** Return the JSON serialized string of `InfoResponse { name: "mitm_scheduler-server".to_string(), version: env!("CARGO_PKG_VERSION").to_string() }` to the socket (matching the current raw JSON return style of the scheduler).

## Phase 3: HTTP Server Dispatcher
- [ ] **Task 4: Implement IPC Fetchers in HTTP Server**
  - **File:** `core-layer/mitm_http-server/src/ipc_client.rs`
  - **Action:** Create two new asynchronous helper functions: `query_iam_info(socket_dir: &Path)` and `query_scheduler_info(socket_dir: &Path)`.
  - **Details:** These functions will open a UnixStream, send the `GetInfo` request, read the response, and return the version string. If the connection fails or times out, return `"offline"`.

- [ ] **Task 5: Refactor HTTP `/info` Endpoint**
  - **File:** `core-layer/mitm_http-server/src/handlers/mod.rs`
  - **Action:** Update `handle_info`.
  - **Details:** Replace the hardcoded `"1.2.0"` strings by awaiting the new `query_iam_info` and `query_scheduler_info` functions. Inject the dynamically fetched version strings into the JSON response.

## Phase 4: Release
- [ ] **Task 6: Documentation & PR**
  - **Action:** Verify all code compiles across the entire workspace (`cargo check --workspace`). Update CHANGELOGs. Commit, push, and open the Pull Request.
