# Code review: the app-diary fixes, batch 1 (gaps/papercuts), 2026-10-04 (grok)

- **Family:** xAI — `grok-4.7` via `grok -p`, reasoning effort xhigh, in the gaps/papercuts worktree, told to modify nothing (the tree was clean after).
- **Method:** one delta round over the round-4 fixes and da47e8e57, blind to the other family's round-5 review (Grok was signed in only after rounds 1–4). The author (Claude, Opus 5.5, orchestrating) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** the one new defect (an explicit contact point out of view was never scrolled in) is fixed in the commit after this round: an out-of-view explicit point brings its view's middle in, a visible one is left alone. 4a8/4b3 partials: 4a8 queued (QUEUE.md); 4b3 is the same fix.

---

Checked the fix commits against the current tree, and probed the pan release in headless Chrome and the clone path under Hermes and Bun. No repository files were changed.

**4a1. fixed** — On a native module, `settle` claims an unclaimed non-stream fetch before it will fail an answer as pending on nothing, including on a final wake. The castle followup covers a second fetch made after awaiting another answer’s fetch.

**4a2. fixed** — Capture skips a directory whose realpath is `--out`, so `web-out/.gen/typescript` is not walked back into the stage.

**4a3. fixed** — Enter arms a one-shot window bubble listener that submits only while `defaultPrevented` is still false, and a contract `preventDefault` sets that flag on the live keydown.

**4a4. fixed** — While capture is deferred, window capture-phase move, up, and cancel watch the contact, and a release outside flushes the pan and drops it. In Chrome, mouse and touch pointerup both reach that listener; `setPointerCapture` returns without throwing and the pan delta still runs.

**4a5. fixed** — Own keys are created with `Object.defineProperty`, so a cloned own `__proto__` stays a data property and does not become the object’s prototype.

**4a6. fixed** — The local monitor and the agent both use `routeKey`: shortcuts and authored `key` handlers run before menu and dialog defaults, and a prevented key is swallowed. The canvas chord path takes Space and Enter only.

**4a7. fixed** — Each pending ticket keeps a `release`, and fulfillment, rejection, and forgetting call it. The reused-controller checks expect equal add and remove counts.

**4a8. partly** — Hermes watches through Ibex abort hooks, which run before listeners. The browser module realm loads `prelude.js` alone, so `watchAbort` (`js/src/prelude.js:411`) is still `addEventListener`; an earlier `stopImmediatePropagation` skips it and the promise stays pending until a reply, which then rejects because fulfill rechecks `signal.aborted`.

**4a9. fixed** — The copy selects the captured constructor for the brand from the intrinsic `@@toStringTag` getter. Hermes has no `Float16Array`, and that getter does not throw on a Proxy.

**4a10. fixed** — `toSorted` reads every index into a fresh dense array before the intrinsic sort, so a hole is an own `undefined` and a following `map` visits `length` times. It is installed only where `toSorted` is absent.

**4b1. fixed** — The lock is gone. macOS and Linux stores are `<storage>.r<pid>-<tag>.t<n>` under the launch environment’s `HOME` or absolute `XDG_CACHE_HOME`, swept only when that pid is dead, and removed after the test. Simulator and device reuse `.t<n>`, and each launch terminates the previous process.

**4b2. fixed** — An unsupported drag keeps `delivery: "unsupported"` and its reason, and `delivered` fails the step on `error` or `unsupported`, so later expects do not run.

**4b3. partly** — Reveal is skipped when `drag.from` or `down.at` is set, so a visible explicit point is no longer scrolled off a tall canvas. An explicit point that is currently outside the viewport is never scrolled in.

**4b4. fixed** — `typeControl` returns the read-only error before assigning or firing `input` and `change` for a textarea and for the input types HTML’s `readonly` applies to, including date, time, and datetime-local. Checkbox, range, color, and select stay settable. The JS host uses the same function.

**New defect**

- **should-fix** — `scripts/agent.mjs:990`. A below-the-fold `tap canvas drag 100 0 from 20 20` (or `down at`) used to center-scroll the node first. Reveal is now skipped whenever an explicit offset is present, so `dragTap` throws `leaves the viewport` and the gesture never starts. Scroll that contact point into view when it lies outside, and leave scroll unchanged when it is already visible.

Verdict: LAND WITH FIXES
