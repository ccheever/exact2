# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), recheck2, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `1f36dc33ba`.
- **Method:** one brief (sha256 `476ef1a76781a3f322e2fe569a8955a0010c9204e8582374c4769e156a8167dc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
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

I'll re-check only HEAD's docs note against the hosts it names, starting with the two briefs and the commit itself.HEAD only rewrites the LLP 1139 accent note. I'll check each host claim against the schema and the readers it names.No findings. The note at `llp/1139-component-library.rfc.md:582-588` matches what each host actually resolves for an app that leaves `AccentColor` unset.

- **Apple.** `AccentColor` is `@tint` on both platforms (`kernel/tables/schema.json:5735-5742`): the inherited tint, and on the Mac `controlAccentColor`, which the session re-reports when system colours change (`host/apple/Sources/ExactKit/SystemColor.swift:175-183`, `Session.swift:554-563`). `AccentColorText` is `whiteColor` / `alternateSelectedControlTextColor`, `#ffffff` in both schemes (`schema.json:5745-5750`).
- **Android.** The reader’s palette is the device’s Material roles (`host/android/canvas/src/main/java/dev/exact/canvas/Palette.kt:31-38`): dynamic colours from API 31, `Legacy.palette()` below Android 12 and when a TV image has no colours. `AccentColor` and `AccentColorText` are that palette’s `primary` and `onPrimary` (`host/linux/src/presenter/palette.rs:139-148`, `host/android/README.md:353-355`).
- **Web.** The button is `var(--exact-accent, AccentColor)` and `var(--exact-accent-text, AccentColorText)` (`kernel/src/style/roles.rs:750-766`). `host/web/neutral.css:43` clamps the browser’s `AccentColor` per scheme, and `neutral.css:116` sets the ink to black or white from that lightness.
- **Desktop Linux.** `show_neutral_look` selects the neutral column and reports no palette (`host/linux/src/app.rs:644-652`, `palette.rs:11-13`), so the button is `#0b66d6` / `#68a5fe` with ink `#ffffff` / `#000000` (`schema.json:5739-5752`).
- **Terminal.** Boot sets only the metrics column (`host/terminal/src/host.rs:240`). The colour column stays Apple (`kernel/src/style/roles.rs:1069`), so the button is `#0088ff` / `#0091ff` under white.
- `actions.compose-tint` is the same `AccentColor` role (`packages/actions/actions.tamber:49`, `packages/cards/cards.tamber:52-53`).

VERDICT: CLEAN
