---
name: technical-docs-sync
description: Maintains English technical documentation under /doc so it mirrors the codebase and runtime config (Docker, OpenAPI, Pydantic). Use when updating or reviewing docs, syncing documentation after code changes, refactoring APIs, adding endpoints, changing environment variables, Docker, deployment, or when the user asks to fix outdated markdown documentation.
---

# Technical documentation sync

## Principles

- **Source of truth**: application code, schemas, and runtime configuration actually used (including Docker Compose and Dockerfiles). Documentation only reflects them; never invent behavior.
- **Location**: user-facing technical docs live in **`doc/`** at the repository root (create it if missing). **Language: English only** for all `doc/**/*.md` files.
- **README**: keep the root README as a short overview plus **one clear link** to the doc index, e.g. `Full documentation: [doc/README.md](doc/README.md)`.

## Analysis order (efficient)

1. **Entry points**: app bootstrap, routers, CLI (`main.py`, `app/`, etc.).
2. **Public contracts**: request/response models, status codes, response content types, file formats.
3. **Deployment**: `Dockerfile`, `docker-compose.yml`, health checks, ports—values must match files, not assumptions.
4. **Environment variables**: document only what code reads (`os.getenv`, settings modules). If compose declares vars the app does not read yet, note that gap accurately (e.g. "set in Compose for future use" vs "consumed by …").
5. **Optional verification**: if the app exposes OpenAPI (`/openapi.json`), use it to align paths and schemas.

## Doc structure (adapt to project)

Prefer:

- `doc/README.md` — index and links to other pages.
- `doc/architecture.md` — components and data flow (add Mermaid only when the system is non-trivial).
- `doc/api.md` — HTTP API (methods, payloads, examples with real paths).
- `doc/deployment.md` — Docker, compose, production notes.
- `doc/configuration.md` — env vars, voice paths, feature flags.

Use **`doc/reference.md`** only for long tables or verbose material (progressive disclosure).

## Quality rules

- **`curl` examples** must use real paths, ports, and JSON fields from code.
- **Remove or update** stale sections; do not leave contradictory instructions between README and `doc/`.
- **Diff-first** (when the user provides a diff or file list): prioritize touched areas before broad rewrites.

## Relationship to AGENTS.md / internal specs

- Do not duplicate full user documentation inside agent-only specs. Prefer linking **`doc/`** from `AGENTS.md` when operators need the same facts.

## Additional material

For section templates and a longer checklist, see [reference.md](reference.md).
