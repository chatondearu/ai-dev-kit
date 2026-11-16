---
name: cda-browser
description: >-
  Open a URL or local HTML in the human's Zen browser on neo-nix (niri).
  Use when handing a prototype, DESIGN.md loop, local docs, or any page the
  human should see — not for scraping (use fetch/curl).
---

# Open Zen

On this machine the browser is **Zen** (`zen`, niri `app_id`: `zen`). Firefox is
installed but not the daily driver.

## Do

```bash
bash ~/.cursor/skills/cda-browser/open.sh <url-or-path>
```

The script prefers `zen --new-tab`, spawned via niri when available. A relative
path becomes `file://`. Done when Zen is running with that URL (or the script
printed a fallback).

Hand the human the **full URL** in chat too (`file://…` or `http://127.0.0.1:…`).
