# Review: LLP 1104 steps 1–2 code (08e7e2ff4), 2026-10-07 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, at `08e7e2ff4` (`git diff 561abadcc..08e7e2ff4`).
- **Method:** a code-review brief (sha256 `c35db0512a00178ee0c24ce305b2834462eeab1feda9c3b5979f5f0849795fc0`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** all eight findings went to fix lanes F1 (kernel and runner: 2, 3, 5, 6, 8), F2 (Apple: 1, 7) and F3 (Linux: 4); see LLP 1104 §7, r9.

---

**Verdict: DO NOT LAND.**

**Findings**

1. **Replacement plans fail on iOS/tvOS — blocking.**  
   [host/apple/Sources/ExactKit/Session.swift:854](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Session.swift:854)

   `prepare` attaches the control cache to the candidate text engines only on macOS. On iOS/tvOS, the installed `controlText` callback receives the uncached candidate and returns a zero-sized font; Rust rejects it with `InvalidEnv`. This breaks plan replacement and selected-plan launches, including plans containing no fields.

   **Fix:** Install the cache on both candidate engines on every Apple platform before preparation. Test initial selected-plan launch and replacement of a running, fieldless plan.

2. **Chrome settlement can publish the provisional minimum height — material.**  
   [kernel/src/layout/fields.rs:93](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout/fields.rs:93)

   Replacing a provisional chrome answer preserves `field_minima`. If the exact answer changes only the nonzero minimum height—for example, 34 → 44 with identical insets—the reconstructed Taffy style remains unchanged. Layout stays cached, never recomputes the minimum, and reports the old height as final. A constrained-height field encountering a new font can therefore remain undersized.

   **Fix:** Invalidate layout and recompute the derived minimum when chrome geometry changes. The existing settlement test returns identical provisional and final geometry; make those answers differ.

3. **Region chrome misses bypass the presentation barrier — material.**  
   [kernel/src/kernel.rs:436](/Users/ccheever/projects/exact2-wt-1104/kernel/src/kernel.rs:436)

   `compute_region_layout` checks only the shell tree’s provisional flag. Pending and candidate branches use separate `Derived` trees whose flags are discarded. A pending branch containing a field with an uncached font can publish guessed chrome while the shell reports no provisional work. Apple consequently skips its retry and presents the batch; the provisional counter also misses it.

   **Fix:** Propagate provisional status from every contributing region tree and settle chrome before accepting or publishing its geometry. Cover a cold field in the pending branch.

4. **Linux/Windows native fields can render invisible text — material.**  
   [host/linux/src/paint/control.rs:103](/Users/ccheever/projects/exact2-wt-1104/host/linux/src/paint/control.rs:103)

   The native fill is fixed white in light mode, but text still inherits its ancestor’s color. A dark container with `color="white"` and an unstyled, nonempty input produces white text on a white field. Linux supplies no control text environment; its new test explicitly preserves inheritance. This also departs from D4/D7’s host-owned text style and removes the previous field’s protective ink default.

   **Fix:** Supply the painted host’s native field text defaults, preserving explicit field overrides and bare-field inheritance. Test nonempty fields under contrasting ancestor colors.

5. **Native fields defeat hypothetical auto-height measurement — material.**  
   [kernel/src/layout/fields.rs:94](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout/fields.rs:94)

   `measure_auto_height` temporarily sets the engine’s height to `auto`, then calls `compute`. `prepare_fields` immediately replaces that style with the authored arena style. Thus `measure("editor")` on a native `textarea rows=2 height=200` returns the authored 200-point content height plus chrome, instead of its two-row auto height.

   **Fix:** Apply chrome without erasing temporary measurement overrides. Add a native-field regression comparing `measure` against an otherwise identical auto-height field.

6. **Resize events count native chrome as content — material.**  
   [runner/src/runner/resize.rs:215](/Users/ccheever/projects/exact2-wt-1104/runner/src/runner/resize.rs:215)

   Resize calculation subtracts authored padding and borders, but chrome now exists only in derived engine geometry. On Linux, a native `width=100 height=30` input therefore reports a `118×44` content rectangle instead of `100×30`. Applications feeding the observed size back into field dimensions grow the field repeatedly.

   **Fix:** Obtain native field content dimensions from the published field content rectangle. Test resize payloads with chrome and authored padding.

7. **Native-region projection drops the editor rectangle — material.**  
   [host/apple/src/layout.rs:214](/Users/ccheever/projects/exact2-wt-1104/host/apple/src/layout.rs:214)

   The new `fieldContent` emitter is behind the `native_protected` early exit. Native-region promotion separately copies and emits frames and scroll content, but never field content. Using `boot_native_region` with an input carrying `padding=10` produces no editor-rectangle operation; the native presenter falls back to the platform text rectangle and loses the authored inset.

   **Fix:** Capture and emit `field_content_rect` from the selected immutable publication during promotion, including clearing it. Test the resulting batch, beyond the kernel’s stored rectangle.

8. **Oversized line height produces the wrong baseline — material.**  
   [kernel/src/layout.rs:1098](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout.rs:1098)

   Clamping the centering offset to zero violates D5’s baseline formula when the line exceeds the content height. With `font-size=16 line-height=5 height=40`, the 80-point line requires an offset of −20; the kernel uses zero. Baseline-aligned siblings are positioned 20 points too low relative to the centered native text.

   **Fix:** Preserve the negative centering offset and reconcile the painted-host calculation. Test an authored line height larger than the content box.

**What is right**

Lowering keeps buttons, excluded input types, and explicit bare fields outside the new default. Platform fonts enter computed style through `Env`, chrome probes run on the main thread, and field content geometry uses sparse storage.

Static audit at `08e7e2ff4`; no builds or tests executed, and no repository files changed.
