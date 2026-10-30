# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck2, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `1f36dc33ba`.
- **Method:** one brief (sha256 `476ef1a76781a3f322e2fe569a8955a0010c9204e8582374c4769e156a8167dc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`): a docs-only fix to llp/1139-component-library.rfc.md. Read-only: no checkout, restore, stash, reset or edits.
Context: the EmptyState follow-up's round 2. Both reviewers' P3 was that the note's "elsewhere the app's default" misdescribed what AccentColor is, on each host, for an app that sets none. The briefs are at /tmp/iosfix-rv-es1-brief.txt and /tmp/iosfix-rv-es2-brief.txt.
The note now names each host:
- Apple: the system tint;
- Android: the device's primary and onPrimary;
- the web: the browser's AccentColor, clamped in lightness, its ink black or white (host/web/neutral.css);
- desktop Linux: the table's neutral pair;
- the terminal: the Apple column.
Android now says dynamic colours where the device has them, and Material's baseline palette below Android 12 and on TV (Palette.kt). The previous re-check found an earlier commit was missing this wording; this one carries it.
Check each claim against the code: kernel/tables/schema.json's AccentColor and AccentColorText rows, host/web/neutral.css, host/android/README.md and the Android reader, host/linux, and host/terminal/src/host.rs.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings in HEAD (`1f36dc33ba`). The host descriptions match the code, including Android’s baseline palette below Android 12 and on TV.

VERDICT: CLEAN
