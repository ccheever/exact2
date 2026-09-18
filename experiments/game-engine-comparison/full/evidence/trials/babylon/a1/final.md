Implemented lantern-13 with normal Babylon/Havok gameplay.

Files changed:

- [level.json](/tmp/exact-full-games-20260917/trials/babylon/a1/game/level.json:12) — colliding platform at `[-8, 0.6, 7]`, size `[3, 1.2, 3]`; lantern-13 at `[-8, 1.2, 7]`.
- [index.html](/tmp/exact-full-games-20260917/trials/babylon/a1/game/index.html:14) — displayed total and title copy updated to 13.
- [src/main.js](/tmp/exact-full-games-20260917/trials/babylon/a1/game/src/main.js:139) — dynamic victory heading.
- [package.json](/tmp/exact-full-games-20260917/trials/babylon/a1/game/package.json:9) — added proof command.
- [lantern-13-proof.mjs](/tmp/exact-full-games-20260917/trials/babylon/a1/game/lantern-13-proof.mjs) — ordinary-input acceptance route.
- [README.md](/tmp/exact-full-games-20260917/trials/babylon/a1/game/README.md) — controls and proof documentation.
- `dist/` regenerated.
- [proof evidence](/tmp/exact-full-games-20260917/trials/babylon/a1/game/evidence/lantern-13-proof.json) and [victory screenshot](/tmp/exact-full-games-20260917/trials/babylon/a1/game/evidence/lantern-13-won.png).

Tests:

- `bun run build` — passed.
- `BASE_URL=http://127.0.0.1:46167/ bun run test` — passed all existing regression checks.
- `BASE_URL=http://127.0.0.1:39763/ bun run test:lantern13` — final production proof passed:
  - Original twelve leave phase `playing`.
  - 12/13 save and reload correctly.
  - Ground-level `E` is refused.
  - Holding `E` through entry into range does not retrigger.
  - A real jump lands at `y=1.2` on the platform.
  - Fresh `E` press lights lantern-13 and wins at 13/13.
  - Completed victory reloads correctly.

One proof calibration initially released forward movement too early; it was corrected in one repair round. The legacy `full-route.mjs` was deliberately not edited and still encodes the obsolete expectation that twelve wins. No gameplay limitations found.