# The web smoke fails intermittently on 'surface glass: duplicate live publisher ignored'

**Status:** Closed
**Resolution:** Fixed delayed GPU-load race: detached queued canvases no longer claim surface names, and stale publishers yield to live replacements; actual Caltrain reproduction, 66 surface tests and rebuilt web smoke pass.
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

## Later the same day

It stopped reproducing once `main` took the 44 commits between `c1ea6eb3c`
and `fbb33dbf`: 13 runs at `ad57fdc0` and after passed, 2 idle, 3 under a
concurrent release build, 6 with logging at the duplicate check (which never
fired), 2 more after it was removed. The one change in `host/web-js` among
them, `033d1980` (keyed rows), is not what fixed it: with it reversed, three
more runs passed. Not bisected further: the other candidates
(`a0cfeb57`/`aab2074d`, the render host's kernel-free page, which the page
adopts; `309a8027`, the GPU frame) need cold builds of older trees.

If it returns, log what the duplicate check sees before the `console.error`
in `host/web/gpu-glue.js` `surface()`: the old publisher's `view`,
`el.isConnected`, whether `surfaces.get(old.view) === old`, the new `view`,
and a stack. A stale old publisher (element gone, entry still registered)
would mean a create-before-destroy ordering, fixable by replacing a
disconnected publisher rather than ignoring the new one.

## It returned (2026-09-30, evening)

One run of four failed on the same single console error, at 50e47f888 plus
three commits that touch nothing in the web host (the render server's close,
the compiler's bound input `type`, the driver's simulator window). The
failing run took 228.7 s at a load average near 100; the three that passed
took 47 s each, minutes later, load near 40. So it is still there and shows
under load; it had been closed as no longer reproducing after 13 passing
runs at ad57fdc0. The probe above was not applied.

## Reproduced and fixed (2026-10-02)

The delayed-module path retains queued surfaces after their canvases leave.
Holding only `gpu-glue.js` in the real Caltrain page, clicking Sky off and on
with browser mouse events, then releasing the response deterministically
reproduced the error: the queue held old `glass` view 1 with
`isConnected=false` and replacement view 4 with `isConnected=true`. The
detached entry claimed the publisher name before the live entry arrived.

`surface()` now ignores disconnected targets and retires a named publisher
whose canvas or view registration is stale before admitting its replacement.
Two genuinely live canvases still produce the duplicate diagnostic. A late
destroy of the retired view cannot clear the replacement's publication.

The same delayed-load Caltrain drive now reports zero console errors and one
live sky. The existing surface-record suite passes all 66 tests, including
new detached-queue and replacement/late-destroy cases and the retained real
duplicate test. Rebuilt Caltrain web smoke (`--app-only`) passes with its
three Contract tests. Existing fixture mocks were brought up to date with
the host's timeline and document-scroll bookkeeping.
