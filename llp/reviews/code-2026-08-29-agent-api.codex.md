# Code review: the agent API lane (LLP 1012), 2026-08-29 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only -C <capsule> --skip-git-repo-check --ephemeral`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export (the capsule), no network
- **Method:** one-shot code review from the brief (sha256 a35b1733c6b6dba8a4b17696dbfe0c4828e1f69e295f11a8f488a84422103f04), mutually blind to the other family; capsule sha256 02281bb288f90c9e12e07308471751add0647902953f5d0ba406bd3e5969acee (the lane's files whole, after the `acceptsFirstMouse` fix).
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T13:00:02Z, verbatim)

## Overall assessment

Request changes. The authority split and real-input paths are sound, but the central clock guarantee does not hold: web animations run on wall time, timer-started motion depends on seek granularity, and failed advances can leave three clocks disagreeing. `tree`/`state`/`logs`, stdio ordering, pipe framing, Swift’s main-thread ownership, and the no-serde/no-dependency posture otherwise look good.

The capsule lacked build artifacts/manifests, so full smokes were not runnable; JavaScript syntax checks passed.

## Findings

- **HIGH** · `host/web/glue.js:94` · Animations created by a tap or type immediately auto-play. `send()` only records their virtual start at lines 125–128; only a later `clock` pauses/seeks them at lines 141–148. For example, a tap at clock 0 starting a one-second opacity transition advances during the driver’s two animation frames (`scripts/agent.mjs:130,149`), so screenshots taken without a clock call vary with wall time; `clock 0` then visibly rewinds it. Fix by calling `seek(agentClock)` immediately after every `send()` discovers new animations. Discovery itself is reliable: [`getAnimations()` must apply pending style changes before returning a new CSS transition](https://www.w3.org/TR/css-transitions-2/).

- **HIGH** · `runner/src/runner.rs:564` · Timer-triggered motion violates “one seek and sixty give the same bits.” The runner executes timers at their due times but returns receipts without timestamps; both hosts inspect the final kernel only after the whole seek (`host/web/src/host.rs:161`, `host/apple/src/host.rs:187`). Web then records any resulting transition as starting at the requested target (`host/web/glue.js:184-187`), while Apple advances the engine to the target before applying every motion sync (`host/apple/src/host.rs:273-279`). A timer at 1000 ms starting a one-second transition gives its start value after one `clock 1500`, but its halfway value after `clock 1000; clock 1500`. Fix by preserving `(due_ms, receipt)` and committing motion at each due instant before the final seek; the host must see each intermediate target, not reread only the final kernel.

- **HIGH** · `runner/src/runner.rs:576` · A timer refusal after earlier timer successes drops all accumulated receipts. The runner and kernel retain those commits, but `result?` returns only the error at line 589; both hosts apply an empty batch. Web and Apple then still set their host/agent clocks to the requested time and return success (`host/web/glue.js:184-188`, `Agent.swift:129-132`), while `state.clock` remains at the failing timer’s due time. Fix `advance` to return partial timed receipts plus a typed error/current clock, make hosts apply or restart from that outcome, and never move motion or report clock success beyond the runner’s accepted time.

- **MEDIUM** · `host/web/glue.js:174` · `clock settle` computes its target only before advancing timers. If a current transition ends at 1000 ms and a timer crossed on the way starts a transition ending at 2500 ms, the call still replies at 1000 ms. Apple has the same order at `Agent.swift:123-132`. Fix with a bounded fixed-point loop: advance, discover/apply resulting motion, recompute settlement, and repeat until unchanged; explicitly refuse an indefinitely motion-producing timer cycle.

- **MEDIUM** · `host/apple/macos/Sources/ExactMac/Presenter.swift:121` · Host I/O can mutate output between calls without a clock operation. An image completion applies intrinsic size asynchronously at lines 123–134; on web, setting `<img src>` similarly changes natural layout after `ready`. GPU loading and rendering also use wall-time animation frames (`host/web/gpu-glue.js:27-38,47`). The smoke demonstrates this by sleeping and polling (`scripts/smoke.mjs:58,106`). Either gate completions into agent-operation boundaries, or narrow the invariant to timers/motion and restore consistency tokens for asynchronous host changes.

- **MEDIUM** · `host/web/glue.js:225` · `exact.agent` is installed unconditionally, so a normal launch can call `exact.agent({op:"state"})`, `tree`, `logs`, `layout`, or `focus`; only `clock` checks agent mode. This contradicts opt-in posture and keeps the 13.8 KiB agent implementation in the normal wasm through the unconditional export at `host/web/src/abi.rs:197`. Fix by exposing the global only in agent mode and producing separate normal/agent top-level artifacts so the normal wasm does not export `exact_agent`.

- **MEDIUM** · `scripts/agent.mjs:35` · The CDP carrier never handles pipe `end`/`error` or Chrome exit, and commands have no timeout. A Chrome crash during `tree` leaves its pending promise unresolved forever. The 250 ms `frame()` fallback also does not cancel the losing `Runtime.evaluate`, leaving a pending entry if animation frames never resume. Reject all pending calls on pipe/process termination, bound individual commands, and wait for process exit before removing the profile.

- **MEDIUM** · `host/apple/macos/Sources/ExactMac/Agent.swift:80` · Wheel deltas are converted directly from `Double` to `Int32`. A valid request such as `[0,1e20]` traps and kills the process; `[0,0.5]` silently becomes zero while web scrolls fractionally. Validate finite/range-bounded deltas and set the CGEvent point-delta fields without integer truncation, or reject unsupported fractional values consistently on both hosts.

- **LOW** · `runner/src/agent.rs:295` · The flat parser does not enforce a top-level object or field types. `{"meta":{"op":"state"},"op":"tree"}` executes `state`; `{"op":"logs","since":"3"}` silently defaults to cursor 0 and returns the full ring; malformed trailing JSON is accepted. Escaped `\"` does not itself fool `after_key`, but valid surrogate-pair `\u` strings are rejected because each surrogate is decoded separately. Replace the searches with a small single-pass, top-level tokenizer that validates duplicate keys, syntax, types, and surrogate pairs—still without serde.

- **LOW** · `runner/src/agent.rs:165` · Journal edge semantics are inconsistent. With `next=3`, `since=99` returns `from=99,next=3`; unknown `act` refuses before reaching `log_outcome` (`runner/src/runner.rs:538-552`); after an earlier poison, an unrelated `UnknownView` is labeled as having “poisoned the runner” (`runner/src/runner.rs:380-393`). Commands are also logged before an update that can poison and clear them (`runner/src/runner.rs:665-682,705-718`). Clamp `from` to `[journal_start,next]`, route action lookup failures through logging, classify only a false→true poison transition as “poisoned,” and log commands after successful commit.

## What the tests and the smoke do not hold

- No transition fixture exercises immediate freezing, `clock` at the current time, delay/end boundaries, reversal, one large seek versus several seeks, timer-started motion, or settlement after crossed timers.

- Parser tests cover only happy-path flat fields; they omit nested/duplicate keys, wrong types, malformed trailing input, surrogate pairs, and unterminated strings.

- Journal tests omit cursor clamping, unknown actions, pre-poisoned refusals, command rollback, and partial timer failure.

- The ring test performs 5,000 full Caltrain advances (`host/web/tests/agent.rs:97-128`). A runner-level test calling `Runner::log` 4,097 times would hold wraparound, `start`, `next`, and tail slicing without rebuilding the app tree.

- The smoke says a wheel of 300 moves by 300, but checks only `moved > 0` (`scripts/smoke.mjs:93-99`). It says the nested limit is 652 on both hosts, but merely records any runtime limit greater than 100 (`scripts/smoke.mjs:140-147`).

- Screenshot is optional behind `--shot`; pixels, dimensions, Metal behavior, and window-server failure are not normally tested.

- There is no test that a normal web launch lacks `exact.agent`, nor a CDP process-loss, stdio first-request/readiness, EOF, or embedded-newline reply test.

## Spec vs code (LLP 1012)

- §1/§5’s “nothing moves between calls” is false for auto-playing web motion, images, and GPU work.

- §1’s `clock` reply does not guarantee that runner, host, and motion clocks agree after refusal.

- §1 calls layout nodes “on-screen,” but web includes every connected view and macOS every view attached to a window, including clipped/offscreen nodes.

- §1 promises `w` and `h` for screenshots; macOS window screenshots return only `screenshot` and `window` (`Agent.swift:141-146`).

- §3 says eight operations, but the transport also accepts `focus`, `settle`, and macOS `quit`. The public session has eight; the spec should explicitly classify these as private carrier messages if they do not count.

- §4’s exact 300-pixel and 652-pixel claims are not asserted by the smoke.

- The real-input decision is correctly implemented: web uses CDP events, macOS press uses `NSWindow.sendEvent`, and text uses each host’s editor path. Runner/kernel-only reads and host-only layout are also faithful.

## Suggestions

- Add one minimal transition/timer contract used unchanged on both hosts. Pin values after `clock 0`, one versus split seeks, reversal, and `settle`.

- Have `openMac` reject a non-null `ready.error`; currently an invalid custom plan produces a nominally open session.

- Test macOS layout during translate/scale/rotate. `Agent.box` converts the view bounds, while presentation transforms are applied directly to the backing layer (`Presenter.swift:240-245`); verify that the result matches web `getBoundingClientRect`.

- The NUL pipe framing, flattened-session filtering, `awaitPromise`, device metrics override, stdio line framing/order, EOF exit, and main-thread access pattern otherwise look appropriate.

## Open questions

- Does “nothing moves” intentionally exclude image/network/GPU completion? If so, what consistency contract replaces the absolute claim?

- Should `settle` include timer-triggered motion crossed while seeking, and how should perpetual timer-driven motion terminate?

- Should `layout` report transformed presentation boxes or stable layout boxes? Web currently reports the former.

- Are private `focus`/`settle`/`quit` messages exempt from the eight-operation budget?

- Is the 13.8 KiB cost accepted in every normal wasm, despite the repository rule that optional capability should be a separate artifact?
---

## Disposition (orchestrator, 2026-08-29)

Fixed the same day, with tests or smoke assertions: HIGH web animations playing between operations (`applyBatch` freezes every batch's animations at the clock they began; `register`/`seek`); HIGH timer-started motion born at the destination (`Runner::advance_timed` → `Timed{at_ms, receipt}`; the web batch carries `{"op":"at","ms"}` before each commit's ops and its springs after; the Apple host seeks the engine to each commit's time before feeding it; test `an_advance_batch_marks_each_timers_time_and_says_where_the_clock_landed`); HIGH partial timer failure dropping receipts (`Advanced{receipts, now_ms, error}` keeps the commits, the batch trailer carries `clock`, both hosts and both agent glues land there; test `an_advance_stops_at_a_refusing_timer_with_the_refusal_and_the_clock`); MEDIUM `settle` before advancing (a 16-round fixed point on both hosts, `settled: false` at the bound); MEDIUM GPU on wall time (`gpu-glue.js` renders with `exact.now()` in agent mode, never reschedules itself; `clock` asks for a frame); MEDIUM `exact.agent` unconditional (agent mode only); MEDIUM CDP carrier (pipe end/error and Chrome exit fail every pending call; 15 s deadline per command; `close` waits for exit); MEDIUM wheel `Int32` trap (finite required, rounded, bounded); MEDIUM file-server prefix (`dist + '/'`); MEDIUM parser (one-pass top-level scanner: string tokens and nested objects stepped over; surrogate pairs; unit tests); MEDIUM ring-loss signal (`from` clamped to `[start, next]`, driver returns `from`/`next`/`dropped`, transcript marks it); MEDIUM `ready` before the window is key (`windowDidBecomeKey`, or after 1 s); LOW journal edges (unknown `act` journaled; "poisoned" only on the transition; commands journaled after commit; non-numeric `since` refused); LOW ring test (now `Runner::log` × 4106 at the runner, milliseconds); smoke claims made exact (`moved === 300`, `limit === 652`, every countdown, sixty timers in the journal) and the 300 ms retry removed (Chrome chained without it); `openMac` refuses a boot error; `layout` sorted by id, two decimals, macOS with the layer transform; macOS window screenshots return `w`/`h`/`scale`. Not done, recorded in LLP 1012 r2 §8: a separate normal/agent wasm (Charlie's call); a transition fixture under `clock` in the smoke; two timers writing one animatable row inside a seek (net value only); the display link idling under agent mode; `type`-as-first-op test.
