# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck13, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `bc4aa52fd551262d8266a8d3ffc2c9a94a302cfd7a8cc910c10666bbca8d6504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 medium: P2, drop the follow name).
- **Disposition:** taken in 2a7959a93f.

## Brief

Narrow re-check of THREE commits, HEAD~2..HEAD (`git log -p HEAD~3..HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
The brief for HEAD~2 and HEAD~1 is at /tmp/iosfix-rv-g14-brief.txt; read it first. At that re-check, Astra's run failed (the model was at capacity), so please cover those two commits too. Grok found ios-dark-profile-menu had 1,317 px above the padded sheet:
- 408 inside Ada's header `follow` behind the sheet (the adoption);
- 909 in the `profile-more` disc, left pressed by the drive (a flat fill, rgb 95 to 20).
HEAD adds a profile-menu name for `follow` (Ada's header FollowButton, seen behind the sheet), and fixes the test that counts the menu names per host.
Check:
- the iOS pad's numbers against the saved shots (~/x2scratch/iosfix-s15b2-gate/target/stage12-gate/before|after/ios-*-post-menu, ios-*-profile-menu .png/.out);
- the per-host split;
- the new follow name;
- whether the profile-more pressed disc needs anything more than a rerun. It's a drive artifact: the tap that opens the menu leaves the button pressed on one side.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check the three commits on the s15b2 gate branch, starting with the prior brief and the patch range, without changing the worktree.The range is three commits on `HEAD`. Next I’ll read the patches and the saved iOS menu shots they claim to match.The three patches are small. I’ll read how pads and host names are applied, then measure them against the saved menu shots.The naming rules are in `stage12-named.mjs`. I’ll measure the saved menu shots against those boxes, including the new `follow` name.The saved shots predate these commits, so I’ll recompute the named boxes from the layouts and measure which differing pixels they actually cover.[P2] Remove the profile-menu `follow` name — `scripts/stage12-s15b2.mjs:76`

On `ios-dark-profile-menu` the 408 pixels in Ada's header `follow` are the glyph band (x 256–317, y 173–186), and every one of them is a flat gray pair. The largest bucket is `(95,95,95)` → `(20,20,20)`, the same pair as the 767 interior pixels of the `profile-more` disc. `ios-light-profile-menu` has no differing pixels in that button. `profile-ada`'s `follow` button has none on any host — the iOS `profile-ada` shots are byte-identical — so this is the dark menu tint, seen with the sheet up. That box sits outside the padded sheet, and the name is what lets those pixels pass. The tally in `scripts/stage12-named.test.mjs:183` was narrowed to `sheet|groups` so this unscoped entry stays in.

The iOS pad `[28, 8, 40, 8]` matches the shots. Sheet content is the untagged column the re-tag names (`post-menu-sheet` / `profile-menu-sheet`): post `(8, 643.2, 386×189.5)` before and `(8, 682.6, 386×150.8)` after; profile `(8, 303.3, 386×530)` before and `(8, 437.8, 386×395)` after. Chrome outside that column stays within the pad: top at most 25.3 points (the pad is 28), left 8, right 7, bottom at most 32.7. That bottom reach covers the old 36 profile pixels and the post pixel near `(31, 857)`. Nothing differing sits in the slack down to the screen edge. Light post, dark post, and light profile are clean once the pad is applied.

The per-host split holds. On web, web-wasm, macOS, and Linux, every differing menu pixel falls inside the unpadded groups column (handle band included, zero pixels outside it). The iOS sheet pad is what those hosts do not need.

The `profile-more` disc needs a rerun, not a name. On `ios-dark-profile-menu` it is 909 pixels in the unchanged 33×33 box: 767 of them that same `(95,95,95)` → `(20,20,20)` fill, the ellipsis at the center still `(89,96,105)`. Light profile and both post menus, which tap a more control too, show none of it. `profile-ada`'s more button is unchanged. Read the rerun for the follow ink as well; it moved with the disc.

VERDICT: CHANGES
