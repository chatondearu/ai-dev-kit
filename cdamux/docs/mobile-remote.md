# Mobile and remote access (tailnet)

Use your phone or another device on the same Tailscale tailnet — no native cdamux app in v0.

## Browser dashboard (recommended on tailnet)

1. On the host running tmux sessions, enable the daemon HTTP listener in
   `~/.config/cdamux/config.toml`:

   ```toml
   [daemon]
   listen_tailnet = true
   tailnet_port = 7480
   ```

2. Start or restart `cdamuxd`.
3. On the phone, open `http://<tailscale-ip>:7480/` (same IP as
   `tailscale ip -4` on the host).

The static UI lists sessions, shows a pane tail, and can send lines via the JSON API
([tailnet-api.md](tailnet-api.md)). Same-origin only — no extra CORS setup.

## SSH + Mosh (CLI attach)

When you need full terminal attach (ratatui, `cdamux attach`, or raw tmux):

1. Install Mosh on the host and client (`mosh user@100.x.y.z`).
2. From the Mosh session, run `cdamux list` and `cdamux attach <session>` (or
   `tmux attach -t <name>`).

Mosh survives Wi‑Fi handoff; the browser UI is lighter for quick checks and sends.

## Security

- Tailnet bind only — do not expose port `7480` on the public internet.
- v0.2 has no app-level auth; trust your tailnet ACLs.
