# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), narrow re-check 1, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `034d08af8d`.
- **Method:** one brief (sha256 `23e23b5e56d0a8e308421196f3a19baa48741f7bd2d88061f44249a3653d3461`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check HEAD against the round-3 claims on refusal order and the shorthand limit, read-only, and report P1/P2/P3 or clean.No findings.

(1) is right. `order()` in `scan.rs` numbers elements in the same preorder lowering uses: the element, then its children, `if` then-arm before else, `for` body, `match` some before none. Each `Place` gets that number. A style row keeps it in `element` as well (the per-element key); an own row keeps `element` at 0. `grouped` copies `order` onto every place, including places merged as alike. `check` keeps one style refusal, the smallest `element`, and sorts by that number, then style before own, then the error span. On one element the build expands the class before the element's own rows, and it finishes an element before its children, so the first diagnostic is the build's first among the rows it judged. `Place::plain` leaves `order` at 0; the style unit test still selects by `element`, and `refusals_come_in_the_order_the_build_reaches_them` compares the server's first diagnostic with `contract build`. Both cases there fail if the sort is removed: own rows are appended before the style map, and that map is in source order, so `Pad` precedes `Wide`.

(2) is an acceptable stated limit. `kept` still drops shorthand and `flex` rows for own rows and style rows. The build records those in the attribute walk and continues, so a later judged refusal is one the build makes, at that row, in the build's words. It may be the build's second. The `QUEUE` sentence states that, and it does not claim shorthand rows are judged.
