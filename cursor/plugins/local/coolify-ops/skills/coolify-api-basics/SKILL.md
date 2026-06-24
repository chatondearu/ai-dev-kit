---
name: coolify-api-basics
description: Operate Coolify through API-first workflows with safe token handling.
---

# Coolify API basics

## Use when

- You need to query apps, deployments, or server state in Coolify.
- You need to trigger a deployment from automation.

## Required environment

- `COOLIFY_BASE_URL`
- `COOLIFY_TOKEN`

## Workflow

1. Validate access using `/api/v1/version`.
2. Resolve target app UUID from application list.
3. Trigger deployment.
4. Poll deployment status and logs.
5. Return concise summary and next action.

## Guardrails

- Never print raw token values.
- Prefer read-only endpoints before mutating actions.
- Include retry/backoff for transient `429/5xx`.
