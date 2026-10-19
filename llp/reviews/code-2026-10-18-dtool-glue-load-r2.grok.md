# Code review: the JS page's input glue loaded where frames are held (LLP 1122 D8), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `b4cf3fbf75`.
- **Method:** one brief (sha256 `c9211f162cc18bbf4c0b7d8dcb01496689441961f33b44f320e4ae17f29b6e97`), shared by both reviewers, blind to the other review; commit e6598647a0. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken in `74693a204d` (both reviewers' one point): the runtime's pressFeedback handle is its own global, `__exactPressFeedback`, not set on `exact`, whose presence the dev loop takes to mean the runtime is up (a page whose boot threw would otherwise keep a partial `exact` and not be reloaded); the dev page calls it inside the promise, so a throw is caught. Not taken: a cold-load loader test (rt.js's `input()` needs a document; the reviewers' in-memory probes ran it, and the T4 gate's after-rebuild line runs the warm path end to end); the wasm shell's two-frame activation and hidden-tab timer clamps, as in round 1.

**Blocks.** The round 1 point is fixed for a page whose runtime has finished starting. Publishing `pressFeedback` while `rt.js` is still evaluating leaves `globalThis.exact` in place before that startup, and the dev client treats the object's existence as "the runtime is up."

## The point that was taken

A bundled page never serves `rt.js` beside `app.js`, so `holdNow`'s `import(runtime())` 404s and the empty `catch` drops it. After this commit the bundle itself publishes the handle:

```1141:1141:host/web-js/rt.js
export const pressFeedback = () => input(); (globalThis.exact ??= {}).pressFeedback = pressFeedback; // (the dev page's hold asks for it so: a bundled page serves no rt.js)
```

`start()` keeps that property: `Object.assign(globalThis.exact ?? {}, { ready: true, ... })` in `host/web-js/build.mjs:406` mutates the same object. `holdNow` then calls it:

```491:491:host/web-js/inspect-dev.js
  if (!globalThis.exact?.hold) await (globalThis.exact?.pressFeedback ? Promise.resolve(globalThis.exact.pressFeedback()) : import(runtime()).then((rt) => rt.pressFeedback?.())).catch(() => {});
```

On a page that has finished `start()`, that is the right kick. `pressFeedback` is `input`. `input` (`rt.js:1144`) still returns immediately when the glue is already loading, when there is no `requestAnimationFrame`, or under `__exactRender`, and otherwise arms the same frame-or-one-second load as round 1. `glueHold` still waits for `exact-hold`. A bundled page that never called `input()` (no press, pan, or router) now starts the glue, and the 404 import remains for a page that actually serves `rt.js` beside `app.js`. `??=` also keeps a capture script's existing `exact` (its `taps`) and only adds the handle.

## What blocks

`rt.js:1141` runs as the module evaluates, which is before `app()` / `mount` and before `build.mjs:406` publishes `clock`, `state`, `plan`, and `ready`. The dev client, a classic script appended after the deferred module, is already listening. It uses the object as the signal that startup finished:

- `dev.mjs:694` — `globalThis.exact ? Promise.resolve(globalThis.exact.ready) : <poll>`. A missing `ready` is `undefined`, and `Promise.resolve(undefined)` fulfills immediately.
- `dev.mjs:702` — `if (!globalThis.exact) location.reload()`, otherwise checkpoint. `capture` (`dev.mjs:691`) reads `x.clock.now` and `x.state[0]`.

Two cases:

1. **Refused boot, any JS dev page.** `mount` throws at `rt.js:1420` before `dataset.bootMs` and before `Object.assign`. The generated app calls `router` / `navigationRoots` only after `mount` (`host/web-js/src/emit.rs:547` then `:577`), so nothing else had published `exact`. The module has already created `{ pressFeedback }`. The next build's `reload` skips `location.reload()`, `capture` throws on `x.clock`, and the `catch` on `dev.mjs:702` keeps the broken page (`current page retained`). The dev loop stops picking up edits. A plain page with no notifications, sounds, or wakelock — the page this fix is for — did not create `exact` before `start()`.

2. **Rust or mixed page, `load().then(start)` (`build.mjs:419`).** The module task ends on the stub, then the event loop runs while wasm is still loading. A nonce makes `ready()` register with `planDigest: null`. A `reload` checkpoints, throws, and drops the rebuild; `start()` then finishes the old bundle. A replayed hold (`plan` is set on the frame, `scripts/tamber-builds.mjs:222`) can get through `pressFeedback` and `glueHold` before `exact.plan` exists, then `inspect-dev.js:498` returns unsaid because `m.plan !== null`.

A pure JS page whose `start()` returns normally never yields between the stub and `Object.assign`, so the design tool's successful reload does not hit this. The failure window and the deferred-start window do.

Publish the handle where the runtime already publishes `exact` (the `Object.assign` in `start()`, or the end of a `mount` that returned), so a boot that throws still leaves `exact` unset and `location.reload()` still runs.

## Checked, and not a break

**Render host.** `main-server.js` imports `rt.js`, so the VM global gets the stub before `__start`. `input()` still returns at `rt.js:1145` under `__exactRender` before `inflight.n++`. `mount` still reads `exact?.taps?.()`, which the render VM does not set. The render result does not call `pressFeedback`.

**Agent.** `agent.js:298` still calls the module export after `build.mjs:411` has installed the agent on the finished object. That export and `exact.pressFeedback` are the same function; `Inputs` still prevents a second load.

**Tests that import the real `rt.js`** (`boot.test.js`, `live-clock.test.js`) only gain `{ pressFeedback }` on the global. `input()` is not called. `modes.test.mjs` and `files.test.js` mock `rt.js`, so they never run the line.

## What was not taken

The reasons still match the code.

- **No frame-withheld loader test.** `input()`'s frame-or-one-second wait is unchanged, and it needs a document. T4's after-rebuild hold is the offscreen page that had already entered `input()`. Worth knowing: that gate does not exercise the new case, a page that never entered `input()` and is kicked only through `exact.pressFeedback`.
- **Wasm client shell.** `host/web/glue.js:1493` still waits on two animation frames with no timeout before `activate()`, and the glue load sits inside that activation (`glue.js:1473`). The design tool's canvas is this JS runtime. Leaving the wasm shell on the queue is consistent with what this commit touches.
- **A tab hidden long enough for the one-minute timer clamp.** Still the browser's clamp of the same `setTimeout(r, 1000)` in `input()`. This commit does not change it.

## Smaller

`holdNow` attaches `.catch` to the promise, while `pressFeedback()` runs while the ternary is still evaluating. A synchronous throw would skip that catch and reject the hold chain (`inspect-dev.js:467`) with no `held` report. `input()` does not throw on the path above. The bundle build strips comments (`build.mjs:578`), so the reason for the side effect will not be in `app.js`; the assignment itself is a real write and will survive minifying.
