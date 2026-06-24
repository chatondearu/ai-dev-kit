---
name: Coolify Deploy App
description: Trigger deployment for a specific Coolify application.
---

Trigger a deployment (replace `<app-uuid>`):

```bash
curl -sS -X POST \
  -H "Authorization: Bearer $COOLIFY_TOKEN" \
  -H "Content-Type: application/json" \
  "$COOLIFY_BASE_URL/api/v1/deploy?uuid=<app-uuid>" | jq
```

Then poll deployment state with your preferred endpoint/workflow.

Suggested flow:

1. Trigger deployment and capture `deployment_uuid` from response.
2. Poll deployment status every 2-3 seconds.
3. Stop polling on terminal status (`finished`, `failed`, `canceled`).
4. Fetch deployment logs and return a concise summary.
