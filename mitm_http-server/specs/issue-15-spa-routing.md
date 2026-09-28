---
name: Feature Specification (Spec Kit)
about: Propose a new feature or rule.
title: "Feature: SPA Routing and Dynamic Tera Templates"
labels: "feature, spec-kit"
assignees: ""
---

## Feature Intent

To prepare the HTTP Server for a future Angular Single Page Application (SPA), the root routing mechanism must be restructured. The server must serve dynamic Tera templates under a dedicated namespace (`/template/:name`) and use a fallback mechanism (`tower_http::services::ServeDir`) to return `index.html` for any unmatched routes, allowing the Angular client-side router to handle deep linking (e.g., refreshing on `/dashboard`).

## Requirements (EARS Syntax)

1. The HTTP Server shall render dynamic Tera templates exclusively under the `GET /template/:name` endpoint.
2. The HTTP Server shall serve static assets (JS, CSS, images) from the `html/public` directory via `ServeDir`.
3. When the HTTP Server receives a request for an unmatched route (404), it shall return the static `index.html` instead of an error, enabling SPA client-side routing.
4. If a Tera template specified in `/template/:name` does not exist, the HTTP Server shall return a `404 Not Found` error.

## Scope

- **Component:** `core-layer/mitm_http-server`
- **Modules affected:** `src/handlers/mod.rs` (Routing logic, index handler)

## SpecDD Architecture Alignment (Drift Control)

Please confirm that this feature respects the global `mitm-2` constraints defined in `.sdd` files:

- [x] **Architecture:** The layered architecture is maintained (no direct bypass from Collector to Delivery).
- [ ] **Architecture:** Feature affects architecture: SpecKit feature forces update of the SpecDD .sdd
- [x] **Security:** Envelope Encryption (AES-GCM) is NOT bypassed for PII data.
- [x] **Data Model:** Core PostgreSQL schemas remain intact (feature-specific tables are allowed).
- [x] **Standards:** SPDX headers, English documentation, and independent `go.mod` per layer will be maintained.

## Acceptance Criteria

- [ ] `GET /template/index` renders the `index.html` via Tera.
- [ ] `GET /` serves the static `index.html`.
- [ ] `GET /non-existent-path` returns the static `index.html` (SPA fallback).
- [ ] CHANGELOG.md and README.md are up-to-date.
