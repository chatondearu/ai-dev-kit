---
name: cda-kiss-audit
description: >
  Whole-repo quality audit, ranked: bugs, security holes, what breaks under real
  load, risky code with no test, slow paths, and what to delete / merge / split.
  Each finding explained plainly with a concrete case. One-shot report, changes
  nothing. Use for "audit this codebase", "review the whole repo", "find bloat",
  "what can I delete", /cda-kiss-audit.
license: MIT
---

# KISS audit

Audit the repo like the senior who just inherited it and will be paged when it breaks. Order of importance: **correct → safe → holds under load → tested → fast → lean**. Lean still matters — every extra line gets read, tested and fixed later. This is a report the user asked for: give it in full, change no code.

> Lens and ranking adapted from [ponytail](https://github.com/DietrichGebert/ponytail) (MIT). Siblings in this kit: `cda-review` audits one diff/PR; `cda-architecture` hunts module-deepening; this is the whole-repo, ranked, fix-this-first pass. For the write-less attitude it leans on, see `cda-kiss`.

## 1. Map first

- Audit what the user names (a folder, a package) — nothing named → the whole repo.
- Read the README, build/deploy config, dependency list, entry points (main, routes, handlers, jobs, CLI) and the tests. If the repo keeps a Foam graph, skim `foam/decisions/` (ADRs) so you don't re-flag accepted decisions.
- Find the **expected load**: one person running a script, or many users/processes at once. Judge scale against that and state which load you assumed.
- Trace the main flows end to end: where data comes in, what is stored, what goes out. Read those paths fully — user input, money, auth, data writes, background jobs, anything shared between processes.
- Big repo: go deep where a mistake costs most, not file by file. Say which parts you did **not** read.

## 2. Look for

1. **Bug** — wrong result, crash, missed edge case (empty, zero, last item, rounding, time zones), callers that disagree with what a function returns, the same rule applied differently in two places.
2. **Risk** — security holes (injection, weak randomness, secrets in code, missing checks on untrusted input), data loss (errors swallowed, writes in the wrong order, no transaction).
3. **Scale** — fine for one user, wrong for many: check-then-write races, work every process repeats, memory/lists that only grow, a query per item, O(n²) on big input, per-process state that must be shared.
4. **Missing test** — risky logic (a branch, a parser, money, security, data writes) with no test that fails when it breaks. One good test, not coverage.
5. **Speed** — big slowdowns are problems; small wins (work repeated in a hot loop) are suggestions.
6. **Lean** — code that shouldn't exist or should be smaller: dead code / unused options & flags & config / speculative features (delete); two helpers doing the same thing (reuse, name the path); what the stdlib or platform already does (drop the dep); interface with one impl, factory with one product, pass-through wrapper (yagni); near-copies that must change together (merge); one unit doing several unrelated jobs (split by job, never by line count).

## 3. Check before you report

- Every finding needs a **concrete case**: "this input/situation → this wrong result". No case, no finding.
- Before calling code unused, `grep` the whole tree — tests, fixtures, config, string/dynamic references included.
- A `shortcut:` comment that names its limit (the `cda-kiss` convention) is a **decision**, not a finding — unless the expected load already crosses it. A decision recorded as an accepted ADR in `foam/` is not a finding; challenge it only through `cda-foam` if the audit shows it's now wrong.
- Propose the **smallest fix** that works; prefer fixes that delete code. Never add layers/frameworks/config the problem doesn't need.
- No style taste, no "consider", no vague worries.

## 4. Output

Very simple English: short sentences, everyday words; explain a technical term the first time. The reader may never have seen this code.

Start with `What this repo does:` in two or three sentences, and the load you assumed. Then findings in three groups, most important first (skip empty groups):

- **Must fix** — bug, security, data loss, breaks at the expected load.
- **Should fix** — risky code without a test, real slowness, duplication, a unit that mixes jobs, code that shouldn't exist.
- **Nice to have** — small speed-ups, shorter forms.

Number findings **across all groups** (so the user can say "fix 2 and 5"). At most 20; if you left smaller ones out, say how many. Every finding has all four parts, each one or two short sentences:

```
2. **Orders land on the wrong day** (`billing/close_day.py:40-52`)
   - **What this is:** At midnight this job closes the day and bills that day's orders.
   - **Problem:** It takes "today" from the server clock (UTC). An order placed at 00:30 in Berlin is billed the day before.
   - **Fix:** Compute the day in the shop's time zone: `datetime.now(ZoneInfo("Europe/Berlin")).date()`. One line.
   - **If we skip it:** Late orders show the wrong date; accounting fixes them by hand.
```

End with:
- `Verdict:` one line — healthy, or what to fix first.
- `Lean: -<N> lines, -<M> dependencies possible.` when lean findings exist.
- `Not checked:` the parts you didn't read or couldn't run.

Nothing found → `What this repo does:`, then `Healthy. Nothing to fix.` and one line on what you checked. One-shot report; it changes no code.
