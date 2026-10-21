# Code review: overflow-wrap on Apple, break-word and min-content at UAX #14 opportunities, round 2, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x25`.
- **Method:** one brief (sha256 `9c5a457b56c3fd1fd5523a9d4a24206626a3ac7e1bc6a0f68c13be4083507e76`), shared with astra. Round 2, blind to the other review. Reviewed the staged diff (with the round 1 fixes) in a worktree at 652d0f450. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r3):** Astra's three taken. (1) Hanging spaces are now U+0020, the tab and the other Unicode space separators (Zs: U+1680, U+2000-U+200A except U+2007, U+205F, U+3000), no-break spaces staying content (`TextEngine.hangingSpace`), in the fit and in min-content's trimming; a test hangs an ideographic space at the line's end and drops it from a piece. (2) A piece counts a visible hyphen only when the opportunity itself is right after the soft hyphen (as `inkedSoftHyphen` reads the line's last character), not after trimming; tests for a soft hyphen before a space, a tab and a forced break. (3) The region worker now retries with the visible hyphen's extra advance as the paragraph does, and the worker/paragraph parity test adds "WWW" + SHY + "q" at 50 points. Grok: READY, no findings.

---

READY

No findings.

Round 1 is fixed. `TextEngine.breakEnds` is one forward pass: a boundary's own forced break is not content (CR LF as one unit; LF, CR, VT, FF, NEL, U+2028, U+2029), and only U+0020 and U+0009 hang, so NBSP, U+2007, and U+202F stay content. `cssBreak` takes the last opportunity whose content fits at or before the next forced break, and the region worker uses that same pair for `normal` and `break-word`. Min-content pieces trim only spaces, tabs, and forced breaks, and a piece that ends in U+00AD is measured as `"-"`, matching `inkedSoftHyphen`. The new tests cover those cases, including worker and paragraph parity. LLP 1008's overflow-wrap paragraph matches this behavior.
