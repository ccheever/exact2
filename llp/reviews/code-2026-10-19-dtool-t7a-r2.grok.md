# Code review: LLP 1122 T7a (navigate, a status's location, framed copies that say their build; tamber/1.14), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8da06494ec`.
- **Method:** one brief (sha256 `ba3577e2a000e7b09a3317af89ed81f04576a775b2fffd64e3911841d6b5d19f`), the design inline, shared by both reviewers, each blind to the other's review; commits 7465598517, 3e2edd5186 and 8da06494ec, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (after the CLI's own compaction notice).
- **Verdict:** nothing blocks (1 SHOULD-FIX, 3 NIT).
- **Disposition:** taken in the round's fix commit (named in round 3's records):
  - The wait for the router's history write stops once the router's entry is another route (a handler's redirect is committed at once), so nothing waits a second behind it (1).
  - Arrival compares the router's spelling, decoded (2).
  - The doc says `n` is one or two digits, as both pages read it; the tool names 1 to 8 (3).
  - Scheduler test (c) parks a navigation behind a pin and counts routes (4).
- **Not taken:** an agent-keyed copy in the Chrome test. The arrival check reads history.state.url, which has no agent keys, and the copies the tool frames carry none. The page's `//` refusal is on the same `URL` resolution the server's test holds (4).

Nothing blocks. Round 1’s blocker and the taken should-fixes are in the tree, and a location the server accepts stays on the page’s origin. Node’s URL parser, over the printable ASCII locations `LOCATION` allows plus the three dot-segment strings, found no accepted location that changes origin or makes `new URL` throw. The test suites were not run.

## Round 1 dispositions

Astra, all five taken items hold:

1. **Stream binding is fixed on both pages.** The nonce is stamped on the frame at receive, before the queue: `host/web/dev.js:920` then `command` at `:879`, and `host/web-js/dev.mjs:714` then `nav` on `liveQ` at `:712`. `mine()` is checked before any routing and again inside the refusal (`dev.js:474-476`, `dev.mjs:712`). `report` / `rep` capture the nonce at call time and drop the post if that registration was replaced (`dev.js:395-402`, `dev.mjs:706`). A navigate sitting behind a wasm hold (the shared `designing` chain) or a JS pin or design batch keeps the nonce it arrived with. `dev-scheduler` (c) covers the wasm reconnect-before-run race only; see finding 3.
2. **The fragment no longer overwrites a redirect.** It is applied only when `there()` is true (`dev.js:495`, `dev.mjs:712`). A handler that commits another route leaves that route, with no fragment. `scripts/dev-copies.test.mjs:145-147` waits until the JS copy’s status is `/post/9` with heading Post 9. The final place is right; the wait in front of it is finding 1.
3. **Detail bounds hold.** `LOCATION` is printable ASCII, at most 2048 (`scripts/tamber-builds.mjs:64-67`). The page slices the detail to 1024 and cuts the location in that text at 200 (`dev.js:476-477`, `dev.mjs:712`, `host/web-js/router.js:199`). A 2048-character path of `^` still passes `located` and expands to a 6142-character pathname, and the refusal text stays under the server’s 4096 (`tamber-builds.mjs:128`).
4. **Presence is held open** for the whole copies test (`scripts/dev-copies.test.mjs:84-85`, aborted at `:155`).
5. **The JS fixture now has an authored handler, a redirect, a link, and Back.** The wasm fixture reports a hand-set fragment through `hashchange` and refuses a path. A routed wasm fixture was not added, matching the recorded disposition.

Grok:

1. **Agent launch keys no longer defeat arrival or get stripped.** `there()` is `history.state.url === to` or the address (`dev.js:484`, `dev.mjs:712`). `history.state.url` is the route (`host/web/navigation.js:139`); `historyURL` puts `agent`, `seed`, `locale`, `timeZone`, `epoch`, `storage`, and `failFetch` on the address only (`navigation.js:129-137`). The fragment step rewrites `location.origin + location.pathname + location.search + hash`, so those keys stay, and it does not run when the handler redirected. No copies test sends an agent query.
2. **A `//` pathname is refused before history.** The server’s `located` and both pages reject `pathname.startsWith("//")` (`tamber-builds.mjs:67`, `dev.js:481`, `dev.mjs:712`). The three strings from round 1 are in `scripts/tamber-builds.test.mjs:715`. The fragment URL is prefixed with the page origin. A history throw is inside the `try` and answered `unsupported`.
3. **A stale refusal is not journaled.** The seq check returns 204 before the log (`tamber-builds.mjs:442-446`). The builds test’s log is a no-op, so that line is unasserted.
4. **Same frame name sharing an id** — not taken, and unchanged. The tool’s names are unique, and the key is limited to those names.
5. **Bidi is gone** because `LOCATION` is ASCII. **`here()` returning null over 2 KiB** — not taken, and unchanged (`dev.js:458`, `dev.mjs:707`).
6. **“this page has no router”** — not taken. `host/web-js/build.mjs` still installs `goTo` on a dev build.

`exact-copy-<n>` is the same rule on both pages (`dev.js:367`, `dev.mjs:706`): framed, and the name matches, then the key is `exact.dev.build:<name>`; the name is not written and is not in the `{type:'exact-build', id}` message (`dev.js:934`, `dev.mjs:714`). The copies test shows three ids, a lone reload of `exact-copy-1` keeping its id, and those storage keys (`scripts/dev-copies.test.mjs:107-113`).

## Findings

1. **SHOULD-FIX** — A redirect that has already committed still burns the full second, and on the wasm page the leader shows the old location until that second ends. `host/web/dev.js:494` and `host/web-js/dev.mjs:712`.
   Send `/post/0#x` to the copies fixture, whose handler commits `/post/9`. `exact.navigate` applies the batch before it returns (`host/web/glue.js:1376-1380`); the router op calls `navigation.apply`, which `drain`s into `pushState` synchronously (`host/web/navigation.js:340-347`, `:214-241`). The JS `goTo` returns only after the same commit (`host/web-js/router.js:198-203`). `there()` is looking for `/post/0`, which will not appear, so the loop sleeps 100×10 ms anyway. On the wasm page the history hook queues the status on `designing` (`dev.js:442-451`), behind this navigate, so the status that says `/post/9` is posted when the wait ends, and a pin or another navigate on that page waits with it. On the JS page the history hook posts `/post/9` on the next turn (`dev.mjs:711`, not on `liveQ`), and `liveQ` still holds the next command for the rest of the second. The copies test polls for up to 120 s (`dev-copies.test.mjs:145-147`), so it passes either way. Stop the wait once `history.state.url` is already some other route; keep it for a commit whose history write is still queued behind an in-flight `history.go` (`navigation.js:283`).

2. **NIT** — A location containing `|` never counts as arrived, so its fragment is dropped. `host/web-js/router.js:31` and `dev.js:484`.
   `located("/foo|bar")` is true. The URL parser’s pathname is `/foo|bar`; `canonical` encodes the pipe, so the router stores `/foo%7Cbar`, and the address becomes `/foo%7Cbar` too. `there()` compares both with `/foo|bar` and stays false for the whole second. On a one-segment route the page does move, the status says `/foo%7Cbar`, and `#c` is never set. The same character class is the pipe alone among printable ASCII; design-tool paths do not use it. Treat the router’s canonical spelling as arrival, or pass that spelling as `to`.

3. **NIT** — The per-frame key stops at two digits, so `exact-copy-100` shares the tab id. `host/web/dev.js:367`, `host/web-js/dev.mjs:706`, `docs/tamber-protocol.md:1075`.
   The protocol says `exact-copy-<n>`. The regex is `^exact-copy-\d{1,2}$` on both pages. `exact-copy-0` through `exact-copy-99` get their own key; `exact-copy-100` falls through to `exact.dev.build`, the key the app-owned frame uses. The copies test names 1, 2, and 3.

4. **NIT** — The new tests prove the assertions they write, and they leave the binding-behind-a-hold path, the JS page’s binding, agent keys, and the page’s `//` refusal unmeasured.
   `host/web/tests/dev-scheduler.test.mjs:302-312` sends seq 6 and a new hello in one turn, on the wasm page, with no command already in `designing`. That fails if the nonce is read when the command runs, which is the reconnect-before-run bug. It does not park a navigate behind a hold, and the JS page is a different script. `scripts/dev-copies.test.mjs` never loads `?agent=1` and never sends `/foo/..//evil.example/x`. The server refusal for those strings is in `scripts/tamber-builds.test.mjs:715`; `scripts/tamber-pairing-server.test.mjs:383` does not include them. `free()` (`dev-copies.test.mjs:49`) binds port 0, closes, and lets the dev loop bind that port again. The parent reads the frame name from `event.source` (`dev-copies.test.mjs:70`), so a payload that also carried the name would still pass.
