# Orch pr-fix allowlist accepts `nix develop -c make test` (host orch may
# predate scripts/verify.sh allowlisting). Keep the suite in scripts/verify.sh.
.PHONY: test
test:
	bash scripts/verify.sh
