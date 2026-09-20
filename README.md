# MitM-2 Core Layer

This repository contains the core components of the MitM-2 Data Aggregator architecture. The core has been completely ported to a modular Rust microservices architecture.

## Components

- **`mitm_common`**: Shared libraries, crypto functions, database configurations, and standard IPC structures.
- **`mitm_http-server`**: The central API gateway (Admin API). It provides the REST API for the C++ Frontend and other external consumers. Implemented using `axum` and `sqlx`, offering fully paginated JSON:API endpoints for Job Management, DLQ, and RBAC.
- **`mitm_iam-server`**: The Identity and Access Management service. Runs as a background daemon connected via Unix Domain Sockets (UDS), managing envelope encryption for user roles, keys, and master-key derivation (Argon2id).
- **`mitm_scheduler-server`**: The orchestration engine. Handles chron-jobs, prevents concurrent executions, and writes system/audit logs to the database. Listens for `ExecuteJob`, `StopJob`, and `UpdateJobs` commands via IPC.

## Architecture

All microservices communicate via JSON messages over Unix Domain Sockets (UDS). The central communication hub is the PostgreSQL database, managed efficiently via `sqlx` connection pools.

### C4 System Landscape Diagram

```mermaid
C4Context
    title System Landscape Diagram for MitM-2 Data Aggregator

    Person(admin, "Administrator", "Manages the data pipeline, monitors jobs")

    System_Ext(sources, "Source Systems", "Oracle, PostgreSQL, CSV, Kafka, APIs")
    System_Ext(target_saas, "Target SaaS Solutions", "Cority, Apigee, etc.")

    System(mitm, "MitM-2 System", "Secure, decoupled data aggregator that collects, encrypts, and transmits PII data")

    Rel(admin, mitm, "Configures and monitors", "HTTPS / Admin UI")
    Rel(mitm, sources, "Collects data from", "SQL, Kafka, REST")
    Rel(mitm, target_saas, "Transmits JSON payloads to", "HTTPS / REST")
```

### C4 Container Diagram

```mermaid
C4Container
    title Container Diagram for MitM-2 Core Layer

    %% 1. Reihe
    Person(admin, "Administrator", "System operator")
    System_Ext(cpp_frontend, "Admin Frontend", "C++ UI for system management")

    %% 2. Reihe
    Container_Boundary(c1, "MitM-2 Core Layer (Rust)") {
        Container(http_server, "HTTP Server", "Rust, Axum", "API Gateway providing REST JSON:API endpoints")
        Container(iam_server, "IAM Server", "Rust", "Manages identity, access, and envelope encryption")
        Container(scheduler, "Scheduler Server", "Rust", "Orchestration engine for jobs and logging")
    }

    %% 3. Reihe
    System_Boundary(storage, "Storage") {
        ContainerDb(db, "PostgreSQL", "Relational Database", "Central storage for jobs, logs, and encrypted data")
        ContainerDb(fs, "Filesystem", "File Storage", "Local file buffering and extracted data")
    }

    Rel(admin, cpp_frontend, "Uses", "HTTPS")
    Rel(cpp_frontend, http_server, "Makes API calls to", "JSON/REST")
    
    Rel(http_server, iam_server, "Verifies auth & fetches keys", "UDS (JSON)")
    Rel(http_server, scheduler, "Sends commands (Execute/Stop)", "UDS (JSON)")
    
    Rel(http_server, db, "Reads/Writes data", "TCP/SQLx")
    Rel(iam_server, db, "Reads/Writes roles", "TCP/SQLx")
    Rel(scheduler, db, "Writes audit logs", "TCP/SQLx")
    Rel(scheduler, fs, "Reads/Writes files", "I/O")
```

- **Security**: The `MASTER_KEY` is provided at startup via environment variables and is never persisted. PII data and roles are protected using AES-256-GCM envelope encryption.
- **API Standard**: The `mitm_http-server` strictly adheres to the JSON:API specification for all error messages and data structures.

## Build

You can build all core components as statically linked binaries using the provided build script:

```bash
./build.sh
```

The script uses the `x86_64-unknown-linux-musl` target and drops the compiled binaries into the `../bin/` directory.
