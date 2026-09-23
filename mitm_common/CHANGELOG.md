# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-09-23

### Fixed
- **Security/Reliability**: Added length validation to `payload_nonce` in `envelope_decrypt` to return an `Err` instead of crashing the tokio worker thread with an assertion panic from the `generic-array` crate.

## [0.1.1] - 2026-09-23

### Changed
- Added detailed error logging to `config::load_config` when decryption or parsing fails, to help with debugging.

## [0.1.0] - 2026-09-20

### Added
- Initial workspace crate for shared types and IPC models.
- `config::load_config` module for loading `.enc` files and environment variables.

### Fixed
- **Issue #1**: Fixed config precedence. Environment variables now correctly override values defined in the `.enc` configuration files.
