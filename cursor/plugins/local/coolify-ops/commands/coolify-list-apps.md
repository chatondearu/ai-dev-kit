---
name: Coolify List Apps
description: List applications from Coolify API.
---

List applications:

```bash
curl -sS -H "Authorization: Bearer $COOLIFY_TOKEN" \
  "$COOLIFY_BASE_URL/api/v1/applications" | jq
```

Filter by project or environment in your integration layer if needed.
