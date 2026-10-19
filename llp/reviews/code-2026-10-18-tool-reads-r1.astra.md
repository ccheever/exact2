# Code review: the design tool's reads during a scrub, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the first commit (keys on files and writes).
- **Method:** one brief (sha256 `73cced417335e3406ab8857c24295444c4c3f7322bb36dc55c4a17893638b859`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (uses misses packages; live can flip with no file change; a failed uses read never retried).
- **Disposition:** round 1's commit (a plans key, a uses retry).

---

Nothing is blocking. I found three should-fix issues:

1. **Should-fix — [apps/design-tool/app.contract:547](/tmp/rv-dq-1/apps/design-tool/app.contract:547): package edits leave token uses stale.**  
   `files` includes the app’s own Contract sources but excludes imported packages; `uses` compiles the entire imported graph. Editing a linked package to add/remove a token use can rebuild the running page without changing `changes.files`. The displayed count and source locations then remain outdated. **Fix:** invalidate uses with a revision covering all compilation inputs, including packages; add a package-only edit regression.

2. **Should-fix — [apps/design-tool/app.contract:692](/tmp/rv-dq-1/apps/design-tool/app.contract:692): `hello.live` can change without a file-key change.**  
   The wasm loop recalculates TypeScript presence and Rust policy after rebuilding, and its `live()` depends on those values ([host/web/dev.mjs:1251](/tmp/rv-dq-1/host/web/dev.mjs:1251)). A manifest-only change can therefore flip `live` while Contract/design digests remain unchanged. The tool retains the old value, mislabels proposals and incorrectly enables/disables scrubbing. **Fix:** refresh hello on capability changes or structural build reloads, without refreshing on every status frame.

3. **Should-fix — [apps/design-tool/app.contract:547](/tmp/rv-dq-1/apps/design-tool/app.contract:547): failed uses reads no longer recover automatically.**  
   If `loadUses` times out once, subsequent successful polls with unchanged files never retry it. `advance` retries failed hello/design reads, but not uses; selecting the same token also leaves its arguments unchanged. **Fix:** retry failed uses reads with backoff, or provide an explicit retry action, while retaining file-based invalidation for successful reads.

The ordering, counter behavior across undo/gaps/restarts, other-client commit invalidation, and `scrubShown` path look sound. Builds update through events; the current JS loop’s `hello.page` is a fixed origin, so ordinary rebuilds introduce no separate page/frame issue.

Read-only mocked-fetch checks passed. Full suites were not run because they create/build fixtures; existing metadata tests reload the tool and do not cover the live-change case above.
