# Code review: stage 3 commit 9 follow-up: Astryx's AccentColor offer, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4501b4c3c03f5fd699334d647830b0477ba2f6ec63a0b7d8950009f075eeb259`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: changes requested (a rebinding that orphans a dial; a duplicate under another spelling); Grok: nothing blocking (the same duplicate, non-blocking).
- **Disposition:** applied without re-review in the second-round commit

---

I'll review the last two commits read-only, starting with the review guidance and the diffs themselves.The request is a read-only review of the last two commits, with specific checks on role splitting. I'll load the reviewer brief and collect that diff without touching the worktree.`HEAD~1` is an unrelated queue note; the LLP 1121 pair is `HEAD~2` (the accent offer) and `HEAD` (the review fixes). I'll review those two and the role-axis rules they have to satisfy.Nothing blocking. The accent offer and the follow-up are `cd6c271` (`HEAD~2`) and `ad00d8c` (`HEAD`). `HEAD~1` is an unrelated queue note (`QUEUE.md` only), so it is not part of this review.

Accepting the import operations, plus every offer that has operations, does not produce a role whose folded `css()` differs across tuples. An existing role that could start to differ is named with no operations. The extra names are the safe kind: a note, no edit, and they do not sit on a role an offer rebinds.

**`split_roles` does not miss a role that accepting can split.**

- Role spellings are followed. `chain` resolves each word with `token_index`, which matches a role under any spelling (`tamber/src/import/mod.rs:500`).
- Dials are not a hole. A colour dial is a literal: the checker refuses a token or a host role in a dial value (`tamber/src/check.rs:346`), and neither importer writes dials. A role that only names a dial cannot start varying. A dial written beside a token inside `light-dark()` or `color-mix()` does not hide that token; `words_of` still yields it and `chain` follows it.
- `light-dark()` is walked. `words_of` keeps dotted names and splits on `(`, `)`, and commas (`tamber/src/import/mod.rs:514`), the same split the folder uses for an ident. A reference on either side is in the chain. A `light-dark()` that is the same text on every tuple is one value, so it does not fail `tamber-role-axis` (`contract/lower/src/theme.rs:248`).
- A role an offer rebinds is skipped only when that offer has operations (`tamber/src/import/mod.rs:460`). Those operations set the role to the token and clear its `when` lines. Astryx does that only when the token has no `when` (`tamber/src/import/astryx.rs:276`), and its mapped values are closed colours, not references. shadcn does it only when `varies` is false, and `varies` follows mapped `when` lines and references inside the new value, including inside `light-dark()` (`tamber/src/import/shadcn.rs:456`). If the target will vary, the offer has no operations, so the role stays in the naming passes.

Over-naming is acceptable. Any `when` anywhere in the chain counts, including one that still folds to the same colour, one shadcn will unset on its own `theme` axis, and one a rebind will clear. The note says the build may refuse the role, and the offer carries no operations.

**No new duplicate or contradiction with shadcn's own pass.** A rebound is detected with `role_key`, so a second note is not added under another spelling (`tamber/src/import/mod.rs:460`, `tamber/src/import/shadcn.rs:416`). shadcn's own pass already records a role under the design's spelling; `split_roles` then skips that exact string (`tamber/src/import/mod.rs:473`). The two empty `AccentColor` lines in `a_role_whose_token_varies_once_accepted_is_not_offered` are the older pair (the withheld offer, then the existing binding) and are what that test expects.

**Tests.** `astryx_names_a_design_role_a_changed_value_could_split` and `shadcn_names_a_design_role_chained_to_a_rebound_role` lock both review designs: the kept `#111` / `#111111` when, and the role chained to a rebound accent. The contract test compiles the neutral Astryx import with both offers accepted. They do not lower the split designs; the offer lists are what the new code changes, and those assertions match the paths above.

One non-blocking gap: the "already named" skip compares the offer string to the design's spelling (`tamber/src/import/mod.rs:473`), while Astryx's withheld offer is always the canonical name (`tamber/src/import/astryx.rs:281`). A design that spells the role `accentcolor` and points it at an `astryx.accent` that keeps a `when` gets two empty offers, `AccentColor` and `accentcolor`. Neither has operations. Comparing with `role_key`, as the rebound check already does, would collapse them.
