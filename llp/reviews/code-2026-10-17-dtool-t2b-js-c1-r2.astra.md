# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `244e62f8d`.
- **Method:** one brief (sha256 `3fc662431c107d854b2e38e5632dc46f3e33fcddc8d59f873e7e55a58ce428e4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken. A plan published by a JS reload may leave a native on the resident producer on its plan, taking the design as a batch. So `designServed` is that build's design and the source its own map records (`built.designSource`), not null. A publish that lands while a later edit was requested no longer clears it either. Not taken: a regression test with a native build kept on its plan; the gate's natives on the JS loop cover it.

---

**Medium — A JS reload clears certification for native builds that take the design live.** [host/web-js/dev.mjs:374](/tmp/rv-dt-js1r2/host/web-js/dev.mjs:374)

Change a token from `AccentColor` to a literal colour. The JS target reloads because its accent companion rules change, while the resident/native plan can remain unchanged and receive the batch sent at line 384. That native build now reports the current design, but its map records the previous `.tamber` bytes. Clearing `served` makes [designMoved](/tmp/rv-dt-js1r2/scripts/tamber-inspect.mjs:98) reject it as stale, leaving valid selections read-only until another kept/value-only build restores certification.

The same clearing at [line 330](/tmp/rv-dt-js1r2/host/web-js/dev.mjs:330) discards the certificate for the newer snapshot explicitly retained when an overtaken build publishes. Unlike the wasm loop, publishing a JS plan does not necessarily replace every build’s live design.

**Fix:** retain the compiled source hash alongside the committed snapshot. Certify the snapshot broadcast to natives using `built.designSource`, and preserve the newer snapshot’s certificate in the overtaken-publication branch. Continue returning null during previews. Add a regression covering a JS reload with a native build retaining its plan.

The other round 1 fixes are correct:

- Fast edits hash the exact compiler input; kept builds obtain the hash from the candidate’s map before discarding it.
- Failed startup now caches the preserved `dist` plan/map pair.
- The test observes registration while Chrome remains open and checks the map’s digest. It meaningfully tests initial reporting, but not retention, lookup or `designServed`; those tests remain deferred.

`exact.plan`, the copied `app.plan`, and the map digest use identical bytes. Map retention covers initial, failed and published builds, including overtaken publications; discarded candidates and scrub compilations do not replace the running plan’s map. The no-digest fallback preserves native behavior. I found no request-writable map source or additional map-publication ordering defect.

Read-only review; no builds or tests run.

**Verdict: Request changes for the remaining `designServed` reload case.**
