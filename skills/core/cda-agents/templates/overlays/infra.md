
## Stack — Infrastructure / Nix

- **Default: no `nix develop`.** Host / activated Mise / direnv first;
  `nix develop -c` only after `command not found` / wrong version, once per
  batch (**nix-develop-shell**, ADR-013). Never wrap `git`/`gh`/OS tools
  because `flake.nix` exists; never a fresh develop per small command.
- New projects: **nix-direnv-setup** for flake + direnv scaffold.
- Deployment / hosting decisions → ADR in `foam/decisions/`.
