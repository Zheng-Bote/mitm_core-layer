# mitm_iam-server

The IAM Server (Identity and Access Management) manages envelope encryption for user roles, cryptographic keys, and master-key derivation using Argon2id. It runs as a background daemon connected via Unix Domain Sockets (UDS).

## Architecture & Components

**Security Note**: The `MASTER_KEY` is provided at startup via environment variables and is never persisted. PII data and roles are protected using AES-256-GCM envelope encryption.

### C4 Component Diagram

```mermaid
C4Component
    title Component Diagram for MitM-2 IAM Server

    Container_Ext(http_server, "HTTP Server", "Rust", "Requests encryption/decryption and role checks")
    ContainerDb_Ext(db, "PostgreSQL", "SQL", "Central Database")

    Container_Boundary(iam_container, "IAM Server") {
        Component(ipc_server, "IPC Server", "Unix Domain Sockets", "Listens for IAM requests")
        Component(crypto_engine, "Crypto Engine", "AES-256-GCM / Argon2id", "Master key derivation and envelope encryption")
        Component(db_layer, "Database Layer", "sqlx", "Stores encrypted Data Encryption Keys (DEKs) and roles")
    }

    Rel(http_server, ipc_server, "Requests crypto ops via", "UDS/JSON")
    
    Rel(ipc_server, crypto_engine, "Routes payloads to")
    Rel(crypto_engine, db_layer, "Fetches encrypted keys/roles")
    
    Rel(db_layer, db, "Executes queries", "TCP/SQLx")
```

## Configuration

The IAM Server expects the following environment variables:

- `MITM_DB_URL` (optional): PostgreSQL Connection String. Overrides the `.enc` config.
- `MASTER_KEY` (required): The cryptographic master key for AES-GCM envelope encryption. Must be provided at startup via ENV and is never persisted.
- `IAM_SOCKET_PATH` (optional): Override the Unix Domain Socket path (defaults to `/tmp/mitm_iam.sock`).

## Execution

Run the server directly (ensure PostgreSQL is running):
```bash
MASTER_KEY="your-secure-key" cargo run -p mitm-iam-server
```
