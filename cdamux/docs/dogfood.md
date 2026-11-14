# cdamux dogfood checklist

Run on maintainer NixOS/Linux host with tmux + Ghostty.

## Prerequisites

- `tmux` ≥ 3.2
- Rust toolchain (`nix develop` at ai-dev-kit root, or system `cargo`)
- `cursor-agent` or configured agent in `~/.config/cdamux/config.toml`

## Config bootstrap

```bash
mkdir -p ~/.config/cdamux
cp /hdd/dev/chatondearu/ai-dev-kit/cdamux/config/cdamux.example.toml \
  ~/.config/cdamux/config.toml
# Edit paths under [[projects.roots]] for your machine.
```

Example roots in the template: `/hdd/dev`, `/home/chaton/etc/nixos`.

## Build

```bash
cd /hdd/dev/chatondearu/ai-dev-kit/cdamux
cargo build
cargo test
```

## Daemon (optional, tailnet HTTP API only)

```bash
cargo run -p cdamuxd   # requires daemon.listen_tailnet = true
# `cdamux list` and the TUI read tmux directly; no socket or cache.
```

## tmux popup bind (kit target)

```tmux
bind-key -n M-p display-popup -E -w 80% -h 75% "cdamux"
```

Install the `cdamux` binary on `PATH` (e.g. `cargo install --path crates/cdamux-cli`).

## tmux sidebar bind (example, not installed)

```tmux
bind-key -n M-s run-shell "cdamux sidebar --client '#{client_name}'"
# left side, wider:
# bind-key -n M-s run-shell "cdamux sidebar --left --width 40 --client '#{client_name}'"
```

`--client '#{client_name}'` makes the bind target the client that pressed it; without
it tmux picks its most recent client, wrong with two clients attached.

## Clickable PR / issue links (OSC 8)

The Fleet badges (`PR#12`, `#7`) and the Review header are OSC 8 hyperlinks. Inside tmux
they only reach the terminal with the `hyperlinks` feature (tmux ≥ 3.4); add it to your own
config (cdamux never edits it):

```tmux
set -as terminal-features ',*:hyperlinks'
```

Without it (or in a terminal without OSC 8) the labels are plain text; `o` still opens the
PR, else the issue, in the browser.

## Scenarios (v0)

- [ ] Configure two `[[projects.roots]]` paths
- [ ] `cdamux doctor` passes (tmux, config, agent ids)
- [ ] `cdamux spawn --agent cursor --path <git-repo>` creates tmux session `c-<basename>`
- [ ] `cdamux spawn -i` — skim project tree + agent
- [ ] `cdamux list` shows `name`, `path`, `state` columns
- [ ] `cdamux --pick` from `tmux display-popup -E` closes popup after selection
- [ ] `cdamux` (no args) opens ratatui dashboard; `q` quits; Enter attaches; `n` spawn (`-i` skim)
- [ ] `cdamux sidebar` (bind above): j/k on a session row switches the client and the pane follows; `Enter` focuses the session pane; `q` closes; with a second client attached its view never changes
- [ ] Alt+p popup runs full dashboard (`cdamux`); `--pick` optional for skim-only
- [ ] Repo at `org/nested/repo` appears when `discover = "git_only"` (`cdamux projects`)
- [ ] `list --group-by cwd` prints `##` group headers (text mode)
- [ ] `list --json --group-by project` returns grouped JSON
- [ ] `cdamux send <session> <message>` delivers text without attaching (targets session **active window / active pane** via `tmux send-keys -l -t`)
- [ ] `cdamux send <session> --enter` (or message + `--enter`) submits with Enter
- [ ] Spawn a claude session, `tmux kill-session` it, `cdamux restore --all` brings it back on the same conversation (`--resume <id>`)
- [ ] Spawn `cdamux orch new` + a worker, kill both hosts, `cdamux restore <orch>` brings orch then workers back (resume ids + `@cdamux_parent` kept)
- [ ] The killed session shows as a dimmed `◌ … dormant` row in the Fleet; `Enter` restores it, `x` `x` forgets it
- [ ] `cdamux restart --all` from a shell pane of an agent session: other idle agents restart first, a `running` one is skipped, the current one restarts last (see `restart.log`)

## After a kit update

Agents keep running the old skills, hooks and config until they restart. Restart them
on their own conversations:

```bash
cdamux restart --all          # skips agents mid-turn (@cdamux_state running)
cdamux restart --all --force  # also restarts busy ones
cdamux restart c-myapp        # one session
```

The home session is never killed. Sessions with no recorded conversation id restart
fresh (`fresh (no id)`).

## Send (without attach)

```bash
# Literal line to the active pane of session c-myapp
cdamux send c-myapp "fix the flaky test in auth"

# Same, then press Enter (e.g. submit a prompt)
cdamux send c-myapp "run cargo test" --enter

# Enter only
cdamux send c-myapp --enter
```

Targeting matches tmux: `-t <session>` uses that session's current window and active pane.
Core helper: `cdamux_core::send_to_session` (for future `cdamuxd` HTTP).

## Hunk review → agent

