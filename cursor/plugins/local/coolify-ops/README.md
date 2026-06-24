# coolify-ops

Cursor plugin scaffold to operate Coolify with an API-first approach.

## Included components

- `commands/`
  - `coolify-health.md`
  - `coolify-list-apps.md`
  - `coolify-deploy-app.md`
  - `coolify-deploy-and-watch.md`
- `skills/`
  - `coolify-api-basics/SKILL.md`
- `examples/`
  - `coolify-client.ts` (TypeScript API client with deploy polling/retries)
  - `deploy-and-watch.ts` (CLI runner executable with `tsx`)

## Quick start

1. Generate a Coolify API token from `/security/api-tokens`.
2. Export environment variables:
   - `COOLIFY_BASE_URL`
   - `COOLIFY_TOKEN`
3. Use command templates from `commands/` to drive workflows.

### CLI runner example

```bash
COOLIFY_BASE_URL="https://coolify.example.com" \
COOLIFY_TOKEN="***" \
tsx examples/deploy-and-watch.ts --app <app-uuid>
```

## Notes

- This scaffold is intentionally minimal.
- It is safe to extend with rules, agents, hooks, and MCP server wiring.
