---
name: Coolify Health
description: Verify Coolify API connectivity and authentication.
---

Check Coolify API availability with:

```bash
curl -sS -H "Authorization: Bearer $COOLIFY_TOKEN" \
  "$COOLIFY_BASE_URL/api/v1/version"
```

If this fails:

- Validate `COOLIFY_BASE_URL`.
- Validate token scope in Coolify API tokens.
- Check network/TLS from your runtime environment.
