# Code review: LLP 1122 T1 commit 2 (the design tool's skeleton and its connection), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a44b4e3a5` (commit `0beef42fd` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `7ffbbbea1ef3129db43e3f6b5c6db6b4d266675de45f032ab18940edd1e62c0c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium).
- **Disposition:** both taken, as in the astra record: reuse compares `app.js` and `app.plan` as well as the page; signal handlers installed before `await tamber.ready` close the design server and exit 130, 143 or 129, and come off before `listen` adds its own.

---

Rounds 1 and 2 hold on the paths they named: the token stays on loopback headers, `--via` is address-checked and fail-closed, the long poll backs off instead of spinning, `missed` clears the list, and a down server returns buffered events once before 503. Two problems remain.

## Medium

**1. Reuse still treats a matching `index.html` as this checkout's tool.** `scripts/tamber-server.mjs:736`

After the design directory matches, reuse accepts the listener when the served `index.html` equals this checkout's file (base href rewritten the same way on both sides). The JS-target page (`host/web-js/build.mjs:536`) inlines CSS and then loads `<script type="module" src="./app.js"></script>` with no content hash, so `app.js` and `app.plan` are outside the comparison. `.exact-build.json`, which does hash those files, cannot be fetched: `serveTool` rejects a dot segment (`scripts/tamber-server.mjs:107`).

Checkout A is already serving `tamber tool /shared/app`. Checkout B, with a newer `apps/design-tool/app.ts` or a contract change that leaves the emitted CSS alone, builds into its own `target/design-tool-web`. The hello app path matches, `index.html` is byte-identical, and B prints the URL and exits 0. The browser keeps running A's `app.js` and `app.plan`. A TypeScript-only fix of this protocol client is the diff the check misses. An unbuilt tool is refused, because that 404 body is not the HTML.

Fetch `app.js` and `app.plan` the same way and require those bytes to match the build just validated, together with `index.html`. Keep the current "another build" refusal when any of them differ.

**2. Ctrl-C during the cold start does not stop the design server.** `scripts/tamber-server.mjs:764`

`toolCommand` awaits `tamber.ready` before `listen()`. That ready is the first hello, and the ask has no timeout because a cold `cargo run` can take minutes (`scripts/tamber-server.mjs:237`). `SIGINT`, `SIGTERM`, and `SIGHUP` are installed only inside `listen` (`scripts/tamber-server.mjs:657`), which is also what binds port 7740. `spawnContract` starts cargo detached (`scripts/tamber-server.mjs:132`), outside the terminal's foreground process group. `close()` (`scripts/tamber-server.mjs:626`) is what signals that process group, and this path never calls it.

Ctrl-C while the compiler is still building kills `bun exact.mjs` and leaves cargo running. Help says the tool runs until ctrl-c (`scripts/help.mjs:295`). A second `tamber tool` starts another compile against the same project. When the orphan finishes, `Server::start` takes `.exact/tamber.lock` (`contract/cli/src/tamber_serve.rs:285`) and only then sees stdin EOF and exits (`tamber_serve.rs:86`), so the new command can fail with "another tamber server" for a process the terminal can no longer signal. The port stays unbound through this window, and a server that is already down still returns 1 without listening.

Install the three handlers before `await tamber.ready`, have them call `tamber.close()` and exit 130, 143, or 129, and bind the port only after ready succeeds. Remove those listeners before `listen` adds its own, so ctrl-c once the tool is up still stops the HTTP server and exits 0.

Verdict: Request changes — the stale-tool check and the cold-start signal gap are still open.
