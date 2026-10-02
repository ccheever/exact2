# The web smoke fails intermittently on 'surface glass: duplicate live publisher ignored'

**Status:** Closed
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

## Fixed

The hypothesis was correct. A batch can create a replacement surface after
DOM reconciliation has disconnected the old canvas but before the old view's
`destroy` operation runs. `surfaces` therefore still contained the retired
entry, and `publishers` treated its name as live even though the same `live()`
predicate used by input, records, and agent operations already rejected it.

Surface creation now replaces a publisher that fails `live()`. Destruction of
the stale entry cannot clear its replacement because destruction already
checks publisher identity. A deterministic create-before-destroy test covers
that ordering, while the existing test continues to require an error for two
connected publishers. The JS target copies and uses the same `gpu-glue.js`, so
the fix applies to both web targets.
