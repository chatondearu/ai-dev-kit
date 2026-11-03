# Tailnet HTTP API (cdamuxd)

When `daemon.listen_tailnet = true`, `cdamuxd` binds HTTP **only** on the machine’s
Tailscale IPv4 address (via `tailscale ip -4`). It does not listen on `0.0.0.0`.
Set `daemon.tailnet_port` (default `7480`) in `~/.config/cdamux/config.toml`.

For tests or dev without Tailscale, set `CDAMUX_TAILNET_BIND_ADDR` to a local
address (e.g. `127.0.0.1`). Unspecified addresses (`0.0.0.0`, `::`) are refused.
When the address cannot be resolved (no `tailscale`, refused bind address) or bound,
`cdamuxd` logs the error and exits 1, so `Restart=on-failure` / launchd `KeepAlive`
retries it.

## Authentication

Set `CDAMUX_TAILNET_TOKEN` in the daemon environment to require every `/v1/*`
request to carry the header `X-Cdamux-Token: <token>`. Requests without it or
with a different value get `401 {"error":{"code":"unauthorized",...}}`. The
comparison is constant-time. Static dashboard assets (`/`, `/app.js`,
`/style.css`) stay public; the dashboard reads the token from
`localStorage.cdamux_token` (set it once in the browser console:
`localStorage.setItem("cdamux_token", "<token>")`).

When the variable is unset the API stays open to every tailnet peer and the
daemon logs `tailnet API without token` once at startup. Rotation: change the
variable and restart `cdamuxd`. The token is checked before the request body is read.

## Request rules

These apply to every `/v1/*` request, with or without a token:

| Rule | Rejection |
| ---- | --------- |
| `Host` must equal the bind `ip:port` (open the dashboard by IP, e.g. `http://100.x.y.z:7480/`, not a MagicDNS name) | `421` |
| `POST` must send `Content-Type: application/json` | `415` |
| Body at most 64 KiB | `413` |
| `send` `line` must not contain control characters (newline, tab, escape, …): one line per request | `400` |

The `Host` check blocks DNS rebinding; the JSON-only `POST` means a page on another
origin cannot reach `send` without a CORS preflight, and preflights (`OPTIONS`)
get `405`.

## Endpoints

| Path | Method | Description |
| ---- | ------ | ----------- |
| `/v1/health` | GET | Daemon version and whether tmux is reachable |
| `/v1/sessions` | GET | Grouped session list (same JSON as `cdamux list --json`) |
| `/v1/sessions/:id/pane` | GET | Captured pane text (bounded tail, plain text without ANSI escapes) |
| `/v1/sessions/:id/send` | POST | Send a literal line; optional Enter |

Session `:id` is the tmux session name (URL-encode special characters).

Errors return JSON: `{"error":{"code":"...","message":"..."}}`.

## Example config

```toml
[daemon]
listen_tailnet = true
tailnet_port = 7480
```

## curl examples

Replace `100.x.y.z` with your Tailscale IP (`tailscale ip -4`).

```bash
export CDAMUX_API="http://100.x.y.z:7480"
# When CDAMUX_TAILNET_TOKEN is set on the daemon, add: -H "X-Cdamux-Token: $CDAMUX_TAILNET_TOKEN"

curl -sS "$CDAMUX_API/v1/health" | jq .
curl -sS "$CDAMUX_API/v1/sessions" | jq .
curl -sS "$CDAMUX_API/v1/sessions/c-myapp/pane" | jq .
curl -sS -X POST "$CDAMUX_API/v1/sessions/c-myapp/send" \
  -H 'Content-Type: application/json' \
  -d '{"line":"status","enter":true}' | jq .
```

## Security

Exposure is limited by binding to the tailnet interface only (never `0.0.0.0`),
the request rules above, and the optional shared token. `POST /v1/sessions/:id/send` types into
agent shells: always set `CDAMUX_TAILNET_TOKEN` unless every tailnet peer is
trusted. `?repo=` paths are canonicalized and must resolve under a configured
project root; error bodies never echo the requested path.
