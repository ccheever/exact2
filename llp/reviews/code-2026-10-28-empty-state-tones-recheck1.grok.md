# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck1, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3d15e42170`.
- **Method:** one brief (sha256 `53fe00df620ea3fd0adf4fa6803ec9087c09bfe317851232b97649db22212e7b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes.
- **Disposition:** as Astra's; re-checked as recheck2.

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

I'll re-check only HEAD's docs note against the host accent-color code, read-only, and compare it to the two prior briefs.The HEAD note is shorter than the five-host wording in the brief. I'll read that paragraph and check each host claim against the accent tables and readers.[P3] Name desktop Linux and the terminal in the unset-accent sentence — llp/1139-component-library.rfc.md:582

The new sentence names Apple, Android, and the web, then says the button follows a device or browser colour change. Desktop Linux calls `show_neutral_look` before boot (`host/linux/src/app.rs:651`), so `AccentColor` and `AccentColorText` stay the schema's neutral pair (`#0b66d6`/`#68a5fe` and `#ffffff`/`#000000`, `kernel/tables/schema.json:5735`). The terminal sets only the metrics column (`host/terminal/src/host.rs:240`) and leaves the role column at its start value, Apple (`kernel/src/style/roles.rs:90`), so it shows the Apple snapshot `#0088ff`/`#0091ff` with white ink in both schemes. Those pairs stay put when a device or browser accent changes. Windows, which shares `show_neutral_look`, is the same fixed neutral pair.

[P3] Say the web accent is lightness-clamped and its ink is black or white — llp/1139-component-library.rfc.md:584

The clause says the web gets the browser's `AccentColor` (the user agent's or the system's) under `--exact-accent`. `host/web/neutral.css:43` writes `--exact-accent` as `oklch` from `AccentColor` with lightness clamped (`min(l, .53)` in light, `max(l, .72)` in dark). The button ink is `--exact-accent-text`, black or white from that colour's lightness (`host/web/neutral.css:116`), which is what the kernel emits for `AccentColorText` (`kernel/src/style/roles.rs:762`). The browser keyword `AccentColorText` stays white on any accent. The later white-ink sentence is the Apple column only (`kernel/tables/schema.json:5749`).

[P3] Name Android's primary and onPrimary, including the baseline scheme — llp/1139-component-library.rfc.md:583

The clause says Android's accent is the device's, from its Material 3 dynamic palette. The mapping is `AccentColor` = `primary` and `AccentColorText` = `onPrimary` (`host/linux/src/presenter/palette.rs:139`, `host/linux/src/presenter/palette.rs:148`; `host/android/README.md:353`). The reader fills those from the dynamic palette on API 31 and up (`host/android/canvas/src/main/java/dev/exact/canvas/Palette.kt:33`) and from Material 3's baseline scheme before Android 12 and on Android TV (`host/android/canvas/src/main/java/dev/exact/canvas/Palette.kt:20`, `host/android/canvas/src/main/java/dev/exact/canvas/Legacy.kt:108`).

Apple's "system tint" holds: both Apple platforms cross `AccentColor` as `@tint` (`kernel/tables/schema.json:5737`, `host/apple/src/colors_tests.rs:522`), and on the Mac that is `controlAccentColor` when nothing authors one. `actions.compose-tint` is the same role (`packages/actions/actions.tamber:49`, `packages/cards/cards.tamber:52`).

VERDICT: CHANGES
