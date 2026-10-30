# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck3: the brand-fill revert, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `48870b6a8c`.
- **Method:** one brief (sha256 `b263bcf533bd7a47ae440bff10ab92bbcaf9032e37d2f07f862377c8f01f1473`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 low: P3, the same).
- **Disposition:** taken in 6c0e1192e8.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the reviewed EmptyState follow-up. Read-only: no checkout, restore, stash, reset or edits.
Context: theme2 ruled at 23:51 that the brand fills must NOT be the platform accent. White on Apple's default accent is 3.52:1 in light and 3.23:1 in dark, under 4.5:1 for the 13-point label. So this commit:
- reverts cards.empty-button to light-dark(#006aff, #0f73ff) and cards.empty-button-ink to #ffffff scheme=fixed (the values 2c landed with);
- rewrites LLP 1139's follow-up note: the button stays the brand literal, an imported accent reaches it through the importer (dtool3's item), and a pre-existing gap is LISTED (the dark default, white on #0f73ff, is 4.27:1);
- fixes the agents doc's buttonTone wording.
The earlier briefs are at /tmp/iosfix-rv-es1-brief.txt and /tmp/iosfix-rv-es2-brief.txt.
Check:
- the revert is complete: no AccentColor left in the cards tokens or docs for this button; restyle reads, cells and tests consistent;
- the contrast figures (compute white on #006aff and on #0f73ff, and on Apple's #0088ff/#0091ff accent from kernel/tables/schema.json);
- the note's accuracy and wording.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD against the EmptyState follow-up: the brand-fill revert, the LLP 1139 note, and the contrast numbers. Read-only, no checkout or edits.HEAD reverts the primary button off the platform accent. I'll check the tokens, docs, restyle reads, and the contrast numbers against the schema.**P3** — `llp/1139-component-library.rfc.md:583`. The follow-up says an imported accent already reaches this button because the importer maps it onto the brand tokens and picks the ink for contrast (`dtool3`'s item). The importers that handle an accent write a role override: shadcn's `--primary` becomes `AccentColor` (`tamber/src/import/shadcn.rs:743`) and Astryx's accent becomes `AccentColor` (`tamber/src/import/astryx.rs:509`). Neither writes `cards.empty-button`, `cards.empty-button-ink`, `actions.follow-fill`, or `actions.follow-ink`, and neither chooses an ink for those tokens. No QUEUE entry, LLP 1122 section, or design-tool source records an item under that name. With the button back on the literal, an accepted import recolors `AccentColor` and leaves this fill at `light-dark(#006aff, #0f73ff)` under fixed white.

The rest of the commit holds.

- The revert matches what batch 2c landed. `packages/cards/cards.tamber:52-53` are `light-dark(#006aff, #0f73ff)` and `#ffffff scheme=fixed`. No `AccentColor` remains on the cards tokens, in `empty-state.contract`, or in the EmptyState docs. The secondary pair is unchanged. `docs/contract-for-agents.md:4163` calls primary the brand blue.
- Restyle reads, library-gate cells, and the batch 2 consumer key the token names and the same node paths. The accent commit never retargeted them, so leaving them matches this revert.
- White on the fill, WCAG 2, the same formula as `tamber/src/lib.rs` `ratio`. Floor and round agree at two places:
  - `#006aff` 4.66:1 (light brand, over 4.5)
  - `#0f73ff` 4.27:1 (raw 4.271)
  - `#0088ff` 3.52:1 (Apple light, `kernel/tables/schema.json:5739`)
  - `#0091ff` 3.23:1 (Apple dark, `kernel/tables/schema.json:5740`)
- The listed gap is the dark literal. The light literal clears 4.5:1. The label is `0.81875rem` at weight 500 (`cards.empty-button-size`, `empty-state.contract:122`), below WCAG large text, so 4.5:1 is the bar the note uses.

VERDICT: CHANGES
