# Feature: Port Configuration & Crypto Logic to Rust (mitm_common)

## Goal
Implement the central configuration loading and cryptographic primitives (AES-256-GCM + Argon2id) in the `mitm_common` shared Rust crate. This will replicate the behavior developed in the Go `mitm_scheduler`, ensuring all three Rust servers (HTTP, IAM, Scheduler) can load their configurations securely using the same strict precedence rules.

## Context
The legacy Go codebase used a 4-step config loading priority (CLI Param -> Default File -> ENVs -> Hardcoded Defaults). The configuration files (`config.enc`) are encrypted. We must provide the same functionality in Rust.

## Tasks

### 1. Implement Crypto Module
- [ ] Create `mitm_common/src/crypto.rs`.
- [ ] Implement Argon2id key derivation matching Go defaults (Time=3, Memory=64MB, Threads=1, Length=32).
- [ ] Implement AES-256-GCM encryption/decryption using the `aes-gcm` crate.

### 2. Implement Config Module
- [ ] Create `mitm_common/src/config.rs`.
- [ ] Define the `DBConfig` and `DBConnectionConfig` struct matching the JSON schema (using `serde`).
- [ ] Implement `load_encrypted_config` that reads and decrypts `config.enc`.
- [ ] Implement `load_from_env` that reads from `MITM_*` environment variables with correct defaults.
- [ ] Implement the `load_config` orchestrator enforcing the priority:
  1. Commandline parameter (`cli_param`).
  2. Default config files (`<binary_dir>/config.enc` or `cfg/config.enc`).
  3. Environment variables (triggering on `MITM_DB_HOST`).
  4. Internal defaults.

### 3. Expose API and Test
- [ ] Export modules in `mitm_common/src/lib.rs`.
- [ ] Write inline unit tests for crypto and config fallback behavior.
