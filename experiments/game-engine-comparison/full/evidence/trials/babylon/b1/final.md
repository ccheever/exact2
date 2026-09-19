Implemented and verified the return-home objective.

Behavior now:

- Lighting all 12 lanterns starts a persistent “Return to the starting shore” stage.
- The game remains `playing`; the timer continues.
- Winning requires the fox to be within 2 horizontal units of `[0, 0, 12]`.
- Timeout during the return stage loses.
- Save/reload preserves the stage, timer, lanterns, player, and physics state.
- Existing inspection commands remain compatible, with additive `returningHome`, `objective`, and `home` fields.
- Geometry, physics, controls, Fox asset, and attribution are unchanged.

Proof results:

- Remote 12/12: still `playing`, 10.487 units from home, no win.
- Reload: restored pending return at tick 1482 with 155.3 seconds remaining.
- Timeout: reached `lost` at tick 10800 with all 12 lit.
- Physical return: reached `won` at home distance 1.949, using normal movement.
- Evidence: [full-route.json](/tmp/exact-full-games-20260917/trials/babylon/b1/game/evidence/full-route.json), [self-test.json](/tmp/exact-full-games-20260917/trials/babylon/b1/game/evidence/self-test.json)

Files changed:

- [src/main.js](/tmp/exact-full-games-20260917/trials/babylon/b1/game/src/main.js)
- [index.html](/tmp/exact-full-games-20260917/trials/babylon/b1/game/index.html)
- [style.css](/tmp/exact-full-games-20260917/trials/babylon/b1/game/style.css)
- [full-route.mjs](/tmp/exact-full-games-20260917/trials/babylon/b1/game/full-route.mjs)
- [README.md](/tmp/exact-full-games-20260917/trials/babylon/b1/game/README.md)
- `dist/` regenerated
- `evidence/` regenerated, including return-home and win screenshots

Exact tests:

```sh
bun run build
BASE_URL=http://127.0.0.1:41713/ bun run test
BASE_URL=http://127.0.0.1:41713/ bun run test:route
```

All passed. No repair rounds were needed.

Limitations: saves remain explicit localStorage saves, matching the existing game behavior. Home proximity intentionally ignores height and measures only horizontal X/Z distance as requested.