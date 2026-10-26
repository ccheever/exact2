# Code review: stage 14 C1: macOS draws a template-path icon as its path, round 2 of 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `878a3bfa2a`.
- **Method:** one brief (sha256 `4d742c2eb0257586141677cf1457afa93a0c8786bde198a09880d95f80d64135`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**One MINOR finding; no production defect found in either commit.**

- **MINOR — The translucent-tint test derives its expected alpha from the implementation.** [IconShapeMacTests.swift:399](host/apple/tests/ExactKitTests/IconShapeMacTests.swift:399) reads `leaf.alphaValue` and uses it for the reference fill at line 413. Setting every tint’s leaf opacity to zero would pass: opacity is below one, overlap equals non-overlap, and both images are empty. The appearance/emphasis test also checks only RGB.
  
  **Fix:** Assert independently expected alpha—`128 / 255` for the literal and independently resolved `labelColor` alpha—and use that expected value in the reference. Add differing-alpha appearance and emphasis transitions to catch stale opacity.

The requested checks otherwise look correct by inspection:

| Check | Assessment |
|---|---|
| Round-1 translucent overlap | Fixed: every path receives opaque RGB, and the leaf applies alpha once with `allowsGroupOpacity = true`, consistent with [Apple’s group-opacity semantics](https://developer.apple.com/documentation/quartzcore/calayer/allowsgroupopacity). |
| Every re-tint route | Appearance and row emphasis call `tintIconShape`; `retint`, style updates, and `systemColorsChanged` reach it through `applyStyle → updateSymbol`. Every call updates both RGB and alpha. |
| CSS opacity, disabled state, visibility | Tint alpha belongs to the child leaf; CSS opacity and disabled dimming remain on `NodeView`. Visibility hides the clip and leaf. Re-tinting overwrites neither parent alpha nor hidden state. |
| Placement and clipping | The group frame is the placed em square within the content box. Subtracting the square’s origin makes paths local to that group. The outer clip retains content-box clipping and the existing rounded-corner mask. Layout recomputes placement; content-box updates request layout. |
| Implicit animations | Group and shape geometry/paint actions are disabled; the leaf’s alpha write uses a disabled-actions transaction. I found no newly introduced implicit-animation path. This follows [Apple’s action suppression rules](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/ReactingtoLayerChanges/ReactingtoLayerChanges.html). |
| Backing-scale test | Meaningful callback coverage: it changes the window’s reported scale, calls the production override, and checks both group and shape scales. Removing `redraw()` would break its second assertions. It does not qualify a physical display move. |
| Whole-branch scope | Path selection remains restricted to `symbolPath` without an SF name. SF/raster routing, accessibility, intrinsic image size, and control-face image remain intact. Path → symbol → path removes the previous leaf. iOS and Snapshot behavior are unchanged. |

The clipping test meaningfully checks overflowing ink and empty gutters. Broader geometry cases—other fits, padding, rounded corners—and alpha composition with CSS/disabled state are not covered by the new pixel tests.

`git diff --check HEAD~2..HEAD` passed; no files changed. XCTest was not run: this read-only checkout has no built test artifacts.

**Verdict: Both round-1 fixes are correct by inspection; one MINOR test correction remains, with no new production regression found.**
