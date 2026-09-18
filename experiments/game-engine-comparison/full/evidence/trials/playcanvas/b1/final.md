Implemented the return-home objective while preserving twelve lanterns, geometry, physics, controls, and inspection commands.

Changed:

- [src/main.js](/tmp/exact-full-games-20260917/trials/playcanvas/b1/game/src/main.js:216) — pending-return state, two-unit horizontal home check, timeout behavior, save/load persistence.
- [index.html](/tmp/exact-full-games-20260917/trials/playcanvas/b1/game/index.html:39) — explicit return-home instruction.
- [styles.css](/tmp/exact-full-games-20260917/trials/playcanvas/b1/game/styles.css:32) — instruction banner styling.
- [README.md](/tmp/exact-full-games-20260917/trials/playcanvas/b1/game/README.md:3) — updated rules and persistence notes.
- [tests/return-home-proof.mjs](/tmp/exact-full-games-20260917/trials/playcanvas/b1/game/tests/return-home-proof.mjs:1) — real-browser, ordinary-control proof.
- Rebuilt `dist/game.js`, `dist/index.html`, and `dist/styles.css`.

Verification:

```sh
bun run build
./node_modules/.bin/http-server dist -p 4187 -c-1
LANTERNS_URL='http://127.0.0.1:4187/?agent=1' bun tests/return-home-proof.mjs
```

Proof results:

- All lit remotely: remained `playing`, pending return, 11.02 units from home.
- Reload: restored `playing`, 12/12, pending return, and identical tick `1405`.
- Timeout during return: `lost` at tick `10800`, remaining `0`.
- Return home: `won` at horizontal distance `1.946`.

No teleport or state-setting commands were used; the proof physically pushes the crate, jumps to the ledge, lights every lantern, and walks home. Level data, Fox asset, and attribution remain unchanged.

Limitation: the proof script defaults to the locally cached Chromium path; set `CHROME_BIN` when running elsewhere.