#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"

# Fake PATH with only a stub "claude"
FAKE="$(mktemp -d)"
_ORIG_PATH="$PATH"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE"' EXIT
STUB_BASH="$(command -v bash)"
cat >"$FAKE/claude" <<EOF
#!$STUB_BASH
echo "claude-ok \$*"; exit 0
EOF
chmod +x "$FAKE/claude"
cat >"$FAKE/custom-stub" <<EOF
#!$STUB_BASH
echo "custom-stub-marker \$*"; exit 0
EOF
chmod +x "$FAKE/custom-stub"

# PATH with stub claude first; real agent/cursor-agent CLIs hidden
STUB_PATH="$FAKE:$(orch_test_path_without "$FAKE" agent cursor-agent)"
PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
kind="$(orch_detect_agent)"
assert_eq "$kind" "claude" "detect claude"
PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
orch_detect_agent >/dev/null
assert_eq "${ORCH_AGENT_BIN:-}" "claude" "ORCH_AGENT_BIN set"

PROMPT="$FAKE/prompt.txt"
echo "hi" >"$PROMPT"
out="$(PATH="$STUB_PATH" orch_agent_run "$FAKE" "$PROMPT" 2>&1 || true)"
assert_contains "$out" "not running claude" "no ORCH_REPO: settings never written in the worktree, claude refused"
out="$(PATH="$STUB_PATH" ORCH_REPO="$FAKE/repo" orch_agent_run "$FAKE" "$PROMPT" 2>&1 || true)"
assert_contains "$out" "claude-ok" "invoke stub"

PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
orch_detect_agent >/dev/null
assert_eq "${ORCH_AGENT_BIN:-}" "claude" "BIN set before ORCH_AGENT override"
export ORCH_AGENT="$FAKE/custom-stub"
out_override="$(PATH="$STUB_PATH" orch_agent_run "$FAKE" "$PROMPT" 2>&1)"
assert_contains "$out_override" "custom-stub-marker" "orch_agent_run honors ORCH_AGENT when BIN stale"

# Override
export ORCH_AGENT="$FAKE/claude --print"
kind2="$(orch_detect_agent)"
assert_eq "$kind2" "custom" "custom override"

# Resolution order per role (ADR-010)
unset ORCH_AGENT
TAB=$'\t'
FAKE2="$(mktemp -d)"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE" "$FAKE2"' EXIT
for c in cursor-agent claude codex; do
  printf '#!%s\necho "%s-ok $*"; cat\n' "$STUB_BASH" "$c" >"$FAKE2/$c"
  chmod +x "$FAKE2/$c"
done
P2="$FAKE2:$STUB_PATH"

r="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_VERIFIER=codex orch_resolve_agent verifier)"
assert_eq "$r" "codex${TAB}role-override" "role override beats ORCH_AGENT"
r="$(PATH="$P2" ORCH_AGENT_PR_FIX=codex orch_resolve_agent pr-fix)"
assert_eq "$r" "codex${TAB}role-override" "pr-fix maps to ORCH_AGENT_PR_FIX"
r="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_VERIFIER=codex orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}override" "global override for other roles"
r="$(PATH="$P2" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "CLAUDECODE=1 beats cursor-agent on PATH"
r="$(PATH="$P2" CODEX_THREAD_ID=t1 orch_resolve_agent worker)"
assert_eq "$r" "codex${TAB}parent" "CODEX_THREAD_ID -> codex"
r="$(PATH="$FAKE" CLAUDECODE=1 CURSOR_INVOKED_AS=agent orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "env marker order: claude first"
r="$(PATH="$P2" orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}path" "no marker, no ancestor -> PATH order"
mkdir "$FAKE2/only"; cp "$FAKE2/cursor-agent" "$FAKE2/only/"
r="$(PATH="$FAKE2/only" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}path" "parent CLI not installed -> PATH"

# Fake ancestry: <start> -> 500 bash -> 400 node (args: cursor-agent script)
cat >"$FAKE2/fake-ps" <<STUB
#!$STUB_BASH
pid="\${*: -1}"
case "\$*:\$pid" in
  *comm=*:400) echo "1 node" ;;
  *comm=*:500) echo "400 zsh" ;;
  *comm=*) echo "500 bash" ;;
  *args=*:400) echo "node /opt/bin/cursor-agent -p" ;;
  *args=*) echo "zsh -c orch.sh" ;;
esac
STUB
chmod +x "$FAKE2/fake-ps"
r="$(PATH="$P2" ORCH_PS="$FAKE2/fake-ps" orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}parent" "ps ancestry -> cursor-agent"
r="$(PATH="$P2" ORCH_PS="$FAKE2/fake-ps" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "env marker beats ps ancestry"

