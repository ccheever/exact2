# Code review: Excord's windowOf row reuse, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `5d14a01fbe`.
- **Method:** one brief (sha256 `6868d6cee22d1adb2ebf559bab1ef5bc0624afcda6a3c3db7e449bf6956ad25b`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The round-1 gap is closed. The fixup keeps every row from before an older page, including the New-divider row, and renews only the row that gains a predecessor. A deletion just before UTC midnight renews only the next row and checks its `prevAt`. Moving the New divider across that midnight renews exactly the row it leaves and the row it reaches. The seeded sequence now ingests older pages (37-minute gaps, so midnights and group breaks), asks around a held message as well as the tail, and checks that an unchanged re-ask returns every row as the same object. Those five tests pass (9060 assertions).

The cache key in `windowOf` is the rendered row, `since`, and `previous?.id` (`render.ts:416-431`). The renderer returns that row object only while the store message, the predecessor object, and the directory inputs it reads are unchanged, and the store replaces a message instead of writing its id in place. Grouping and the day divider live on that row as `grouped` and `prevAt`, so a predecessor change misses the map and the row is built again. `isNew` depends only on `since`, the message id, and the predecessor id (absent, null, or a snowflake compare), which that key covers. A hit returns the stored object and does not write it. The New-divider row is a shallow copy, so the renderer's own row stays `isNew: false`.

VERDICT: APPROVE

No findings.
