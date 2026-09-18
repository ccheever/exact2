Implemented the return-home objective while preserving twelve lanterns, geometry, physics, movement, inspection commands, and attribution.

Key behavior:

- All twelve lit now remains `playing`.
- A prominent “Return home before night” instruction appears.
- Timer continues during the return.
- Nightfall takes precedence over arriving on the deadline tick.
- Win requires horizontal distance ≤2 from spawn `[0,0,12]`.
- Save/reload restores the pending return, timer, player, crate, and lantern state.
- Inspection state now includes `returningHome` and derived `home` information.

Files changed:

- [src/main.js](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/src/main.js:6)
- [index.html](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/index.html:24)
- [styles.css](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/styles.css:33)
- [README.md](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/README.md:3)
- [tests/return-home-proof.mjs](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/tests/return-home-proof.mjs:1)
- Generated production files: `dist/game.js`, `dist/index.html`, `dist/styles.css`

Exact verification:

```sh
bun run build
./node_modules/.bin/http-server dist -p 4174 -c-1
bun tests/return-home-proof.mjs
```

Proof output:

```text
PASS remote all-lit stays playing at tick 1486, distance 10.38
PASS reload restores pending return and timer at tick 1486
PASS pending return times out and loses at tick 10800
PASS ordinary movement returned home and won at tick 1606, distance 1.99
```

The proof used ordinary inputs, including physically pushing the crate and jumping onto the ledge—no teleport or state-setting commands. It required two harness repair rounds. Port 4174 was used because 4173 was already occupied; only my server was stopped afterward.

Limitation: the added proof script references the warmed local Playwright and Chromium paths supplied in this environment. Game runtime has no new dependency. Fox assets and [attribution](/tmp/exact-full-games-20260917/trials/playcanvas/b2/game/assets/Fox-LICENSE.md) were unchanged.