# orch_agent_run picks the CLI of ORCH_ROLE; codex runs `exec -` on stdin
out_role="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_REVIEWER="$FAKE/custom-stub" ORCH_ROLE=reviewer orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_role" "custom-stub-marker" "orch_agent_run honors ORCH_ROLE"
out_codex="$(PATH="$P2" ORCH_AGENT=codex orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_codex" "codex-ok exec --sandbox workspace-write -m gpt-6.1-sol -c model_reasoning_effort=high -" "codex exec flags + medium tier"
assert_contains "$out_codex" "hi" "codex reads the prompt on stdin"

out_dry="$(PATH="$P2" ORCH_AGENT_VERIFIER=codex CLAUDECODE=1 orch_print_agents)"
assert_contains "$out_dry" "agent[worker]: claude (parent)" "dry-run line worker"
assert_contains "$out_dry" "agent[verifier]: codex (role-override)" "dry-run line verifier"

# Tiers (config/tiers.tsv): role defaults, per-role and current-tier env, per-CLI flags.
export ORCH_RUN_ID=t1 ORCH_REPO="$FAKE2"
S="$FAKE2/.orch/run/settings-$(basename "$FAKE").json"
assert_eq "$(orch_tier_for_role worker)" "medium" "default tier: medium"
assert_eq "$(orch_tier_for_role integrator)" "small" "integrator default: small"
assert_eq "$(orch_tier_for_role reviewer)" "large" "reviewer default: large (M14)"
assert_eq "$(ORCH_TIER_PR_FIX=large orch_tier_for_role pr-fix)" "large" "ORCH_TIER_PR_FIX"
assert_eq "$(ORCH_TIER=large ORCH_TIER_WORKER=small orch_tier_for_role worker)" "large" "ORCH_TIER (current) beats role tier"
assert_eq "$(orch_tier_flags agent large)" "--model claude-opus-5-5-high" "agent uses cursor rows"
out_t="$(PATH="$P2" ORCH_AGENT=claude orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "claude-ok -p --permission-mode dontAsk --settings $S --model sonnet --effort high" "claude medium flags"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_ROLE=integrator orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "--model sonnet --effort low" "integrator runs small"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIER_REVIEWER=large ORCH_ROLE=reviewer orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "--model opus --effort high" "ORCH_TIER_REVIEWER=large"
out_t="$(PATH="$P2" ORCH_AGENT=cursor-agent orch_agent_run "$FAKE" "$PROMPT" </dev/null)"
assert_contains "$out_t" "cursor-agent-ok -p --model auto hi" "cursor medium flags before prompt"

# --model in the agent command → pinned, no tier flags.
out_t="$(PATH="$P2" ORCH_AGENT="claude --model haiku" orch_agent_run "$FAKE" "$PROMPT")"
assert_eq "${out_t%%$'\n'*}" "claude-ok --model haiku -p --permission-mode dontAsk --settings $S" "pinned: no tier flags"
assert_eq "$(orch_agent_tier "codex -m gpt-x" worker)" "pinned${TAB}" "-m pins too"

# ORCH_TIERS_FILE replaces the kit table; a missing one is an error.
printf '# custom\nclaude\tmedium\t--model haiku\n' >"$FAKE2/tiers.tsv"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIERS_FILE="$FAKE2/tiers.tsv" orch_agent_run "$FAKE" "$PROMPT")"
assert_eq "${out_t%%$'\n'*}" "claude-ok -p --permission-mode dontAsk --settings $S --model haiku" "ORCH_TIERS_FILE row"
rc=0; err="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIERS_FILE="$FAKE2/nope.tsv" orch_agent_run "$FAKE" "$PROMPT" 2>&1)" || rc=$?
assert_eq "$rc" "1" "missing ORCH_TIERS_FILE fails"
assert_contains "$err" "tiers file not readable" "missing ORCH_TIERS_FILE message"

# CLI without a row: no flags, one warning per run.
err="$( { orch_agent_tier "$FAKE/custom-stub" worker; orch_agent_tier "$FAKE/custom-stub" integrator; } 2>&1 >/dev/null)"
assert_eq "$(grep -c 'no tier row for custom-stub' <<<"$err")" "1" "unknown CLI warns once per run"
assert_eq "$(orch_agent_tier "$FAKE/custom-stub" worker 2>/dev/null)" "medium${TAB}" "unknown CLI: no flags"
unset ORCH_RUN_ID

out_dry="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_INTEGRATOR="claude --model haiku" orch_print_agents)"
assert_contains "$out_dry" "tier[worker]: medium --model sonnet --effort high" "dry-run tier line"
assert_contains "$out_dry" "tier[integrator]: pinned" "dry-run pinned line"

