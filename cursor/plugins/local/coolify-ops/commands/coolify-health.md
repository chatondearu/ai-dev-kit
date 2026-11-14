---
name: Coolify Health
description: Verify Coolify CLI context connectivity and authentication.
---

Check the configured Coolify context:

```bash
coolify context verify
coolify context version
```

If this fails:

- Run `coolify-env-sync` after exporting `COOLIFY_BASE_URL` and `COOLIFY_TOKEN`.
- Or `coolify context add -d -f <name> "$COOLIFY_BASE_URL" "$COOLIFY_TOKEN"`.
- Validate token scope in Coolify API tokens.
- Check network/TLS from your runtime environment.
