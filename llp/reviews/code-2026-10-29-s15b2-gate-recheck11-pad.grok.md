# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck11-pad, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `699e74d03abd8a230461fc3da2d57d28b1bfe173f0329f375612e0a5685b31bf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** taken in 8dfddf0646 and f7baf1b7af.

## Brief

Narrow re-check of TWO commits, HEAD~1 and HEAD (`git show HEAD~1 HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
The previous re-check (/tmp/iosfix-rv-g12-brief.txt, of the commit naming the first full run's differences) found:
- Astra P2: the iOS menus' sheet names missed UIKit's own sheet frame (10-12k px outside);
- both: new-chat wasn't in the keyboard rule;
- Astra P3 and Grok P2: the video panel name covered every host, though only the Mac's panel grows.
These commits:
- add a named entry's `pad` [top, right, bottom, left] for a frame the platform paints past its container. The menus use UIKit's sheet, 8 each side and 28 above, said in the entry's change. Measured on the run's iOS shots: content roots (8,643,386x223) before and (8,683,386x183) after; differences 0..401 x 618..857.
- add new-chat to KEYBOARD_UP;
- let a named entry name its `hosts`: the video feed is named by its follow button everywhere, and by its bottom panel on the Mac alone.
Check:
- whether a pad is acceptable under theme2's condition (b), "the diff's bounding box must lie inside the container", with the container now padded and the reason stated;
- the pad's numbers against the shots (target/stage12-gate/before|after/ios-light-post-menu.png and .out);
- host filtering through namedOf, namedReport and the shot records;
- the keyboard rule for new-chat (an autofocused search field);
- tests.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check the two commits on the s15b2 gate branch against the prior findings, read-only. First I'll read the previous brief and the commit diffs.The sheet testId sits on a shorter column than the route the brief measured. I'll diff the shots against both boxes.The measurements are already in hand. I'll confirm the cited lines in the two commits, then write the findings.The pad and host filters are in the two commits. Next I’ll check those boxes against the saved iOS shots and the keyboard rule.The sheet test id sits on the content column. I'll measure that padded box against the saved shots.The pad is a valid reading of condition (b) for pixels inside the widened rect, and the host filter and the new-chat keyboard rule are right. The menu shots still leave differing pixels outside those rects.

**P2 — The padded menu column still misses the dark profile backdrop. `scripts/stage12-s15b2.mjs:75`**

`profile-menu-sheet` is padded `[28, 8, 0, 8]`, so on these shots the union is `[0, 275, 402, 559]` (y through 833). `ios-dark-profile-menu` has 183,682 differing pixels and a box of `[0, 162, 401, 865]`. Of those, 1,317 sit above the pad, at y 162–194 and x 256–369, over the header `follow` button `(241.7, 162, 91×34)` and `profile-more`. 835 of them are exactly `95,95,95` → `20,20,20` (max channel gap 75). The same points on `ios-dark-profile-ada` are the header’s own colors and are identical on both sides, so this is the sheet’s gray backdrop, and the 28-point top pad does not reach it. No other name on `profile-menu` covers that band. Strict zero still fails this shot.

**P2 — Bottom pad 0 leaves the measured sheet box outside the named column. `scripts/stage12-s15b2.mjs:74`**

The same pad is on `post-menu-sheet` (`scripts/stage12-s15b2.mjs:75` for the profile sheet). The roots in the commit note, `(8, 643, 386×223)` and `(8, 683, 386×183)`, are the route column at `app.contract:3590`. The test id sits on the inner column, about 33 points shorter (`386×189.48` before, `386×150.75` after). `namedOf` pads that inner column (`scripts/stage12-named.mjs:85`). Sides of 8 match the frame: content is inset 8 on a 402-wide shot and the diffs run x 0..401. The top 28 covers the painted band with a little slack: post-menu diffs start at y 618, and the padded top is y 615 (content y 643.23, so the band is about 25 points). That brings `ios-light-post-menu` from 10,420 pixels outside the raw column down to one, and `ios-dark-post-menu` to zero (the raw column left 4,891). The one that remains is `(31, 857)`, `211,211,212` → `211,211,211`. It is inside the measured box `0..401 × 618..857` and inside the padded route (which runs through y 865), and outside the padded sheet, which ends at y 833. `ios-light-profile-menu` still has 36 pixels outside, all a one-level gap, scattered in y 834–866 at the side edges of the home-indicator band (x about 11–16 and 386–388). The padded route still misses one of them, `(319, 866)`, because the route ends at 866 and the rect is half-open. Bottom pad 0 is why.

Condition (b) holds for a name’s own reported box. `pixelDiff` builds that box only from pixels inside the rect (`scripts/stage12-named.mjs:52`), so it lies inside the padded container, and the change text states the UIKit reason. A pad is acceptable on those terms. These two shots’ diff boxes are not inside that container.

**P3 — The UIKit pad is applied on every host. `scripts/stage12-s15b2.mjs:74`**

`hosts` exists (`scripts/stage12-named.mjs:76`) and this commit uses it for the video panel, but neither menu entry sets it. Web, Mac, and Linux sheets are full-bleed (web’s column starts at y 12; Mac and Linux at y 0), and every current diff on those shots is already inside the unpadded column (web light post-menu’s box is `[24, 56, 395, 186]`). The extra top band is slack and currently holds no differing pixel.

Host filtering checks out. `namedOf` drops an entry whose `hosts` omit the drive’s host, and the shot record passes that host (`scripts/stage12-gate.mjs:209`). `namedReport` (`scripts/stage12-named.mjs:128`) matches target and screen suffix, so a Mac-only target is NAMED from the Mac shots and is absent from the others. The follow entry stays first, so on the Mac the button’s pixels count as the button and the rest as the panel. Panel height changes only on the Mac (`181.97` → `182.78`); iOS, Linux, and web keep the same height while the button grows, and their pixels outside the follow-button union are 0, or one web edge pixel the noise floor drops. `web-wasm-dark-video-devi` is the exception: 3,033 pixels outside the button (box `[12, 725, 240, 880]`) with the panel bounds unchanged. The old every-host panel name was covering those; this split stops doing so. Web dark has none, so it reads as that one shot rather than a panel-growth miss.

The keyboard rule checks out. `KEYBOARD_UP` is `['composer', 'new-chat']` (`scripts/stage12-keyboard.mjs:7`). The redrive uses `ALLOW['new-chat'].target`, `chat-search` (`scripts/stage12-gate.mjs:71`, `scripts/stage12-gate.mjs:203`), and `screenOps` appends `layout chat-search` after the whole layout. The field is `autofocus` (`chat.contract:1498`). All four saved iOS new-chat shots already have `keyboard-inset-height` 328. Those `.out` files have only the whole-layout reply; a new drive gets the single-node reply last, as the composer shots already do.

Tests: `stage12-named.test.mjs` covers the pad clip (`[5, 2, 100, 20]` → `[0, 15, 43, 35]`) and a Mac-only entry returning nothing on web. `stage12-gate.test.mjs:530` expects the new `KEYBOARD_UP`. `bun test` on those two files: 38 pass, 2 skip, 0 fail. Nothing asserts the menu pad numbers or that the pad is iOS-only.

VERDICT: CHANGES