# Per-worker json gets cli/model/effort/tier of the worker's agent.
if command -v jq >/dev/null 2>&1; then
  source "$TROOT/../lib/progress.sh"
  export ORCH_PROGRESS=0 ORCH_REPO="$FAKE2"
  PATH="$P2" ORCH_AGENT=claude ORCH_WORKER_ID=5 orch_agent_run "$FAKE" "$PROMPT" >/dev/null
  assert_eq "$(jq -c '[.id, .cli, .model, .effort, .tier]' "$FAKE2/.orch/run/5.json")" \
    '["5","claude","sonnet","high","medium"]' "worker json records the worker tier"
fi

# The agent runs as the worker's CDAMUX agent, never as orch (its CDAMUX_AGENT_ID).
printf '#!%s\necho "aid=${CDAMUX_AGENT_ID-unset}"\n' "$STUB_BASH" >"$FAKE2/aid-stub"
printf '#!/bin/sh\nexit 1\n' >"$FAKE2/only/tmux"
chmod +x "$FAKE2/aid-stub" "$FAKE2/only/tmux"
export ORCH_REPO="$FAKE2" CDAMUX_AGENT_ID=a-parent
waid="$(orch_cdamux_worker_agent_id 8)"
out_t="$(ORCH_AGENT="$FAKE2/aid-stub" ORCH_WORKER_ID=8 orch_agent_run "$FAKE" "$PROMPT" 2>/dev/null)"
assert_eq "$out_t" "aid=$waid" "inline: agent sees the worker agent id"
assert_eq "$CDAMUX_AGENT_ID" "a-parent" "orch keeps its own CDAMUX_AGENT_ID"
out_t="$(PATH="$FAKE2/only:$PATH" ORCH_TMUX=1 ORCH_AGENT="$FAKE2/aid-stub" ORCH_WORKER_ID=8 \
  orch_agent_run "$FAKE" "$PROMPT" 2>/dev/null)"
assert_eq "$out_t" "aid=$waid" "tmux new-session failure: inline agent sees the worker agent id"
out_t="$(ORCH_AGENT="$FAKE2/aid-stub" ORCH_ROLE=integrator orch_agent_run "$FAKE" "$PROMPT" 2>/dev/null)"
assert_eq "$out_t" "aid=unset" "no worker id: agent does not inherit orch's id"
unset CDAMUX_AGENT_ID

# ADR-019 worker permissions: no bypass flag; the settings template is rendered per worktree.
assert_eq "$(grep -c 'dangerously\|--force' "$TROOT/../lib/agents.sh" | tr -d ' ')" "0" "no bypass flag in agents.sh"
for f in worker-settings.json worker-push-guard.mjs; do
  same=no; cmp -s "$(dirname "$ORCH_SETTINGS_TEMPLATE")/$f" "$TROOT/../../../../../cdamux/config/$f" && same=yes
  assert_eq "$same" "yes" "kit and cdamux $f are identical"
done
render() { orch_render_settings "$1" "$2" /w/t /g/guard.mjs <"$ORCH_SETTINGS_TEMPLATE"; }
r="$(render worker/r1/x main)"
assert_eq "$(jq -r '.permissions.allow | index("Bash(git push origin worker/r1/x)") != null' <<<"$r")" "true" "push to own branch allowed"
assert_eq "$(jq -r '.permissions.deny | index("Bash(git push * main)") != null' <<<"$r")" "true" "push to default denied"
assert_eq "$(jq -r '.permissions.deny | index("Bash(gh pr merge *)") != null' <<<"$r")" "true" "gh pr merge denied"
assert_eq "$(grep -c '__' <<<"$r" | tr -d ' ')" "0" "no placeholder left"
r="$(render main main)"
assert_eq "$(jq '[.permissions.allow[] | select(startswith("Bash(git push"))] | length' <<<"$r")" "0" "on the default branch: no push allowed"
r="$(render worker/r1/x "")"
assert_eq "$(jq '[.permissions.allow[], .permissions.deny[] | select(test("git push origin|git push \\* $"))] | length' <<<"$r")" "0" "unknown default: no push allowed, no empty deny"
G="$(mktemp -d)"
git -C "$G" init -q -b dev && git -C "$G" commit -q --allow-empty -m init
git -C "$G" update-ref refs/remotes/origin/dev HEAD && git -C "$G" symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/dev
git -C "$G" switch -q -c feat/a
assert_eq "$(orch_default_branch "$G")" "dev" "default branch from origin/HEAD (not hardcoded main)"
sf="$(ORCH_REPO="$G" orch_agent_settings "$G")"
assert_eq "$(jq -r '.permissions.deny | index("Bash(git push * dev)") != null' "$sf")" "true" "settings file denies push to dev"
assert_eq "$(jq -r '.permissions.allow | index("Bash(git push -u origin feat/a)") != null' "$sf")" "true" "settings file allows own branch"
sf="$(ORCH_REPO="$G" ORCH_PUSH_BRANCH=pr-head orch_agent_settings "$G")"
assert_eq "$(jq -r '.permissions.allow | index("Bash(git push origin HEAD:pr-head)") != null' "$sf")" "true" "ORCH_PUSH_BRANCH overrides the checked-out branch"
rm -rf "$G"

# Agents run in a child `bash -c` (orch_sandbox_wrap): the template path must reach it.
for f in $(declare -F | awk '{print $3}' | grep -E '^_?orch_'); do export -f "$f"; done
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_REPO="$FAKE2" bash -c 'orch_agent_run "$1" "$2"' _ "$FAKE" "$PROMPT")"
assert_contains "$out_t" "--permission-mode dontAsk --settings $S" "child bash: claude gets --settings"
rc=0; err="$(PATH="$P2" ORCH_AGENT=claude ORCH_SETTINGS_TEMPLATE="$FAKE/none.json" orch_agent_run "$FAKE" "$PROMPT" 2>&1)" || rc=$?
assert_eq "$rc" "1" "claude refuses to run without rendered settings"
assert_eq "$(grep -c claude-ok <<<"$err" | tr -d ' ')" "0" "claude not started without settings"
assert_contains "$err" "cannot render worker permission settings" "missing settings message"
r="$(render feat/head main)"
assert_eq "$(jq -r '.permissions.allow | index("Bash(git push origin HEAD:feat/head)") != null' <<<"$r")" "true" "pr-fix push to head ref allowed"

