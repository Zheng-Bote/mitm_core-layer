# mitm_scheduler-server

The Scheduler Server is the orchestration engine of the MitM-2 system. It manages cron-jobs, ensures that jobs do not run concurrently, spawns the respective collector/delivery processes, and writes centralized system and audit logs.

## Architecture & Components

### C4 Component Diagram

```mermaid
C4Component
    title Component Diagram for MitM-2 Scheduler Server

    Container_Ext(http_server, "HTTP Server", "Rust", "Sends manual execution commands")
    ContainerDb_Ext(db, "PostgreSQL", "SQL", "Central Database")

    Container_Boundary(scheduler_container, "Scheduler Server") {
        Component(ipc_server, "IPC Server", "Unix Domain Sockets", "Listens for ExecuteJob, StopJob, UpdateJobs")
        Component(orchestrator, "Cron Orchestrator", "Rust", "Manages cron schedules and job queues")
        Component(job_runner, "Job Runner", "Rust", "Spawns collector processes and manages state")
        Component(db_layer, "Database Layer", "sqlx", "Records audit logs and job history")
    }

    Rel(http_server, ipc_server, "Sends commands to", "UDS/JSON")
    
    Rel(ipc_server, orchestrator, "Updates schedules / Triggers jobs")
    Rel(orchestrator, job_runner, "Dispatches jobs to")
    
    Rel(job_runner, db_layer, "Updates status & writes logs")
    Rel(orchestrator, db_layer, "Reads configurations")

    Rel(db_layer, db, "Executes queries", "TCP/SQLx")
```

## Configuration

The Scheduler Server relies on the following environment variables:

- `MITM_DB_URL` (optional): PostgreSQL Connection String. Overrides the `.enc` config.
- `MASTER_KEY` (required): Passed via IPC to the HTTP Server during startup handshakes if needed, though primarily managed by IAM.
- `SCHEDULER_SOCKET_PATH` (optional): Override the Unix Domain Socket path (defaults to `/tmp/mitm_scheduler.sock`).

## Execution

```bash
cargo run -p mitm-scheduler-server
```
