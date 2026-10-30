# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck10-named, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `e4f756fda00187affcfb495bb9ddec1692a805fcf48647a7c1ecd9f683960718`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (2 medium).
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

I'll re-check only HEAD against the gate brief and the five named pixel differences, without changing the worktree.HEAD is the pixel-naming commit. I'll read the full diff and the gate rules those names have to satisfy.The commit only touches the gate name list and the caret allowlist. I'll check those against theme2's naming rule and the components they point at.The clone is outside this repo. I'll read its follow button, video panel, menus, and new-chat search against the names in HEAD.The re-check is still open on two claims: whether the video panel actually grows with the taller button, and whether the new-chat caret allowlist matches the existing keyboard rule. I'll read those paths and finish the verdict.[P2] Name video-devi by the follow button, and drop the panel-growth sentence — `scripts/stage12-s15b2.mjs:65`

The new clause says the overlay button's 32→33 makes the bottom panel, its gradient, and its text shift, and the name is `vfeed-panel-at://did:plc:fixture3/app.bsky.feed.post/fx9`. That 32→33 is the web/Linux button, where the root is 16: padding 8+8 plus a 17px line, with the icon omitted (`icon="following"` and Devi is not followed). The author block beside it is already taller. The name line is `1.25rem` (20) and the handle line is `1.0625rem` (17), stacked with no gap, so 37. The flex line's cross size is the largest outer size, and this engine adds the cross-axis margin before taking that max. The old button's margin box is 32+4 = 36; the new wrapper's is 33+4 = 37. The row is `align-items: center` and stays 37, so the caption, controls, scrubber, and panel gradient stay put. The smallest testId that holds the change is still `vfeed-follow-at://did:plc:fixture3/app.bsky.feed.post/fx9`. iOS's root is 17, so the author block stays above the button's margin box there as well, and the button is not 33 points. macOS's root is 13, where the fixed 32px avatar can lose to the button's margin box and the row can grow, at a different height than 32→33. The panel name is shared by every host, so a caption, controls, or scrubber diff on web, Linux, and iOS passes as this follow-button change.

[P2] Retry new-chat while the iOS keyboard is down, as composer does — `scripts/stage12-gate.mjs:71`

The new allow entry copies the composer's caret onto `chat-search`: the field's leading 4px, capped at 40. `allowOf` adds the system-keyboard band only when that reply's inset is already above zero. `KEYBOARD_UP` is still only `composer` (`scripts/stage12-keyboard.mjs:7`), so new-chat is not driven again while the keyboard is down and a still-down shot is not failed. The sheet's search field is a native `input type="search"` with `autofocus`, and iOS autofocus calls `becomeFirstResponder`, which raises the software keyboard. The composer is retried because that service drops under load. One side up and the other down shortens only one viewport, and the `chat-people` list (`flex: 1`) lays out at a different height. Those pixels sit outside the 4px caret. The keyboard rectangle covers the band below the viewport on the side that already has an inset, not the reflowed list. `chat-search` is the app's own field, so the caret strip does not allowlist a library box.

`follow` on author-bo is the header button (Bo is signed out and followed-by, so it reads "Follow back"). The actions row is `justify-content: flex-end`, so the narrower pill's leading edge stays inside the union of the two `follow` boxes and `profile-more` stays at the trailing edge. Both menu texts match the old content-box item (min-height 44 under 10-point padding, border box 64) and the package's border-box 44. `post-menu-sheet` and `profile-menu-sheet` wrap the handle and the groups; signed-out Translate/Copy and Ada's profile menu both stay above the column's min-height of 128, so the fit-content sheet tracks the shorter groups. settings-account is not named.

VERDICT: CHANGES
