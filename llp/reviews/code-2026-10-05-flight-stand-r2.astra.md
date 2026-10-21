# Code review r2: a flight's arriver draws the leaver's image until its own lands, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `9f9276ddea7c9087a3d3b318d1d6ac8092bb379d65e63f17fbd3c829b4303e86`), shared with grok. Round 2, a delta review of the round-1 dispositions, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. The deferred cases (a tinted image's `draw(_:)` path, an animated leaver's first frame, no test locking the interrupted path) stay DEFERRED.

---

**READY.** No new defects found in the reviewed delta.

Both hosts now keep raster and natural size consistent during interruption and clear `flightLook` on destruction/reset. The revised tests cover raster replacement at its own aspect and teardown while the view remains retained.

`git diff --check` passes. XCTests were not rerun during this read-only review; the reported result is 272 tests, 0 failures.
