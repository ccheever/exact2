Implemented and verified lantern-13.

Changes:

- [level.json](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/level.json): added the `3 × 1.2 × 3` platform at `[-8, 0.6, 7]` and lantern-13 at `[-8, 1.2, 7]`; original twelve positions unchanged.
- [src/main.js](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/src/main.js): stone platform appearance, thirteen-lantern victory text, and legacy save normalization.
- [main.js](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/main.js): kept duplicate source consistent.
- [index.html](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/index.html): displayed total and introduction updated to thirteen.
- [README.md](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/README.md): documented the new platform and persistence behavior.
- [tests/lantern13-proof.mjs](/tmp/exact-full-games-20260917/trials/playcanvas/a1/game/tests/lantern13-proof.mjs): deterministic browser gameplay proof.
- Rebuilt production files in `dist/`, including `game.js`, `index.html`, and `level.json`.

Exact verification:

- `bun run build` — passed.
- Served `dist/` on port 4813 and ran headless Chromium with debugging on port 9323.
- `bun tests/lantern13-proof.mjs` — passed:
  - HUD and state total: `13`.
  - Ground interaction at `[-8.33, 0, 8.88]`: count remained `0`.
  - Real ground jump landed on the collider at feet `y=1.1999966`.
  - Lantern-13 lit through a press-edge interaction.
  - Page reload plus save/load preserved lantern-13.
  - All original twelve lit with lantern-13 dark: `count=12`, `phase="playing"`.
  - Jumping to and lighting lantern-13: `count=13`, `phase="won"`.

Limitations: the proof runner expects a local server on port 4813 and Chromium’s debugging endpoint on 9323. Headless Chromium emitted harmless software-WebGL warnings. No gameplay shortcuts, teleports, dependency changes, asset changes, evaluator edits, or commits were used.