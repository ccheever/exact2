# Code review: LLP 1122, the design tool's reads follow the files, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `51702c2e8`.
- **Method:** one brief (sha256 `f4d70969420c8f71bc85fd0ce06f4208ce656ea9ebc8eb7a6beb110cb96c3110`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** one medium (the design reader getting ready doesn't re-propose); the read-skip, the pool, the build key and both tests hold.
- **Disposition:** taken in `199e1beef` (the same fix as Astra's medium). Smaller points: the overtaken read's next describe is taken. The hello that fails with `live: false` is taken (the chip uses the live value it was asked with). The stub's coarse class is left as is: it stands in for the loop's answer.

One thing should be fixed. The read-skip, the pool, the build key, and both contract tests hold. Round 1's two taken fixes are in the right place. The proposal key still misses one input the chip depends on.

## P2 — The design reader becoming ready does not re-propose

`apps/design-tool/app.contract:565` re-asks a proposal when the draft, `filesAt`, `buildsAt`, `hello.live`, or `epoch` moves. The JS loop's class does not. `proposeHere` (`scripts/tamber-server.mjs:552`) downgrades a live edit only when `inlineWhy` returns a reason, and `inlineWhy` returns null whenever the design reader is not ready (`host/web-js/dev.mjs:617`). The reader becoming ready is a `console.log` (`host/web-js/dev.mjs:264`). No file changes, no build is re-announced, and the JS loop's `hello.live` stays `true` (`host/web-js/dev.mjs:672`).

Pages are already being served while `cargo build --profile host-dev -p exact-web-js` is still running (`buildTool` starts at loop startup, `host/web-js/dev.mjs:275`). Draft an inline token in that window — a font-weight number, an SVG attribute, anything `designOf` marks `inline`. `inlineWhy` skips, the planner's class stands, the chip says Live, and Scrub live is enabled (`app.contract:1290`, `:1309`). The reader then finishes. Nothing in the proposal key moves, so the chip stays Live. Scrub still refuses: `scrubPreview` reads the design through the fallback compiler while the reader is down and through the reader once it is up (`host/web-js/dev.mjs:303`, `:597`), and both report the token inline. Apply still writes the reviewed bytes. The same stickiness runs the other way when compiler sources the loop does not watch go stale (`toolFresh` at `host/web-js/dev.mjs:269` calls `buildTool` and returns false, again with no poll event): a re-propose during that rebuild locks in Live, and the chip stays there after the reader is ready again.

Before this change any poll event re-proposed, so the next build status healed it. `buildsAt` covers the case round 1 actually described a fix for: a source rebuild that changes a token's usage publishes a new plan, and the plan digest is in the key. It does not cover the reader-ready transition, which has no event. The contract test cannot catch it either. The stub treats any `web-js` build as enough to call `opacity.muted` a rebuild (`apps/design-tool/stub/tamber-stub.ts:40`), with no reader-ready state. Fixing this wants a signal from the loop (a builds re-announce when the reader becomes ready or goes stale is enough). The same residual that was declined in round 1 is still declined: a server-side build that stops before the design and leaves the old build registered (`contract/cli/src/tamber_serve.rs:347`, `:1174`) still has no event, and the server judges the write.

## Round 1

Both taken items landed in `c06d9ee5b` and do what they claim. The proposal follows `buildsAt` (id, host, plan digest) and `hello.live`. Describe and the read's trailing hello wait `READ_WAIT`, which is `WRITE_WAIT` (130 s). The tests step past the first advance, and the stub's `pass` defaults to none. The two items left aside are still aside: that build-stopped-before-the-design case, and a 15 s propose or axes describe behind a batch. Four resolves at a time keep that queue well under 15 s.

## The pool

`readDesign` (`apps/design-tool/app.ts:225`) holds on every case you named.

`resolved[i]` is the resolve of `ir.tokens[i]`. Each worker claims `i = next++` before the await (`app.ts:239`). One JavaScript thread, so the claim is race-free, and the success path maps by that index (`app.ts:260`, consumed at `:705`).

An empty token list starts zero workers (`Math.min(4, 0)`). `Promise.all([])` resolves, `resolved` is `[]`, and the digest checks pass. The grid has no rows.

A resolve error does not stop its siblings. After the pool drains, `wanted()` is checked first (`app.ts:243`), then `find` returns the first `r.error` (`app.ts:244`). While `wanted()` stays true every index is claimed, so there is no hole. `call` returns `{error}` rather than throwing, except `bake`, which `loadDesign` rethrows (`app.ts:779`).

`wanted()` going false stops new claims. Up to four resolves already in flight finish and are not aborted, so they briefly overlap the new read's own four. `readDesign` then returns `overtaken by a newer read`, and `loadDesign` turns that into `noDesign` (`app.ts:694`). The runner drops it: the new `loadDesign` incremented `issued` synchronously (`app.ts:691`) and the web runtime forgot the old ticket before this promise settles (`host/web-js/rt.js:521`, drop at `:444`). The shown design stays the last one. The snapshot is stored only while `number === issued` (`app.ts:763`).

`issued` is incremented only in `loadDesign`, and `loadDesign` is the only caller of `readDesign`. A refresh or a new `(file, filesAt, epoch)` is a new invocation and a new number. `loadAppAxes` increments `axesReads` only (`app.ts:1307`) and returns immediately when `file` is `""`. No second caller cancels a read that should be shown.

## `buildsKey()` and `state()`

`state` is a closure created before the events loop (`app.ts:990`) and calls `buildsKey()` when it returns. On the answer that is shown, that return is after the events loop (`app.ts:1036`), after a resync's `takeBuilds` (`app.ts:1075`), and after `place` is updated (`app.ts:1079`). The two early returns (`app.ts:991`, `:992`) happen before this batch is applied. The mid-resync `gen !== polls` return (`app.ts:1074`) is dropped with the ticket. That superseded poll can already have mutated `buildsNow` and bumped `fileChanges` (`noted` is not idempotent, `app.ts:615`). The newer poll then applies the same events again, so the count can move twice and one extra design read can follow. The design it shows is the current files.

## `hello.live`

Keying the proposal on the other resource's displayed boolean behaves. A hello refresh that leaves the boolean alone does not re-propose. On a success, `loadHello` assigns `liveTable` before it returns (`app.ts:684`), and the re-ask starts with `liveTable` matching that answer. The chip reads `liveTable` after the propose await (`app.ts:824`), which is the value the argument was there to invalidate. The wasm loop's `live()` flip is covered: a successful rebuild reloads pages and pushes `rebuilt` (`host/web/dev.mjs:390`, `:1225`), the poll's seq moves, `advance` refreshes hello (`app.contract:641`), and `hello.live` is in the key even when id and plan digest stay put. The one server queue is FIFO, so an older hello's continuation assigns `liveTable` before the newer hello's reply.

## The two tests

Judged from the code. They were not executed.

The status-and-scrub test (`app.test.contract:378`) fails on the old `since` key. The injected `builds`, `build-status`, and `scrub` events bump seq. `take` logs `event: 'changed'` and then spreads the event (`stub/tamber-stub.ts:87`), so `e.event` wins and `noted` does not count them (`app.ts:614`). `filesAt` stays put. With the old key the design is read again, `refuseDescribe` with `pass` omitted sets `describePasses` to 0 (`stub/tamber-stub.ts:317`), describe returns 400, `design.ok` is false, and `design-error` appears (`app.contract:1136`), which fails both the missing-error assertion and the dial still being there. `refuseDescribe` is load-bearing: without it a re-read would succeed and the test would pass on the old key. The first `clock +1` is the one legitimate read, which matches the round 1 correction. The restart at `:395` is required. The stub is one process, and a leftover `refuseDescribe` fails every later describe.

The proposal test (`app.test.contract:397`) fails without `buildsAt`, and it also fails without the stub's `classOf` / `INLINE` change. A `web-js` builds event leaves files, `hello.live` (already true), epoch, and the draft alone, so the old key keeps the chip on Live. With `buildsAt`, `buildsKey()` gains `web-js-1 web-js p1`, propose runs again, and `classOf` returns `rebuild` because `opacity.muted` is inline and a `web-js` build is present (`stub/tamber-stub.ts:41`). `liveTable` stays true, so the Rebuilds chip is the class. `part-amount-field` is the control that token's number editor uses. The restart at `:417` clears `LIVE` and `BUILDS`. Left set, later `opacity.muted` proposals would come back rebuild. `describePasses` is not cleared on restart (`stub/tamber-stub.ts:226`). That is harmless: the gate is `refuseDescribe && …` (`:158`), and restart clears `refuseDescribe`.

## Smaller

An overtaken read can issue one more describe when the consistency check fails during the trailing hello. The attempt loop does not re-check `wanted()` before the next describe (`app.ts:227`), and the success return at `:260` does not either. The runner drops that answer and the snapshot is not stored.

A hello that returns `{ok: false, live: false}` replaces the displayed boolean and re-asks the proposal, while `liveTable` stays at the last success (`app.ts:682`). The chip can say Live, Scrub is disabled because `hello.live` is false, and the unavailable line is hidden because it renders only when the chip is not live (`app.contract:1313`). A down server usually fails the propose too, so the chip is replaced by the error. `advance` refreshes hello while `hello.ok` is false.

The stub's class is coarser than the loop's: any `web-js` build, no `toolFresh`, no real inline flags.
