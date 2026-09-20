# mitm_common

This is the shared library for the MitM-2 core layer. It contains common types, IPC structures, and configuration logic used across all microservices (HTTP, IAM, Scheduler).

## Features

- **Configuration Loading**: Loads configurations (e.g. database credentials) via the custom `.enc` file format, gracefully overridden by Environment Variables.
- **IPC Schemas**: Defines the strongly-typed JSON structures for Unix Domain Socket communication (`SchedulerRequest`, `AuthResponse`, etc.).
- **Shared Errors**: Standardized error handling.

## Usage

This library is not meant to be run standalone. It is linked as a workspace dependency in the other microservices.
