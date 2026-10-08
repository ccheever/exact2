# Review: LLP 1104 r6, 2026-10-06 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, `-C` the worktree at `856ee5cdb`.
- **Method:** a full brief (sha256 `6044f668763f88d85fb01567af71e1bb0ea2989c043fdc0b87847f85e98c9520`); blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited (codex printed it twice; one copy kept).
- **Verdict:** NOT READY.
- **Disposition:** folded into r7; see LLP 1104 §7.

---

**Verdict: NOT READY.**

The direction is sound. The blockers are unresolved classification rules and an incomplete connection between native typography, kernel layout and focus ownership.

## Findings

1. **Native typography must enter computed style before layout** — **blocking** — **D4–D5**

   Returning a default font from the measurement hook is insufficient. The kernel currently inherits typography through ancestors ([arena.rs:559](/Users/ccheever/projects/exact2-wt-1104/kernel/src/arena.rs:559)), resolves `em` lengths during commit ([relative.rs:55](/Users/ccheever/projects/exact2-wt-1104/kernel/src/txn/relative.rs:55)), and sends computed inherited rows to Apple ([style.rs:823](/Users/ccheever/projects/exact2-wt-1104/host/apple/src/style.rs:823)).

   For example, a native field with `padding="1em"` inside a 32px parent must resolve padding against its control font. A font discovered only during measurement arrives after that decision. This also threatens web-target parity: the JS target preserves relative units, whereas ordinary wasm CSS emission uses resolved values ([css.rs:32](/Users/ccheever/projects/exact2-wt-1104/host/web/src/css.rs:32)).

   **Change:** Specify a resolved control text style that participates in inheritance, relative-unit resolution, text measurement and presentation. Define the reset mask—including line height—and how clearing an authored row restores the platform default. Trait changes must invalidate resolved lengths and text/layout caches, not just the chrome cache.

