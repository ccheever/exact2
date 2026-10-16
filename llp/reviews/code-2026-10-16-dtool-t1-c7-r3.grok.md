# Code review: LLP 1122 T1a commit 7 (import review), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `2f7aee4ad` (commit `ed457d779` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `1cdd18f8cb8d0b06bc89bbf25d811e9de43ec9818a91e1a9a2e2eed64a2d8594`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 3 low).
- **Disposition:** taken but 1 and 3 in part: an edit now holds again only what the card has chosen (the tokens not left out, the overrides ticked), with the edit, its review renumbered to those items and its overrides on, so an unticked override is never checked with an edit it has no part in and the card's diff is what Accept writes; a reference is followed in the same mode through what the import adds or sets, and a design token's residual through its fallback; several files' diagnostics are named by their message alone, no place invented; the report of an import with nothing to propose names each value's block; the stub renumbers a recomputed import's review as the server does; tests untick the override on the edited card through to the bytes, edit and accept a Figma mode's value, and check a reference painted as its token in that mode. 1 in part: an edited value that makes an offered role vary by an axis is not caught before accepting (the build refuses it, `tamber-role-axis`). 3 in part: a reference whose target has modes it has not is not given rows for them. Round 3 was the last.

---

The round-2 selection baseline holds. An accept uses the same mark as the card (`replaces` or review `kept`), ticks carry onto the new proposal, and an edited import shows its notice and its diff. These are the problems that are still open.

## Medium

**Alias swatches follow only an `add-token`'s default value.** `apps/design-tool/app.ts:487`, `apps/design-tool/app.ts:493`, `apps/design-tool/app.ts:646`

`added` records `add-token` name to `value` and nothing else. A reference is painted from that string, then from the design snapshot's first tuple. A `set-token` is invisible, a referenced token's `whens` are never consulted, and a residual design token (a role or platform colour) contributes `css`, which is the role name. The grid paints that same token with its fallback `srgb`. Any non-empty string becomes a swatch box (`apps/design-tool/app.contract:779`).

Import the Figma fixture. `border.primary` is `light-dark` of `{color.gray.900}` and `{color.gray.50}`. Those strings are the same in Brutal and Modern, so the alias has one row. Its swatches are Brutal gray.900 `#33057e` and gray.50 `#eedeff`. Modern is `#202020` and `#f1f1f1`, and that colour is only on the primitive's when row. Accept the alias and import again after `dtcg.color.gray.s900` is a `set-token` to a new hex: the alias still paints the colour already in the file. A reference to the fixture's `text.label` (`-exact-label`) draws an empty box.

Resolve a reference through the target's `add-token` or `set-token` value for the same mode, and through `light-dark` for the side. For a design token, use the tuple for that mode and the fallback `srgb` when the value is residual. Leave `light` and `dark` empty unless the result is a colour a page can paint.

**The gate test still does not prove the paths round 2 asked to lock.** `scripts/design-tool.test.mjs:594`, `scripts/design-tool.test.mjs:632`

The new steps edit the recomputed card and accept it while AccentColor stays checked, and they check that some second mode's `value` is non-empty and that every token mode's `light` is non-empty. Unticking AccentColor on that edited card, typing into a non-default mode, and comparing a swatch to the referenced token's colour are all unasserted. `light !== ''` stays green for the Brutal hex, a wrong hex, or a role name.

Drive both checkbox directions on the edited card through to the bytes, type a Figma when-value and accept it, and assert the alias swatch equals the target's colour in that mode.

## Low

**The edited card's diff is every operation, not the selection.** `apps/design-tool/app.ts:1071`, `apps/design-tool/app.contract:766`

An edit proposes the full operation list. The orange line says to review that diff and accept it, and the heading still says overrides are off whenever this card is not itself a recompute (`kept` is false). Tick AccentColor, leave `shadcn.foreground` out, and change a hex. The diff still adds foreground and the other four roles. Accept does not write that diff; it recomputes. Propose the operations the carried ticks would send, or say on the card that the diff is the whole import and the checkboxes are the write.

**An import that maps nothing still hides where each unmapped value came from.** `apps/design-tool/app.contract:833`

The held card prints the block. The empty report prints `name = value: why` only. Import two DTCG files that are only spacing, with the same path in each. Both rows look identical, so the file that needs a fix is not on the row. Use the same text as the held card, including the block.

**A multi-file refusal names the wrong file.** `contract/cli/src/tamber_import.rs:176`

Every diagnostic's `file` is `files.first()` after the sort. DTCG's own message already names the bad file (`tamber/src/import/dtcg.rs:406`). Pick a good Brutal file and a broken Dark file. The tool shows `Primitives — Completed.Brutal Theme.json:1 tamber-import-source: Tokens — Completed.Dark.json: …`. Put the source name on the diagnostic, not the alphabetically first file.

The stub's partial accept still renumbers operations and leaves `review.tokens` and offer `items` on the old indices (`apps/design-tool/stub/tamber-stub.ts:88`). Unticking an offer on that card toggles an index the new proposal does not have. Remap the review the way `scripts/tamber-server.mjs` does around its recompute.

Verdict: request changes — alias swatches still paint the wrong colour, and the gate test does not catch it.