```bash
# From repo cwd of session (tmux session_path) or explicit --path
cdamux review c-myapp

# Override repo root when session_path is wrong
cdamux review c-myapp --path /path/to/git-repo

# Custom lead line (hunks appended after)
cdamux review c-myapp --instruction "Focus on error handling in these hunks."
```

TUI keybindings:

| Key | Action |
| --- | --- |
| `j` / `k` or arrows | Move selection |
| `Space` | Toggle hunk |
| `a` | Select all / clear all |
| `Enter` | Send composed prompt to session (`send-keys` + Enter) |
| `q` / `Esc` | Quit without sending |

Diff source: `git diff` plus `git diff --cached` in the resolved repo directory.

## Large `$CDAMUX_HOME` (Alt-P / Fleet)

Dashboard open indexes `$CDAMUX_HOME/runs` and `agents` once per dir fingerprint
(shared across projects). Hundreds of stale finished runs still cost that first
parse — periodically prune with `cdamux state gc` (see `docs/architecture.md`).

## Mission control dashboard (v2)

- [ ] `cdamux` shows project → session tree with status icons (○ ● ◆)
- [ ] Preview sidecar updates when selecting a session
- [ ] Foam tab lists `foam/**/*.md` for the session’s git toplevel / project root (empty/missing foam → clear empty state; CLI `cdamux notes` unchanged)
- [ ] Foam tab: `[`/`]` select a listed `.md` → sidecar shows rendered preview under the ▸ row (`── preview ──`; long lists stay windowed so preview stays visible; oversize files truncate with a visible cue)
- [ ] Preview text mentioning `foam/...` deep-links into the Foam tab (selects the row; missing path → empty/error; non-foam ignored)
- [ ] Review tab: `Ctrl+R` or Enter opens hunk review TUI for selected session
- [ ] Composer (`Space`) sends to session without attach
- [ ] `n` spawn modal: running / discovered / custom path; optional shell-only and pane split
- [ ] App tabs `1`–`5`: Fleet, Projects, Sessions, Agents, Settings (`S` saves config)
- [ ] `w` waiting filter; `F` fold/unfold all; `G` cycle `group_by` (tree layout); `?` help overlay
- [ ] Fork a session (`f`): the fork nests under its source (`h`/`l` fold); `ORCH_TMUX=1 orch.sh ...` on a throwaway repo: worker session `orch-<run>-<id>` appears, pane preview is live (throwaway tmux server only)
- [ ] `./install.sh --cdamux-hooks` (temp HOME), start a Claude sub-agent inside tmux: a sub-agent row appears under its session ([subagents.md](subagents.md))
- [ ] `g` full-screen Graph view: drawn DAG, `j`/`k` select node, detail panel, Enter jumps to a session named like the node, `Tab` next run of the same session (header `i/n`), `g`/Esc back
- [ ] Two sessions in one repo each run `graph-node.sh init` (plus one `orch.sh` outside tmux): each `◈ run` row sits under its own session, the headless one under the project; selecting each session shows its own graph; the project sidecar lists the live runs and recent ones
- [ ] Fleet footer shows `m` rename / `f` fork / `x` kill; `?` lists the same keys grouped
- [ ] Below 100 columns the sidecar stacks under the tree; the footer drops items to fit but keeps `? help · q quit`; status messages sit on the row under it
- [ ] Fleet session rows show agent tag (`@cdamux_agent`); missing option shows `—` (same as Preview chrome)

### Settings tab (`5`)

The old letter shortcuts (`a` attach, `g` group_by, `+`/`-` preview, `t` tailnet) are gone: focus a row instead.

- [ ] `j`/`k` moves focus; focused row is highlighted with `›`
- [ ] Enums (`attach_mode`, `group_by`, `discover`): `←`/`→` or `Enter` cycle
- [ ] `listen_tailnet`: `Space` / `Enter` toggle
- [ ] Numbers (`preview_max_lines`, `tailnet_port`, `idle_seconds_for_needs_input`, `max_depth`): `Enter` opens an input; non-numbers rejected
- [ ] Roots: `a` add (label + absolute path, `Tab` switches field), `Enter` edit, `d` delete (`y` confirm); empty / relative path rejected
- [ ] Dirty `*` next to config path; `S` validates (`tailnet_port` ≠ 0, `max_depth` ≥ 1) then saves; restart sees the new roots
- [ ] Rename modal (`m`, Fleet) types and backspaces like the Settings inputs; non-ASCII chars ignored

