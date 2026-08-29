# Code review: the agent API lane (LLP 1012), 2026-08-29 (grok)

- **Family:** grok (xAI): `grok -p <prompt> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`; the prompt embedded the brief and every capsule file; the CLI has file tools and its working directory was the capsule (a repository export) — it did not run anything. (A first launch with `--always-approve` was stopped after three minutes for that flag's write hazard; nothing was written; no output from it is kept.)
- **Method:** one-shot code review from the brief (sha256 a35b1733c6b6dba8a4b17696dbfe0c4828e1f69e295f11a8f488a84422103f04), mutually blind to the other family; capsule sha256 02281bb288f90c9e12e07308471751add0647902953f5d0ba406bd3e5969acee (the lane's files whole, after the `acceptsFirstMouse` fix).
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T13:00:31Z, verbatim)

> Provenance note: the CLI's leading status lines ("I'll start by…") are its own narration while it read the offloaded prompt and the capsule; preserved as received.

I'll start with the review skill, the binding rules, and the capsule files so the findings stay grounded in the actual code.The capsule is the review surface; next I’ll read the spec, the runner/host paths, and the driver so findings can cite real lines.Next I’ll read the journal, both host ABIs, glue, and the driver — those are where the clock and delivery-path claims live.The Apple agent path, the JS driver, and the tests are next — that’s where input delivery and clock ownership actually meet.A few concrete risks still need a check: Swift’s JSON number casts, whether the GPU loop respects the agent clock, and the journal-ring test cost.Checking a few remaining line-accurate details: GPU time source, Apple commit order, and how the parser treats a key that only appears inside a value.## Overall assessment

The lane is the right shape: eight operations, `tap`/`type` on the real input path, `tree`/`state`/`logs` answered once from the runner, `layout` from what the host renders, no serde, no new dependency, no sixth check. The journal, the ABIs, the stdio carrier’s one-at-a-time discipline, macOS `acceptsFirstMouse`, and the `wall()`/`now()` split are sound.

The load-bearing claim is not. On the web the driver does **not** own the clock: CSS/WAAPI animations keep playing on the wall clock between calls, `clock` attributes timer-created transitions to the destination instant, and the GPU module renders on `requestAnimationFrame` / `performance.now()`. Two `layout`s or `screenshot`s with no `clock` between them can disagree, and one `clock +N` is not sixty `clock +1`s for motion. LLP 1002 D3 is not true of the web host. The smoke never looks at motion, so it stayed green.

## Findings

**HIGH** · `host/web/glue.js:125` · After a `tap` that starts a CSS transition or `Element.animate` spring, `send` records the animation in `starts` and returns. Nothing `pause()`s it. The ticker is off; the browser’s timeline is not. Two `screenshot`s 200 ms apart, no `clock` between them, show different pixels. `clock` to the *current* value then does freeze them (`seek` at line 187), so `clock(now)` then `screenshot` disagrees with `screenshot` alone. The reversing rule is also non-deterministic: a second style change sees whatever wall-clock `currentTime` the first animation has reached.

Fix: at the end of every `send` in agent mode, `seek(agentClock)` (pause and set `currentTime`, same helper `clock` already has). New animations then sit at local 0 until a later `clock` moves them.

**HIGH** · `host/web/glue.js:185` · `clock` sets `agentClock = to`, then `exact_advance(to)` applies every timer’s net style change as one batch, then `send` registers every new animation with `starts = to`, then `seek(to)` sets `currentTime = 0`. Concrete: a timer at t=1000 starts a 500 ms opacity 1→0 transition; `clock +2500` as one jump. True seek: the transition ended at 1500, opacity is 0. This code: the batch writes the final `cssText`, the browser starts a *new* 1→0 transition attributed to t=2500, `seek` leaves it at local 0 → **opacity 1**. Sixty `clock +1000` calls would register the transition at 1000 and seek it to completion. One call and sixty do not give the same bits. `document.getAnimations()` after `apply` *does* typically include that turn’s CSS transitions in Chrome (it flushes style); that is not the bug. The batch has no per-timer timestamp, so intermediate start times cannot be reconstructed.

Fix: while applying an `exact_advance` jump, cancel/finish animations created by that apply (they are the net style delta, not in-world motion), or apply the jump with transitions disabled (`transition: none` / `animation.cancel()`), and keep per-animation seek only for animations that already existed before the jump. A true mid-transition value after a jump that *started* the transition needs per-timer batches; Caltrain does not need that, the API does if it claims D3.

**HIGH** · `host/web/gpu-glue.js:27` · `frame(now)` is a `requestAnimationFrame` loop; `now` is the rAF timestamp. `ResizeObserver` renders with `performance.now()` (line 47). Neither reads `agentClock`. Caltrain’s aurora/map canvases keep animating between agent calls. `clock settle` also ignores GPU (it maxes springs + WAAPI/CSS `endTime` only), so settle-then-screenshot still moves.

Fix: export `now()` on `globalThis.exact`; `gpu_render` takes that; in agent mode do not `schedule()` except from `clock` / `layout` / `screenshot`. Disconnect the observer from wall time the same way.

**MEDIUM** · `host/apple/src/host.rs:274` · Same D3 hole on native, for timer-started motion. `commit` does `engine.advance(now_ms/1000)` *then* `motion_sync` of every receipt from the jump. New transitions are born at the destination instant, not at the timer’s `at`. Stepwise `clock +1000` × 60 and one `clock +60000` disagree if a timer writes an animatable row. Pre-existing order; the agent API made large jumps the supported path.

Fix: `motion_sync` each receipt at that timer’s due time, then `advance(to)` once; or disable transitions for a multi-timer `advance` and apply final targets.

**MEDIUM** · `host/apple/macos/Sources/ExactMac/main.swift:53` · Agent mode stops the 250 ms timer (line 77) but not the display link. While `batch.motion` or a canvas wants frames, `Frames.tick` still runs every vsync. `Exact.tick(now: agentClock)` is probably idempotent; `canvases.tick` is only frozen if the GPU module uses `now` alone. Pair with the web GPU fix: do not request frames in agent mode except from `clock`.

**MEDIUM** · `host/apple/macos/Sources/ExactMac/main.swift:152` · `{"ready":true}` is printed *before* `app.run()`. `makeKeyAndOrderFront` / `activate` have been called, but the window is often not key until the run loop processes events. `acceptsFirstMouse` (Presenter.swift:328) saves a press. `type` still does `makeFirstResponder` + `currentEditor()` (Agent.swift:108–109); a `type` as the first op after `open()` can return `"the field has no editor"`. The smoke taps first, so it hides this.

Fix: emit `ready` from `windowDidBecomeKey`, or `DispatchQueue.main.async` after `app.run()` has started.

**MEDIUM** · `scripts/agent.mjs:77` · `path.startsWith(dist)` is a prefix check after `resolve`. `dist = /tmp/exact-web-dist` allows `/tmp/exact-web-dist-evil/x.js`. Loopback and ephemeral; still the check as written does not hold.

Fix: `path === dist || path.startsWith(dist + '/')` (and keep the extension allow-list).

**MEDIUM** · `runner/src/agent.rs:339` · `after_key` returns the first `"key"` substring followed by `:`. Input `{"text":"{\"op\":\"logs\"}","op":"tree"}` makes `field_str(..., "op")` return `"logs"`. Unterminated strings correctly return `None`; `\"` and `\uXXXX` work when the real key is first (the unit test only covers that). Today’s driver always puts `"op"` first and `type`’s text never goes through this parser, so the three live ops are safe. The parser is still not JSON.

Fix: scan only at depth 1 (skip quoted values), or require the request object’s first field to be `op` and document it.

**MEDIUM** · `scripts/agent.mjs:242` · The library keeps `logCursor` and returns only `{lines, host}`. After a ring wrap, `from > since` and dropped lines vanish with no gap marker. `since` past `next` (the web test uses `since:99` on a short journal) replies `from=99` and `next` at the real end; the library then sets `logCursor = next` and will replay. Fine for Caltrain; not for an hour of 1 Hz timers if the agent polls slowly.

Fix: surface `from`/`next`; if `from > since`, include a line that the ring dropped `from - since` entries. Clamp `from = min(since.max(start), start + len)`.

**MEDIUM** · `scripts/smoke.mjs:144` · Nested-scroll chaining is allowed a 300 ms wall `sleep` plus a retry tap. `clock` did not replace `wait` for scroll. `--disable-smooth-scrolling` makes the happy path sync; this is the leftover flake bandage.

**LOW** · `runner/src/agent.rs:326` · `{"op":"logs","since":"3"}` → `field_num` sees `"`, parses empty, `None`, `unwrap_or(0.0)`. You get the whole journal, not from 3. Live `since` is always a number from the driver.

**LOW** · `host/web/glue.js:225` · `globalThis.exact.agent` is installed on every launch, not only `?agent=1`. `clock` refuses without agent mode; `layout` / `focus` / `tree` / `state` / `logs` do not. No app JS, not a network boundary; LLP 1012 §3 is still false of the code. Opt-in for *clock ownership* is real (`?agent=1`, `EXACT_AGENT=1`); opt-in for the export is not.

**LOW** · `host/web/tests/agent.rs:101` · 5,000 `Host::advance`s on Caltrain (full settle + tree each second) to wrap a 4,096 ring. ~4.7 s debug. The property is “`journal_start` moves and `from`/`next` match.”

Fix: `runner.log("x")` in a loop of `JOURNAL_RING + 10` on a dummy plan, or a timer whose action is a no-op. Keep one Caltrain `advance` test for the `t=…000` stamp if you want it.

The rest of the asked surface is fine, in one pass: CDP NUL framing (JSON cannot contain a raw NUL); flattened `attachToTarget` + `sessionId` filter; `Runtime.evaluate` + `awaitPromise` for the rAF helper and `/__plan`; `Input.dispatchMouseEvent` / `insertText` completing the sync click/`input` handler and wasm commit before the command returns (the two-rAF `frame()` is for paint, with a 250 ms fallback); process-group kill + profile dir; stdio `readLine` + `DispatchQueue.main.sync` while `NSApplication.run` owns main (no deadlock on this code; replies are compact JSON so they do not split on a raw newline); stdin EOF → `exit(0)`; macOS wheel pixel units and sign (`wheel1 = -dy`) as held by `sy === 100`; `convert(_:to: clip)` minus `bounds.origin` as held by `y < 0`; `insertText(_:replacementRange:)` as held by `query === "Palo"`; `cacheDisplay` honestly missing Metal; `agentClock` written only on main via `main.sync`. `tap` on a button with a text child works because `NodeView.mouseDown` without a press handler forwards through `NSResponder` to the parent — DOM bubbling by another name, not `Runner::dispatch`.

## What the tests and the smoke do not hold

- **Any CSS/`Element.animate`/`clock` interaction.** LLP 1012 §8 says so. The web clock bugs above cannot fail CI.
- **GPU pixels under a frozen clock.** Smoke only waits until the module loads (`gpuMs != null`).
- **`clock settle`.** Smoke uses `clock +60000` only.
- **Spec §4’s numbers.** “A wheel of 300 moves it by 300” is `moved > 0`. “Every countdown decrements by one” is the first countdown. “The limit is 652 on both hosts” is not asserted (only `limit > 100` and then the page moves).
- **`type` as the first op** after macOS `ready` (smoke taps first).
- **`screenshot` / `window: true` / Metal.**
- **Parser:** `"op":` inside a prior string value; `"since":"3"`; unterminated string as a request (returns `"no op"`, untested).
- **Journal:** `since > next`; ring wrap as seen through `scripts/agent.mjs` (the ring test talks to `Host::agent` directly and never uses `logCursor`).
- **CDP `buttons` bitmask, `frame()`’s 250 ms fallback winning over rAF, path-prefix escape.**
- **Decision 1 in unit tests.** `host/web/tests/agent.rs` presses via `Host::dispatch_at`, not CDP/`sendEvent`.

## Spec vs code (LLP 1012)

True of the code: eight ops; wheel is a form of `tap`; `tree`/`state`/`logs` from the runner; `layout` from DOM boxes / clip-view space; `tap`/`type` not `Runner::dispatch`; journal boot line, refusal line, advance summary at destination time, timer-refusal at fire time; `settle` in wasm is springs/engine only, CSS `endTime` folded in in glue; transcript fixture; gpu-glue observer disconnect; `wall` vs `now`.

Not true:

- §3 “`exact.agent` is a global on the page in agent mode only” — always installed (`glue.js:225`).
- §1/`§5` “nothing moves between two calls unless a call moved it” — false on web for CSS/WAAPI and GPU; false on macOS for the display link if the GPU module does not honour `now`.
- §1 `clock` as `Animation.currentTime` seek, and LLP 1002 D3 “one call and sixty give the same bits” — false for timer-started transitions on both hosts (web glue start map; Apple `commit` order).
- §4 “a wheel of 300 … moves it by 300”; “every countdown”; “the limit is 652 on both hosts” — not what `scripts/smoke.mjs` checks.
- §4 “nothing waited” — logo poll is `sleep(50)` × 40; GPU poll `sleep(50)` × 60; nested scroll may `sleep(300)`.
- §2 `from` above `since` when the ring has moved — true of `agent::logs`; not of the driver object the smoke uses.

Where the spec and the code disagree, the spec says the code wins. Those §4 sentences are then just wrong, not aspirational.

## Suggestions

- After every agent-mode `send`, `seek(agentClock)`. That single change makes `tap` then `layout` a freeze, and makes `clock` when `to` equals now a no-op for pixels.
- Point the GPU loop at `exact.now()`; in agent mode render only when `clock`/`layout`/`screenshot` asks.
- Replace the 5,000-advance ring test with `JOURNAL_RING + 10` `Runner::log` calls.
- Assert the fixture limit (652) and `moved === 300` if those are the parity numbers you want to keep; otherwise take them out of §4.
- `path.startsWith(dist + sep)`. Emit macOS `ready` once the window is key.
- The 13.8 KiB wasm delta is the module (`tree`/`typed_json`/`quote`/`handle`). Nothing obviously wasteful; do not micro-optimize it.

## Open questions

- Does Chrome ever create a CSSTransition only on the *next* frame after a `cssText` write, so `getAnimations()` in the same `send` misses it even with a freeze-on-send fix? If yes, an `animationstart` hook or a forced `getComputedStyle` before the scan is required.
- Does the GPU module (not in this capsule) sample any clock other than the `now` passed into `gpu_render` / `canvases.tick`? If it does, freezing the argument is not enough.
- `Input.insertText` after `select()` replacing, vs appending, across Chrome versions — smoke holds it today for `"Palo"`; a value that already contains text would be the real check.
- `JSONSerialization` `as? Double` / `as? [Double]` on integer `NSNumber`s is load-bearing (`clock.to`, `wheel`, and also every batch `frame`). Presenter already depends on it, so this toolchain presumably bridges; it remains a Swift footgun if anyone runs the same casts differently.

---

## Disposition (orchestrator, 2026-08-29)

Fixed the same day, with tests or smoke assertions: HIGH web animations playing between operations (`applyBatch` freezes every batch's animations at the clock they began; `register`/`seek`); HIGH timer-started motion born at the destination (`Runner::advance_timed` → `Timed{at_ms, receipt}`; the web batch carries `{"op":"at","ms"}` before each commit's ops and its springs after; the Apple host seeks the engine to each commit's time before feeding it; test `an_advance_batch_marks_each_timers_time_and_says_where_the_clock_landed`); HIGH partial timer failure dropping receipts (`Advanced{receipts, now_ms, error}` keeps the commits, the batch trailer carries `clock`, both hosts and both agent glues land there; test `an_advance_stops_at_a_refusing_timer_with_the_refusal_and_the_clock`); MEDIUM `settle` before advancing (a 16-round fixed point on both hosts, `settled: false` at the bound); MEDIUM GPU on wall time (`gpu-glue.js` renders with `exact.now()` in agent mode, never reschedules itself; `clock` asks for a frame); MEDIUM `exact.agent` unconditional (agent mode only); MEDIUM CDP carrier (pipe end/error and Chrome exit fail every pending call; 15 s deadline per command; `close` waits for exit); MEDIUM wheel `Int32` trap (finite required, rounded, bounded); MEDIUM file-server prefix (`dist + '/'`); MEDIUM parser (one-pass top-level scanner: string tokens and nested objects stepped over; surrogate pairs; unit tests); MEDIUM ring-loss signal (`from` clamped to `[start, next]`, driver returns `from`/`next`/`dropped`, transcript marks it); MEDIUM `ready` before the window is key (`windowDidBecomeKey`, or after 1 s); LOW journal edges (unknown `act` journaled; "poisoned" only on the transition; commands journaled after commit; non-numeric `since` refused); LOW ring test (now `Runner::log` × 4106 at the runner, milliseconds); smoke claims made exact (`moved === 300`, `limit === 652`, every countdown, sixty timers in the journal) and the 300 ms retry removed (Chrome chained without it); `openMac` refuses a boot error; `layout` sorted by id, two decimals, macOS with the layer transform; macOS window screenshots return `w`/`h`/`scale`. Not done, recorded in LLP 1012 r2 §8: a separate normal/agent wasm (Charlie's call); a transition fixture under `clock` in the smoke; two timers writing one animatable row inside a seek (net value only); the display link idling under agent mode; `type`-as-first-op test.
