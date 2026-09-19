Implemented lantern-13 and its raised platform.

Changes:

- [level.json](/tmp/exact-full-games-20260917/trials/babylon/a2/game/level.json): added the static collidable platform at `[-8,0.6,7]`, size `[3,1.2,3]`, and lantern-13 at `[-8,1.2,7]`.
- [src/main.js](/tmp/exact-full-games-20260917/trials/babylon/a2/game/src/main.js): updated victory copy to thirteen.
- [index.html](/tmp/exact-full-games-20260917/trials/babylon/a2/game/index.html): updated HUD, title, and sign copy.
- [README.md](/tmp/exact-full-games-20260917/trials/babylon/a2/game/README.md): documented lantern-13 and its proof command.
- [package.json](/tmp/exact-full-games-20260917/trials/babylon/a2/game/package.json): added `test:lantern-13`.
- [proof-lantern-13.mjs](/tmp/exact-full-games-20260917/trials/babylon/a2/game/proof-lantern-13.mjs): new ordinary-input end-to-end proof.
- Regenerated production [dist](/tmp/exact-full-games-20260917/trials/babylon/a2/game/dist) and [evidence](/tmp/exact-full-games-20260917/trials/babylon/a2/game/evidence).

Proof results:

- Ground act at `[-7.939,0,5.006]` refused to light lantern-13.
- Real jump began grounded at y=0, peaked at y=1.807, and landed grounded on the platform at y=1.2.
- Lighting all original twelve left `phase: "playing"` and lantern-13 unlit.
- Press-edge act on the platform produced victory at `13 / 13`.
- Save, page reload, and load preserved lantern-13 and the won state.
- Shared Fox asset and license hashes remain unchanged.

Exact commands passed:

```sh
PORT=34541 bun run start
BASE_URL=http://127.0.0.1:34541/ bun run test:lantern-13
BASE_URL=http://127.0.0.1:34541/ bun run test
bun run build
```

The legacy `full-route.mjs` was intentionally not edited or run: it hard-codes the obsolete expectation that twelve lanterns wins, which now contradicts the requested behavior. No gameplay limitation found; proof used only normal movement, jump, act, fixed-step, save, and load commands—no teleport or state-setting shortcut.