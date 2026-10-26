# PLAN-61 — spawn modal + pane splits (v2 phase 3)

- **Status**: in-progress
- **PRD**: [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#61](https://github.com/chatondearu/ai-dev-kit/issues/61)
- **Parent**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Stack**: PR base phase 2 branch

## Summary

In-TUI `n`: running / discovered / custom; shell-only; single | split-h | split-v.

## Tasks

- [ ] Modal state machine (path mode, agent, shell_only, layout)
- [ ] Wire discover + active session projects
- [ ] Shell-only spawn (bash, no agent cmd)
- [ ] tmux split after create; primary pane rule documented
- [ ] Refresh tree + select new session

## Verification

Three path modes; splits create two panes; `cargo test`
