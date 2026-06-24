# Changelog

## 0.1.0

- Initial Coolify Cursor plugin scaffold.
- Added command templates for health check, app listing, and deploy.
- Added TypeScript Coolify API client example.

## 0.2.0

- Added `coolify-deploy-and-watch` command template.
- Extended TypeScript client with deployment status polling.
- Added retry/backoff for transient API failures.
- Added deployment logs endpoint helper.

## 0.3.0

- Added runnable `tsx` script: `examples/deploy-and-watch.ts`.
- Added CLI arg parsing and env validation for deploy watcher flows.
- Added JSON output contract for success/failure execution paths.
