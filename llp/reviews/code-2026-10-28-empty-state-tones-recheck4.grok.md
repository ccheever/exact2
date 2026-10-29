# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck4: the pending-importer note, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0a9d319061`.
- **Method:** one brief (sha256 `95a3b8a958d2483f25c3a75411a03371722f6f071acb28d8d0ebbc31c61211e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 low: P3).
- **Disposition:** taken in 6c0e1192e8 (each fill and its ink by full name).

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`): a docs-only fix to llp/1139-component-library.rfc.md. Read-only: no checkout, restore, stash, reset or edits.
Context: the previous re-check (/tmp/iosfix-rv-es5-brief.txt) found one P3 from both reviewers: the note said an imported accent already reaches EmptyState's button through the importer. At HEAD~1, the shadcn and Astryx importers write AccentColor only (tamber/src/import/shadcn.rs, astryx.rs). The note now says the mapping onto the brand tokens is PENDING (dtool3's item from theme2's 23:51 ruling), and that until then an app sets the tokens by full name.
Check that the note is accurate and complete, and that nothing else in the commit changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD against the prior finding: the docs note on accent tokens, and whether anything else in that commit changed. Reading the review instructions and the previous brief first.The prior finding was that the note claimed an imported accent already reaches EmptyState. I'll read HEAD and the importer code to see whether the rewritten note matches that.The commit touches only that RFC paragraph. Next I’ll check the importers and the 23:51 ruling so the rewritten note can be judged against the code.**P3** — `llp/1139-component-library.rfc.md:585`. The interim instruction names only the two fills. Theme2's 23:51 note reaches FollowButton's primary by setting `actions.follow-fill` and `actions.follow-ink` by full name, and EmptyState's pair is the same: `cards.empty-button` and `cards.empty-button-ink` (`packages/cards/cards.tamber:52-53`, `packages/actions/actions.tamber:131-132`). The rewritten sentence lists `cards.empty-button` and `actions.follow-fill`, then says an app sets "those tokens" by full name. Setting only the fills leaves the inks at fixed white, so an imported accent that is not the dark brand blue stays under the 4.5:1 bar the pending ink choice is there to fix.

The rest of the note matches the code. The shadcn and Astryx importers still only offer `AccentColor` (`tamber/src/import/shadcn.rs:743`, `tamber/src/import/astryx.rs:509`). Neither writes `cards.empty-button`, `actions.follow-fill`, or either ink token, and tailwind and DTCG do not write an accent role. Calling that mapping pending, and dtool3's item from the 23:51 ruling, matches the ruling's own wording with the tense corrected. An app override by full name is the real mechanism (`packages/cards/cards.tamber:5-6`, `llp/1121-tamber.rfc.md`).

The commit changes nothing else. `git show HEAD` is one file, `llp/1139-component-library.rfc.md`, and the hunk is only this paragraph. The brand literals, the 3.52:1 / 3.23:1 contrast lines, and the listed 4.27:1 dark gap are unchanged.

VERDICT: CHANGES
