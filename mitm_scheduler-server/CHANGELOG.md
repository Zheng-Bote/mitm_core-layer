# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-09-20

### Added
- **Issue #4**: Implemented `ExecuteJob` IPC integration to orchestrate jobs dynamically via the API Gateway.
- **Issue #5**: Implemented full Job Lifecycle tracking. Added `StopJob` IPC handling, mapping active PIDs in `JobOrchestrator`, and utilizing `SIGTERM` for process termination.
- **Issue #13**: Ported missing backward-compatibility features from the legacy Go scheduler:
  - Added `restart_on_exit` loop functionality to restart crashed child jobs.
  - Added 5-second `SIGKILL` timeout escalation for frozen tasks.
  - Integrated real-time PID synchronization with PostgreSQL upon process spawn.
  - Added Graceful Shutdown logic (trapping `SIGINT`/`SIGTERM`) to terminate active children on exit.
