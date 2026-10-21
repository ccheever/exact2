# Code review: the JS loop's design reader (value-only edits without a build), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `f96c979e88d9019702a0252abdf3a489647e33560fa67826d2d491981ec6917a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

No P0. Two P1s are real disagreements with the slow path. Shape, accent, and a compiler refusal still fall through to the build, and the scrub updates match the `keep` branch.

### P1 — a value the plan structure keeps is sent as a batch

`sameShape` (`host/web-js/dev.mjs:144`) is shape, token names, and accent. The `keep` branch (`dev.mjs:190`) also requires `next.structure === design.structure`. That digest is `Plan::structure`, which still contains a theme or slice value whose text contains `(--` (`plan/build.rs:792`, profile colours and the same substring in a slice).

Change `wide = color(--display-p3 0.2 0.3 0.4)` to another `color(--display-p3 …)`. Shape, names, and accent stay put, so the fast path (`dev.mjs:171`) sends a batch and leaves `dist`. `keep` fails, the slow path reloads onto a new build. The page in `dist` keeps the boot-time profile colour. A connected page applies the live batch. A page that loads later gets the snapshot, not the rebuilt CSS.

A normal colour in a file that also has a profile colour still matches `keep`: the structure bytes change only when that `(--` value itself changes.

### P1 — a source edit already on disk is dropped after the fast path stamps `since`

`rebuild` sets `since` before it decides (`dev.mjs:154`). The watcher then ignores any fresh path whose mtime is older than that (`dev.mjs:256`). The slow path still compiles whatever is on disk. The fast path reads only the app design text and returns (`dev.mjs:184`).

Save `app.contract`, then save `app.tamber` before the contract event is delivered. The design edit takes the fast path and stamps `since`. The contract event arrives with an older mtime and is ignored. `dist` and the open page stay on the old program. Only the colour moves. A delivery that lands during the design read with a newer mtime is queued (`again`) and does build.

### P2 — the host-dev cargo child outlives the loop

`stop` kills the app build and the resident child (`dev.mjs:495`) and then `process.exit(0)`. It does not kill `tool.child`. `watchLauncher` calls `stop` when the launcher is gone, including after a SIGKILL, which forwards nothing (`host/web/serve.mjs:916`). SIGTERM and SIGHUP take the same path. The `cargo build --profile host-dev -p exact-web-js` started at `dev.mjs:128` keeps writing `target/host-dev`. Ctrl-C is different: the terminal signals the whole group.

`buildTool` does kill and replace (`dev.mjs:127`). It does not wait for the old cargo to exit. The exit handler ignores that child (`dev.mjs:129`), and `ready` stays false until the replacement exits 0, so a design read will not exec a half-written binary.

### P2 — a throw or a failed spawn sticks `designing`

`designing` is set around the read (`dev.mjs:167`) with no `finally`. `readFileSync` sits in that window (`dev.mjs:169`). If it throws, `rebuild()` rejects with no catcher (`dev.mjs:264`) and `designing` stays true. Later edits hit `dev.mjs:153`, set `again`, and return. The loop builds nothing until restart.

`designOf` (`dev.mjs:132`) and the cargo child have no `error` listener. A spawn failure emits `error` and does not settle the promise.

### P3 — a failed tool build stays silent

Cargo is spawned with `stdio: 'ignore'` (`dev.mjs:128`). A non-zero exit leaves `ready` false and prints nothing. Every later design edit takes the full build until some non-design edit calls `buildTool` again.

### Checked, and they match the slow path

- **Shape, accent, refusal.** A new token, a token that starts varying (slice count is in `design::shape`), or an accent flag change fails `sameShape` and builds. `designOf` returning `{error}` falls through at `dev.mjs:171` and the build surfaces the refusal.
- **Another `.tamber`.** The compiler reads only the app design (`contract/cli/src/sources.rs:137`). A sibling `brand.tamber` takes the fast path, finds no token change, and sends nothing. `keep` would also keep the page and delete the stage, so `dist` stays either way.
- **Scrub.** The fast path copies the `keep` updates (`dev.mjs:172`). A preview stays up and the committed snapshot is stored in `aside`. A released scrub ends, and the batch’s `base` is the preview’s digest. A page that connects during a preview gets that snapshot. A page that connects after the fast path gets `notice(snapshot)` and, if its baked digest differs, fetches `/__dev/design`.
- **Digests, when the plans match.** `base` is `from.digest` (`scrubbed` or the previous design), `design` is the new digest, and the wire `structure` is `shape`. Same as `dev.mjs:206` and `:213`.
- **Same `modes::every`.** Both the design command (`host/web-js/src/main.rs:53`) and the dev build (`main.rs:266`) call `modes::every(true)` immediately before `modes::design`. The compiles are not the same call: the tool uses `compile_path_all(path, false)` (`main.rs:51`); the build uses `compile_path_all_with_warnings(path, sites)` and then `contract::check` (`main.rs:216`, `:223`). Surfaces are on in both. `mapped` adds the source map. I did not find a colour edit that passes the tamber checker and fails `contract::check`.
- **Tool races that stay on the slow path.** `ready` is cleared before the new cargo starts (`dev.mjs:126`). Until that build exits 0, a design edit takes the full build, including the first edits after startup. A non-design edit does the same. The binary marked ready is the one cargo just finished, not a half-written file.
