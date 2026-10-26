# Bootstrap prompt — cda.aitools / ai-dev-kit on a new machine

You are setting up a **developer machine** to use [ai-dev-kit](https://github.com/chatondearu/ai-dev-kit)
and optionally the experimental **cda.aitools** companion. Follow steps in order.
Do **not** invent package-install steps inside the kit’s `install.sh`.

## Goals

1. Kit skills/rules linked (portable path).
2. Host agent CLIs available (official installers).
3. Optional: Nix `cda.aitools` for tmux/cdamux configs with relocatable root.
4. `doctor.sh` green enough to run orch/headless.

## Steps

### A — Clone the kit

```bash
git clone git@github.com:chatondearu/ai-dev-kit.git
cd ai-dev-kit
```

Pick a stable path (examples: `~/dev/ai-dev-kit`, `/cda/dev/ai-dev-kit`).

### B — Portable kit install (no apps)

```bash
./install.sh --dry-run
./install.sh
```

This only symlinks skills/rules/hooks. It must **never** install Cursor, Claude,
tmux, or cdamux.

### C — Host agent CLIs (official channels)

Install as needed (see `docs/recommended.md`):

- Cursor Agent / `claude` / `opencode`
- `gh`, `git`, `jq`, `flock`

Then:

```bash
./scripts/doctor.sh
```

### D — Optional Nix companion (`cda.aitools`)

Only if the machine uses Nix (nix-darwin / NixOS / home-manager):

1. Add this flake as an input.
2. Import `darwinModules.cdaAitools` or `nixosModules.cdaAitools`.
3. Set:

```nix
cda.aitools = {
  enable = true;
  configRoot = "/cda/aitools"; # or omit for ~/.config/cda-aitools
  installMethod = "auto";
};
```

4. Copy templates from `aitools/templates/` into `$CDA_AITOOLS_HOME`.
5. On Darwin without Nix: `aitools/scripts/cdamux-align-mac.sh` (build from in-tree `cdamux/`).

Read `aitools/README.md` and foam ADR-003.

### E — Non-Nix session tools

If skipping Nix companion: install tmux and build `cdamux` from `cdamux/` yourself;
copy `aitools/templates/tmux.conf` into your tmux config manually.

### F — Verify orch cleanup exists

```bash
bash skills/core/cda-dev/scripts/orch-cleanup.sh --help
```

## Out of scope

- Do not add app installs to `install.sh`.
- Do not install this prompt as a project skill under `~/.cursor/skills/`.
- Do not merge agent worktrees with `.orch/worktrees` automatically.
