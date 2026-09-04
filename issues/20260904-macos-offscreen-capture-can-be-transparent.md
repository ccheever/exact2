# macOS offscreen screenshot can be fully transparent

**Status:** Open
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
