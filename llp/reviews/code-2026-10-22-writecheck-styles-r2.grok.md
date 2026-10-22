# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), round 2, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fd7aa4d783`.
- **Method:** one brief (sha256 `9833135a2c6fbf38e337f4f3f98c61e8f1c7304b16c5a29108b7012d2a175698`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll start with the review skill and the specified commits so the second-round review stays aligned with the round-1 findings.This is a targeted round-2 review of the style-row write check, not a GitHub `/review` run. I'll load the reviewer persona and inspect the specified commits next.I'll pull the round-1 fix, the full style-row change, and any prior review notes so the second pass has the right baseline.I'll read the scan, check, and lowering paths next so I can compare class-row judging with what the build actually does.The next pass is lowering order in `nodes.rs` versus `order()`, including `when`/`each`/`match` and inlined components.Checking one more lowering-order detail: whether skipped `for`/`match` applications can still be the build's first refusal, and whether `grouped::style` errors leave descendant class rows judged.Round-1 numbering is the build’s lowering order on the expanded tree. Two gaps remain: first-element words skip `for`/`match some` applications, and `grouped::style` `Err` still lets descendant class rows get judged.

`order()` in `scan.rs` is preorder on expanded `root.view` and matches `Lowerer::node`: element, then children; `if` then/else; `for` body; `match` some then none. Inlined uses are already spliced, so each instance is its own node (round 1 (c)). Uninlined `Use` is skipped by both the scan and lowering.

**[P2] First-element words are first judged, not first lowered — `contract/lower/src/theme/scan.rs:471`**

`class_left` drops every class row under `for` and `match some` (`local`). `check` then keeps the smallest `Place::element` among the places that remain (`rows.rs:591-595`). The build still lowers those region bodies and judges the same literal class rows there (`lib.rs:1023`, `:1045`). Own literals under a `for` are already judged; class literals are not.

A style applied first under a `for` (or `match some`) and later outside is refused in the later element’s words. Kind errors for `padding="gap"` match anyway. Font-face errors do not: `bundled_faces` names the node’s family (`type_tokens.rs:200`). The mixed test puts `class=Pad` on the column first, so `same_words` never sees this.

Fix: if any application of the row has a smaller `order()` number than the judged ones and was skipped only because of `local`, leave the row to the build. Recording those skipped `(row, element)` keys is enough; judging class literals under `for` would contradict the new only-each/only-match tests.

**[P2] `grouped::style` `Err` still judges descendant class rows — `contract/lower/src/theme/scan.rs:149`**

Round 1 (b) drops the list’s class rows unless `grouped::style` is `Ok(None)` (`scan.rs:497`). Descendants are excluded only when `grouped_list` is true, and that helper is false on `Err` (`scan.rs:140-149`, `:538`). `prepare_element` returns at `grouped::style` (`nodes.rs:189`) and never lowers children.

A `list appearance="auto" listStyle="nope"` with a child `class=Pad` can be refused at `Pad` while the build’s first diagnostic is `lower-grouped-list`. `seen_failed` closes this after the background build; the first transaction is the QUEUE window, now on class rows the round-1 filter was meant to keep off that path.

Fix: when `grouped::style` on the list is not `Ok(None)`, set `left` so descendants inherit it (or treat `Err` like `Some` for the `grouped` flag).

**[P3] Tests never lock inlined-instance keys or region-first words — `contract/cli/tests/it/tamber_rows_style.rs:133`**

Two `Card()` instances applying one style with different fonts would have shared a span before (c) and would still pass today’s tests. The for/match case with the region application first, and a child class under a grouped list, are also untested. Add those three; the unit test in `rows.rs` already covers min-element among judged places.

Round 1 (a) (whole `button`), (d) (one style refusal, own rows undedupe’d), (e) (only-under-for/match accepted), per-element keys, and `order()` vs `when`/`each`/`match`/inlined uses look right. Remaining miss paths (SVG, `audio`, box `transform`, choice of styles) stay on the QUEUE’s “leave to the build” side.