Form pieces (Select, Toggle, TextInput, FieldList, text modal) live in
`crates/cdamux-cli/src/widgets.rs`; Settings and the rename modal consume them (#79).

| Key | Action |
| --- | --- |
| `j`/`k` (`↓`/`↑`) | Move the selection (always, whatever the sidecar tab) |
| `l`/`→` · `h`/`←` | Expand · collapse the selected project or parent session |
| `Tab` | Cycle Preview / Foam / Review / Orch graph |
| `PgUp`/`PgDn` · `[`/`]` | Scroll the preview into older / newer lines (page · 3 lines), clamped to the capture |
| `Ctrl+R` | Open review TUI |
| `n` | Spawn modal |
| `m` | Rename selected session |
| `f` | Fork selected session (same cwd, new name) |
| `x` | Kill selected session |
| `w` | Toggle waiting-for-input (◆) filter on Fleet tree |
| `F` | Expand or collapse all Fleet groups |
| `g` | Full-screen Graph view of the selected row's orch run (drawn DAG + node detail); inside it `Tab` cycles the session's runs, newest first |
| `G` | Cycle `sessions.group_by` (differs from agent-manager group tree) |
| `?` | Keyboard help (Esc to close) |
| `1`–`5` | Fleet / Projects / Sessions / Agents / Settings |

## Orchestration loop (live, ≤ 15 min)

Real Claude orchestrator + 2 workers on a **scratch git repo** (not a repo you care
about): proves what the stub-based test ([`orch_loop_tmux.rs`](../crates/cdamux-core/tests/orch_loop_tmux.rs))
cannot — real clients, real hooks, real status transitions. Needs `install.sh --cdamux-hooks`
and a `cdamux` binary on `PATH`.

```bash
git init -b main /tmp/dogfood && cd /tmp/dogfood && git commit --allow-empty -m init
mkdir -p /tmp/specs
printf 'Create hello.txt containing "hi". Then ask me which greeting file name you should use BEFORE writing it.' > /tmp/specs/a.md
printf 'Create bye.txt containing "bye", commit it, then report done.' > /tmp/specs/b.md
```

| # | Step (≈ min) | Do | Expect |
| - | ------------ | -- | ------ |
| 1 | Orchestrator (1) | `cdamux orch new --path /tmp/dogfood`, attach | Claude boots and announces skill `cda-orchestrator`; Fleet shows an orch glyph row |
| 2 | Spawn (2) | In the orchestrator: ask it to run `cdamux worker spawn a --prompt-file /tmp/specs/a.md` and `… b --prompt-file /tmp/specs/b.md` | Two `c-…` worker sessions nest under the orchestrator; `cdamux worker ls` lists both; each first prompt starts with the worker preamble, then the **whole** spec |
| 3 | Peek (1) | `cdamux worker peek a` | Last lines of a's pane, spec visible |
| 4 | Ask (3) | Worker `a` reaches its question and runs `cdamux msg send --kind ask` | Worker row `blocked` + `?`; orchestrator row `✉1`; orchestrator pane shows a `📨 N messages` nudge (idle) or digests it at its next prompt (busy). `nudged` or `queued`, never lost |
| 5 | Answer (2) | Orchestrator answers (`cdamux msg send --to a --kind answer "use greeting.txt"`, or `answer` in the Fleet composer) | `blocked` and `?` clear; text lands in a's pane (`pasted`) or at its next prompt (`queued`); `✉` on the worker is 0 once seen |
| 6 | Done (2) | Worker `b` finishes (`--kind done`), then `a` | Rows turn `done`; orchestrator `✉` counts up, digest lists `[done] from …` lines; `cdamux msg read --ack` (as orchestrator) zeroes `✉` |
| 7 | Review (3) | In the orchestrator: ask it to review each worker with `gh pr diff <n>` (or `git -C <worktree> diff main`) before accepting | Orchestrator reads the diff itself and answers `accept` / `fix` to the worker; it never merges |
| 8 | Cleanup (1) | `tmux kill-session` both workers and the orchestrator, `rm -rf /tmp/dogfood /tmp/specs` | No stray `c-*` sessions; `cdamux state gc` if the Fleet still lists dormant rows |

Pass = every "Expect" holds. Any miss: file an issue (`area:cdamux`, parent #315) with the
step number and a `cdamux worker peek` / `cdamux state get agents <id>` excerpt.

## Regression vs skzv/ccmux

- [ ] Can still use skzv/ccmux in parallel on same tmux server (document session name collisions on `c-*`)
- [ ] `aitools` templates unchanged until explicit switch

## Notes

Record frictions in GitHub issue [#40](https://github.com/chatondearu/ai-dev-kit/issues/40) comments.

## Sign-off (issue #48)

| Evidence | Source |
| -------- | ------ |
| `cargo test` / `cargo build` | GitHub Actions on PRs #49–#52 and closure PR for #48 |
| Merged v0.1 scope | #41–#44 → PRs #49, #51, #50, #52 |
| Tailnet API | `cdamux/docs/tailnet-api.md` + unit tests in `tailnet_api.rs` |
| Extract checklist | `cdamux/docs/extract-repo.md` validation table |

**Maintainer (interactive tmux host):** confirm the scenario checkboxes above on the
NixOS maintainer machine, then comment on [#48](https://github.com/chatondearu/ai-dev-kit/issues/48)
with `Dogfood sign-off: OK` or list blockers. Worker closure does not replace this step.

**v0 success metrics (PRD-39):**

- [ ] Spawn 3 projects from 2 roots; pick via popup; no stuck overlay
- [ ] Second host `curl` tailnet `/v1/health` when `listen_tailnet = true`
