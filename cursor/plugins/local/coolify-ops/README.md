# coolify-ops

Cursor plugin to operate Coolify via the **official CLI** (`coolify`).

Install the binary with the kit flake / companion (see `aitools/README.md`):

```bash
nix build .#coolify-cli
# or: cda.aitools.coolify.enable = true;
```

## Included components

- `commands/` — health, list apps, deploy, deploy-and-watch (CLI)
- `skills/coolify-api-basics/` — agent workflow for the CLI
- `examples/` — **legacy** TypeScript/curl helpers (not the primary path)

## Quick start

1. Create an API token in Coolify (`/security/api-tokens`).
2. Export host secrets (never bake the token into Nix):

   ```bash
   export COOLIFY_BASE_URL="https://coolify.example.com"
   export COOLIFY_TOKEN="***"
   coolify-env-sync   # writes ~/.config/coolify context
   ```

3. Run command templates under `commands/`.

## Notes

- Prefer the CLI over inventing or wiring an MCP for Coolify ops.
- Upstream CLI may expose `coolify mcp`; this kit still treats the CLI as the
  agent-facing interface.
- Token stays host-side (`COOLIFY_TOKEN` + `coolify-env-sync` / `coolify context`).
