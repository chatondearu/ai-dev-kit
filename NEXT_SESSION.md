# NextSession — ai-dev-kit (handoff 2026-11-11, Mac → Linux)

## Reprendre en 3 commandes

```bash
cd <chemin>/ai-dev-kit && git fetch origin && git switch main && git pull --ff-only
gh pr view 240 --web          # PRD-239 à approuver
claude                        # puis colle le prompt en bas de ce fichier
```

## Où on en est

| Sujet | État |
| ----- | ---- |
| cdamux spawn UX (agent repris, bascule auto, popup sur session courante) | ✅ mergé (#222) |
| Statuts Fleet : bug `#{pane_idle_time}` inexistant → `window_activity` | ✅ mergé (#220) |
| Règles : commit/push libres hors branche par défaut, fast lane fix trivial, bootstrap foam sur demande | ✅ mergé (#221) |
| Carte explore « Unified cdamux state model » (#228) | ✅ fermée, 7 tickets tranchés |
| ADR-011 modèle unifié (§1 entités, §2 `CDAMUX_HOME`, §3 statuts, §4 messages, §5 orch.sh) | ✅ accepté, mergé (#238) |
| PRD-239 orchestrateur (epic #239) | ⏳ **PR #240 en review — à approuver** |
| Bugs trouvés en route : #236 (orch run-state leaks), #237 (KISS hook ignoré par Cursor) | ouverts |

## Prochaines étapes (dans l'ordre)

1. **Approuver le PRD** : relire « Requirements » dans PR #240, merger (statut → `approved`).
2. **Tranche 1 — statuts réels** : `cda-plan` → `plans/PLAN-<n>-*.md`, intake `cda-dev` (issues enfants
   de #239), worker. Contenu : `CDAMUX_HOME` / `CDAMUX_CONFIG`, `cdamux state set|get|append`,
   id `CDAMUX_AGENT_ID`, noms clients `claude|cursor|opencode|codex`, 7 statuts + précédence,
   hooks de statut dans `install.sh --cdamux-hooks`. ~½ journée.
3. Tranches suivantes (PRD-239) : 2 messages/workers/orchestrateur → 3 bascule `orch.sh` → 4 Fleet UI, restore, GC.

## À faire sur le poste Linux (spécifique)

- **Vérifier la question ouverte n°1 du PRD** : est-ce que `cursor-agent` déclenche vraiment ses hooks
  (`sessionStart`, `beforeSubmitPrompt`, `stop`) ? Test : un `.cursor/hooks.json` de projet jetable
  qui écrit dans un fichier, puis `cursor-agent -p "hi"`. Le classifieur l'a bloqué sur le Mac.
- **cdamux à jour** : sur le Mac il vient de nix-darwin (store Nix). Sur Linux, rebuild ta config
  (home-manager / NixOS) après `nix flake update` de l'input ai-dev-kit, ou
  `nix build .#cdamux` dans le repo pour tester.
- **Popup** : vérifier que `M-p` sur une session la présélectionne, et qu'Entrée sur une nouvelle
  session ferme la popup (dépend de `TMUX_PANE` absent dans une popup — non vérifié).
- **Règles Claude** : `claude/CLAUDE.md` du repo est la source ; vérifie que ton `CLAUDE.md` Linux
  pointe dessus (symlink posé par `install.sh`). Cursor : recoller les règles dans Settings → Rules.
- Jamais `install.sh` depuis un test : `HOME`, `CLAUDE_CONFIG_DIR`, `CURSOR_HOME`,
  `XDG_CONFIG_HOME` en temp dirs.

## Liens

- Epic : https://github.com/chatondearu/ai-dev-kit/issues/239
- PRD (PR) : https://github.com/chatondearu/ai-dev-kit/pull/240
- ADR : `foam/decisions/ADR-011-cdamux-unified-state-model.md`
- Recherches : branches `research/state-stores-inventory`, `research/client-capabilities`

## Prompt à coller dans la nouvelle session

```
Reprise du handoff NEXT_SESSION (branche handoff/next-session). Contexte : epic #239,
PRD-239 (PR #240), ADR-011. Si la PR #240 est mergée, lance cda-plan pour la tranche 1
(statuts réels) du PRD-239, puis l'intake cda-dev pour créer les issues enfants de #239.
Sinon, rappelle-moi d'approuver la PR #240 d'abord. Vérifie aussi la question ouverte
« Cursor CLI hooks » sur ce poste Linux.
```