2. **The conditional devolve rule tests the wrong invariant** — **blocking** — **D2**

   A conditional disabling row does not necessarily imply a native/bare transition. A permanent background already makes the node bare; a conditional border cannot change that. Likewise, classes that supply a background in one branch and a border in the other are always bare.

   Class expansion synthesizes separate conditional expressions by attribute name, including `none` branches ([class.rs:46](/Users/ccheever/projects/exact2-wt-1104/contract/lower/src/class.rs:46)). The existing border-shorthand/class test demonstrates overlapping declarations ([visible_fields.rs:165](/Users/ccheever/projects/exact2-wt-1104/contract/cli/tests/it/visible_fields.rs:165)). Conversely, a binding is not necessarily always present: the runner clears optional values and unset expressions ([instance.rs:885](/Users/ccheever/projects/exact2-wt-1104/runner/src/instance.rs:885)).

   **Change:** Classify the complete effective declaration set as always native, always bare, or potentially mixed, after resolving classes, shorthands and overrides. Refuse only mixed/unknown cases that could change representation. Define conditional child handling similarly, and distinguish an outer `when` selecting two nodes from conditional content within one node. CSS’s rule concerns surviving author-origin declarations, not syntactic row occurrence. [CSS UI §7.2.1](https://drafts.csswg.org/css-ui-4/#appearance-disabling-properties)

3. **D3 contradicts the adopted button refusals** — **blocking** — **D2–D3, D9**

   “A row that doesn’t devolve … goes to” the native control conflicts with 1069.011.001 D12, which still refuses `box-shadow`, `filter`, `font-family`, `direction`, `press-scale` and other non-disabling rows. A default button with only `box-shadow` has two incompatible specified outcomes.

   There is another collision: `buttonStyle="filled"` plus an authored background devolves without error under D2, but D1 preserves `buttonStyle`’s native-only restriction; lowering currently rejects it on a bare button ([controls.rs:77](/Users/ccheever/projects/exact2-wt-1104/contract/lower/src/controls.rs:77)). Finally, D2’s “third child” example incorrectly rejects a title, subtitle and symbol—the three-child face expressly admitted by 1069.011.001 D3.

   **Change:** Give one ordered admission policy for explicit `none`, explicit `auto`, and inferred `auto`. Scope D3 to the admitted rows, or explicitly amend D12’s remaining refusals. Specify the error for native-only attributes surviving devolution. Use the adopted face grammar rather than a child-count shortcut. Run the migration classifier against these complete rules, including handlers and contexts—not just styling.

4. **The field measurement response does not establish correct geometry** — **material** — **D3, D5**

   The claim that chrome metrics depend only on kind, control size and traits is unsupported when authored fonts and field sizes vary. Native text rectangles are bounds-dependent APIs; the RFC needs evidence that the proposed invariant insets remain valid. [UITextField documentation](https://developer.apple.com/documentation/uikit/uitextfield)

   The baseline also needs an explicit rule. Today layout reports the measured text baseline plus the top padding/border ([layout.rs:1030](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout.rs:1030)). Adding a native minimum height or an authored tall field can introduce vertical centering that this calculation does not represent.

   **Change:** Specify the box-model equation, treatment of explicit/min/max dimensions, editor text rectangle and resulting baseline. Probe multiple font sizes/families and constrained heights. Cache only metrics proven independent of those inputs; otherwise extend the request/key. Define textarea and secure-field metrics separately where necessary.

5. **Replacing the ring does not migrate UIKit focus** — **material** — **D6**

   An iOS bare pressable cannot acquire a system halo merely by setting `focusEffect`. Exact currently excludes `NodeView` and native buttons from its focus-item search ([FocusSearchIOS.swift:31](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/FocusSearchIOS.swift:31)); Tab selects a first responder and manually draws its ring ([PresenterIOS.swift:448](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:448)). A halo decorates a UIKit focus item. [UIFocusHaloEffect](https://developer.apple.com/documentation/uikit/uifocushaloeffect)

   Moving native-button focus also leaves existing activation code trying to focus the wrapper node ([NativeButtonsIOS.swift:73](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/NativeButtonsIOS.swift:73)). Focus, blur, collection pinning and programmatic focus have similar assumptions.

   **Change:** Define one node-to-focus-owner mapping used by traversal, actions, activation, collection retention and event routing. Explicitly reconcile UIKit focus with first-responder focus for bare pressables. Require tests for `tabindex`, programmatic focus/blur, `retainFocus`, canceled key activation, removal of a focused control and exactly one activation.

6. **Bare web buttons lose their keyboard focus indicator** — **material** — **D6, D8**

   D8 retains `all: unset` for bare buttons, while D6 deletes `button:focus-visible`. That combination removes the UA outline. The current restoration is at [index.html:48](/Users/ccheever/projects/exact2-wt-1104/host/web/index.html:48), following the reset at line 36. Most existing app buttons remain bare under this proposal, so this is a broad regression.

   **Change:** Retain a scoped `:focus-visible { outline: auto; … }` restoration for reset controls. This still lets the browser draw focus. Test explicit `none` and automatically devolved controls as well as native ones. CSS UI specifically calls for restoring focus indicators after `all: unset`. [CSS UI §7.2.2](https://drafts.csswg.org/css-ui-4/#appearance-effects)

7. **The reset split exceeds D1’s stated scope** — **material** — **D1, D8**

   D8 changes all `input` and `textarea` elements without `appearance="none"`, although D1 excludes several input types and Markdown editors. Today the blanket reset also affects checkbox, radio, range, file and date inputs; their later rule restores only selected properties, not the whole UA stylesheet ([index.html:36](/Users/ccheever/projects/exact2-wt-1104/host/web/index.html:36), [index.html:54](/Users/ccheever/projects/exact2-wt-1104/host/web/index.html:54)).

   Markdown additionally starts as a real textarea before replacement with a contenteditable element ([markup-editor.js:99](/Users/ccheever/projects/exact2-wt-1104/host/web/markup-editor.js:99)). An unscoped reset change can alter that initial editor and its transition.

   **Change:** Specify selectors and emitted appearance markers for precisely D1’s domain. Preserve excluded controls’ existing behavior, or explicitly amend their contracts and add regression cases. Cover the Markdown loading and source-mode paths.

8. **The newly included hosts lack complete control contracts** — **material** — **D5–D7, §4**

   The terminal’s D7 paragraph specifies only a field look. Its painter has no `Control` rendering branch ([paint.rs:195](/Users/ccheever/projects/exact2-wt-1104/host/terminal/src/paint.rs:195)), while kernel `Control` children are not laid out ([node.rs:135](/Users/ccheever/projects/exact2-wt-1104/kernel/src/node.rs:135)). Making buttons native therefore requires a terminal face renderer and measurement path, not merely relocating field colours.

   tvOS also needs a textarea carve-out: its existing text views are read-only ([TextAreaIOS.swift:109](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:109)). And “bare pressables keep their tvOS treatment” preserves an Exact-drawn ring ([NodeViewIOS.swift:334](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:334)), contradicting §4’s blanket requirement for no Exact ring on Apple.

   **Change:** Define terminal button measurement, face painting and keyboard behavior; drive Todo and Harness. State tvOS multiline editing availability and either retain its existing bare-control ring as an explicit exception or replace it deliberately.

9. **The verification plan cannot prove two central claims** — **material** — **§4**

   The proposed launch film is not a recording of initial presentation. `film()` takes screenshots interleaved with virtual-clock advances after attachment and refuses platform timing ([agent.mjs:1307](/Users/ccheever/projects/exact2-wt-1104/scripts/agent.mjs:1307)). It can miss the first-frame jump entirely.

   JS-versus-wasm conformance also cannot establish correct UA behavior when both share the changed stylesheet: the JS build explicitly copies it ([build.mjs:421](/Users/ccheever/projects/exact2-wt-1104/host/web-js/build.mjs:421)).

   **Change:** Add a cold-cache presentation assertion that no provisional layout is presented, plus cache-miss tests when controls appear after launch. Compare fields against independently configured UIKit/AppKit controls and plain browser elements without Exact’s reset. Include mixed conditional styles, relative units, bare-control focus and the excluded input types.

10. **The six-day estimate omits required dependency stages** — **material** — **§6**

   The stated prerequisite is 1069.011.001 steps 1–2. But that RFC puts web/Linux in step 3, macOS in step 4, and invokers, symbol rows, padding/radius, disabled behavior and pointer events in step 5 ([1069.011.001:274](/Users/ccheever/projects/exact2-wt-1104/llp/1069.011.001-native-button-fidelity.rfc.md:274)). Those are all consumed by r6’s “unchanged” button contract.

   **Change:** Mark the estimate explicitly incremental to completion of every consumed dependency stage, with ownership identified. Otherwise include those stages. Re-estimate the kernel work after specifying typography resolution and invalidation, and include terminal application drives; the present half-day kernel and painted-host entries do not account for those paths.

11. **Correct the field typography premises** — **minor** — **§2, D3–D4**

   The built default field’s colour does **not** inherit: lowering injects `color` itself ([fields.rs:127](/Users/ccheever/projects/exact2-wt-1104/contract/lower/src/fields.rs:127)). D3’s macOS `font` mapping does not implement arbitrary `letter-spacing`; that needs attributed-text/editor handling or a declared stand-in. Chrome also overrides textarea’s font family to monospace after its common control-font rule. [Chromium UA stylesheet](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/html/resources/html.css)

   **Change:** Correct these statements and separate the field table’s font and tracking rows. Also enumerate the actual CSS disabling properties rather than `background-*`: §7.2.1 does not include every background property.

## What is right

- The native-default direction, shared literal switch and explicit platform differences follow the owner’s ruling.
- Keeping fields on the text-measurement path preserves the right basis for content sizing, rows and baselines.
- Removing r4’s compiled chrome and consolidating measurement with the button work are sensible.
- Real app drives, light/dark fixtures and explicit stand-in reporting are appropriate acceptance requirements.
