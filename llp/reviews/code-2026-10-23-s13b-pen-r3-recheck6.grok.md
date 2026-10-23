# Code review: stage 13 B, the pen importer, round 3's re-check 6 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `26fc6751f9`.
- **Method:** one brief (sha256 `7230295ccd4c4a972ba59d164c7bd06ff7cb1781ccb460ac032559785951cb4f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

None. In `Resolve::seen` (`tamber/src/import/dtcg.rs:986-993`) the byte length of the written name, the resolved colour, and the target path is spent twice before the memo insert and the return. A 128-character prefix is at most 512 bytes, so each alias spends about 1 KiB per row. The import budget is `3 << 24` (`tamber/src/budget.rs:18`). A scheme plus six binary axes is 128 rows (`dtcg.rs:1110-1135` counts axis tuples only; both scheme sides are still resolved), and that many rows still runs out of budget after a few hundred aliases, at tens of megabytes kept. The memo hit at `dtcg.rs:946` clones an entry whose first return was already charged; the kept pair stays those two copies.

**Should-fix**

- `tamber/src/import/dtcg/pen.rs:285-293` (side test at `:231`, stop test at `:238`). A `descendants` value that is an object is walked with `key` set to the node id, and that id becomes `parent` for the properties inside it. `descendants: {"strokeWidth": {"top": "$n"}}` binds a pixel side, so a number nothing actually reads as pixels imports as a length. Under a gradient, `descendants: {"colors": {"position": "$n"}}` binds a non-pixel stop; next to `gap: "$n"` the same number imports unitless. A non-object value binds nothing (`:282`).

**Nits**

- `tamber/src/import/dtcg.rs:1301`. A spend that lands on zero returns ok (`tamber/src/budget.rs:62-65`); the sentinel `spend(1)` then refuses an import that used the budget exactly.
- `scripts/tamber-import-review.mjs:21`. The notes comment still says `*1.16*`.

The 1.17 clause says what changed. `docs/tamber-protocol.md:162-165` adds kind `pen`, `notes` on an import’s review when it has any, and a 128-character prefix for every kind. `contract/cli/src/tamber_serve.rs:77` is `"tamber/1.17"`. The import section marks those three as `*1.17*` (`docs/tamber-protocol.md:1097-1098`, `:1146`), and the serve tests expect `tamber/1.17`.