# dontAsk needs explicit edit / read rules; the push guard hook is wired with the branches.
r="$(render feat/a main)"
assert_eq "$(jq -r '.permissions.allow | index("Edit(//w/t/**)") != null' <<<"$r")" "true" "edits allowed in the worktree only"
assert_eq "$(jq -r '.permissions.allow | index("Bash(git merge origin/main)") != null' <<<"$r")" "true" "merge of the default branch allowed"
assert_eq "$(jq -r '.hooks.PreToolUse[0].hooks[0].command' <<<"$r")" "node '/g/guard.mjs' 'feat/a' 'main'" "push guard hook wired"
assert_eq "$(render "a'b" main | jq -r '.hooks.PreToolUse[0].hooks[0].command')" "node '/g/guard.mjs' '' 'main'" "unsafe branch name: no push"
rc=0; orch_render_settings feat/a main "/w/(t)" /g <"$ORCH_SETTINGS_TEMPLATE" >/dev/null || rc=$?
assert_eq "$rc" "1" "unsafe worktree path refused"

# Push guard (PreToolUse hook): argv-level refusals the text rules miss.
NESTED='bash -c x'
for _ in 1 2 3 4 5; do NESTED="bash -c $(printf %q "$NESTED")"; done
guard() { jq -Rn --arg c "$1" '{tool_name:"Bash",tool_input:{command:$c}}' \
  | node "$(dirname "$ORCH_SETTINGS_TEMPLATE")/worker-push-guard.mjs" feat/a main 2>/dev/null; }
HEREDOC_COMMIT="$(printf 'git commit -m "$(cat <<%sEOF%s\nfix: avoid git push -f origin main\nEOF\n)"' "'" "'")"
for c in 'git push origin feat/a' 'git push -u origin feat/a' 'git push origin HEAD:refs/heads/feat/a' \
  'git commit -m x && git push origin feat/a' 'git status' \
  'git commit -m "docs: never git push -f origin main"' \
  'gh pr create --title t --body "run git push origin main after merge"' \
  "rg 'git push' skills" "git log --oneline | grep 'git push'" "$HEREDOC_COMMIT"; do
  rc=0; guard "$c" || rc=$?; assert_eq "$rc" "0" "guard allows: $c"
done
for c in 'git push -uf origin feat/a' 'git push origin +feat/a' 'git push origin :feat/a' 'git push -d origin x' \
  "git push origin 'HEAD:refs/heads/main'" 'git -C . push origin main' 'git push --mirror origin' 'git push' \
  'bash -c "git push -f origin feat/a"' 'echo $(git push origin main)' 'gh pr merge 3' \
  'git -c alias.p=push p -f' 'git push --force-with-lease origin feat/a' 'nix develop -c git push origin main' \
  'sh -lc "git push origin main"' 'eval git push origin main' 'echo "$(git push origin main)"' \
  'gh -R o/r pr merge 3' 'gh api -X PUT repos/o/r/pulls/3/merge' 'gh api repos/o/r/git/refs/heads/main -X PATCH' \
  'git send-pack origin main' "$NESTED"; do
  rc=0; guard "$c" || rc=$?; assert_eq "$rc" "2" "guard blocks: $c"
done

summary
