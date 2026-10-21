# Code review: overflow-wrap on Apple, break-word and min-content at UAX #14 opportunities, round 3, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x25`.
- **Method:** one brief (sha256 `0f196f105cbb4b723e1d1e35fce0487701fd78cb19fc0c52f4c3ca2b7e6e494c`), shared with astra. Round 3 (the last), blind to the other review. Reviewed the staged diff (with the round 2 fixes) in a worktree at 652d0f450. The author (Claude) is not a reviewer. The review compacted its context mid-review and re-checked the staged diff before its verdict.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** Round 3 was the last. Astra's should-fix taken after it: a soft hyphen's break that cannot show its hyphen now gives way to the opportunity before it, measured against the whole room (it shows no hyphen); only when that break is the line's first opportunity does the room less the hyphen's advance decide (a break inside, or the overflow). One helper, `TextEngine.fitSoftHyphen`, serves the paragraph and the region worker; a test checks "hi WWW " + SHY + "q" keeps "hi WWW " in both wrap modes, and the worker parity test adds the same text. This change after round 3 was not reviewed again; macOS XCTest (760) and iOS XCTest pass on it except the known failures on main, as do the five checks. Grok: READY, no findings.

---

READY

No findings.

The three round-2 items are in the staged diff and agree with layout:

1. `TextEngine.hangingSpace` (`TextBreaks.swift:75`) is U+0020, tab, U+1680, U+2000–U+200A except U+2007, U+205F, and U+3000. `breakEnds` and `pieces` both use it, so the fit and min-content trim the same characters. `testOtherSpaceSeparatorsHang` hangs U+3000 and drops it from the piece. U+00A0, U+2007, and U+202F stay content.
2. A piece adds "-" only when the raw opportunity ends on U+00AD (`TextBreaks.swift:131`), the same character `inkedSoftHyphen` paints. A soft hyphen before a space, tab, or forced break stays in the piece unreplaced (`OverflowWrapTests.swift:96`).
3. The region worker retries once with the visible hyphen's extra advance and restores the boundary cursor (`RegionWorkerLayout.swift:131`). The parity case includes `WWW` + SHY + `q` at 50 pt (`OverflowWrapTests.swift:123`).

The rest of the change holds together: forced breaks are capped (CR LF as one unit), hanging spaces are excluded from the fit, normal overflows and break-word takes the last fitting cluster when no opportunity fits, `anywhere` stays on CoreText, and the LLP 1008 paragraph matches that.
