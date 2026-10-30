# Code review: the JS target's port of #1095's collection anchor, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `192633e74d`.
- **Method:** one brief (sha256 `e996fe7d134293f3ef86d9b58ed16be4dd0fc2c9ebb4269381ee8c199fb7e46c`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The port matches `7b0352262f`. Every anchor branch that commit added is in `host/web-js/list.js`, the callers pass the port, and the new mirror is red on the old restore.

1. Semantics match the Rust diff. `beyond` is `max(0, raw - max)` at capture time (`list.js:191`, `index.rs:648`). `raw` is the unclamped offset (`list.js:177`, `index.rs:649`). `anchorAt` stores `beyond: 0, raw: 0` (`list.js:193`, `index.rs:689-690`). `restoreAnchor` (`list.js:204-209`) follows `restore_anchor` (`index.rs:728-739`): a followed end is `max + beyond`; no row and `beyond > 0` returns `raw`; no row and `beyond == 0` is `clamp(raw)`, which is `max(0, min(raw, maxOffset))`, the same as `clamp_offset`; a live row is `min(max(position, -leading), max) + beyond`; every old key gone returns `0` and does not add `beyond`. Row search still uses the clamped offset (`list.js:177-178`), same as `clamp_offset` then `at`. `anchorOffset` uses `maxOffset(port)` while `atEnd`, and `extent` otherwise (`list.js:342`, `start.rs:59-64`). `startAtEnd` still corrects to `extent` (`list.js:346`), as `start_at_end` still uses `scroll_extent`.

Call sites that `#1095` changed all pass the port: `Collection.anchor` (`list.js:338`), all four `reportAnchor` captures (`list.js:689-694`, same order as `inset.rs:37-65`), and `dropAnchor` (`reorder.js:91`). `travelWithin` still captures `f.offset` directly (`list.js:705`), matching `within.rs:78`, which `#1095` did not change. Nothing still calls `anchorOffset` with one argument.

Not in this commit, and not a JS collection branch: the `AnchorCorrection` doc comment in `api.rs` and the `CollectionIOS.swift` comment. The 25k test’s `window.offset + beyond` (`index_tests.rs:878-884`) is the row-plus-overshoot formula, covered below by restoring 269 rather than 250. There is no JS 25k traversal to update.

2. `list.test.mjs:725-771` uses the same heights, port `150`, `pulled = max + 19`, `k0 → 110`, follow capture at `pulled + 10`, `k3 → 125`, and the five rowless triples `(short, 150, 19)`, `(padded, 600, 0)`, `(padded, 600, 19)`, `(after, 100, 450)`, `(after, 100, 300)`, then `k0 → 400` staying at `19`. On the parent, `restoreAnchor` returned `max` for a followed end and `clamp(within)` for no row, so the first restore is 250 rather than 269, the follow restore is 260 rather than 279, and the padding-after-rows case at 300 returns 0. `anchorAt` (`list.test.mjs:755-769`) expects `beyond`/`raw` of 0 and a restore of 105 (`prefix(k1) + 5`). On the parent those fields are missing, so the assertion fails. The opening-list test (`list.test.mjs:775-792`) expects `beyond === 0`; on the parent `anchor()` does not return `beyond`.

Two assertions are green on the parent as well, and they match the Rust cases. The padded port at offset 0 restores to 0 on both versions. The all-keys-gone case (`list.test.mjs:757-770`) hits `offset === null ? 0` (`list.js:209`), which `#1095` left as 0. That assertion does catch a restore that adds `beyond` onto the missing row (19 instead of 0). The other cases are what make the mirror red on the parent.

3. With `beyond === 0`, follow, reorder, saved place, and inset behave as before. A chat at the end still restores to `max` (`list.js:206`). `anchorAt` still adds 0, so a saved place (`list.js:592`, `list.test.mjs:38`) and the anchorAt boundaries (`list.test.mjs:343-388`, expected `[-20, false] … [40, false]`) stay put. `dropAnchor` only changes while `atEnd`, where the offset becomes `maxOffset(port)` instead of `extent`, which removes a false overshoot of about one port. Existing drop coverage is `list.test.mjs:234` and the relative-correction test above. Inset’s four captures now take that same offset; the padding values and the recapture order are unchanged. Overscroll while following, dropping, or reporting is the new behavior, and the index mirror covers it. `list.test.mjs:666` still sees a clamped edge: the glue removes the rubber-band from the reported offset before the runner anchors (`collection-glue.js:126-142`).

4. The comments name the right symbols. `index.rs:72-73` points at `SizeIndex.anchor`, `anchorAt`, and `restoreAnchor`. `index.rs:724` points at `restoreAnchor`. `list.js:173-174` points at `Anchor`, `capture_anchor_excluding`, `anchor_at`, and `restore_anchor`, and the hash is `7b0352262f`. The Rust diff in this commit is those comments only. `anchorOffset`’s comment (`list.js:341`) matches `start.rs:57-58`.

5. File lengths stay under the 1,500-line source cap (`list.js` 995, `list.test.mjs` 793, `reorder.js` 482, `index.rs` 1026). `caps.mjs` was not run.
