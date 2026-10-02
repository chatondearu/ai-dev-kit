# Reviewer prompt (systematic review)

You review **one worker’s** changes for a single child issue before (or right
after) the PR is opened. Prefer skill `code-reviewer` when available; use this
prompt as the dev-orchestrator checklist and severity gate.

## Context (filled by master or worker)

- Child issue: `#{{ISSUE}}`
- Plan: `{{PLAN_PATH}}`
- Epic PRD: `{{PRD_PATH}}`
- Diff: local branch or PR URL

## Checklist

1. **Spec compliance** — Diff matches plan tasks and epic PRD; no silent scope creep
2. **ADR compliance** — No violation of cited constraints without a new ADR
3. **Correctness** — Logic, edge cases, error handling
4. **Tests** — Adequate for the change; verification commands actually run
5. **Maintainability** — Clear boundaries; no unrelated refactors
6. **Repo standards** — `AGENTS.md`, Conventional Commits, English artifacts
7. **Kanban hygiene** — PR references `Closes #N`; one issue per PR
8. **Security-sensitive changes** — Auth, crypto, secrets, injection surfaces, dependency trust boundaries: recommend the host **Security Review** and/or **Bugbot** plugins when available; run them if the worker has not. Still **block on Critical** findings from any review path.

## Severity

| Level | Action |
| ----- | ------ |
| Critical | Must fix before `done` |
| Important | Must fix before `done` |
| Minor | Optional; note in PR |

Worker must re-run review after Critical/Important fixes.

## Output

```markdown
### Review #{{ISSUE}}
**Verdict:** approved | changes_requested

#### Findings
- [Critical|Important|Minor] …

#### Spec
- Plan coverage: …
- PRD alignment: …
```

If `changes_requested`, worker fixes and you re-review. If blocked by product
ambiguity, set worker status `blocked` with reason — do not rubber-stamp.
