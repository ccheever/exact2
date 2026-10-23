# Code review: stage 13 B, the pen importer, round 3's re-check 8 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `894b0b3b08`.
- **Method:** one brief (sha256 `eedbd90609c7e7f993017b5af5b51336291d3354be3ca475676e3c6e913c0b77`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** None.

**Should-fix:** None.

**Nits:** None.

The landing walk in `tamber/src/import/landing.rs:301-334` builds the “who names whom” graph once, charges every word and every step, and refuses the same tokens as the old fixpoint. Names are unique before `landing`, so the closure is exactly the tokens that name a refused one. A 4,000-colour chain against `pen.c0000: length` stays inside the import budget; the old per-pass linear scan is gone. What a 1 MiB `.pen` can still make that walk do is charged, and the budget refuses it first.

An override with no `type` is held as `"override"` at `tamber/src/import/dtcg/pen.rs:275`, so `inputs`, `metadata`, and `uniforms` are opaque the way they are on a typed node. An input named `width` stays a number. A real `width` or `strokeWidth.top` on that override stays a length, and a descendant id `uniforms` stays a pixel `width`.

`cargo test -p exact-tamber --test it import` passed (65), including the re-check 7 chain and the descendants cases. The landing unit tests passed (2), including `it names t.gap`.
