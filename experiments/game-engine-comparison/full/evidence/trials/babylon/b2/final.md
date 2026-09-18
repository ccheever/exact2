Implemented and verified the return-home objective.

Changes:

- [src/main.js](/tmp/exact-full-games-20260917/trials/babylon/b2/game/src/main.js): pending-return state, timer continuation, 2-unit home check, persistence, win/loss events, updated terminal messaging.
- [index.html](/tmp/exact-full-games-20260917/trials/babylon/b2/game/index.html): explicit return-home HUD instruction and updated introduction.
- [style.css](/tmp/exact-full-games-20260917/trials/babylon/b2/game/style.css): persistent highlighted objective styling.
- [return-home-proof.mjs](/tmp/exact-full-games-20260917/trials/babylon/b2/game/return-home-proof.mjs): full ordinary-input acceptance proof.
- [package.json](/tmp/exact-full-games-20260917/trials/babylon/b2/game/package.json): added `test:return-home`.
- [README.md](/tmp/exact-full-games-20260917/trials/babylon/b2/game/README.md): documented the new rule and proof command.
- Regenerated `dist/` and `evidence/`.

Exact successful commands:

```sh
bun run build
BASE_URL=http://127.0.0.1:45293/ bun run test
BASE_URL=http://127.0.0.1:45293/ bun run test:return-home
```

The proof confirmed:

- All twelve lit remotely remains `playing`; timer advanced another 30 ticks.
- Page reload preserved all twelve lights, pending return, position, ticks, and remaining time.
- Expiring the timer during the return produced `lost`.
- Normal movement home produced `won` at distance `1.959526` from `[0,0,12]`, with `151.45s` remaining.
- No teleport/debug commands were used.

Evidence: [return-home-proof.json](/tmp/exact-full-games-20260917/trials/babylon/b2/game/evidence/return-home-proof.json), [return-home.png](/tmp/exact-full-games-20260917/trials/babylon/b2/game/evidence/return-home.png), and [home-win.png](/tmp/exact-full-games-20260917/trials/babylon/b2/game/evidence/home-win.png).

Limitations: the untouched legacy `full-route.mjs` still expects the obsolete immediate win and is therefore no longer a valid assertion for the changed rules. Geometry, physics, controls, inspection commands, Fox asset, and attribution were unchanged.