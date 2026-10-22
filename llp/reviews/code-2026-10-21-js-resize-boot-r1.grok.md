# Code review: the JS target's first-size and geometry boot races, round 1 of 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `10a88642c4191f27039bb664aa070dc225e112ad5690afb02663c5765508b7aa`), shared by both reviewers, each blind to the other's review; HEAD `ed8faf130b`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (1 SHOULD-FIX, 1 NIT).
- **Disposition:**
  Nothing blocks from either reviewer. All taken, in the round 1 commit:
  - Astra 1 (a view removed, or an action that throws, kept the hold until the bound): the hold is let go with the view's scope (`onEnd`), and after the action whatever it does (`finally`). The bound's timer is cleared on release.
  - Astra 2, Grok 2 (scheduling-dependent tests): the reports are held until the page itself says so, 300 ms into the settle (in the same evaluate, no driver gap). The geometry module is served only then, and the state is read in the same evaluate as the settle. Checked by mutation: main fails (widths []); the fix without the geometry chain fails (zeros); the fix without the no-box release fails (2.1 s).
  - Astra 3, Grok 1 (no-box release untested): the box with no layout is never reported in the test, and the settle must answer under 1.5 s, well inside the 2 s bound.
  - Astra 4 (the wasm host has the same races): QUEUE'd with the glue.js lines. Conform's reference won the race in every recorded run, and this change is the JS side's.

1. **SHOULD-FIX** — `host/web-js/resize-boot.test.mjs:89`  
   The “no box holds nothing” claim is not enforced. `hidden <= 1` still passes if Chrome delivers one 0×0 report, and `Date.now() - started < 10_000` still passes if that element keeps the settle for the full 2 s. A dropped immediate `landed()` only fails if the hold lasts past 10 s. Assert that this settle returns in well under 2 s (the tab reports are delayed 400 ms).

2. **NIT** — `host/web-js/resize-boot.test.mjs:63`  
   The 400 ms delay starts when the browser invokes `ResizeObserver`, not when `agentSettled` starts. If the driver reaches the settle call more than ~400 ms after that callback, `widths` is already filled and the assertions pass without the `inflight` hold. Gate the callback on settle having started, or assert that the settle waited.

The two causes are both real. `agent.js:376` waits while `exact.inflight.n` is above device holds, and the old `observers.js` `finally` dropped that count when `observeResize` returned. `rt.js:1089` `geoRead` returns six zeros while `Geo` is null, which is the clone’s `min(max(45, 0), 0 - 12) = -12`. `frame()` is only fetched if the plan uses it (`emit.rs` calls `geo()` after `mount()`), and `geo()`’s promise fulfills only after `Geo` is assigned or the load has failed.

Each `onResize` has its own `inflight.n++` and its own `first` flag, including many elements on the one shared `Resizes` promise and the one `ResizeObserver`. The count comes down once: the first delivered callback (`observers.js:20`), the no-box check (`:21`), the 2 s timer (`:22`), or the import/`observeResize` rejection (`:24`). A later callback or timer hits `first` and does nothing. An action that throws does not skip the release: `act` runs it inside `commit`, which catches. An element already disconnected, or with no client rects, at observe time releases immediately and does not arm the timer. One removed after that check is not delivered (`resize-glue.js:12` drops a disconnected target), so the 2 s timer is the release. That wait is parallel across elements, not per element in series.

That cannot pin a settle. The timer is 2 s from observe, and `agent.js:371` already stops the settle at 20 s and answers `settled: false`. A hang of the resize-glue import is the same as before: the timer is armed only after that promise fulfills, and `geo()` is already holding `inflight` for the same wait. A normal page does not paint through `inflight`. The extra wait is `Promise.all` of the resize glue and `geoLoading()`, and only when `geo()` has already set `Geometry`. `__exactRender` returns before the increment (`observers.js:12`, `rt.js:1083`), so server render is unchanged. With the agent off, nothing waits on the count; the first `resize` action simply runs after geometry has loaded, which is the boot fix for any page that reads `frame()`.

`geoLoading()` is called from the shared promise’s animation-frame callback, and `geo()` runs synchronously after `mount()` in the generated module, so `Geometry` is set before that callback. A plan that never reads geometry leaves it null; `geoLoading()?.catch(() => {})` is then `undefined`, and `Promise.all` does not wait or throw. If the geometry load fails, `geo()`’s own `catch` fulfills the promise, the resize action still runs, and `geoRead` answers unavailable zeros. That is the existing unavailable result, not a stuck settle.

The wasm host has the same shape and it does not affect this mismatch. `host/web/glue.js:558` observes with no `inflight` hold, and `:1440` installs geometry on a later frame; `exact_geometry.read` returns 0 until that assignment. Conform’s recorded failure was wasm `63.70` against js `-12`, so the oracle had already won the race. This patch only makes the JS side wait. A later run where wasm loses would show the opposite diff; that is outside this change.
