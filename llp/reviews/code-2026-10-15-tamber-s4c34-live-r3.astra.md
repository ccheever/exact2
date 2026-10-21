# Code review: stage 4 commits 3 and 4: the live table, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Method:** one brief (sha256 `536e3c1bf3dce6c28fb827bdafb3cea95b9594a8b2c64fe4a027390fa78f4aaa`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a snapshot checked before its body was read; a failed build forgetting edits; natives fetching the JS loop's snapshot).
- **Disposition:** applied without re-review (the last round) in the third-round commit; the natives' snapshot is commit 5's one-source relay.

---

Reviewed all three commits and both prior fix lists. **Three blocking findings remain.**

1. **P1 — An old snapshot can still overwrite a new generation.** [host/web/dev.js:282](/tmp/rv-s4-6/host/web/dev.js:282)  
   The generation check runs before `await response.json()`, and `apply()` also awaits readiness. A new generation with the same structure can land during either wait; the old snapshot then overwrites its values. If the subsequent snapshot request returns 204, those incorrect values remain. Round 2’s fix is incomplete: validate the generation immediately before applying, after all awaits.

2. **P2 — Failed builds forget unpublished asset changes.** [host/web-js/dev.mjs:118](/tmp/rv-s4-6/host/web-js/dev.mjs:118)  
   `edited` is cleared before the build succeeds. Edit an asset alongside an invalid `.tamber` file, let compilation fail, then fix only the design. The next build qualifies as value-only because assets affect neither structure nor `logicRevision`; `keep()` discards the rebuilt asset and sends only a token batch. Retain pending changes until a successful publication.

3. **P2 — Native clients receive the wrong producer’s snapshot.** [host/web-js/dev.mjs:236](/tmp/rv-s4-6/host/web-js/dev.mjs:236)  
   Native generation requests use the resident producer, but `/__dev/design` always serves the JS loop’s snapshot. Changing a token to or from `AccentColor` makes the JS loop reload and clear its snapshot, while the resident producer sends a batch without publishing a generation. Native snapshot requests then receive 204: polling Linux clients and reconnecting Apple sessions retain old values. Route snapshots to the corresponding producer, or preserve the documented native reload fallback.

The three JS mode tests pass. In-memory replays using extracted source reproduced these control-flow failures. Rust/Swift builds were not run under the read-only constraint. The working tree is unchanged.
