# Code review: the JS dev loop's builds set aside and stopped, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `701e2313f` (a quick round on the round 2 fixes).
- **Method:** one brief (sha256 `b41104a264eafb187d921b14872ddc8f720779fda27d233f7c366b8e26603e9e`), shared by the reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (1 blocking: the cap switched the checks off; absent candidates, the design's default path, a masked change time, the kill timer).
- **Disposition:** fixed in `40dc70a19` (see its message).

---

1. **blocking — [host/web-js/dev.mjs:513](/private/tmp/rv-jv-3/host/web-js/dev.mjs:513), also line 178.** The retry cap disables all supersession checks. A fourth build can overwrite a newer live batch with older design/snapshot/dist state. Startup’s third attempt can permanently lose edits already consumed by its temporary watcher. **Fix:** keep publication checks unconditional; bound aborts, distinguish generated writes, and preserve startup edits.

2. **should-fix — [host/web-js/dev.mjs:114](/private/tmp/rv-jv-3/host/web-js/dev.mjs:114).** `packages.sources` includes deliberately absent resolution candidates—for example, nearer `node_modules/ui/package.json` paths when an ancestor supplies `ui`. Treating every missing path as changed causes three startup attempts and four builds per edit without further changes. **Fix:** distinguish actual deletions from paths absent when consulted.

3. **should-fix — [host/web-js/dev.mjs:82](/private/tmp/rv-jv-3/host/web-js/dev.mjs:82).** `designPath()` returns null when absent, so creating `app.tamber` during a build escapes the final filesystem check if its watcher event trails publication. The parent caught this case. **Fix:** track the configured/default design path even when absent.

4. **should-fix — [host/web-js/dev.mjs:113](/private/tmp/rv-jv-3/host/web-js/dev.mjs:113).** A preserved future `mtime` masks a current `ctime` through `Math.max`. Restoring edited content with that timestamp during a build can therefore publish stale output before its watcher fires. **Fix:** check each timestamp independently, or compare captured contents.

5. **should-fix — [host/web-js/dev.mjs:41](/private/tmp/rv-jv-3/host/web-js/dev.mjs:41).** The escalation timer survives confirmation that the original group exited. If its PGID is reused within five seconds, it kills an unrelated group. **Fix:** cancel the build’s escalation timer when `groupGone` confirms exit.

In-memory checks confirmed these code paths; integration tests weren’t run. The added tests miss these cases, particularly the retry cap and delayed watcher events.
