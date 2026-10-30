# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck13, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `bc4aa52fd551262d8266a8d3ffc2c9a94a302cfd7a8cc910c10666bbca8d6504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (or its tail where the run failed).
- **Verdict:** clean.
- **Disposition:** none needed.

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

No P1/P2/P3 findings across the three commits.

- [stage12-s15b2.mjs:74](/tmp/iosfix-rv-g15/scripts/stage12-s15b2.mjs:74): iOS padding covers all sheet differences, including bottom paint. Combined bounds are `[0,615,402,259]` for post-menu and `[0,275,402,599]` for profile-menu. Other hosts’ unpadded groups leave zero unexplained menu pixels.
- [stage12-s15b2.mjs:76](/tmp/iosfix-rv-g15/scripts/stage12-s15b2.mjs:76): `follow` correctly covers the 408 changed pixels behind the dark iOS sheet.
- [stage12-named.test.mjs:183](/tmp/iosfix-rv-g15/scripts/stage12-named.test.mjs:183): filtering to sheet/group names preserves the per-host assertion. Three relevant read-only tests passed.
- The remaining 909 pixels are entirely within `profile-more` at `[337,162,33,33]`, consistent with the pressed-disc artifact. Rerun the capture; no additional name or padding is warranted. The earlier web-wasm video discrepancy likewise warrants a rerun.

The saved dark profile-menu capture still needs that rerun before the pixel gate is green. No files changed.

VERDICT: CLEAN
