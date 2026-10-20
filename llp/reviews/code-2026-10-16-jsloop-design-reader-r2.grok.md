# Code review: the JS loop's design reader (value-only edits without a build), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the change and the first round.
- **Method:** one brief (sha256 `6200052f6c6b29100d87b1e685c8cc00c7c8fbaa35ee07d5098f322435da8474`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

Nothing blocking. Round 2's listed fixes hold: structure (profile colours), only the app's own design file, depfile freshness, `since` only when a full build starts, and a thrown read or a failed spawn no longer leaves `designing` set. Scrub, release, and the later page match the keep branch.

The fast path at `host/web-js/dev.mjs:187-206` follows the keep branch at `226-241`. Batch `base` is the digest pages are on (`scrubbed ?? design`), `design` is the new digest, and the wire `structure` is `design.shape`. While a scrub is only previewing, the file edit updates `aside` and does not touch `snapshot` or the pages. A release clears the scrub and sends from the preview to the written design. Dist is left in place. A page that loads later gets `notice(snapshot)` (`413`), goes stale, and fetches `/__dev/design`. Shape, accent, a compiler refusal, and any `.tamber` that is not `designPath(app.dir)` fall through to `build()`. A profile colour changes `Plan::structure`, so `sameShape` (`163-164`) refuses the fast path.

### P2 — `host/web-js/dev.mjs:192`
A repaired value-only edit sets `error = null` and sends the batch (`199-203`) without `push({ error: null })`. The page clears the overlay only when `m.error` is present (`336`).

A bad token takes the full build and the overlay appears. Fixing that token to a same-shape value applies the colour and leaves the refusal on screen until it is clicked. A new connection does not see it, because the hello omits a null error (`411`).

The same assignment also drops a failure this read did not re-check. A design-only full build that failed for another reason (the build that just ran `contract::check`, a cargo failure) is forgotten on the next successful colour tweak. The keep branch would have built again and pushed the error again.

### P2 — `host/web-js/dev.mjs:191-206` vs `213-215`
The keep branch reloads unless `logic === logicRevision` (the Rust wasm and TypeScript chunk hashes in `.exact-dev-logic.json`). The fast path never looks at that.

If the app's own design file is also a module input (`include_str!`, or imported by `app.ts`), a colour edit updates live CSS and returns. Dist keeps the old module. The keep branch would have seen the hash change and reloaded. Round 2 closed this for every other `.tamber` (`187`) and left it open for the one file the fast path reads.

### P3 — `host/web-js/dev.mjs:127`, `148-159`, `517-524`
`buildTool` signals only the cargo pid (`kill()`, default SIGTERM), then starts another cargo on the same target directory. `stop` does the same with SIGTERM and then `process.exit(0)` without waiting. The installer child just below is waited on; this one is not. rustc is not signaled by this code. A `design` process from `designOf` is not stored, so stop never signals it.

Ctrl-c during the host-dev link can leave cargo/rustc holding `target/.cargo-lock` after the loop is gone. The next loop's reader build waits on that lock. A design read in flight outlives the loop.

`buildTool`'s `error` handler (`131`) clears `tool.child` and does not log. Cargo missing from `PATH` leaves design edits on the full build with no message. A non-zero exit does log (`136`).

`designOf` parses stdout on `exit` (`157`), which can run before the last chunk. Truncated JSON becomes `{ error }` and the edit takes a full build. It does not send a partial batch.

Every non-design save calls `buildTool` (`210`), which sets `ready = false` and kills the in-flight reader build. Saves closer together than an incremental `host-dev` check keep the reader unready, so colour edits keep taking the full build until there is a quiet gap. That matches the commit ("any edit that is not a design's"), and it is why the fast path stays dark during ordinary source editing.

### Races and the compiler
An edit during the read sets `again` (`172`) and is rebuilt after the in-flight read. A full build in flight does the same via `building`. Before `tool.ready`, `toolFresh` (`141-146`) is false and the edit takes `build()`. `ready` is cleared before cargo is spawned, and `designOf` snapshots `tool.ready` in the same turn as `spawn`, so this loop does not exec a half-written binary. A scrub during a reader rebuild sees `ready === false` and uses `webCompiler()` (debug). After a compiler edit under `contract/` or `kernel/`, the depfile check fails, the tool is rebuilt, and that edit takes the full build.

The tool does not disagree with a full build on a value-only edit both accept. Both call `modes::design` under `modes::every(true)` (`main.rs:52-54` and `265-269`). `--sites` only adds the source map. They are not the same binary or the same entry: the fast path is `host-dev` `exact-web-js design` (`compile_path_all`, no `contract::check`); the build is `webCompiler()` debug `js --dev-reload`, which then runs `contract::check` (`main.rs:223`). The comment at `dev.mjs:121-123` that this is the compiler the builds use is wrong. Pure colour values are exactly what `Plan::structure` leaves out, and `check`'s boot checks are on that structure, so a same-shape colour edit should not flip `check`. The missing `logic` hash above is the real case where the two paths publish different programs.
