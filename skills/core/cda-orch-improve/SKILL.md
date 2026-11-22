---
name: cda-orch-improve
description: "One worker writes an orchestrator session retro and ranked fixes. The orchestrator does not run this in its own pane."
disable-model-invocation: true
---

# Orch session retro

You are a worker. A cdamux orchestrator dispatched you. You write the retro.
The orchestrator does not audit, diagnose, or edit a repo in its own pane.

This is not `cda-retro` (coding-environment retro) and not a second kanban.
Foam `foam/board/` stays the only board. Do not add a status column.

## Steps

1. Read `cda-orchestrator` hard rules and ADR-009, ADR-011, ADR-015, ADR-016.
   Do not re-flag an accepted ADR unless this session proves it wrong.
   Done when those five files have been opened.

2. Use the session evidence in your spec. Do not re-investigate the live fleet
   from scratch. Open code only to pin a file and a line on a miss you already
   have.
   Done when each miss in the spec has a cite, or the words "not in this repo".

3. Write an English report at the path the spec names. Follow `cda-kiss-audit`:
   what the system does, the load you assumed, then findings in Must / Should /
   Nice, at most 20, each with What / Problem / Fix / If we skip. Add the
   life-cycle diagrams the spec asks for (global → project orch → worker, worker
   states, message sequence).
   Done when the file exists and the first section states the load.

4. Stay inside the spec. A skill or a one-line doc is allowed only when the
   spec says so. No cdamux behaviour change unless the spec allows that one line.
   Done when `git diff --name-only` stays inside the spec.

5. Report with the worker preamble's `done` line: report path, PR url or none,
   issue number or none, and the top three must-fixes. Do not merge.
   Skills are outside the ADR-016 auto-merge allowlist.
   Done when that message is sent.

## Do not

- Run a whole-repo `cda-kiss-audit`. The subject is the orchestrator system.
- Spawn more workers. You are the retro.
- Merge, force-push, or `--here` onto `main`.
