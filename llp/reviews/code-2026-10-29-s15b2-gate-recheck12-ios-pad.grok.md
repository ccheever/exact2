# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck12-ios-pad, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `cc4b39d6a49c2512dc568a4fb388e760f4311abfcd18855a93fa007c0c41ba0f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 high: P1).
- **Disposition:** taken in f54c97c4a0, then reverted at its ask in 2a7959a93f.

## Brief

Narrow re-check of TWO commits, HEAD~1 and HEAD (`git show HEAD~1 HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
The previous re-check (/tmp/iosfix-rv-g13-brief.txt) found:
- both: the pad missed UIKit's bottom paint (36 px of profile-menu and (31,857) of post-menu, below the root column);
- both: the pad applied on every host, where web, Mac and Linux are full-bleed and their differences sit inside the groups.
These commits:
- split each menu's name: hosts ['ios'] the sheet with pad [28, 8, 40, 8] (40 below, to the home indicator); hosts web, web-wasm, macos and linux the groups column, unpadded, as before;
- add tests that a pad is iOS's alone, said as UIKit's, and that every menu screen is named on every host.
Check:
- the numbers against the saved iOS shots: ~/x2scratch/iosfix-s15b2-gate/target/stage12-gate/before|after/ios-*-post-menu and profile-menu .png/.out. The roots in those .out have no testId yet (the copy was re-tagged after), but their bounds are there;
- that no differing pixel would fall outside.
Also: web-wasm dark video-devi had 3,033 px outside the follow button with the panel's bounds unchanged. I take that as one flaky shot (web dark had none) to rerun, not a name. Say if you disagree.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only `HEAD~1` and `HEAD` against the earlier brief and the saved iOS shots, without changing the tree.The two commits only split the menu names and add the pad tests. Next I’ll measure the saved iOS bounds against the pad and see whether any differing pixel still falls outside.The sheet roots are untagged in those shots, so I’ll match them by bounds and compare every differing pixel to the padded box.P1 `scripts/stage12-s15b2.mjs:76`. On `ios-dark-profile-menu` the padded sheet is `[0, 275, 402, 599]` (y 275–874). 1,317 differing pixels sit above it, at y 162–194.

- 408 are inside `follow` (`241.7, 162, 91.3×34`). The before shot draws the copy's button (`profile.contract`); the after shot draws package `FollowButton`. The light shot of this screen is identical in that box. These pixels are the adoption, and they stay outside this name.
- 909 are the `profile-more` disc (`337, 162, 33×33`): a flat fill, `rgb(95,95,95)` to `rgb(20,20,20)`, icon center unchanged. The control's contract is the same on both sides (`background-color="contrast_50"`). The sibling `profile-bell`, same token, is `rgb(23,23,23)` on both dark shots. The drive left `profile-more` pressed. A rerun that has released the press can clear only these 909.

The sheet root those pads apply to is the `screens.contract` column (the node the re-tag names `post-menu-sheet` / `profile-menu-sheet`), not the route column above it. The route column ends at y 866, about 33 points below the content.

| Shot | Content root (bottom) | Padded box | Outside |
|---|---|---|---|
| ios-light-post-menu | `(8, 643.23, 386×189.48)` bottom 832.71 | `[0, 615, 402, 259]` | 0. Unpadded, the one pixel `(31, 857)` is below the root. |
| ios-dark-post-menu | same root | same box | 0 |
| ios-light-profile-menu | `(8, 303.32, 386×530.03)` bottom 833.35 | `[0, 275, 402, 599]` | 0. Unpadded, 36 pixels, y 834–866, are below the root. |
| ios-dark-profile-menu | same root | same box | 1,317, all above |

`[28, 8, 40, 8]` matches that root. The first sheet pixel is 3 rows inside the top (y 618 against a top of 615). Diffs touch x 0 and x 401, and the 8-point side pads include them. The lowest bottom pixel is y 866; 40 below the content reaches it (the shorter root, bottom 832.71, pads to 872). The gap from the content to the screen bottom is 40.65–41.29. The rows past y 866 do not differ.

Every other menu shot's differing pixels sit inside its name. The four non-iOS hosts, both schemes, both menus: the unpadded groups union contains them all (0 outside). Those groups are full-bleed (`x 0`, width 420 on a 420-wide viewport). `web-wasm` is in that set and is clean. The test at `scripts/stage12-named.test.mjs:178` locks the pad to `hosts: ['ios']` with `UIKit` in the change, and `:182` locks both menu screens to all five hosts with the iOS pad `[28, 8, 40, 8]`.

web-wasm dark `video-devi`: I agree, one shot to rerun, not a name. Under the gate's web edge rule it is 3,052 pixels outside the follow button (your 3,033), in the author, caption, and reply bands, while the bottom panel stays `0, 722.3, 420×104.7` and the caption, scrubber, and reply boxes do not move. The author row narrows by about 4 points on every host, the same way, and web dark has none of those pixels outside the button. Light wasm is clean too.

VERDICT: CHANGES
