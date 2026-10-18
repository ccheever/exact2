# Code review: the JS loop's design reader (value-only edits without a build), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the whole range.
- **Method:** one brief (sha256 `c18a656b414f0ab7292b0af3c628a6c4c015ed9d529c948ab8098459a6573792`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** applied without re-review (the last round).

---

No P0. The value-only colour path matches the slow path when the reader and the check build agree. Three defects are worth fixing.

[P1] A repaired design never clears the open page's error overlay — `host/web-js/dev.mjs:206`

A value-only fix sets `error = null` before `hadError` is captured at line 230, and it never pushes `{error: null}`. The clear at line 251 then sees `hadError === null`, so it does not run. The batch handler returns before the overlay logic. Fix a refused design to a new colour: the open page applies the colour and keeps the old error overlay until a refresh. A page that connects in that window sees no error, because the server copy is already null.

[P2] An overtaken check build still publishes `dist` and does not reload — `host/web-js/dev.mjs:76`

`replaceBuild` runs inside `build()` before the caller can discard. Line 238 returns without a reload whenever a later value-only edit bumped `fastSeq`. Change a colour that is a module input (so the check build's logic hash changes and `keep` is false), then change it again while that build runs: `dist` becomes the first check's program, open pages stay on the old one, and a page that loads in the gap gets the new one. It self-heals only when edits pause long enough for a check to finish.

[P2] Stopping the loop during a design read starts a build that nothing kills — `host/web-js/dev.mjs:549`

`stop` kills `tool.reading` and then awaits. That close resolves the read with a failure, and `rebuild` falls through to `build()` at line 234. `building?.kill` has already run, so the new `build.mjs` is reparented on `process.exit` and can still `replaceBuild` into `dist`.

[P3] A fallback `cargo run` design read is not killed as a group — `host/web-js/dev.mjs:156`

The host-dev tool cargo is detached and stopped with `stopGroup`. The read itself is only `tool.reading?.kill()`. After a compiler edit, before either binary is fresh, a scrub takes `webCompiler()`'s `cargo run`. Ctrl-c kills `cargo` and leaves `rustc` holding the target lock.

What holds: shape, accent, a refusal, and a `.tamber` that is not the app's all miss `sameShape` or `fast` and take the full build. Batch `base`/`design` are the previous and next `digest`s, same as `keep`. While a scrub previews, the fast path writes `aside` and does not touch the live snapshot; a release clears the scrub and sends from the preview's digest. `modes::every(true)` then `modes::design` is what both the `design` command and `dev-design.json` use, so a hex or `light-dark` value matches what a keeping build would send. `contract::check` and module inputs are only on the check build, which is what round 2 added. The tool cargo is dated to the build's start, replaced on the next non-design edit, and not used until that cargo exits, so a half-written binary and a compiler source saved mid-build do not take the fast path. Edits before `tool.ready` take the full build.
