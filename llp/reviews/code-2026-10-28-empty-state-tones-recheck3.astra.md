# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck3: the brand-fill revert, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `48870b6a8c`.
- **Method:** one brief (sha256 `b263bcf533bd7a47ae440bff10ab92bbcaf9032e37d2f07f862377c8f01f1473`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 low: P3).
- **Disposition:** taken in 6c0e1192e8 (the importer mapping is pending).

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

- **P3 — [llp/1139-component-library.rfc.md:583](/tmp/iosfix-rv-es5/llp/1139-component-library.rfc.md:583):** The importer mapping reads as implemented. At HEAD, the importers offer `AccentColor` overrides but do not map the button’s brand tokens or select contrasting ink. Mark this explicitly as pending dtool3 work.

The revert is complete; restyle reads, cells, tests and `buttonTone` wording remain consistent. Six selected read-only tests passed.

Computed white contrast: `#006aff` **4.66:1**, `#0f73ff` **4.27:1**, `#0088ff` **3.52:1**, `#0091ff` **3.23:1**. The figures and listed pre-existing dark gap are accurate.

VERDICT: CHANGES
