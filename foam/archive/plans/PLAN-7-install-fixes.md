# PLAN-7 — Install fixes

- **Status**: done
- **PRD**: [[../../prd/PRD-6-kiss-audit-fixes|PRD-6]]
- **GitHub**: [#7](https://github.com/chatondearu/ai-dev-kit/issues/7)
- **Date**: 2026-10-13

## Summary

Make `install.sh` honor the active Claude config dir for every artifact, show its full help, warn instead of silently skipping on invalid JSON, and guard its config merge with a committed test.

## Constraints (from ADRs)

None recorded. Never cut validation, error handling or the non-destructive merge guarantees (`kiss`).

## Tasks

- [ ] The assembled global `CLAUDE.md` is linked into the resolved Claude config dir (the same one the hooks and skills use), not a hard-coded home path
- [ ] Help output covers the entire usage header, including the hooks paragraph, without a magic line number that drifts again
- [ ] A hooks/statusline merge that cannot parse the target file prints one clear skip warning naming the file and leaves it untouched
- [ ] Add a hook-wiring test next to the kiss lib test that runs the installer against temp config dirs

## Test plan

Seam: the installer's observable effect on a config dir (files + JSON content), driven through its CLI with env-var-overridden dirs.
- [ ] Pre-existing hooks and keys survive; second run is a no-op (each kiss hook exactly once)
- [ ] `--uninstall` removes only kiss entries and only a kiss-owned statusLine
- [ ] Invalid JSON target → warning printed, file byte-identical
- [ ] `CLAUDE.md` link lands in the overridden Claude dir

## Verification

- [ ] `for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done`
- [ ] `bash skills/quality/kiss/hooks/test.sh`
