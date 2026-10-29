# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), r2, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `491a982552`.
- **Method:** one brief (sha256 `47512ebd8044280541cf9f060ad2a0665f25523ea1aa39f28b1dd8e52e0e6241`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 low: P3, the same as Astra's).
- **Disposition:** taken in 1f36dc33ba.

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

I'll review the two commits read-only, starting with round 1's brief and the branch diff against the merge-base of `origin/main`.The brief is a two-commit follow-up: EmptyState variants, then the accent-token swap and the contract-doc fix. I'll collect the merge-base diff and check those claims against the component, readers, and host role tables.The diff is eight files. Next I’ll check the component against the docs, then every reader of the old button literals and what `AccentColor` actually resolves to on each host.Auto-compacting conversation (83% full)...
The remaining question is whether LLP 1139's note about an unset accent is accurate on every host. I'll confirm that sentence against the role tables before writing the verdict.**P3** — `llp/1139-component-library.rfc.md:583` says an app that sets no `AccentColor` gets "on Apple the system tint (which the user may change on a Mac), elsewhere the app's default." The Apple half is right. "Elsewhere the app's default" is not.

An unset `AccentColor` is the platform accent, and it is not one app-owned blue:

- Web uses the browser's `AccentColor`, clamped to lightness 0.53 in light and 0.72 in dark (`host/web/neutral.css:39`). On Safari that is the Mac user's accent, so a Mac user can change it off Apple too. The ink is black or white from that lightness in both schemes (`host/web/neutral.css:117`).
- Android uses the device's Material `primary` and `onPrimary` (`host/android/README.md:353`).
- The neutral pair in `kernel/tables/schema.json:5735` (`#0b66d6` / `#68a5fe`, ink `#ffffff` / `#000000`) is what an unreported Linux host paints. The Android note says that painter blue is desktop Linux's alone.
- The terminal never calls `roles::set_column` (`host/terminal/src/host.rs:240` sets only the metrics column), so it stays on the Apple column (`#0088ff` / `#0091ff`, white ink in both schemes), not a live tint and not that neutral pair.

`actions.compose-tint` is the same `AccentColor` role (`packages/actions/actions.tamber:49`), so that parallel holds. The token assignments do too: `cards.empty-button = AccentColor` and `cards.empty-button-ink = AccentColorText` (`packages/cards/cards.tamber:52`). The package comment at `packages/cards/cards.tamber:41` already says the accurate short form ("an app's AccentColor, else the platform's"). The next LLP sentence, that Apple's `AccentColorText` is white in both schemes, matches `kernel/tables/schema.json:5744`.

VERDICT: CHANGES
