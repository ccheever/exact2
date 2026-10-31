# Code review: the JS target's port of #1095's collection anchor, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `0744cf9b2b`.
- **Method:** one brief (sha256 `c1abee0a156484cd73d348d20fd0cdef0db69c7dd031e1f9a30e6a191e725832`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

Round 2 is resolved, and the port matches `7b0352262f`.

1. The traversal requires a finite restore before it compares. `host/web-js/list.test.mjs:828-835` counts every sample and increments `finite` only when both `restored` and `want` are finite, then `list.test.mjs:835-836` requires `checked = finite = 2000` and `misses = []`. A NaN restore fails that count (`|NaN - x| > 1e-6` is false, so the old compare would have passed).
2. The opening anchor is given an input past the end. `list.test.mjs:789` calls `anchorOffset(extent, port)` and `list.test.mjs:799` requires that result to be `maxOffset(port)`. With this list (40 rows at 50 px, port 300), the extent is the content end and `maxOffset` is shorter by the port, so an identity `anchorOffset` fails. `list.test.mjs:798` still checks the settled offset.
3. The formula’s domain is stated and checked. `list.test.mjs:805-807` says `clamp(offset) + max(0, offset - max)` holds for `max >= 0`, and `list.test.mjs:834` asserts that. The seeded index is 500 rows, 107 of them zero height, total 17458, so `max` is 17358. The padded `max < 0` case stays in the rowless assertions: `list.test.mjs:747-750` and `list.test.mjs:767` (padded port 600, `maxOffset` = -17, restore equals the raw input via `list.js:207`).

The runner behavior is the same at each branch `7b0352262f` added. `list.js:177-191` stores `beyond = max(0, raw - max)` and the unclamped `raw`. `list.js:193` sets `anchorAt` to `beyond: 0, raw: 0`. `list.js:204-209` restores `max + beyond` when following, `raw` when there is no row and `beyond > 0`, `clamp(raw)` when there is no row and `beyond == 0`, and `clamp(position) + beyond` for a live row, or `0` when every key is gone. `list.js:342` takes an opening anchor at `maxOffset(port)`. Call sites pass that port: `list.js:338`, `list.js:689-694`, and `reorder.js:91`. `list.js:705` still anchors the reported offset directly, as `within.rs` does; `startAtEnd` still uses the content extent, as `start.rs` `start_at_end` does. The Swift and `api.rs` edits in `7b0352262f` are comments. The new Rust comments are at `index.rs:72-73` and `index.rs:724`.

The tests fail on the parent `175bdab9` (already on `origin/main`). There, a restore stops at the end, so `list.test.mjs:763` sees 250 instead of 269, and the followed restore is 260 instead of 279. `beyond` is absent, so `list.test.mjs:797` and `list.test.mjs:769` fail. `anchorOffset` returns the extent, so `list.test.mjs:798-799` fail. Of the 2000 seeded offsets, 42 sit past `max`; the parent restore drops that overshoot, so `list.test.mjs:836` fails.

APPROVE
