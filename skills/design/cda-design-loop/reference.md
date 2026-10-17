# Visual rubric (`cda-design-loop`)

Severity labels: **Blocker** | **High** | **Medium** | **Nitpick**.

Inside the loop, **fix only Blocker and High** findings. After the iteration cap, emit leftover Medium and Nitpick for the human.

## Must check (Blocker / High if failed)

| Check | Blocker / High when |
| ----- | ------------------- |
| Brief direction | Tone, typography family, and palette intent do not match `DESIGN.md` — reads as generic AI landing |
| Hierarchy | No clear primary action or headline in the first viewport |
| Layout at breakpoints | Overflow, clipped text, or overlapping content at **375** or **1440** |
| Control labels | Focusable controls lack accessible names; icon-only buttons are the sole affordance with no label |
| Console on load | Uncaught errors on load of reviewed routes |

## Should check (Medium)

- Spacing rhythm vs brief; body text contrast
- Empty, loading, and error states when those surfaces were in scope
- Motion: intentional, not decorative noise

## Anti-slop flags

Apply when the brief forbids them (deterministic checks when possible):

- Inter, Roboto, or Arial as the **sole** stack
- Default purple-gradient-on-white when the brief is not that aesthetic
- Card-grid wallpaper with no hierarchy
- Emoji-as-icon clusters

## Triage rule

1. Score each finding with Blocker | High | Medium | Nitpick.
2. If no Blocker and no High remain → exit success for the loop.
3. Otherwise fix Blocker + High only, then repeat (cap 3).
