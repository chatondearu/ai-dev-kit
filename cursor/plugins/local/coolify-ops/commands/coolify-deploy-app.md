---
name: Coolify Deploy App
description: Trigger deployment for a Coolify application by UUID.
---

Trigger a deployment (replace `<app-uuid>`):

```bash
coolify deploy uuid <app-uuid>
# optional: coolify deploy uuid <app-uuid> --force
```

Then follow status:

1. Capture the deployment UUID from the response.
2. Poll: `coolify deploy get <deployment-uuid>` (every 2–3 seconds).
3. Stop on a terminal status (`finished`, `failed`, `canceled`, etc.).
4. Fetch logs: `coolify app deployments logs <app-uuid>` (see `--help` for flags).
5. Return a concise summary.
