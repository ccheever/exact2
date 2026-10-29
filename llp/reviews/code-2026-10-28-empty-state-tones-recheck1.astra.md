# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck1, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `3d15e42170`.
- **Method:** one brief (sha256 `53fe00df620ea3fd0adf4fa6803ec9087c09bfe317851232b97649db22212e7b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 low: P3).
- **Disposition:** that commit lacked the staged wording (an amend without -a); the full wording and the Android baseline-palette caveat are 1f36dc33ba, re-checked as recheck2.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`): a docs-only fix to llp/1139-component-library.rfc.md. Read-only: no checkout, restore, stash, reset or edits.
Context: the EmptyState follow-up's round 2. Both reviewers' P3 was that the note's "elsewhere the app's default" misdescribed what AccentColor is, on each host, for an app that sets none. The briefs are at /tmp/iosfix-rv-es1-brief.txt and /tmp/iosfix-rv-es2-brief.txt.
The note now names each host:
- Apple: the system tint;
- Android: the device's primary and onPrimary;
- the web: the browser's AccentColor, clamped in lightness, its ink black or white (host/web/neutral.css);
- desktop Linux: the table's neutral pair;
- the terminal: the Apple column.
Check each claim against the code: kernel/tables/schema.json's AccentColor and AccentColorText rows, host/web/neutral.css, host/android/README.md and the Android reader, host/linux, and host/terminal/src/host.rs.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

Reviewed HEAD `3d15e421`. It does **not** contain the full wording described in your request.

- **P3 — [llp/1139-component-library.rfc.md:582](/tmp/iosfix-rv-es3/llp/1139-component-library.rfc.md:582):** The host account remains incomplete. Linux’s neutral pair and the terminal’s Apple column are omitted. The web description also omits the lightness clamp and black/white ink implemented in `host/web/neutral.css:43` and `:116`.

- **P3 — [llp/1139-component-library.rfc.md:584](/tmp/iosfix-rv-es3/llp/1139-component-library.rfc.md:584):** Android is described unconditionally as using a dynamic palette. `Palette.kt:31–38` uses Material’s baseline palette below Android 12 or when resources are unavailable, including Android TV. Naming `primary`/`onPrimary`, with dynamic colours where available, accurately covers these paths.

VERDICT: CHANGES
