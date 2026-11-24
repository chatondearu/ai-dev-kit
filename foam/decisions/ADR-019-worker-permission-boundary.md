# ADR-019 — Worker permission boundary

- **Status**: accepted
- **Date**: 2026-11-23
- **Tags**: #security #tooling #cdamux #autonomy

## Context

The 2026-11-23 orchestration audit (C3, H4, L4) found the permission boundary was
prompt-only:

- headless `orch.sh` workers ran `claude -p --dangerously-skip-permissions`,
  `codex exec --dangerously-bypass-approvals-and-sandbox`, `cursor-agent -p --force`;
- the graph integrator ran with the same flags **inside the maintainer's checkout**;
- interactive `cdamux worker spawn` workers ran a bare `claude` with the user's settings,
  and the `cda-orchestrator` skill told the orchestrator to "relay the answer" to a
  permission prompt, i.e. to type the approval the human's classifier had refused;
- the tailnet API served without a token and `send` typed into any tmux session;
- briefs, inboxes and events under `$CDAMUX_HOME` were world-readable.

Owner decisions Q2, Q3=B, Q7=A, Q24, Q25 (2026-11-23 grill) settle the boundary; on
review of #433 the owner chose `dontAsk` over `acceptEdits` (2026-11-23).

## Decision

1. **Escalation, never relay.** A permission prompt or a denied command is an
   escalation to the maintainer; the orchestrator never relays an approval and never
   runs the command itself. Same sentence in `cda-orchestrator`, `cda-dev/prompts/worker.md`
   (BLOCKED) and `cdamux/prompts/worker-preamble.md`.
2. **Every Claude Code worker runs in `--permission-mode dontAsk` with the kit
   allowlist and push guard**, headless (`orch.sh`: worker, pr-fix, open-PR step,
   reviewer, verifier, integrator) and interactive (`cdamux worker spawn`, and
   `cdamux restore` of a worker). `dontAsk` runs only what an allow rule covers and
   denies the rest without a dialog, in `-p` and interactively alike, so one mode serves
   both (`acceptEdits` would show interactive dialogs, and a "Yes, don't ask again"
   there writes `.claude/settings.local.json` at the main checkout root, widening every
   worker of the repo). No bypass flag anywhere.
3. **One template, two renderers.** `cdamux/config/worker-settings.json` (embedded by
   cdamux) and `skills/core/cda-dev/templates/worker-settings.json` (orch.sh) are
   identical (`test_agents.sh` checks), and so are the two copies of
   `worker-push-guard.mjs`. Placeholders: `__BRANCH__` (the worker branch),
   `__DEFAULT__` (the repo default branch, from `origin/HEAD`, else `gh repo view`;
   never hardcoded), `__WORKTREE__` (physical path), `__GUARD__` (hook path). When the
   branch is unknown, detached, outside `[A-Za-z0-9._/@+-]`, or equal to the default
   branch, or the default is unknown, no push is allowed (fail closed); a path with a
   quote, paren or `*` is refused.
   - **files**: `Edit(//<worktree>/**)` (all edit tools), `Read(//**)` minus `~/.ssh`,
     `~/.gnupg`, `~/.aws`, `~/.config/gh`; `mkdir`. Writes outside the worktree are denied,
     except the orch.sh reviewer's verdict file: `Edit(//<TMPDIR>/orch-review.*/verdict)`
     (a glob, so the settings never name the random review dir).
   - **allow** (Q24): `git add/commit/restore/rm/mv/fetch/rebase/merge-base`,
     `git merge [--no-edit] origin/<default>`;
     `git push` / `git push *` (the push guard decides the branch, see below and the
     2026-11-24 amendment; pr-fix renders its PR head ref as own branch);
     `gh pr create/view/checks/diff/list`, `gh issue view`, `gh repo view`;
     `cdamux msg`, `cdamux state get`, `jq`, `foam-board.sh list-ready|path|reconcile|
     claim|set-status` and the listed read-only kit scripts; the repo's own tooling run
     inside the worktree: `bash|sh|./` + a script file of the worktree (guard-checked,
     amendment), `cargo`, `pnpm install/run/test/build/
     lint/typecheck`, `npm run/test/ci/install`, `yarn install/run/test`, `make`, `just`,
     `mise run`, `go test/build/vet`, `pytest`, `uv run pytest`, `nix develop -c bash
     scripts/*|cargo` (last resort, nix-develop-shell). No command runner that takes an
     arbitrary command (`mise exec`, `pnpm exec/dlx`, `uv run <any>`, `nix develop -c <any>`).
   - **deny**: `gh pr merge` (orch.sh itself merges ADR-016 foam-only PRs, not the
     worker), any plain force push (`--force`, `-f`, `+refspec`), push to the default branch
     (`… <default>`, `…:<default>`, `refs/heads/<default>`), ref deletion (`--delete`,
     `-d`), `--mirror`, `--all`, `git -C|-c … push`, `git rebase --exec|-x`,
     `sudo`, global installs (`npm|pnpm -g/--global` anywhere, `npm --location/--prefix`,
     `pnpm --prefix/--global-dir`, `yarn global/--prefix/--global-folder`,
     `cargo [+tc] install`, `go install`, `pip install`, `brew`, `nix profile`, `nix-env`).
     The denies matter because allow rules add up across scopes: a user-level
     `Bash(git:*)` would otherwise allow any push.
   - **push guard**: a `PreToolUse` hook on Bash in the same settings file
     (`node worker-push-guard.mjs <own> <default> <worktree> <kit>`): a strict
     plain-line whitelist for any text that mentions a push, without shell parsing (see
     the 2026-11-24 amendment; the first argv-parsing version was bypassed). Text rules
     alone miss spellings such as `-uf`, `:ref`, quoted refs or `bash -c`.
   - Anything else is denied. The worker then asks through the orchestrator (point 1).
     orch.sh refuses to start Claude when the settings cannot be rendered (no jq, node or
     template), rather than run on user scope alone.
   - Once a branch is pushed the worker publishes a rebase with `--force-with-lease` on
     its own branch, or merges the updated base (amendment 2026-11-24); orch.sh's own
     rebase step (Q4) is not an agent command.
