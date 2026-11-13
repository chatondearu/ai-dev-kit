# Research: flake.nix surfaces vs Mise for daily toolchain

**Ticket:** [#285](https://github.com/chatondearu/ai-dev-kit/issues/285) — *Research: what flake.nix provides today vs what Mise would cover*  
**Map:** [#284](https://github.com/chatondearu/ai-dev-kit/issues/284) — *Dev toolchain: flake.nix vs Mise for daily agent loops*  
**Date:** 2026-11-13  
**Scope:** Inventory only. No ADR; no `mise.toml`. Does not recommend a final choice.

## Classification key

| Code | Meaning |
| ---- | ------- |
| **(a)** | Daily PATH toolchain — binaries agents/`scripts/verify.sh` expect on PATH for day-to-day loops |
| **(b)** | Package build — Nix derivation that produces an installable binary/library |
| **(c)** | System module / delivery — NixOS/nix-darwin modules, nix-maid home files, activation oneshots, services |

Surfaces can feed more than one class (e.g. a package used by a module). Primary class is listed first.

## Sources inspected

- Root `flake.nix` + `flake.lock` inputs (`nixpkgs` = `nixos-unstable`, `nix-maid`)
- `nix/maid.nix`
- `aitools/nix/*` (modules, packages, activation helpers)
- `cdamux/nix/package.nix`
- Consumers: `scripts/verify.sh`, `AGENTS.md`, `skills/dev/nix-develop-shell/SKILL.md`, ADR-001 / ADR-003 notes in foam

---

## 1. Root `flake.nix`

### Inputs (infrastructure, not classified as a/b/c product surface)

| Input | Role |
| ----- | ---- |
| `nixpkgs` (`nixos-unstable`) | Pin for all packages + `devShells.default` |
| `nix-maid` | Maid home-file delivery for kit assets and `cda.aitools` deploy |

### Outputs

| Surface | Class | What it does |
| ------- | ----- | ------------ |
| `lib.maidConfig` | **(c)** | Re-exports `nix/maid.nix` for external flakes / docs |
| `packages.default` / `packages.activate` | **(c)** | `nix-maid pkgs (import ./nix/maid.nix {})` — activation package that links kit skills/agents/plugins into home |
| `packages.moshi-hook` | **(b)** | Call `aitools/nix/moshi-hook-package.nix` |
| `packages.cdamux` | **(b)** | Call `cdamux/nix/package.nix` |
| `packages.graphify` | **(b)** | Call `aitools/nix/graphify-package.nix` |
| `devShells.default` | **(a)** | `mkShell` with: `bash jq gh git cargo rustc tmux flock nodejs util-linux` (comment: util-linux for `setsid` / flock; absent on macOS host otherwise) |
| `nixosModules.default` (`aiDevKit`) | **(c)** | Enables nix-maid + installs kit assets for a user/tools list (`cursor`/`claude`/`opencode`/`agents`) |
| `nixosModules.cdaAitools` | **(c)** | Entry → `aitools/nix/nixos.nix` |
| `darwinModules.cdaAitools` | **(c)** | Entry → `aitools/nix/darwin.nix` |
| `formatter` (`nixpkgs-fmt`) | **(a)\*** | Used via `nix fmt`, not injected into `devShells.default` PATH |

\*Formatter is flake-driven formatting only; not part of the daily agent PATH story unless someone adds it to the shell.

### `devShells.default` tool-by-tool (a) — Mise / host replaceability

`scripts/verify.sh` needs at least: `bash`, `node`, `cargo` (+ transitive `rustc`), and (indirectly via tests) tools often assumed present: `jq`, `gh`, `git`, `tmux` in some orch/cdamux paths. Standing preference (map #284 Notes + `nix-develop-shell`): prefer host PATH; wrap with `nix develop -c` only as fallback.

| Tool in shell | Used for (examples) | Mise replace? | Host / other replace? | Pin/repro note |
| ------------- | ------------------- | ------------- | --------------------- | -------------- |
| `bash` | All scripts | No (not Mise’s job) | Always host | Host is fine; nixpkgs bash only for hermetic scripts |
| `jq` | Hooks, tests, JSON glue | Weak (aqua/asdf plugins possible) | apt / brew / NixOS `jq` | Version rarely critical; host OK |
| `gh` | Kanban / PR scripts | Weak | Official GitHub CLI install or NixOS | Pin optional; host OK (ADR-001 style) |
| `git` | Everywhere | No | Host / NixOS | Never wrap per skill |
| `cargo` / `rustc` | `cdamux` tests in `verify.sh` | **Yes** — Mise `rust` backend or rustup | Distro rustc (often too old) | Mise/rustup pins toolchain; loses single nixpkgs pin unless versions locked in `mise.toml` |
| `tmux` | orch / cdamux integration | Weak | Host / `cda.aitools` systemPackages | Session tool; already (c) via aitools when enabled |
| `flock` | Concurrent orch / scripts | No | util-linux on Linux; may be missing on macOS | Host on Linux; macOS may need brew `flock` or keep Nix fallback |
| `nodejs` | floorplan + cdamux web selfchecks | **Yes** — Mise `node` | nvm/fnm/brew/NixOS | Mise pins Node well; matches agent pain (repeated `nix develop` for Node) |
| `util-linux` | `setsid`, `flock` bits | No | Linux distro package; not on Darwin | Darwin gap is why shell pulls it from nixpkgs |

**Summary for (a):** Mise is a strong substitute for **Node** and **Rust** only. The rest of the shell is OS/CLI tooling better left to host or NixOS/home-manager packages. Full `nix develop` hermeticity (one lockfile for every binary) is **not** what Mise replaces one-for-one.

---

## 2. `nix/`

| File | Class | Notes |
| ---- | ----- | ----- |
| `nix/maid.nix` | **(c)** | Declares `file.home` links: skills (`SKILL.md` trees), Cursor agents/plugins, `claude/CLAUDE.md`. Eval-time tree walk. No PATH tools. |

Mise cannot replace this. Delivery stays `install.sh` and/or nix-maid / `nixosModules.default`.

---

## 3. `aitools` Nix

Experimental companion (`cda.aitools`). Never enabled by `install.sh` (ADR-001 / ADR-003). Map #284 marks rewriting these modules in Mise as **out of scope**.

### Packages (b)

| File | Class | Notes |
| ---- | ----- | ----- |
| `aitools/nix/moshi-hook-package.nix` | **(b)** | Fetches pinned unfree CDN tarball per system; `allowUnfreePredicate` for `moshi-hook` in flake |
| `aitools/nix/graphify-package.nix` | **(b)** | Pinned PyPI wheel `graphifyy` + relaxed tree-sitter deps; incomplete grammar coverage vs `uv` path |

### Module / delivery (c)

| File | Class | Notes |
| ---- | ----- | ----- |
| `aitools/nix/common.nix` | **(c)** | Shared option schema (`enable`, `installMethod`, `deploy`, `cdamux`, `tmux`, `graphify`, services, …) |
| `aitools/nix/module-core.nix` | **(c)** (+ consumes **b**) | Assertions; `environment.systemPackages` (tmux, cdamux, moshi-hook, graphify/uv); env vars; optional maid deploy |
| `aitools/nix/nixos.nix` | **(c)** | NixOS entry: core + `linux-services.nix`; builds in-tree cdamux/graphify/moshi packages |
| `aitools/nix/darwin.nix` | **(c)** | nix-darwin entry: core + `darwin-services.nix` |
| `aitools/nix/module.nix` | **(c)** | Deprecated alias → `nixos.nix` |
| `aitools/nix/linux-services.nix` | **(c)** | mosh; systemd user oneshots/services (graphify, cdamux hooks, cdamuxd, moshi-hook) |
| `aitools/nix/darwin-services.nix` | **(c)** | launchd equivalents |
| `aitools/nix/maid-deploy.nix` | **(c)** | Maid fragments: `~/.tmux.conf`, XDG cdamux/tmux configs from templates |
| `aitools/nix/graphify-activation.nix` | **(c)** | Shell script for graphify skill register oneshot |
| `aitools/nix/cdamux-hooks-activation.nix` | **(c)** | Shell script: `cdamux-hooks wire` |

### What Mise cannot replace here

- System packages + systemd/launchd wiring for cdamuxd, moshi-hook, graphify activation, cdamux hooks  
- nix-maid config deploy for tmux/cdamux  
- In-tree Rust/Python package builds consumed by modules  

Host alternatives already exist in docs (`cdamux-align-mac.sh`, `uv tool install graphifyy`, `install.sh --cdamux-hooks`) — still not Mise.

---

## 4. `cdamux/nix`

| File | Class | Notes |
| ---- | ----- | ----- |
| `cdamux/nix/package.nix` | **(b)** | `buildRustPackage` for CLI + daemon; ships hooks under `share/cdamux/hooks` with bundled `node` symlink and `cdamux-hooks` wrapper (`jq`, `coreutils` on PATH) |

Daily *development* of cdamux uses shell **(a)** `cargo`/`rustc` (or host/Mise). Shipping the binary to machines is **(b)** + **(c)** via aitools. Mise `cargo install` / rustup can replace **local build PATH** but not the NixOS module install path without a conscious ADR.

---

## 5. Cross-cutting picture

```text
(a) Daily PATH          ←── flake.devShells.default
                          └── (candidate: Mise for node+rust; host for jq/gh/git/tmux/…)

(b) Package builds      ←── .#cdamux .#graphify .#moshi-hook
                          └── also called from aitools modules

(c) System / delivery   ←── nix/maid.nix, nixosModules.default,
                            nixosModules/darwinModules.cdaAitools,
                            packages.default/activate (maid)
```

| Question for later map tickets | Fact from this inventory |
| ------------------------------ | ------------------------ |
| Keep Nix `devShell` only? | Shell is a short list of CLIs; most are host-suitable; only Node/Rust are classic Mise pins |
| Mise alongside flake? | Would overlap **(a)** only; **(b)/(c)** stay Nix |
| Replace daily shell with Mise, keep Nix for packages/modules? | Matches the split above; Darwin `util-linux`/`flock`/`setsid` still need a host or Nix fallback story |
| Drop Nix packaging/modules? | Out of scope for #284; inventory shows they are the bulk of the flake surface |

---

## 6. Facts useful to #284 (no decision)

1. **(a)** is exclusively `devShells.default` (+ optional `formatter` via `nix fmt`). Everything else in the flake tree is **(b)** or **(c)**.
2. Mise (or rustup + fnm/nvm) can cover the **painful** part of (a) — Node and Rust — without touching aitools modules.
3. Host packages already align with ADR-001 and `nix-develop-shell` for `git`/`gh` and most CLIs; repeated `nix develop` cost is mostly evaluating a shell that re-provides those plus Node/Rust.
4. Pin/repro trade-off: flake shell = one `flake.lock` for all shell inputs; Mise = separate version files for language toolchains only; OS tools remain host-skewed either way if agents prefer bare PATH.
5. `packages.default` (maid activate) and `aiDevKit` / `cda.aitools` modules have **no Mise analogue** in this repo’s intended design.

---

## References

- `flake.nix` (devShell packages list, lines ~34–38)
- `nix/maid.nix`
- `aitools/README.md`, `aitools/nix/module-core.nix`
- `cdamux/nix/package.nix`
- `scripts/verify.sh`
- `skills/dev/nix-develop-shell/SKILL.md`
- Foam: ADR-001 (host tools vs kit), ADR-003 (cda.aitools)
)
