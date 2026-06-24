---
name: Coolify Deploy And Watch
description: Trigger a deployment, poll status, and return recent logs.
---

Deploy and watch:

1. Trigger deployment for `<app-uuid>`.
2. Poll deployment status until complete.
3. Print a short summary:
   - deployment UUID
   - terminal status
   - duration
4. Fetch and show only the latest log tail.

Recommended constraints:

- Poll interval: 2 seconds
- Timeout: 10 minutes
- Retry on transient errors (`429`, `5xx`) with exponential backoff
