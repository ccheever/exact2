# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), r2, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `491a982552`.
- **Method:** one brief (sha256 `47512ebd8044280541cf9f060ad2a0665f25523ea1aa39f28b1dd8e52e0e6241`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 low: P3).
- **Disposition:** taken in 1f36dc33ba (LLP 1139 names each host's accent).

## Brief

Round 2 of 2 reviewing the branch: diff of the merge-base of origin/main and HEAD..HEAD, two commits. Read-only: no checkout, restore, stash, reset or edits.
Round 1's brief, with the change and Bluesky's reference paths, is at /tmp/iosfix-rv-es1-brief.txt. Read it first.
Round 1: Grok was CLEAN. Astra had one P3, that docs/contract-for-agents.md still listed EmptyState's old 8 props. It's FIXED in the second commit.
The second commit also folds in theme2's 22:43 brand-fill ruling (the cards half):
- cards.empty-button = AccentColor and cards.empty-button-ink = AccentColorText, where they were Bluesky's blue and white as literals that an app's or an import's accent couldn't reach. The secondary button stays literal.
- LLP 1139's note says what AccentColor is for an app that sets none (the platform's accent; on Apple the system tint, which a Mac user may change; elsewhere the app's default, as actions.compose-tint already does). It also notes that Apple's AccentColorText is white in both schemes (the QUEUE item on Realworld's and Expose's dark fills).
- A clone that wants Bluesky's exact blue sets cards.empty-button by full name.
Check:
- the doc text against the component;
- the accent change: anything that read cards.empty-button as a literal (restyle reads, cells, tests, the gallery), and whether AccentColorText on the accent reads in both schemes;
- the LLP text's accuracy (what AccentColor resolves to on each host: kernel/tables/schema.json, the hosts' role tables);
- anything round 1 missed in the first commit.
Checked on the tip: the batch2_exports consumer, library-gate.test and stage12-gate.test pass, the web build passes, and the s13c restyle on terminal and web-js is exact for every case (EmptyState tokens 266 of 266, params 40 of 40; the line's FAIL is only that the other hosts weren't run).
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P3 — [llp/1139-component-library.rfc.md:583](/tmp/iosfix-rv-es2/llp/1139-component-library.rfc.md:583):** “Elsewhere the app’s default” misdescribes Android and web. Android resolves `AccentColor` from the device’s Material `Primary` palette; web derives it from the browser’s `AccentColor`. Document these host defaults so authors know the button can follow device/browser color changes without an app override.

VERDICT: CHANGES
