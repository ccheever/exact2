# Code review: the terminal taps a pressable box (QUEUE), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1e1cb58bd6`.
- **Method:** one brief (sha256 `633401a2692ff48e5037a155a16091090b9172310ced7a5713e9cefe7a5ff1d6`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX).
- **Disposition:** both taken ("Round 2").

**Verdict: SHOULD-FIX.**

1. **SHOULD-FIX — [paint.rs:230](host/terminal/src/paint.rs:230): disabled and inert surfaces remain actionable.** Round 1’s eligibility finding is only partially fixed. The new predicate checks visibility but neither `disabled` nor inert ancestry. A visible `column press=post disabled=busy`, or a pressable box inside `inert=true`, enters both `shown` and `hits`. Agent taps and mouse clicks reach `press_with_inner()`, which dispatches without rejecting either condition. Suppress these activations without making disabled children activate enclosing controls. The added tests cover hidden boxes, but not these cases.

2. **SHOULD-FIX — [pointer.rs:89](host/terminal/src/pointer.rs:89): plain inline labels now bypass button activation behavior.** For an `appearance="none"` button containing a paragraph with styled inline text, `press_run()` finds the **button’s ancestor handler**, even when the run owns no handler or link. It then calls `press_with(run, false)`: the button does not take focus, and `invoke()` receives the text node’s key, so the button’s `commandfor`/popover action is skipped. Previously, clicking this label activated the button directly. Only let an inline action/link intercept; otherwise retain the original hit. The new focus test uses ordinary text and misses this regression.

By inspection, the other round 1 fixes work: clipped/hidden box targets fail `on_screen`, inline handlers beat enclosing boxes, slot fills receive coordinate clicks, and pressing a tall box avoids focus-driven scrolling. Child hit ordering and modal clearing remain correct.

`handlers_of` performs an indexed lookup and scans only that node’s handlers; nonempty results allocate. It runs for visible non-controls, with no whole-tree handler scan. Timing remains unmeasured. Pointer, hover and double-click handlers should not count here: the terminal does not dispatch them.

Removing `marker-toggle` has no live script/doc dependency; slot toggles remain. The new tests are synchronous and show no timing-flake mechanism.

Validation: Cargo and the gate’s IR test were blocked by filesystem permissions; the gate parser test passed. Tree unchanged.
