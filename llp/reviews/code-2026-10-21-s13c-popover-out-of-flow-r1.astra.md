# Code review: An open popover is out of its parent's flow (kernel, landed under the main-red permission), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `22f93142e6`.
- **Method:** one brief (sha256 `747d379de3a3281caa3c817e47765bfade3f83df7399ebec5b8e677dc513e95f`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in e22015495e/fa4343f928 (the scroller walk, sticky, coverage); the containing block QUEUE'd

VERDICT: WRONG

1. **should-fix — [kernel/src/kernel/cover.rs:280](/tmp/iosfix-rv-oof1/kernel/src/kernel/cover.rs:280).** `scroller_insets_bottom` still treats an uncovered popover as in flow because it checks authored `position_type`. With `[scroller, popover]`, opening the popover makes this walk miss the inset scroller and restore the route’s bottom padding, shrinking the scroller. UIKit’s updated `inFlow` disagrees. Update this predicate and test a popover **after** the scroller; the updated TabEdge fixture places its menu before it.

2. **should-fix — [kernel/src/style.rs:1490](/tmp/iosfix-rv-oof1/kernel/src/style.rs:1490).** Absolute positioning removes flow participation but retains the authored containing block. [hoist.rs:32](/tmp/iosfix-rv-oof1/kernel/src/layout/hoist.rs:32) stops at a positioned parent. An auto-sized flex item containing a popover can now collapse when the popover leaves flow, then constrain that popover’s automatic width to its minimum-content width. iOS and painted macOS consume that size and only reposition it; context previews inherit it. Top-layer sizing should use the viewport/initial containing block, or an independent host block. Native NSPopover already has the latter. [CSS top-layer rules](https://drafts.csswg.org/css-position-4/#top-layer-styling).

3. **should-fix — [kernel/src/style.rs:1321](/tmp/iosfix-rv-oof1/kernel/src/style.rs:1321), [kernel/src/kernel/sticky.rs:91](/tmp/iosfix-rv-oof1/kernel/src/kernel/sticky.rs:91).** A `position: sticky` popover becomes absolute only after lowering has erased its insets. It also still receives a sticky constraint, which Apple’s presenter applies as an additional scroll translation after lifting. Coercing relative/sticky to absolute is correct for the top layer, but the inset lowering and sticky machinery must agree with that effective position.

4. **nit — [kernel/src/kernel/cover_tests.rs:203](/tmp/iosfix-rv-oof1/kernel/src/kernel/cover_tests.rs:203).** The regression test covers initial creation with a fixed height. Add retained-node set/clear coverage for the new transaction branches and a content-sized popover; neither behavior is asserted here.

No separate sizing regression found in native NSPopover, native pull-downs/confirmations, Linux/Android’s independent surface sizing, or dialog-only sheets. Terminal consumes the changed kernel geometry; web wasm delegates layout to the DOM. Static review only; no tests run or files changed.
