# Code review: the JS target's port of #1095's collection anchor, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `d5608abb78`.
- **Method:** one brief (sha256 `476735e0299b7df85349d848bc557c1f131aff5c290a21987449f9d8cd542d61`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Both round-1 items are resolved. The runtime port matches `7b0352262f` branch for branch, and the new oracles check the behavior they name.

**Astra 1 is resolved.** `host/web-js/list.test.mjs:783` awaits `exact.lists.pending()` (the controller import, `host/web-js/list.js:944`) and `exact.lists.settle()` before reading geometry. The assertions at `list.test.mjs:796-798` check the opening anchor: `beyond === 0`, `anchorOffset(...) === maxOffset(port)`, restore to that max. There is no row-growth claim.

Setting `c.atEnd = true` at `list.test.mjs:786` is the real branch. `anchorOffset` is `this.atEnd ? this.index.maxOffset(port) : offset` (`list.js:342`), the same predicate as `anchor_offset` in `runner/src/instance/collection/start.rs:59-64`. `settle()` has already run `settleStart` (`list.js:358-361`), which clears `atEnd` once the list is open, so the flag has to be turned back on to observe that branch. The calls are the production methods (`anchor`, `anchorOffset`, `restoreAnchor`), and the result is not stubbed. The assertion that pins the `#1095` change is `offset === max` (`list.test.mjs:788`, `797`). With this list (40 rows of 50 px, port 300), a function that still returned `extent` (`scroll_extent`) would yield 2000 against `maxOffset` 1700, and `beyond` would be 300 because `anchor()` feeds that value through. While `atEnd` is set, `follows()` is true (`list.js:340`), so the anchor follows the end and `restoreAnchor` is `max + beyond` (`list.js:206`). `beyond === 0` and `restored === max` are that anchor. `gap === 0` (`list.test.mjs:784`, `799`) is the settled DOM, measured before the flag is forced.

**Astra 2 is resolved.** `list.test.mjs:804-827` draws 2,000 offsets over 500 seeded rows. Replaying that generator: 110 rows are height 0, the extent is 17458, `maxOffset(100)` is 17358, and 42 offsets sit past that end (about 0.21 through 17655). `expect(misses).toEqual([])` records any miss.

The oracle `clamp(offset) + max(0, offset - max)` (`list.test.mjs:821`) is the Rust line `window.offset + beyond` (`runner/src/instance/collection/index_tests.rs:880-884`). It matches `restoreAnchor` for every `follow === false` anchor whose `maxOffset >= 0`:

- A zero-height row does not break it. `find` returns a row whose prefix is at most the clamped offset, and `within` rebuilds that offset, so restore is `clamp + beyond`. All 2,000 samples hit this row path (`max` is 17358, every offset is positive and below the extent).
- An offset in the leading padding, with `maxOffset >= 0`, restores to 0 on both sides (`clamp`). The loop never draws a negative offset.
- An offset in the trailing padding, with `maxOffset >= 0` and no row, returns `raw` when `beyond > 0` (`list.js:207`). That equals `clamp + beyond` because `clamp` is `max`. This loop leaves `trailing` at 0, so the search offset stays below the extent and a row is found.

The identity stops when `maxOffset < 0` (leading padding and an extent shorter than the port). There is no row and `beyond > 0`, so restore returns `raw`, while the formula returns `offset - max`. The sibling case already locks the right value: leading 92, port 600, `maxOffset` −17, offset 19 restores to 19 (`list.test.mjs:749-750`, `767`). The formula would expect 36. The traversal never builds that geometry, so it does not bless the wrong identity.

It cannot pass vacuously. Those 42 past-end offsets restore to the raw offset. The pre-change restore clamped them to `max` and dropped `beyond`, so they miss. `Math.abs(NaN) > 1e-6` is false, so a NaN restore would be ignored, but every value this generator produces is finite.

Call sites that `#1095` changed all pass the port: `anchor` (`list.js:338`), the four `reportAnchor` captures (`list.js:689-694`), and `dropAnchor` (`host/web-js/reorder.js:91`). `travelWithin` (`list.js:705`) still captures `f.offset` with `followEnd`, matching `within.rs`, which `#1095` also left off `anchor_offset`. `anchorAt` carries `beyond: 0, raw: 0` (`list.js:193`). A followed end restores to `max + beyond`, a row to the clamped position plus `beyond`, no row with overshoot to `raw`, no row without it to `clamp(raw)`, and a vanished row to 0 (`list.js:204-209`).

**Blocking**

none

**Non-blocking**

none
