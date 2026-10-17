# Reviewer prompt (systematic review)

You review **one worker’s** changes for a single child issue before (or right
after) the PR is opened. Prefer skill `cda-review` when available; use this
prompt as the cda-dev checklist and severity gate.

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
6. **Leanness & reuse** (KISS) — Could it fully work with less code? Reinvents a
   repo helper/component/pattern, or stdlib/platform/installed-dep behavior?
   Unrequested abstraction/options/"for later" code? Read the touched code *and
   its callers* (an untouched caller that now breaks is the worst finding). Never
   count removing validation/security/a11y/error-handling as "leaner". See `cda-kiss`.
7. **Repo standards** — `AGENTS.md`, Conventional Commits, English artifacts
8. **Docs updated** — If the diff changes public behavior, API, config/env,
   deployment, or architecture, the matching `doc/` pages are updated in the same
   change (and the relevant foam ADR when it's an architectural decision). Run
   `cda-docs` in **check mode** on the diff; a behavior change whose doc
   page is left untouched is an **Important** finding — fix via `cda-docs`.
9. **Kanban hygiene** — PR references `Closes #N`; one issue per PR
10. **Security-sensitive changes** — Auth, crypto, secrets, injection surfaces, dependency trust boundaries: recommend the host **Security Review** and/or **Bugbot** plugins when available; run them if the worker has not. Still **block on Critical** findings from any review path.

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
