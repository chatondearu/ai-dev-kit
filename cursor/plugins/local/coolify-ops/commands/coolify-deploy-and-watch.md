---
name: Coolify Deploy And Watch
description: Deploy by UUID, poll status, and show recent logs via the Coolify CLI.
---

Deploy and watch:

```bash
coolify deploy uuid <app-uuid>
# poll:
coolify deploy get <deployment-uuid>
coolify app deployments list <app-uuid>
coolify app deployments logs <app-uuid>
```

Print a short summary:

- deployment UUID
- terminal status
- duration (if available)

Constraints:

- Poll interval: ~2 seconds
- Timeout: ~10 minutes
- Prefer CLI retries; do not fall back to inventing an MCP workflow
