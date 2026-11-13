# Research: Mise cost/benefit for multi-agent verify loops

**Ticket:** [#286](https://github.com/chatondearu/ai-dev-kit/issues/286) — *Research: Mise cost/benefit for multi-agent verify loops*  
**Map:** [#284](https://github.com/chatondearu/ai-dev-kit/issues/284) — *Dev toolchain: flake.nix vs Mise for daily agent loops*  
**Related inventory:** [`research/flake-vs-mise-inventory.md`](flake-vs-mise-inventory.md) (#285)  
**Date:** 2026-11-13  
**Scope:** Facts about cold start, disk, shims, CI, and documented Mise limits vs Nix flakes for a Rust+Node+bash kit like this one. **No product decision.**

## Question

What does Mise buy for agent-heavy workflows (cold start, disk, shims, CI) compared to host PATH + occasional `nix develop`?

## Baseline in this repo (context, not a recommendation)

- `devShells.default` provides: `bash jq gh git cargo rustc tmux flock nodejs util-linux` (`flake.nix`).
- Operator verify: `nix develop -c bash scripts/verify.sh` (`AGENTS.md`).
- Agent skill `nix-develop-shell`: prefer host PATH; wrap with `nix develop -c` only when a project toolchain is missing or wrong-version. Notes that `nix develop` is often hundreds of ms warm and seconds cold, and that many agents wrapping every command hammer disk/I/O.
- Inventory #285: Mise is a strong substitute only for **Node** and **Rust** in class (a); OS CLIs and all (b)/(c) Nix surfaces stay outside Mise’s job.

---

## 1. What each mechanism actually does

| Mechanism | What it pins | How agents get tools | Hermeticity |
| --------- | ------------ | -------------------- | ----------- |
| **Host PATH** | Nothing in-repo | Bare `node` / `cargo` / `bash` | Machine-local; skew across hosts |
| **Occasional `nix develop -c …`** | Full `devShell` closure via `flake.lock` + nixpkgs | Flake eval → shell env → one command | Pins compilers, shared libs, and CLI packages in the shell |
| **Mise (activate / exec / run)** | Declared `[tools]` (+ optional `mise.lock`) | PATH rewrite, or one-shot env around a command/task | Pins **runtime/tool versions**, not the host’s system library closure |
| **Mise shims** | Same as above | Symlink to `mise` on PATH; resolve on every exec | Same as Mise; plus documented shim limits (below) |

Sources: [mise.dev-tools](https://mise.jdx.dev/dev-tools/), [nix.dev flakes](https://nix.dev/concepts/flakes.html), LocalEnvAuto toolchain comparison (mise/asdf vs Nix hermeticity).

---

## 2. Cold start

### Nix flakes / `nix develop`

Documented flake properties ([nix.dev](https://nix.dev/concepts/flakes.html)):

- Builds/shells are cached; identical later builds reuse the store.
- First use on an empty store downloads or builds the **whole dependency closure** (binary cache common case: download, not compile).
- Flakes copy the flake directory into the store; large repos pay evaluation/copy cost.
- Pure mode + `flake.lock` pin the package graph revision.

Local observation on this machine (2026-11-13, after some store paths already present): a warm-ish `nix develop -c true` completed in about **1s wall** while still fetching remaining shell inputs from `cache.nixos.org` (e.g. cargo). Cold (empty store) is dominated by closure download size for `nodejs` + `rustc`/`cargo` + the rest of the shell list — seconds to minutes depending on cache and bandwidth. The kit skill’s “hundreds of ms warm / seconds cold” order of magnitude matches that pattern once the store is populated.

**Agent-loop implication:** every fresh `nix develop -c …` re-enters flake evaluation / shell construction. Parallel agents that wrap *every* toolchain call pay that tax repeatedly even when host already has usable binaries.

### Mise

Documented install / first-use path ([getting started](https://mise.jdx.dev/getting-started.html), [CI](https://mise.jdx.dev/continuous-integration.html), [lockfile](https://mise.jdx.dev/dev-tools/mise-lock)):

- First `mise install` / `mise exec` downloads tool archives into mise’s download cache, then extracts under the data dir (defaults: `~/.cache/mise`, `~/.local/share/mise`).
- Without `mise.lock`, each tool may hit a remote version list (cache TTL; default remote-list cache on the order of an hour). With lockfile + `mise install --locked`, CI skips version lookup and uses pinned URLs/checksums where the backend records them.
- **Node** (`core:node`): archive download — cold start ≈ download + extract of one Node build.
- **Rust** ([lang/rust](https://mise.jdx.dev/lang/rust.html)): goes through **rustup** (`rustup toolchain install`); toolchains live in `RUSTUP_HOME` (default `~/.rustup`), not only under mise `installs/`. Cold start ≈ rustup-init (if needed) + toolchain download — similar order of magnitude to installing Rust outside Nix, independent of nixpkgs.
- After tools are installed, `mise exec -- cmd` / activated PATH does **not** re-evaluate a flake; cost is config read + env overlay (see §3).

**Compared to host PATH:** cold start is zero if the right binaries are already on PATH. Mise/Nix only pay when installing or entering an env.

**Compared to each other (cold, empty machine):** both download large Node + Rust payloads. Nix also pulls the rest of the shell closure (jq, gh, git, tmux, util-linux, …) into `/nix/store`. Mise typically downloads only declared language tools; OS CLIs stay on the host (or mise `[bootstrap.packages]`, which are machine-wide — see §5).

---

## 3. Per-command / shim overhead (warm path)

### Host PATH

- Near-zero wrapper cost: `execve` of the real binary.
- Skill ladder already encodes this as the preferred agent path when versions are acceptable.

### `nix develop -c <cmd>`

- Pays flake/shell entry on **each** wrap (unless the process is already inside `IN_NIX_SHELL` / direnv).
- Cost is evaluation + env setup, not shim-per-binary; wrapping a whole `verify.sh` once is cheaper than wrapping every `node`/`cargo` inside it.

### Mise activation (interactive / long-lived shell)

- [Shims docs](https://mise.jdx.dev/dev-tools/shims.html): `mise activate` puts real tool `bin` dirs on `PATH` and refreshes on prompt/`cd`.
- After activation, `node` / `cargo` are normal binaries → **no per-call mise process** for those tools.
- `[env]` variables appear in the shell only under activation (or via `mise exec` / `mise run`).

### Mise shims (non-interactive / IDE / agent without activate)

Documented behavior ([shims](https://mise.jdx.dev/dev-tools/shims.html), [jdx.dev shim post](https://jdx.dev/posts/2024-04-13-shims-how-they-work-in-mise-en-place/)):

- Unix shims are **symlinks to the mise binary**; each invocation starts mise, loads config for cwd, then execs the real tool.
- Recommended by upstream for CI/scripts/IDEs that never run a prompt hook; **not** recommended as the primary interactive mode (`which` shows the shim; `[env]` not visible until a shim runs).
- Documented limits that matter for agents/scripts:
  - `command -v` / `which` can **false-positive** “installed” because the shim exists even when the version is missing for this directory.
  - Unresolved shim can fall through to the next same-named binary on PATH (`not_found_system_fallback`) — silent wrong tool risk for names like `python3`.
  - Shims do not run `cd`/`enter`/`leave` hooks; they do apply tool env when the shimmed command runs.

**Measured overhead (upstream, experimental fast-path PR notes on linux-x64):** on current main without `MISE_EXEC_CACHE`, shim / `mise x -- true` on the order of **~10–11 ms** wall vs **~2 ms** for raw `node --version`; with opt-in `MISE_EXEC_CACHE=1` hit path ~**4–6 ms**. Residual cost dominated by fat-binary startup. Treat as order-of-magnitude from maintainer benches, not a guarantee for every host.

**Agent-loop implication:**

| Pattern | Overhead shape |
| ------- | -------------- |
| Host PATH | Best for many short tool calls inside `verify.sh` |
| One `nix develop -c bash scripts/verify.sh` | One shell entry + full suite |
| One `mise exec -- bash scripts/verify.sh` (or `mise run`) | One mise env load; children see real PATH bins |
| Shims on PATH for every `node`/`cargo`/`npm` inside scripts | **N × shim startup** — worst for chatty bash/node suites |
| Agents wrapping every command with `mise exec` or `nix develop -c` | Same class of mistake: multiply wrapper cost |

Upstream explicitly points scripts/CI at **`mise exec` / `mise run`**, not at requiring activation ([CI docs](https://mise.jdx.dev/continuous-integration.html)).

---

## 4. Disk

| Store | Typical contents for this kit’s (a) tools | Notes |
| ----- | ---------------------------------------- | ----- |
| **Host** | Distro / manual installs | No kit-managed footprint; versions unbound |
| **`/nix/store`** | Content-addressed closure for every shell input + GC roots | Shared across projects; grows with generations; GC reclaimable; hermetic but heavy ([LocalEnvAuto](https://www.local-environment-automation.com/developer-onboarding-architecture-friction-mapping/toolchain-version-management/)) |
| **Mise data** (`~/.local/share/mise` default) | Per-version installs (Node, plugins, shims) | [Directories](https://mise.jdx.dev/directories.html): do not copy installs between unrelated machines |
| **Mise cache** (`~/.cache/mise`) | Version lists, download CAS, env caches | Clearable; CI docs prefer caching **downloads-cas** keyed by `mise.lock` |
| **Rust via Mise** | `~/.rustup` / `~/.cargo` (or `settings.rust.*`) | Separate from mise `installs/`; can duplicate a Nix-provided `rustc`/`cargo` if both exist |

**Multi-agent / multi-worktree:** Mise and rustup installs are usually **user-global** and shared across worktrees (one Node 24, one rustup toolchain). Nix store paths are also shared machine-wide once fetched. Neither creates a full second toolchain tree per worktree by default. Disk pain for agents is more often **repeated evaluation/I/O** (Nix) or **duplicate stacks** (host + mise + nix all installed) than per-worktree isolation.

---

## 5. Documented Mise limits vs Nix (Rust + Node + bash)

Facts from Mise docs and comparative write-ups; useful constraints for later ADR tickets — still **not** a choice.

1. **Hermeticity gap.** Mise/asdf-class managers pin runtime versions; they **inherit host system libraries** (OpenSSL, glibc, libpq, C toolchain). Nix flakes pin the **entire closure** via `flake.lock`. Same Node major can still behave differently across machines under Mise if native addons or linked libs differ.
2. **`[bootstrap.packages]` are not a per-project Nix shell.** Mise can install apt/brew/nix profile packages for the **machine**, without per-directory version switching or shims ([bootstrap packages](https://mise.jdx.dev/bootstrap/packages/)). The `nix` manager uses `nix profile install` and **rejects version pins** like `nix:ripgrep@14`; pin the flake/registry source instead ([nix packages](https://mise.jdx.dev/bootstrap/packages/nix.html)).
3. **Rust is rustup, not nixpkgs.** Mise sets `RUSTUP_TOOLCHAIN` and installs via rustup. Lockfile records the top-level Rust version; the language installer resolves the rest ([mise.lock backend table](https://mise.jdx.dev/dev-tools/mise-lock)). That diverges from `cargo`/`rustc` taken from a single nixpkgs revision in `devShells.default`.
4. **Node is a first-class core tool.** Strong fit for pinning and CI lockfiles; does not by itself provide `jq`/`gh`/`tmux`/`flock`/`setsid`.
5. **Bash / util-linux.** Not Mise’s product surface. Darwin gaps for `flock`/`setsid` (why the flake pulls `util-linux`) remain a host-or-Nix problem under a Mise-only daily PATH (see #285 inventory).
6. **Shim semantics vs agent heuristics.** False `command -v` success and system fallback can confuse “is the toolchain present?” checks agents use before deciding to wrap.
7. **Trust / config execution.** Project `mise.toml` with `[env]`, hooks, templates requires trust; `mise install` / `exec` / `run` trust the project ([getting started](https://mise.jdx.dev/getting-started.html)). Relevant for agents auto-running in cloned worktrees.
8. **Nix still owns (b)/(c) in this kit.** Package builds and nix-maid / NixOS / Darwin modules have no Mise replacement in map #284 scope (#285).

---

## 6. CI angle (factual)

Mise ([continuous integration](https://mise.jdx.dev/continuous-integration.html)):

- Commit `mise.toml` + `mise.lock`; `mise install --locked`; run via `mise exec` / `mise run` (no shell activation).
- Prefer caching download CAS (`~/.cache/mise/downloads-cas`) keyed by OS + `hashFiles('mise.lock')`; verify checksums from the lockfile.
- Pin mise itself separately if the pipeline must control the manager version.

Nix:

- CI reuses `/nix/store` + binary cache; flake lock pins inputs.
- This repo currently has **no** GitHub Actions PR checks (`AGENTS.md`); orch may allow empty checks. So CI cost is mostly **local/agent verify loops**, not GHA minutes — but the same cold-start/disk mechanics apply on AFK hosts running `orch-auto` / workers.

---

## 7. Cost/benefit matrix for multi-agent verify loops (descriptive)

| Factor | Host PATH + rare `nix develop` | Mise for Node/Rust (+ host for the rest) | Always `nix develop -c` for toolchain |
| ------ | ------------------------------ | ---------------------------------------- | ------------------------------------- |
| **Cold start (empty machine)** | Install host tools somehow; Nix only when fallback fires | Download Node archive + rustup toolchain | Download full shell closure into store |
| **Warm per-verify** | Fast if tools present; one Nix wrap if fallback | Fast if activated or one `mise exec` around verify; slow if every binary hits shims | One flake shell entry per wrapped verify (OK); many wraps (bad) |
| **Disk** | Smallest kit footprint | mise + rustup homes; can coexist with Nix → duplication | `/nix/store` growth; shared, GCable |
| **Version alignment across agents** | Weak unless hosts are uniform | Strong for declared Node/Rust (+ lockfile) | Strong for entire shell via `flake.lock` |
| **Native/lib reproducibility** | Host | Host (+ optional machine-wide bootstrap) | Shell closure hermetic |
| **Fit to `scripts/verify.sh`** | Matches current skill ladder | Covers `node` + `cargo`/`rustc` needs; not bash/jq/gh alone | Covers whole declared shell list |
| **Shim hazard for agents** | N/A | Real if shims on PATH without activate | N/A (different failure mode: eval spam) |

---

## 8. Findings useful to map #284 (still no decision)

1. **Mise’s clear buy for agent loops** is cheap, per-project **Node + Rust** selection without paying flake eval on every command — provided agents use **host/activated PATH or one outer `mise exec`/`mise run`**, not per-binary shims for chatty scripts.
2. **Mise does not remove** the kit’s need for host or Nix for bash/OS CLIs, Darwin `util-linux` bits, or any (b)/(c) packaging/modules (#285).
3. **Host PATH + occasional Nix** already targets the documented pain (I/O from wrapping); Mise competes with that pattern on the **language toolchain** slice, not with NixOS/nix-maid delivery.
4. **Shim tax (~ms per call, upstream benches) and `command -v` false positives** are concrete agent hazards if shims are the default PATH strategy.
5. **Reproducibility trade:** `flake.lock` hermetic shell vs `mise.lock` + rustup/Node archives without system-lib pinning — different guarantees; both are lockable within their model.

---

## References

### Mise (primary)

- https://mise.jdx.dev/getting-started.html
- https://mise.jdx.dev/dev-tools/
- https://mise.jdx.dev/dev-tools/shims.html
- https://mise.jdx.dev/dev-tools/mise-lock
- https://mise.jdx.dev/continuous-integration.html
- https://mise.jdx.dev/directories.html
- https://mise.jdx.dev/cache-behavior.html
- https://mise.jdx.dev/lang/rust.html
- https://mise.jdx.dev/dev-tools/backends/
- https://mise.jdx.dev/bootstrap/packages/
- https://mise.jdx.dev/bootstrap/packages/nix.html
- https://jdx.dev/posts/2024-04-13-shims-how-they-work-in-mise-en-place/
- https://github.com/jdx/mise/pull/12291 (shim/`mise x` warm-path timing notes)

### Nix

- https://nix.dev/concepts/flakes.html

### Comparative (hermeticity claims)

- https://www.local-environment-automation.com/developer-onboarding-architecture-friction-mapping/toolchain-version-management/
- https://betterstack.com/community/guides/linux/nix-vs-asdf/

### This repo

- `flake.nix` (`devShells.default`)
- `scripts/verify.sh`
- `AGENTS.md`, `skills/dev/nix-develop-shell/SKILL.md`
- `research/flake-vs-mise-inventory.md` (#285)
)
