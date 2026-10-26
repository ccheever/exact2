# Code review: the terminal taps a pressable box (QUEUE), round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `cbaf1291a6`.
- **Method:** one brief (sha256 `4c018c5ead5014b8bac089bda2c1bb114386629a611e946a913f44b2a4d12bc7`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (3 SHOULD-FIX, 1 NIT).
- **Disposition:** each taken in the next amend (the commit message's "Round 1").

**Verdict: SHOULD-FIX.**

1. **SHOULD-FIX — [paint.rs:222](host/terminal/src/paint.rs:222): pressable containers swallow inline handlers.** Inline text targets live in `presented.runs`, but `click()` consults those only when `hits` finds nothing. Adding a container’s rectangle therefore makes `column press=outer` containing a paragraph’s `text "open" press=inner` execute `outer` when clicked. Previously, the inline handler worked. Resolve boxes and inline runs in compatible paint order.

2. **SHOULD-FIX — [pointer.rs:231](host/terminal/src/pointer.rs:231): visible ancestors admit invisible descendants.** `on_screen()` walks upward until any ancestor has a nonempty hit rectangle. The newly listed pressable containers therefore make their scrolled-out or `display="none"` descendants pass this check. Agent `tap` can activate an unseen button; `type` can focus and reveal an unseen field; `wheel` can pass the same incorrect guard. Validate the target’s visibility and clipping before accepting its ancestor.

3. **SHOULD-FIX — [paint.rs:219](host/terminal/src/paint.rs:219): hidden and disabled surfaces become actionable.** The new predicate ignores `visible`, `disabled`, and inert ancestry. A `box press=… visibility="hidden"` is registered despite painting nothing; disabled or inert boxes likewise reach their handlers. Neither `press_with_inner()` nor runner dispatch rejects these presses. Apply eligibility checks without making disabled surfaces activate an enclosing control.

4. **NIT — [screen.rs:760](host/terminal/tests/it/screen.rs:760): slot click coverage is overstated.** `filled` and `slotted` receive only addressed taps, which bypass coordinate hit selection. The mouse checks cover only `card text`, `tap`, and `inner`. Click both slot targets and assert their independent effects. The added test otherwise uses deterministic, synchronous rendering.

Ordinary child ordering and modal hit clearing look correct. Tab traversal and physical wheel routing use separate paths. This terminal emits `Press` for activation, without dispatching pointer, hover, or double-click handlers; listing those would not implement them.

`handlers_of` uses indexed lookup and scans only the node’s handlers, allocating for nonempty results. The closure adds one allocation per scene. No quadratic walk; timing remains unmeasured.

No live test, doc, or gate caller references `marker-toggle`; the mentioned `stage12-gate.mjs` is absent.

Validation: Cargo and the gate’s IR test were blocked by filesystem permissions; the parser test passed. Tree unchanged.
