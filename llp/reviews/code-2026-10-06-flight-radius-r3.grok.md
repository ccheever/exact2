# Code review r3: a landed flight's view takes its clip and corners from its style (LLP 1013.000 D4/D4.4), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x2fr-rv`.
- **Method:** one brief (sha256 `8f332d931793968d6cf48036dd81e9d371b9c9c66a688956cc38d667abaabe4b`), shared with the other reviewer. Round 3, a confirmation of the post-r2 rewrite of macOS `overflowClips` only; blind to the other's round-3 review. Reviewed commit 212ebc2ac in a detached worktree. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** none needed. Landed after a rebase onto origin/main with the five checks, Mac XCTest (800, 0 failures) and iOS XCTest (363, 0 failures) green.

---

I'll confirm the rewrite of `overflowClips` is equivalent for every style, and that `git diff 1badc497c HEAD -- host` contains no other code change.The host diff is only `overflowClips`. I'll compare both predicates against the style accessors and confirm the commit's parent is the round-2 tree.**READY.** The rewritten `overflowClips` matches the round-2 predicate on every style. No findings.

HEAD is `212ebc2ac`, the worktree is clean, and that commit and the round-2 commit `1badc497c` share the parent `6aeacdd3f`. `git diff 1badc497c HEAD -- host` is only the comment and body of `overflowClips` in `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift`. The file is 1,497 lines. The only other difference between those commits is the two round-2 review writeups.

Let `X` and `Y` be `style["overflow_x"]?.string ?? "visible"` and the same for `overflow_y`, and let `C` be `number("line_clamp") > 0`. `BatchValue.string` is a pure projection (a string, a profile key, or nil), so a missing key and any non-string value both become `"visible"` in both forms.

- For `kind == "text"`, the round-2 form forces the axis locals to `"visible"`, then returns true when either raw axis is not `"visible"` or `C` is true. The new form is `over.contains { $0 != "visible" } || C`. Same result.
- For every other kind, both return `X == "hidden" || Y == "hidden"`. `line_clamp` is not read.
- `number("line_clamp")` still runs only for text whose axes are both `"visible"`. Each overflow key is still read once.

That split covers every string, including nil. A 2,116-point mirror of the two functions (CSS keywords, empty strings, case and whitespace variants, and both clamp flags) had 0 mismatches, and the ternary keeps `|| number("line_clamp") > 0` in the text branch: a non-text node with `line_clamp > 0` and visible overflow stays unclipped.

I did not re-run XCTest. The reported 799 tests and 0 failures sit on a predicate identical to the one round 2 accepted, and this host diff has no other code.
