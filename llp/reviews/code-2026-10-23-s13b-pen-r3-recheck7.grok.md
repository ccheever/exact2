# Code review: stage 13 B, the pen importer, round 3's re-check 7 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0a474445b2`.
- **Method:** one brief (sha256 `82a880ff223a31d8d640891de966009d117dae4202bbd052892c456c28bd1191`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- `tamber/src/import/dtcg/pen.rs:275`: A property override has no `type` (Pencil: `type` present means the node is replaced). `inputs`, `metadata`, and `uniforms` become opaque only when the object that holds them has a type, so on an override they do not. `descendants: {"chart": {"inputs": {"opacity": "$pad"}}}` next to a real `gap: "$pad"` binds `opacity` and imports a length as a unitless number. The same for `metadata`, and an input named `width` or `gap` imports as `px`. A replacement that repeats `type: "script"` stays opaque.

**Should-fix**

None. The cycle string is charged at `tamber/src/import/dtcg.rs:985` (`2 * why.len()` before the clone at `:991`). A memo hit at `:946` returns the copy that charge already counted. Under 1 MiB, 128 tuples (six binary axes plus a scheme) retain on the order of tens of megabytes before `IMPORT` (`3 << 24`) refuses. An id `type` no longer sets gradient context (`pen.rs:270`), and an id `strokeWidth` or `colors` is not the parent of `top` or `position` (`pen.rs:290`); a real override `{"node": {"strokeWidth": {"top": "$n"}}}` still reads as a pixel side.

**Nits**

- `tamber/src/import/dtcg.rs:1301`: A `spend` that lands on zero succeeds (`tamber/src/budget.rs:62`). The following `spend(1)` then refuses an import that used the budget exactly.
