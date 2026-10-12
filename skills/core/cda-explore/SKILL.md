---
name: cda-explore
description: Chart a large, foggy chunk of work as a map of decision tickets on GitHub, then resolve them one at a time until the way to a PRD is clear. Use when an idea is too big or too uncertain for one session, when unknowns must be settled before a PRD can be written, or when the user asks to scope/de-risk/explore a direction before planning.
---

# cda-explore

A loose idea has arrived, too big for one session and wrapped in fog: the way from here to the **destination** isn't visible yet. This skill charts that way as a **shared map** of GitHub issues, then works its **decision tickets** — questions whose resolution is a *decision*, not a slice of a build — one at a time until the route is clear.

This is the **fog-clearing stage that sits before `cda-prd`**. It produces decisions (recorded as ADRs via `cda-foam`), not deliverables. When the map is clear, hand the destination to `cda-prd` to draft the PRD, then `cda-plan` and `cda-dev` take over.

The destination varies: a spec/PRD to hand off, a decision to lock before planning, or a change made in place (e.g. a data-structure migration). Naming it is the first act of charting — it fixes the scope and shapes every ticket.

## Plan, don't do

cda-explore is **planning** by default: each ticket resolves a decision, and the map is done when the way is clear — nothing left to decide before someone goes and builds the thing. The pull to just start coding is usually the signal you've reached the edge of the map: stop and hand off to `cda-prd`. An effort can override this in its **Notes** (carrying some execution into the map), but absent that, produce decisions, not code.

## Refer by name

Every map and ticket is a GitHub issue, so it has a **name** (its title). In everything the human reads, refer to it by that name, never a bare `#42`. A wall of `#42, #43, #44` is illegible; names read at a glance. The number and URL ride *inside* the name (a Markdown link), never stand in for it.

## The map

The map is a single GitHub issue labelled `explore:map` — the canonical artifact. Its decision tickets are **sub-issues** of the map (GitHub native sub-issues; fall back to a `Parent:` line in the body if sub-issues are unavailable on the repo).

The map is an **index, not a store**: it lists the decisions made and points at the tickets that hold their detail. A decision lives in exactly one place — its ticket (and, once locked, its ADR in `foam/`) — so the map only gists it and links.

See [conventions.md](conventions.md) for the exact labels, the GitHub blocking/frontier mechanics, and the `cda-foam`/`cda-kanban` wiring. Map and ticket bodies use [templates/map.md](templates/map.md) and [templates/ticket.md](templates/ticket.md).

### The map body (low resolution, loaded once per session)

Open tickets are **not** listed — they are open sub-issues, found by the frontier query. The body carries: **Destination**, **Notes** (domain, skills every session should consult, standing preferences), **Decisions so far** (one line per closed ticket + link, cross-linking its ADR when one was recorded), **Not yet specified** (the fog), and **Out of scope**.

## Ticket types

Each ticket carries one `explore:<type>` label. Every ticket is either **HITL** (worked *with* a human who speaks for themselves) or **AFK** (agent alone). A HITL ticket only resolves through live exchange; the agent never answers the human's side of it.

- **research** (AFK): surface a fact a decision waits on from docs, third-party APIs, or local resources. Resolve with a subagent that calls the `deep-research` skill. Capture findings on a throwaway `research/<name>` branch with a context pointer from the ticket; push it, open no PR.
- **prototype** (HITL): raise the fidelity of the discussion with a cheap, rough, concrete artifact to react to. Resolve by calling the `prototype` skill; link the prototype as an asset. Use when "how should it look / behave" is the key question.
- **grilling** (HITL): conversation — the default case. Call the `grilling` skill; for domain/terminology decisions also record vocabulary and ADRs through `cda-foam`.
- **task** (HITL or AFK): manual work that must happen before a *decision* can be made (provision access, sign up for a service so its API can be judged, move data so its shape can be seen). The one type that *does* rather than decides — it earns its place by unblocking a decision. Drive it AFK where possible; otherwise hand the human a precise checklist. The resolution records what was done and any facts later tickets depend on (credential location, new URLs, row counts).

## Fog of war

The map is *deliberately* incomplete: don't chart what you can't yet see. Beyond the live tickets lies the **fog** — decisions you can tell are coming but can't yet pin down because they hang on open questions. Resolving a ticket clears the fog ahead of it, graduating whatever's now specifiable into fresh tickets, one at a time, until no tickets remain.

The **Not yet specified** section holds that dim view. **Fog or ticket?** The test is whether you can *state the question* precisely now — not whether you can answer it.

- **Ticket** when the question is already sharp, even if blocked.
- **Not yet specified** when you can't yet phrase it that sharply. Don't pre-slice the fog into ticket-sized pieces.

## Out of scope

The destination fixes the scope, so work beyond it is **out of scope** — not fog. It gets its own **Out of scope** section and never graduates. When a ticket turns out to sit past the destination, **close it** and leave one line in Out of scope (gist + why), linking the closed ticket. It stays out of Decisions-so-far, which records only the route actually walked.

## Invocation

Two modes. Either way, **never resolve more than one ticket per session**, except `research` tickets (which run in parallel subagents).

### Chart the map

User invokes with a loose idea.

1. **Name the destination.** Call `grilling` to pin down what this map is finding its way to (spec/PRD, decision, or change); record domain terms and any locked constraints as ADRs via `cda-foam`. The destination is settled first — it fixes the scope.
2. **Map the frontier.** Grill again **breadth-first**: fan out across the whole space, surfacing the open decisions and the first steps takeable now. **If this surfaces no fog** (the way is already clear, small enough for one session), you don't need a map — stop and hand straight to `cda-prd`.
3. **Create the map issue** (`explore:map`): Destination and Notes filled, Decisions-so-far empty, the fog sketched into Not yet specified. If the effort is tracked on the kanban board, link the map to its epic (see `cda-kanban`).
4. **Create the tickets you can specify now** as sub-issues of the map, then **wire blocking edges in a second pass** (issues need numbers before they can reference each other). See conventions.md for the GitHub blocking mechanic.
5. **Fire the research subagents** for each `research` ticket, in parallel.
6. Stop: charting is one session's work; it hand-resolves nothing.

### Work through the map

User invokes with a map (URL or number). A ticket is optional — without one, you pick the next decision.

1. Load the **map** (low-res view, not every ticket body).
2. Choose the ticket: the user's, else the first **frontier** ticket in order. **Claim it** by assigning it to yourself before any work.
3. Resolve it as the type its `explore:<type>` label names. Zoom as needed (fetch related/closed ticket bodies on demand); call whichever skills the Notes block names.
4. Record the resolution: post the answer as a **resolution comment**, **close** the issue, append a context pointer to the map's Decisions-so-far, and — when the ticket locked a real decision — record it as an ADR via `cda-foam` and link it.
5. **Graduate the fog**: add newly-surfaced tickets (create-then-wire), clear each graduated patch from Not yet specified, rule out-of-scope anything past the destination, and update/delete tickets the decision invalidated.

When the frontier empties and Not yet specified is clear, the way to the destination is open: **hand off to `cda-prd`**.
