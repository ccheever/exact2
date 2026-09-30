# The web smoke fails intermittently on 'surface glass: duplicate live publisher ignored'

**Status:** Open
**Systems:** web host (GPU glue), smoke
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30

`bun scripts/smoke.mjs web` failed in 6 of 7 runs on 2026-09-30, each time on exactly one failure, a page console error:

```
the host reported errors:
  console.error: exact gpu: surface glass: duplicate live publisher ignored
```

Every other smoke step passed (including the three Contract tests). The message comes from `host/web/gpu-glue.js` (added in a904844e9, 2026-09-17). It is not the LLP 1012.001.000 change: it failed with that change's `host/web-js` files at HEAD, and with its driver scripts at HEAD, and it failed and passed both with and without Homebrew (binaryen 133) on `PATH`. So it reads as a race in how Caltrain's `glass` surface is published (two publishers live at once, perhaps across the material toggle or a reload), not an environment effect.

To look at: what publishes `glass` twice in one page during the smoke's drive, and whether the second publisher is legitimate (then the message is not an error) or a leak (then it is the bug). The Linux smoke is green.