4. **Delivery mechanism: `--settings <file>`**, not `.claude/settings.local.json` in the
   worktree. In a git worktree Claude Code reads and writes the local settings file at
   the main checkout's root, and a hand-written file would be an untracked file a
   worker can commit. The rendered file lives outside the worktree:
   `$ORCH_REPO/.orch/run/settings-<worktree>.json` (orch.sh; no `ORCH_REPO` = claude
   is not started) and
   `$CDAMUX_HOME/settings/<host>.json` (cdamux, 0600). Claude Code concatenates
   `permissions.allow`/`deny` across managed, `--settings`, local, project and user
   scopes and evaluates **deny first** in every scope: a user-level allow cannot reopen
   a template deny (docs: "If a tool is denied at any level, no other level can allow
   it"). Verified on Claude Code 2.1.296 in `-p`: `git push origin HEAD:main`,
   `git push -f` and `HEAD:refs/heads/main` were denied, own-branch pushes ran, no hang;
   in `dontAsk`, Write inside the cwd, Skill and Agent ran, Read outside the cwd and
   `mkdir` needed the rules above.
   - **`cdamux worker spawn --here`** runs in the target checkout itself (refused on the
     repo's default branch from `origin/HEAD`, else `gh`; `main`/`master` when unknown): the `Edit` rule then covers that checkout. Prefer a worktree; use
     `--here` only on a checkout you are ready to let the worker edit.
5. **Integrator in a throwaway worktree.** `orch_integrator_agent_run` adds a detached
   worktree of `HEAD` under `.orch/worktrees/integrator-<run>`, runs the agent there,
   copies `integrator-report.md` / `integrator-ok` back to `$ORCH_REPO/.orch/` and
   removes the worktree. The maintainer checkout is never an agent cwd.
6. **Other CLIs** (documented gaps):
   - **codex**: `codex exec --sandbox workspace-write` replaces the bypass flag. There is
     no per-command allowlist; a linked worktree's git dir is outside the sandbox and
     network is off by default, so commit/push may fail until the operator widens
     `~/.codex/config.toml` (`sandbox_workspace_write`). Prefer Claude for workers.
   - **cursor-agent**: `-p` without `--force`: unapproved shell commands are rejected.
     Its rules (`Shell(<first token>)`) cannot express "push to own branch only"; the
     allowlist lives in the operator's `~/.cursor/cli-config.json`.
   - Interactive non-Claude workers keep their own approval prompts (the human answers).
7. **Tailnet API (Q7=A, amends [[ADR-004-cdamux|ADR-004]])**: `cdamuxd` never serves
   without a token: `CDAMUX_TAILNET_TOKEN`, else `<config dir>/tailnet-token`, minted
   (0600, 32 random bytes) on first start. An empty or unreadable file makes cdamuxd
   print the error and exit 0 (no supervisor restart loop); a token file readable by
   group or others (e.g. 0644) is served with a warning.
   `POST /v1/sessions/:id/send` only reaches sessions with `@cdamux_id` (403 otherwise:
   plain shells, `cdamux-nav`). The owner's Mac sets `listen_tailnet = false` (nix-beam).
8. **State permissions (L4)**: `$CDAMUX_HOME` and every state dir cdamux creates are
   0700; state, inbox, events, say queue, prompt and settings files are created 0600.

## Amendment 2026-11-24 — leased push, plain-line push whitelist, scripts

Owner decision 2026-11-24: the merge rule (Q4) asks workers to rebase on the default
branch, and a rebased branch can only be published with a leased force push; two neomind
workers were blocked on it. Live `dontAsk` runs also denied `git push -u origin <own>`
and `bash <kit>/…/doc-hygiene.sh --check <repo>`.

- **Strict whitelist, no shell parsing (owner decision, second review of #436).** Two
  tokenizer versions (heredoc stripping, then heredoc / comment / substitution parsing)
  were bypassed: a fake `<<'X'` in a message or comment, `<<<`, `$'…'`, `$((1<<2))`,
  `${x#<<X}`, CRLF, quoted parens inside `$(…)`, `xargs`. The guard no longer parses
  bash. If the command text mentions `push`, `send-pack` or `receive-pack` (any case,
  after removing quotes and backslashes, so `pu''sh` / `pu\sh` count, even inside a
  message), it runs only when the whole trimmed text is one plain line with no shell
  metacharacter (`$` `` ` `` `'` `"` `\` `;` `&` `|` `<` `>` `(` `)` `{` `}` `#`, newline,
  CR, tab) of the form `git push [-u|--set-upstream] [--force-with-lease[=<own>[:<40-hex
  sha>]]] origin <own>` (flags in any order, each once; `origin HEAD:refs/heads/<own>`
  for pr-fix, never `HEAD:<own>`, which can resolve to a tag), or bare `git push` /
  `git push origin`. `<own>` must be a plain branch name: one starting with `refs/`,
  `heads/`, `tags/`, `remotes/`, `+` or `-` gets no push (`HEAD:heads/main` reached
  main). Everything else that mentions a push is
  refused: workers write such commit messages with `git commit -F <file>`, PR bodies with
  `--body-file`, reports behind `--ref`.
- **Push config is checked for every accepted form**, named ones included (a
  `remote.origin.push` refmap made `git push --force-with-lease origin <own>` rewrite
  main): in the hook's cwd (repo, global and system config), `push.default` unset /
  simple / current / upstream is the only `push.*` accepted; any `remote.pushDefault`,
  `remote.*.push*` (refmap, pushurl), `remote.*.mirror`, `branch.*.pushRemote`,
  `url.*.insteadOf` or `url.*.pushInsteadOf` refuses the push.
- **Bare `git push` / `git push origin`** also need the hook's cwd on the own branch with
  `branch.<own>.merge = refs/heads/<own>`.
- **Other refusals, by text**: `git` together with `alias.`, `--config-env`,
  `include.path` / `includeIf` (an alias or include could hide a push); `git` together
  with `$`, a backtick, `xargs` or `eval` (a computed subcommand); `gh` together with
  `merge`, `git/refs` or a ref mutation. A false positive is a refusal, never a pass.
  The `gh` rule only holds while no user-level allow covers `gh api` (the template
  allows no `gh api`): an endpoint spelled without those words would pass the guard.
- **Text rules**: `Bash(git push)` and `Bash(git push *)` (still dropped when the branch
  is unknown) with their denies (`*--force`, `*--force *`, `-f`, `+`,
  `*heads/<default>*`, delete, mirror/all, `git -C|-c … push`) as a second layer. The
  hook runs `node … || exit 2`: a missing or crashing guard blocks every Bash call.
- **The guard is the push authority, not a sandbox**: `git --exec-path`, upload-pack /
  receive-pack overrides, abbreviated options and git hooks the worker can edit stay
  accepted residual risks, as does code the worker writes and runs; branch protection on
  the default branch is the backstop.
- **Shell runs, same plain-line rule**: text rules `Bash(bash *)`, `Bash(sh *)`,
  `Bash(./*)` (a `*` cannot resolve a path). A text with a `bash`/`sh`/`zsh`… word or a
  `./` / `../` at a command start runs only as one plain line `[nix develop -c] bash|sh
  <relative path> [args]` or `./<path> [args]`, the path a file that resolves (symlinks
  included) inside `<worktree>` (hook argument), no `..`, no shell option: `bash
  scripts/verify.sh`, `bash cdamux/hooks/test.sh`, `./scripts/x.sh` run.
- **Kit scripts, explicit list in the guard**: `<kit>` = the kit checkout (orch.sh: from
  the template path; cdamux: `<claude config dir>/skills/cda-kanban` resolved through the
  `install.sh` link; none found → none run). Allowed with exactly these arguments: `bash
  <kit>/skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh --check|--report [<worktree>]`
  and `bash <kit>/skills/core/cda-kanban/scripts/foam-board.sh list-ready [--json] |
  reconcile` (read-only). The former `foam-board.sh *` is narrowed to `list-ready
  [--json]`, `path`, `reconcile` (report only), `claim`, `set-status`: `reconcile --pr`
  merges a PR and is not a worker command.
- **Preamble report lines are plain commands**: `cdamux worker spawn` writes the parent
  agent id into the preamble (`__PARENT__`), so each line is `cdamux msg send --to
  <parent> …`, matched by `Bash(cdamux msg *)`; the former `--to "$(cdamux state get …
  | jq …)"` was denied under `dontAsk` (command substitution).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Keep bypass flags, rely on prompts | Prompt-only boundary; the audit's C3 |
| `--allowedTools` on the command line | Same rules but no deny list, long argv, two code paths |
| `.claude/settings.local.json` in each worktree | Read from the main checkout root in a worktree; untracked file can be committed |
| `--permission-mode auto` (classifier) | Not deterministic; headless denial must be predictable |
| `acceptEdits` (first version of this ADR) | Interactive dialogs invite click-through; "don't ask again" widens every worker of the repo |
| Text deny rules only for `git push` | `-uf`, `:ref`, quoted refs, `git -C`, `bash -c` slip past prefix matching |
| Deny `Edit(//**)` + allow the worktree | Deny wins over any allow; no carve-out possible |
| Refuse to start the tailnet API without an env token | Would silently disable the API on hosts whose service has no env wiring; minting keeps it on and still never open |

## Consequences

- **Positive**: the commands a worker types are held to the allowlist and the push
  guard, whatever the user settings allow; the orchestrator cannot launder an approval;
  the integrator never touches the maintainer checkout; remote input only reaches
  agents.
- **Negative**: commands outside the allowlist now stop headless workers (they report
  BLOCKED); codex/cursor workers lose capability until their CLI config allows it;
  the dashboard needs the token from `<config dir>/tailnet-token`.
- **Constraint**: this is not a sandbox. The repo's own tooling is allowed and can run
  anything: a worker can write `scripts/x.sh`, a test, a `build.rs` or an npm script that
  merges, force-pushes or installs globally, then run it through an allowed command;
  git aliases from the user's gitconfig are not resolved by the guard. The allowlist,
  denies and guard stop the commands an agent types, not the code it runs. Server-side
  branch protection (Q6) is the real guard for the default branch; keep user-level
  allows narrow (no `Bash(git:*)`).

## References

- Audit `orchestration-audit-fable.md` C3, H4, L4; owner decisions Q2, Q3, Q7, Q24, Q25
- Code: `skills/core/cda-dev/scripts/lib/agents.sh` (`orch_agent_settings`,
  `orch_render_settings`, `_orch_agent_exec`), `lib/graph.sh`
  (`orch_integrator_agent_run`), `cdamux-core/src/worker.rs`
  (`worker_settings_json`, `with_worker_permissions`), `restore.rs`,
  `cdamux/config/worker-push-guard.mjs`,
  `tailnet_api.rs` (`resolve_tailnet_token`, `send_gate`), `paths.rs`
  (`create_private_dir`)
- Tests: `tests/test_agents.sh`, `tests/test_integrator_worktree.sh`,
  `cdamux-core/tests/worker_tmux.rs`, `cdamux-core/tests/tailnet_send_gate_tmux.rs`
- Claude Code docs: permissions (rule syntax, scope merge, deny precedence), settings
  (precedence, worktree local file), headless (`-p` denial)
- Supersedes the bypass flags in [[ADR-010-worker-cli-inheritance-and-tiers|ADR-010]] §6
