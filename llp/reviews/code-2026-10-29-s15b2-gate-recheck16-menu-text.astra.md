# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck16-menu-text, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review (`8d16b8c300`).
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

No P1/P2/P3 findings in `8d16b8c300`.

- Only the iOS `change` strings at [scripts/stage12-s15b2.mjs:74](/tmp/iosfix2-rv-txt-astra/scripts/stage12-s15b2.mjs:74) and [line 76](/tmp/iosfix2-rv-txt-astra/scripts/stage12-s15b2.mjs:76) changed. Targets, screens, hosts, padding, rules and other hosts’ entries are unchanged. Quoting is intact.
- Both layout files are absent. The wording is consistent with the supplied measurements; those measurements were not independently verified.
- Six relevant read-only tests passed, zero failed. The remaining 35 were filtered out; the full suite requires temporary-file writes.

VERDICT: CLEAN
