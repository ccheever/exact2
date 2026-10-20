# Review round 1, LLP 1121 stage 8 E1 (held states), Grok 4.7 (xhigh)

I'll review HEAD (`5e829a92b`, held states) read-only against the design and the ruling conditions, starting with the commit diff and the cited specs.The server diff is in. Next I’ll read the hold state machine, both web targets, and the tests against the 07:48 ruling.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open on one point: whether the JS target writes `data-exact-on` so an `active` hold can find the press node. I'll confirm that, then lock the verdict against the server replay path and the held-row join.**WRONG.** The hold model matches the ruling on the server in isolation (one hold, write scope, presence, instance path kept, single-site correspondence, attribute looks, `showPress`). On the wasm dev loop that correspondence never runs, and the JS target never reports the rows the design tool joins.

**1. BLOCKING — An in-place plan rebuild deletes the hold as `gone` before replay.** `host/web/glue.js:1354`, `host/web/input-glue.js:334`, `host/web/dev.js:700`, `scripts/tamber-builds.mjs:349`

After the checkpoint document releases, `page.holding` is false, so a new generation calls `root.replaceChildren()`. The hold observer is a mutation microtask queued at that removal, and it runs before `applied` → `status()`. `letGo("gone")` posts `{kind:'held', state:null, reason:'gone', revision}` through the FIFO report queue. `status()` awaits `facts()` before it posts the new `planDigest`, so the server handles `gone` first. At `scripts/tamber-builds.mjs:357` a `gone` at the live revision does `holds.delete`. The later digest change calls `replayHold`, which returns immediately because the hold is already gone. Site correspondence, the kept instance path, and `hold-cleared` never run. That is the set-state rebuild path §10 and D4 describe as replay on a `planDigest` change. `scripts/tamber-builds.test.mjs:564` posts the digest change with no preceding `gone`, so it stays green.

Fix: while `reloadGeneration` tears the tree down, do not report `gone` (skip the observer callback, and have an in-flight `hold()` that outlives the generation drop its report). Let the following status replay or clear. A same-plan unmount that is not a generation swap must still report `gone`. Add a host test that a generation swap does not post `gone` ahead of the new digest.

**2. BLOCKING — The JS target never sends held rows.** `host/web-js/agent.js:167`, `host/web-js/inspect-dev.js:265`

§10 requires `layout`'s node and an inspect selection to carry `held {state, rows}` on both webs. The wasm page does: `host/web/glue.js:1017` assigns `exact.heldOf`, and `host/web/dev.js:504` puts it on the selection. The JS `nodeDetail` return and `selectedReport` omit it. `exact.heldOf` lives in the shared input glue, and `inspect-dev.js` / `agent.js` can apply a hold, but the selection the server joins and `layout <id>` never include `{state, rows}`. Active rows therefore never show up as the lowered D7 names on a JS build.

Fix: on the layout node, `Object.assign(node, globalThis.exact.heldOf?.(el))`. On the selection, `held: globalThis.exact.heldOf?.(el)?.held ?? null`, as in `dev.js`.

**3. SHOULD-FIX — A late or revision-less `held` report rewrites a hold that is already over.** `scripts/tamber-builds.mjs:117`, `scripts/tamber-builds.mjs:357`

`revision` is optional (`maybe`). The stale check requires both a number and a live hold. Two results: `{state:null, reason:'gone'}` with no revision deletes whatever hold is live (`!Number.isSafeInteger(r.revision)` satisfies the delete), and a late `{state:'active', revision:<old>}` after `clear` or a failed correspond (`h` is null) sets `b.held` back to active and broadcasts `held` to every client. The test at `scripts/tamber-builds.test.mjs:581` only rejects `state:'pressed'`.

Fix: require `revision` on `held`. Remember the revision last sent for that build. Apply a report only when it equals the live hold's revision. When the server itself clears (client let-go, client disconnect, no corresponding site), set `b.held` and emit `held` there, so a late host echo cannot republish the state.

**4. SHOULD-FIX — Replay reuses the revision, and a failed correspond never tells the host.** `scripts/tamber-builds.mjs:183`

On success, `replayHold` frames the same `h.revision`. The host's `exact.hold` calls `letGo("replaced")` first, and that callback reports `replaced` at that same revision. The server treats `revision < h.revision` as stale, so an equal revision is applied: every client is told the hold dropped (`replaced` does not delete it), then the following apply report puts it back. A lost apply report leaves the lie. On failure the server deletes the hold and sends `hold-cleared`, and returns without `frame({kind:'hold', hold:null})`. A polled queue only collapses when a new hold frame arrives, so the previous hold frame is still delivered and the host applies a hold the server has dropped. `scripts/tamber-builds.test.mjs:566` expects the reused revision and never checks for a null frame.

Fix: on both paths, bump the revision. Frame the new hold after a successful correspond. Frame `{kind:'hold', hold:null}` when correspondence fails, so the queue collapses to the clear and the host's `replaced` report is strictly older than the live revision.

**5. SHOULD-FIX — A hold does not refresh the build listing.** `scripts/tamber-builds.mjs:488`, `scripts/tamber-builds.mjs:500`

`pin()` calls `announce()` (`scripts/tamber-builds.mjs:427`). `hold()` does not. `clear()` announces only when that client also had pins, and returns first otherwise. Subscribers keep the `builds` snapshot from subscribe time. The `held` event carries `state` and `reason` and not the site or instance path, so another client does not see `{site, instance, state}` until some later `builds` or `build-status`.

Fix: `announce()` at the end of `hold()`, and in `clear()` whenever a hold was removed.

**6. NIT — A host `unsupported` for a hold is fanned out to every subscriber.** `scripts/tamber-builds.mjs:367`, `scripts/tamber-server.mjs:182`

The method 409 `{unsupported:'hold'}` stays on the caller's response. The host report (`what:'hold'`, input glue missing) is `send`ed on the shared bus, which writes every subscriber and every long-poll. The ruling sends that report to the owning client. `docs/tamber-protocol.md:885` says every report goes to every client.

Fix: set `client` from the hold record on a hold `unsupported`, and in the subscribe write and the long-poll filter deliver that event only to that client. Align the protocol sentence with that.
