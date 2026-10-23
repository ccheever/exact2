# Code review: stage 13 B, the pen importer, round 3's re-check 5 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `842c78fabb`.
- **Method:** one brief (sha256 `ebd46e68df3aa74d49b3b8e9792a198cb45f2b0bc3a7b76d8417295c7dfb6875`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- `tamber/src/import/dtcg.rs:970` (memo clone at `989`): a resolved colour alias still stores the target's prefixed name in that tuple's result and again in its memo, and each of those clones is only `spend(1)` at `954`. The new cap is `chars().count() > 128` (`tamber/src/import/dtcg/pen.rs:744`, `contract/cli/src/tamber_import.rs:178`), so 128 four-byte characters (512 bytes) are accepted before any work. A `.pen` under the 1 MiB body holds about 22,000 colour aliases and 64 tuples (six binary axes). About 22,000 × 64 × 2 × 512 bytes is about 1.3 GiB of those strings. Naming spends about 23M of the 50M budget and the walk about 3M, so the budget does not refuse. The isolate's resident limit does. The same copies at 128 rows come from an ASCII prefix plus a scheme: `dtcg.rs:1124` caps axis tuples at 64 and does not count the two sides, while `1133` still resolves both.

**Should-fix**

- `tamber/src/import/dtcg/pen.rs:281`: a key in a `descendants` map is a node id when the value is an object, and those cases are right (a node id `theme` or `descendants`, and that node's own `theme` binds nothing). A string value is bound with the id as the property (`pen.rs:230`). `descendants: {"opacity": "$n"}` next to `gap: "$n"` makes a length unitless.

**Nits**

- `tamber/src/import/dtcg.rs:1298`: the sentinel `spend(1)` also refuses an import that hit the budget exactly. A spend that lands on zero returns ok, and the sentinel then fails.
