# Code review: overflow-wrap on Apple, break-word and min-content at UAX #14 opportunities, round 2, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x25`.
- **Method:** one brief (sha256 `9c5a457b56c3fd1fd5523a9d4a24206626a3ac7e1bc6a0f68c13be4083507e76`), shared with grok. Round 2, blind to the other review. Reviewed the staged diff (with the round 1 fixes) in a worktree at 652d0f450. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r3):** Astra's three taken. (1) Hanging spaces are now U+0020, the tab and the other Unicode space separators (Zs: U+1680, U+2000-U+200A except U+2007, U+205F, U+3000), no-break spaces staying content (`TextEngine.hangingSpace`), in the fit and in min-content's trimming; a test hangs an ideographic space at the line's end and drops it from a piece. (2) A piece counts a visible hyphen only when the opportunity itself is right after the soft hyphen (as `inkedSoftHyphen` reads the line's last character), not after trimming; tests for a soft hyphen before a space, a tab and a forced break. (3) The region worker now retries with the visible hyphen's extra advance as the paragraph does, and the worker/paragraph parity test adds "WWW" + SHY + "q" at 50 points. Grok: READY, no findings.

---

NOT READY

1. [TextBreaks.swift:50](/tmp/x25/host/apple/Sources/ExactKit/TextBreaks.swift:50) — **must-fix:** Restricting hanging characters to space/tab regresses Unicode separators such as U+3000 IDEOGRAPHIC SPACE. With 17 px Helvetica, `"hi hello\u{3000}x"` at width 55 now takes three lines; `"hi hello"` fits on the first line, and its trailing separator should hang. `pieces` also incorrectly includes that separator in min-content. **Fix:** handle CSS’s hangable Unicode separators without reverting the NBSP fix, and add wrapping/min-content regressions. See [CSS Text §4.1.2](https://www.w3.org/TR/css-text-3/#white-space-phase-2).

2. [TextBreaks.swift:125](/tmp/x25/host/apple/Sources/ExactKit/TextBreaks.swift:125) — **should-fix:** Testing for SHY *after trimming* adds hyphens that layout never paints. For pre-wrap `"WWW\u{AD}\nq"`, min-content measures `"WWW-"` (53.50 px in Helvetica 17), although the first line renders `"WWW"` (48.14 px). SHY followed by spaces has the same problem. **Fix:** determine whether the original boundary selects a discretionary hyphen before trimming; add its advance only for that boundary. Test SHY before spaces, tabs, and forced breaks.

3. [RegionWorkerLayout.swift:129](/tmp/x25/host/apple/Sources/ExactKit/RegionWorkerLayout.swift:129) — **should-fix, pre-existing:** Worker/paragraph parity still fails when a visible hyphen exceeds the offer. `"WWW\u{AD}q"` at width 50 under `break-word` produces worker `"WWW-"` at 53.50 px; the paragraph correctly starts with `"WW"`. **Fix:** share the paragraph’s visible-hyphen fit handling with the worker and add this threshold case to the parity test. This also reproduces before the staged change.

The specific round 1 reproductions are resolved. Findings above were checked with read-only native CoreText probes and source review; XCTest was not rerun.
