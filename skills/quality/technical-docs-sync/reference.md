# Technical docs sync — reference

## Doc index template (`doc/README.md`)

Use as a starting point; trim sections that do not apply.

```markdown
# [Product name] — documentation

## Contents

- [Architecture](architecture.md)
- [HTTP API](api.md)
- [Deployment](deployment.md)
- [Configuration](configuration.md)
```

## Page templates (snippets)

### API page

- Base URL / default port (from compose or app).
- Table: method, path, summary, auth if any.
- Request/response JSON with field types; link to Pydantic models by name.
- Example `curl` for each mutating or main read path.
- Error responses: HTTP codes actually raised in code.

### Deployment page

- Prerequisites (CPU/RAM if relevant).
- Build and run commands verbatim from repo.
- Volumes and exposed ports from compose.
- Health check endpoint if present.

### Configuration page

- Table: variable | where set | default | consumed by (file/symbol) | notes.
- Separate "declared in Docker only" from "read by application" when they differ.

## Sync checklist (after substantive code changes)

- [ ] Entry points and public routes match `doc/api.md` (and OpenAPI if used).
- [ ] Env vars table matches `getenv`/settings usage.
- [ ] Docker ports, volumes, healthcheck match compose/Dockerfile.
- [ ] Voice/asset paths and extensions match code (e.g. `.wav` listing logic).
- [ ] Root README still links to `doc/README.md` and is not a duplicate manual.
- [ ] Remove obsolete sections; no conflicting instructions across files.

## Optional: breaking changes

If the project maintains operator-facing changelog, add **`doc/CHANGELOG.md`** or a "Migration" section for breaking API or config changes (optional, project-specific).
