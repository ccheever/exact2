# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck10-named, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `e4f756fda00187affcfb495bb9ddec1692a805fcf48647a7c1ecd9f683960718`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (or its tail where the run failed).
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** taken in 14044412de and 69e1cfb742.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch (reviews otherwise clean: /tmp/iosfix-rv-g11-brief.txt). Read-only: no checkout, restore, stash, reset or edits.
The first full gate run's consumer-pixels found differences outside the named boxes. This commit, and a copy re-tag with three more testIds (no pixel change: post-menu-sheet, profile-menu-sheet, vfeed-panel-<key>), answers them:
- author-bo, every host: Bo's header FollowButton ("Follow back": its plus glyph is now the package's). It's named by its own testId, `follow`.
- video-devi: the overlay button became a point taller (32 -> 33), so the bottom panel holding it (author row, caption, actions over a gradient) grew a point, and its gradient and text shifted. It's named by the new panel testId.
- post-menu and profile-menu on iOS: the package items are 44 points (border-box, Bluesky's), where the copy's content-box min-height under 10-point padding made 64. So the groups shrank, and UIKit's native sheet holding them moved. They're named by the new sheet testIds, and the change texts are corrected.
- new-chat (web and iOS): the sheet opens with its search field focused, so a blinking caret. It's allowlisted (ALLOW['new-chat'], the composer's caret rule on chat-search).
- ios light settings-account: the Dynamic Island showed in one side's shot (system UI). It's not named; that screen gets a --screen rerun.
Check:
- each name is the smallest container holding what changes and what it moves (theme2's condition (a));
- the change texts are accurate;
- the ALLOW entry follows the existing caret rule (and keyboard handling), and no library component's box is allowlisted.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [scripts/stage12-s15b2.mjs:73](/tmp/iosfix-rv-g12/scripts/stage12-s15b2.mjs:73)** (also line 74): The new sheet IDs name content columns, excluding UIKit’s shadow and other painted overflow. Applying their union bounds to the saved iOS light shots still leaves **10,420 post-menu pixels and 11,894 profile-menu pixels outside**. These names do not contain everything the change moves; the gate remains red.

- **P2 — [scripts/stage12-gate.mjs:71](/tmp/iosfix-rv-g12/scripts/stage12-gate.mjs:71)**: `new-chat` gains the caret allowance but remains absent from `KEYBOARD_UP`, which contains only `composer`. A keyboard-service drop therefore bypasses the existing retry and keyboard-down failure checks. Add new-chat to that handling.

- **P3 — [scripts/stage12-s15b2.mjs:65](/tmp/iosfix-rv-g12/scripts/stage12-s15b2.mjs:65)**: The video change text conflates hosts. Saved layouts show unchanged panel bounds on web, iOS and Linux; macOS’s panel grows approximately 0.81 points. Qualify the panel/gradient movement as macOS-specific rather than attributing it universally to 32→33.

VERDICT: CHANGES
