---
name: coolify-api-basics
description: Operate Coolify with the official coolify CLI and safe token handling.
---

# Coolify CLI basics

## Use when

- You need to query apps, deployments, or server state in Coolify.
- You need to trigger a deployment from automation or an agent session.

## Prerequisites

- `coolify` on `PATH` (`nix build .#coolify-cli` or `cda.aitools.coolify.enable`)
- Host env for bootstrap: `COOLIFY_BASE_URL`, `COOLIFY_TOKEN` (optional
  `COOLIFY_CONTEXT`) — then `coolify-env-sync` once

## Workflow

1. `coolify context verify` (or `coolify context version`)
2. `coolify app list` — resolve target UUID
3. `coolify deploy uuid <app-uuid>`
4. Poll with `coolify deploy get <deployment-uuid>` / `coolify app deployments list <app-uuid>`
5. Logs: `coolify app deployments logs <app-uuid> …`
6. Return a concise summary and next action

## Guardrails

- Never print raw token values.
- Prefer read-only commands (`list`, `get`, `verify`) before mutating (`deploy`, `delete`).
- Prefer the CLI over MCP for Coolify ops in this kit.
