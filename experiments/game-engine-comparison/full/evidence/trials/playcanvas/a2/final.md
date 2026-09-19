Implemented lantern-13 and its raised, collidable platform.

Changed files:

- [level.json](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/level.json:12) — platform and lantern-13; original twelve unchanged.
- [src/main.js](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/src/main.js:273) — dynamic 13-lantern victory text and legacy 12-lantern save migration.
- [index.html](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/index.html:15) — displayed total/title copy updated.
- [package.json](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/package.json:10) — proof command.
- [tests/lantern-13-proof.mjs](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/tests/lantern-13-proof.mjs:1) — physics and gameplay proof.
- [README.md](/tmp/exact-full-games-20260917/trials/playcanvas/a2/game/README.md:3) — updated description.
- Rebuilt production files under `dist/`.

Exact verification:

- `bun run build` — passed.
- `bun run proof` — passed first run, no repair rounds.
  - Ground interaction refused at distance `2.371`.
  - Real jump reached feet-height `1.635`.
  - Collision landing: `[-8.000, 1.179, 8.356]`, on the platform footprint.
  - Held act did not retrigger; release and new press lit lantern-13.
  - At 12 lit, phase remained `playing`; lantern-12 remained required.
  - Save round-trip contained 13 entries and restored lantern-13.
- Production manifest/source assertions — passed: total/UI/platform, 1.5 interaction radius, press-edge behavior, victory total, ID-based persistence, and old 12-of-12 win migration.

Limitation: no browser automation runtime was installed, so the movement proof runs against the shipped Ammo.js engine using the game’s exact collision bodies, parameters, controls, and 60 Hz step rather than through a rendered browser session. Shared assets and attribution were unchanged.