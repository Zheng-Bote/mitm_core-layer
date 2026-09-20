# mitm_http-server

The HTTP Server acts as the API Gateway for the MitM-2 architecture. It provides a RESTful interface adhering to the JSON:API standard, serving external consumers like the C++ Admin Frontend.

## Architecture & Components

The server is built on **Axum** and uses **sqlx** for asynchronous PostgreSQL access. It communicates with other internal microservices via Unix Domain Sockets (UDS).

### C4 Component Diagram

```mermaid
C4Component
    title Component Diagram for MitM-2 HTTP Server

    Container_Ext(cpp_frontend, "Admin Frontend", "C++", "External API Consumer")
    Container_Ext(iam_server, "IAM Server", "Rust", "Identity & Access Management")
    Container_Ext(scheduler, "Scheduler Server", "Rust", "Orchestration Engine")
    ContainerDb_Ext(db, "PostgreSQL", "SQL", "Central Database")

    Container_Boundary(http_server, "HTTP Server (Axum)") {
        Component(router, "Router & App State", "Axum", "Routes HTTP requests and manages shared state")
        Component(db_pool, "DB Manager", "sqlx", "PostgreSQL connection pool manager")
        Component(ipc_client, "IPC Client", "Unix Domain Sockets", "Communicates with IAM and Scheduler")
        
        Component(h_jobs, "Jobs Handler", "Rust", "JSON:API endpoints for jobs")
        Component(h_rbac, "RBAC Handler", "Rust", "JSON:API endpoints for roles & users")
        Component(h_dlq, "DLQ Handler", "Rust", "JSON:API endpoints for Dead Letter Queue")
        Component(h_admin, "Admin/Logs Handler", "Rust", "System config & Audit logs")
    }

    Rel(cpp_frontend, router, "Makes HTTPS requests to", "JSON/REST")
    
    Rel(router, h_jobs, "Routes to")
    Rel(router, h_rbac, "Routes to")
    Rel(router, h_dlq, "Routes to")
    Rel(router, h_admin, "Routes to")

    Rel(h_jobs, ipc_client, "Triggers actions via")
    Rel(h_rbac, ipc_client, "Triggers actions via")
    
    Rel(h_jobs, db_pool, "Reads/Writes via")
    Rel(h_rbac, db_pool, "Reads/Writes via")
    Rel(h_dlq, db_pool, "Reads/Writes via")
    Rel(h_admin, db_pool, "Reads/Writes via")

    Rel(db_pool, db, "Executes queries", "TCP/SQLx")
    Rel(ipc_client, iam_server, "Sends UDS messages", "JSON")
    Rel(ipc_client, scheduler, "Sends UDS messages", "JSON")
```

## Configuration

- `MITM_DB_URL` (optional): PostgreSQL Connection String.
- `IAM_SOCKET_PATH` (optional): Path to the IAM UDS.
- `SCHEDULER_SOCKET_PATH` (optional): Path to the Scheduler UDS.

## Execution

```bash
cargo run -p mitm-http-server
```
