# macOS offscreen screenshot can be fully transparent

**Status:** Closed
**Resolution:** does not reproduce on the original app: with Weird Castle brought onto today's exact2 (its branch `fix7/drive`), the report's own drive (`tap title`, `clock settle`, default `screenshot`) gives 840×1800 with every pixel opaque on the title, the login and the signed-in screens, the night shader, the mark, the fields and the buttons all in it; what kept the app from compiling was the compiler refusing its password field's bound `type`, which is admitted again (contract/lower/src/controls.rs)
**Systems:** Apple host, Agent API, Canvas capture
**Severity:** P2
**Author:** Codex, during Charlie Cheever's principles-fix sweep
**Date:** 2026-09-04
**Related:** LLP 1012; LLP 1014.000; LLP 1031

The external Weird Castle app renders and accepts input correctly on macOS,
but the agent's default screenshot returns an 840×1720 PNG whose RGBA bytes
are all zero. The existing window-server capture of the same session shows
the shader, mark, login fields and buttons correctly. Caltrain's offscreen
capture and canvas readback smoke pass on the same host.

Reproduced on Apple M5 Max / macOS 26.6.2 with Exact2 `766ec3d` and the current
external app working tree, after rebuilding through `node exact.mjs macos`:

```js
// From Exact2 with EXACT_APP_DIR=/Users/ccheever/projects/weird-castle.
import { open } from './scripts/agent.mjs';
const s = await open({ host: 'macos' });
try {
  await s.tap('title');
  await s.clock('settle');
  await s.screenshot('/tmp/default.png');
  await s.screenshot('/tmp/window.png', true);
} finally { await s.close(); }
```

The runner reports the login screen with no pending work or error. The
failure is isolated to the default `NSView.cacheDisplay` path in
`host/apple/Sources/ExactKit/Mac/AgentMac.swift`; the specific AppKit cause
is not established. The successful window capture is the verified workaround
and uses its existing screen-capture permission requirement.

Reduce this to a generic cover-window, canvas and ordinary sibling text/input
fixture, then compare offscreen drawing with the existing `Capture.bitmap`
path. Fix the capture once that fixture identifies the cause; no Castle
recognition, automatic screen-permission fallback, or app rendering change.
Done when default capture contains the visible fixture content and the
Caltrain offscreen readback still passes. The application-boundary fix is
independent and remains implemented.

## Closure audit (2026-09-30)

Retained open because the original external app cannot currently compile:
Weird Castle's working tree fails with `type-cannot-infer` on
`app.contract:76`, the unused `setPassword(value)` parameter. Its unrelated
uncommitted edits were preserved. Fresh Caltrain on this lane does not
reproduce: after opening station search, default capture is 840×1800 with
all 1,512,000 pixels nontransparent. The full macOS smoke passes, including
canvas readback (0.52% outside tolerance, mean error 0.50), four subtree
captures, and the deck drive. This confirms the existing Caltrain path, not
a repair of the original Weird Castle report.

## Closed (2026-09-30)

The compile failure the audit above stopped at was not an unused parameter.
Weird Castle's password field is `type=showPassword ? "text" : "password"`,
and since LLP 1069.001 the compiler refused every bound `type`; the type
checker, reaching the field first, gave its `change` handler no payload and
reported the action's parameter instead. A choice between text fields'
literals is admitted again (the node is a text field either way), and a
`type` that is still refused is now reported at the `type`.

Weird Castle itself was a month behind exact2 (the launcher under Node, two
missing `[patch.crates-io]` lines, the hosts' generated entry, the GPU
surface's shared encoder, the runner's new outcome kinds, no logic module for
the web build's JS target). Its port is on its own branch, `fix7/drive`, in
`~/projects/weird-castle`; the main checkout there, which holds another
session's uncommitted work, was not touched.

With that, on this Mac (macOS 27, an M5 Max, the display on), through
`scripts/agent.mjs macos`:

- `tap title`, `clock settle`, `screenshot`: 840×1800, 1,512,000 of
  1,512,000 pixels with alpha above zero, before and after the tap.
- the login's fields typed into and the password shown and hidden: the same.
- signed in against the stand-in Castle, the account menu open: the same.

So the all-zero capture of 2026-09-04 is not present at this main with the
app that reported it, and Caltrain's offscreen readback still passes. The
generic fixture the report asked for was not written: there is nothing left
for it to isolate. One thing seen and not looked into, since it is not this
report: in a default capture taken right after signing in, the signed-in
screen's `iframe` (the Lair deck) shows the sky through its box.
