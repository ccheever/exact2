# Code review: overflow-wrap on Apple, break-word and min-content at UAX #14 opportunities, round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x25`.
- **Method:** one brief (sha256 `0f196f105cbb4b723e1d1e35fce0487701fd78cb19fc0c52f4c3ca2b7e6e494c`), shared with grok. Round 3 (the last), blind to the other review. Reviewed the staged diff (with the round 2 fixes) in a worktree at 652d0f450. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Round 3 was the last. Astra's should-fix taken after it: a soft hyphen's break that cannot show its hyphen now gives way to the opportunity before it, measured against the whole room (it shows no hyphen); only when that break is the line's first opportunity does the room less the hyphen's advance decide (a break inside, or the overflow). One helper, `TextEngine.fitSoftHyphen`, serves the paragraph and the region worker; a test checks "hi WWW " + SHY + "q" keeps "hi WWW " in both wrap modes, and the worker parity test adds the same text. This change after round 3 was not reviewed again; macOS XCTest (760) and iOS XCTest pass on it except the known failures on main, as do the five checks. Grok: READY, no findings.

---

READY WITH CHANGES

1. [Text.swift:1040](/tmp/x25/host/apple/Sources/ExactKit/Text.swift:1040), [RegionWorkerLayout.swift:138](/tmp/x25/host/apple/Sources/ExactKit/RegionWorkerLayout.swift:138) — **should-fix:** The SHY retry subtracts the hyphen’s advance even when reconsidering ordinary breaks. With Helvetica 17, `"hi WWW \u{AD}q"` at 71 pt starts with `"hi "`, although `"hi WWW "` fits at 70.81 pt. This affects both wrap modes; the paragraph issue predates this change and is now mirrored into the worker. **Fix:** evaluate earlier opportunities against the original offer, adding hyphen width only for candidates that render one. Add expected-range assertions for both paths and modes.

The three round 2 reproductions are resolved. Reviewed the staged change and ran read-only CoreText probes; XCTest was not rerun.
