# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck16-menu-text, 2026-10-29 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree of the branch's commit under review (`8d16b8c300`).
- **Method:** one brief (sha256 `bf121aa2a06b53f1a2cf96812bdc014edebee9666763615895dd51d1a13603a1`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, `git show HEAD` (8d16b8c300) on the s15b2 gate branch (x2/s15b2-gate): a text-only change to two NAMED entries' change text in scripts/stage12-s15b2.mjs (the iOS post-menu-sheet and profile-menu-sheet names). Read-only: no checkout, restore, stash, reset or edits.

The new words say that on iOS each 44-point item reads 42.25 points on screen, inside UIKit's sheet, which scales its content about 0.96 on both sides. Author's evidence: in the gate's after layouts, post-menu-translate and post-menu-copy are 42.25 tall on iOS and 44 on web, macOS and Linux; the before side's iOS groups are 170.28 tall where the web's are 178.5 (ratio about 0.954); the iOS sheet is 386 wide in a 402 viewport.
Check:
- only the two iOS names' change text changed; no target, screen, hosts, pad or rule changed; the other hosts' entries are untouched;
- the words are accurate against target/stage12-gate/{before,after}/ios-light-post-menu.out if present in this tree (if not, judge the wording only);
- string quoting is intact, and the gate tests still pass (bun test scripts/stage12-named.test.mjs scripts/stage12-gate.test.mjs, if you can run read-only).
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

`8d16b8c300` changes only the `change` strings of the two iOS sheet names in `scripts/stage12-s15b2.mjs`. The same clause is inserted in both: `; on iOS 42.25 on screen, inside UIKit's sheet, which scales its content about 0.96 on both sides`. `screen`, `hosts`, `target`, and `pad` are unchanged. The web, web-wasm, macOS, and Linux entries at lines 75 and 77 are untouched, and so are the other twelve named entries.

`target/stage12-gate/{before,after}/ios-light-post-menu.out` is not in this tree (`target/` is gitignored and the directory is absent), so the heights were not re-measured from those dumps. On the wording alone, the new clause matches the sheet scale this tree already records: a page sheet on a 402-point iPhone is drawn at `386/402` (0.960), and a 44-point border-box item at that factor is 42.25 on screen. "About 0.96" is that factor. "On both sides" attaches to the scale, not to 42.25; the copy's 64-point items stay in the `where` clause. `170.28/178.5` is 0.954, which is the before group-height ratio, not the sheet's own scale. Both lines still parse: the apostrophes in `UIKit's` are escaped, and the evaluated strings contain the insertion.

`bun test scripts/stage12-named.test.mjs scripts/stage12-gate.test.mjs`: 39 pass, 2 skip (`EXACT_GATE_DRIVES` is unset), 0 fail. The worktree is still clean at `8d16b8c300`.

VERDICT: CLEAN
