# PLAN-61 — spawn modal + pane splits (v2 phase 3)

- **Status**: in-progress
- **PRD**: [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#61](https://github.com/chatondearu/ai-dev-kit/issues/61)
- **Parent**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Stack**: PR base phase 2 branch

## Summary

In-TUI `n`: running / discovered / custom; shell-only; single | split-h | split-v.

## Tasks

- [x] Modal state machine (path mode, agent, shell_only, layout)
- [x] Wire discover + active session projects
- [x] Shell-only spawn (bash, no agent cmd)
- [x] tmux split after create; secondary pane shell
- [ ] Refresh tree + select new session (dashboard exits after spawn today)

## Verification

Three path modes; splits create two panes; `cargo test`
