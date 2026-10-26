# Tailnet HTTP API (cdamuxd)

When `daemon.listen_tailnet = true`, `cdamuxd` binds HTTP **only** on the machine’s
Tailscale IPv4 address (via `tailscale ip -4`). It does not listen on `0.0.0.0`.
Set `daemon.tailnet_port` (default `7480`) in `~/.config/cdamux/config.toml`.

For tests or dev without Tailscale, set `CDAMUX_TAILNET_BIND_ADDR` to a local
address (e.g. `127.0.0.1`).

## Endpoints

| Path | Method | Description |
| ---- | ------ | ----------- |
| `/v1/health` | GET | Daemon version and whether tmux is reachable |
| `/v1/sessions` | GET | Grouped session list (same JSON as `cdamux list --json`) |
| `/v1/sessions/:id/pane` | GET | Captured pane text (bounded tail) |
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

curl -sS "$CDAMUX_API/v1/health" | jq .
curl -sS "$CDAMUX_API/v1/sessions" | jq .
curl -sS "$CDAMUX_API/v1/sessions/c-myapp/pane" | jq .
curl -sS -X POST "$CDAMUX_API/v1/sessions/c-myapp/send" \
  -H 'Content-Type: application/json' \
  -d '{"line":"status","enter":true}' | jq .
```

## Security

v0.2: no application-level auth; exposure is limited by binding to the tailnet
interface only. Do not enable on hosts where the tailnet is untrusted.
