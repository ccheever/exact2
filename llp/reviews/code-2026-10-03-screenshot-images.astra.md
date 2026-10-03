# Code review: Apple screenshot image readiness (astra)

**Reviewer:** Astra (codex gpt-6-astra, xhigh), 2026-10-03, diff 0d1696ed~1..0d1696ed

**Verdict:** SHIP WITH FIXES — two ordering bugs can still produce stale or incomplete screenshots.

1. **Medium — The new canvas render can make iOS screenshots reuse stale pixels.**  
   `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:726` now settles canvases before capture. Rendering clears the module’s dirty flag (`gpu/src/frame.rs:159`), but leaves `Entry.picture` intact. `IOS/GpuIOS.swift:496` then reuses that cached readback when the agent clock is unchanged. After an earlier screenshot, a child image can finish decoding and be uploaded/rendered, yet the next screenshot still returns the earlier pixels. The screenshot reaches this cache through `IOS/NodeViewIOS.swift:1288`.

   **Fix:** Invalidate the cached readback when its canvas content changes or renders, or explicitly refresh it for screenshot capture. Add a regression taking two screenshots at the same clock with a child-image completion between them.

2. **Medium — Images mounted by the final settlement bypass the readiness wait.**  
   `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:722` and `Mac/AgentMac.swift:732` save `waitForImages()`’s count, then call `settlePump()` without checking again. That settlement can build additional rows (`IOS/ScrollPumpIOS.swift:245`, `Mac/PresenterMac.swift:474`). Image intrinsic-size changes can leave collection feedback queued because ordinary feedback is limited to two passes per turn (`Collection.swift:81`). If the final settlement mounts image-bearing rows, capture proceeds immediately and omits `imagesPending` because the saved count was zero.

   **Fix:** Alternate pump settlement and image-readiness checks under one shared three-second deadline. Read the pending count after the final settlement. Cover a late intrinsic-size change that exposes additional image rows.
## Disposition

1. **Fixed (iOS; macOS has no cache).** `IOS/GpuIOS.swift` drops `Entry.picture` when the canvas renders (`renderNow`) and when its children are captured (`capture`). Either can consume the dirty flag the cached readback predates. `Mac/NodeViewMac.swift` reads back on every capture and keeps no picture, so it needs no change. The cheap regression didn't discriminate. In Caltrain on the iOS simulator, "screenshot a · tap material-ink · screenshot b · clock +16 · screenshot c" gave b ≠ a and b = c both with the fix and on a baseline build without it. A material change marks the sky dirty after the screenshot's render, so it doesn't reach the stale path. Reaching it needs a nested canvas whose child image lands between two screenshots at one clock. Caltrain has none (the line map's children are text), so the fix has no regression test yet.
2. **Fixed (iOS and macOS).** `Agent.settleForPicture()` alternates `settlePump()` with the on-screen image wait under one shared 3 s deadline. It returns the pending count read after the last settle, and both `screenshot`s report it as `imagesPending`. `waitForImages(until:)` takes the shared deadline. No fixture covers a late intrinsic size that exposes more image rows. Cold Caltrain launches gave 6 of 6 iOS screenshots identical to the known-good image, and 3 of 3 on macOS.
