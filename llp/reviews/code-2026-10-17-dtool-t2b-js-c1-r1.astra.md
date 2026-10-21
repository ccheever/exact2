# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `358c51bdb`.
- **Method:** one brief (sha256 `9a537e85e39b420ca91dbcde92db7f59c9f043cf343cd7cccfb2b440ceee32f2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium).
- **Disposition:** taken.
  - `designServed` is the wasm loop's now. It is the design the pages run after value-only edits since the plan they booted, or null for the plan's own design, which the join checks against the map. A plan publish clears it. Its source is the SHA-256 of the bytes compiled: the text a value-only edit passed to `designOf`, or, for a kept build, the design file's digest as the candidate's own map records it (`mapDesignSource`, read before the stage goes). The file is never hashed again afterwards.
  - The served page's map is kept at start whether the first build passed or not. With a failed build, dist still serves the last good page and its map.
  - The test reads the page's `planDigest` while the drive has it open (a poll beside the drive), and checks that the map names the same digest.
  - Not taken: a loop-level `designServed` test. Editability on the JS loop is step 4's test, end to end.

---

1. **Medium — The recorded design hash can certify bytes the page never took.** [host/web-js/dev.mjs:272](/tmp/rv-dt-js1/host/web-js/dev.mjs:272), also lines 129, 342 and 361. If the compiler reads design A and the user saves B before the asynchronous operation finishes, `fileDigest()` records B alongside A’s design digest. [The join](/tmp/rv-dt-js1/scripts/tamber-inspect.mjs:98) then treats a page running A as editable against B. This can persist if B’s subsequent build fails. **Fix:** hash the exact text passed to `designOf`; for full builds, return the design source hash from the candidate’s compiler-generated map, including before discarding kept candidates.

2. **Medium — A failed initial build leaves a served plan without its map.** [host/web-js/dev.mjs:124](/tmp/rv-dt-js1/host/web-js/dev.mjs:124). Restart with an existing successful `dist`, but introduce an `app.ts` compile error. The failed build preserves `dist`, and the HTTP handler still serves its page, which reports its old plan digest. However, `pageMaps` stays empty because initialization requires `!error`. That plan unnecessarily loses source lookup and editability despite its matching map remaining on disk. **Fix:** seed the verified plan/map pair from the artifact actually being served, independently of whether the initial build attempt succeeded.

3. **Medium — The new assertion queries the registration after closing its browser.** [scripts/design-tool.test.mjs:771](/tmp/rv-dt-js1/scripts/design-tool.test.mjs:771). `drive()` waits for the agent process to close; the agent’s `finally` closes Chrome. Closing the page removes its stream, and `hello` lists only connected builds. Consequently, `listed.find(...)` becomes undefined and the assertion throws—or depends on delayed disconnect cleanup. **Fix:** fetch and assert while the drive’s browser remains open. Even then, this assertion checks digest reporting only; it does not exercise map retention, lookup, or `designServed`.

The digest computation itself is consistent: `exact.plan` hashes the compiler’s encoded `app.plan`, the build copies those bytes into `dist`, and `map.json` hashes those same bytes. Kept/value-only builds correctly retain the running plan’s map; subsequent failed builds preserve it; published builds overtaken by another request still call `keepPageMap`. Scrub compilation does not replace that artifact.

The no-digest fallback preserves the resident/native meaning; digest-less selections remain read-only. Returning null during a scrub matches wasm’s behavior: it removes the live-design certification, rather than universally disabling editing. I found no request path that writes the map location—checkpoints stay in memory and uploaded traces go elsewhere.

Read-only review; builds and tests were not run.

**Verdict: Request changes for the design-source race, failed-startup map omission, and assertion lifetime.**
