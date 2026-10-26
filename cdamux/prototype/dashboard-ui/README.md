# PROTOTYPE — cdamux dashboard (variant **C+**)

Reference UX for the next PRD/plan iteration. **Default:** open `index.html` (variant C enriched).

## Layout

```text
[ Status: cdamuxd · tailnet · tmux ]

[ Fleet | Projects | Sessions | Agents / CLI | Settings ]  ← app-level tabs

Fleet:
  ┌─ tree ─────────────┬─ Preview | Notes | Review ────────┐
  │ ◇ project (blue)   │  pane tail                         │
  │   ○/●/◆ session    │  ─────────────────────────────    │
  │   (indented)       │  [ composer input → Send ]         │
  └────────────────────┴────────────────────────────────────┘
```

### Tree visual language

| Element | Treatment |
| ------- | --------- |
| Project | Blue text (`--project`), ◇ icon, chevron expand |
| Session | Indented, left border, status icon: `○` idle · `●` active · `◆` needs input |

### New session (`n`)

1. **Path:** running project (session in progress) · discovered repo · custom folder  
2. **Run:** agent CLI **or** empty shell only  
3. **Layout:** single pane · split horizontal · split vertical  

### Preview input

Composer under **Preview** tab = agent-manager inline send (maps to `cdamux send` + Enter). `Space` focuses composer on Fleet.

### Other tabs (requested)

| Tab | Purpose (prototype stub) |
| --- | ------------------------ |
| **Projects** | Roots, discover mode, table of repos |
| **Sessions** | Flat tmux list, focus → Fleet |
| **Agents / CLI** | Configured commands, PATH doctor, AM key reference |
| **Settings** | attach_mode, daemon/tailnet, notes, UI |

## Run

```bash
xdg-open cdamux/prototype/dashboard-ui/index.html
```

## Tracking

**Approved** — [PRD-41](../../../prd/PRD-41-cdamux-mission-control-tui.md), [PLAN-41](../../../plans/PLAN-41-cdamux-mission-control-tui.md), GitHub [#58](https://github.com/chatondearu/ai-dev-kit/issues/58).

Open questions captured in PRD-41 (composer scope, preview without daemon, multi-line input, move session).
