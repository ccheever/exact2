# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e957e19862`.
- **Method:** one brief (sha256 `9020e676f29bb143d7fa0ccfb3d90f24f612d81737d5df365a95c30e3bac8825`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

The kernel and the sheet now treat a grouped-list row that carries `aria-selected` as a choice in every form that compiles, and the only behaviour that changes is the title colour. A choice row stays a button.

1. **Kernel.** `grouped_row` is called on the section group's own child (`kernel/src/grouped.rs:335`, `:353`), so `row.props` is that row. A `button` row's props are the button's. `aria-selected` lowers to `accessibilitySelected` (`contract/lower/src/tags.rs:695`), a bool prop. A false value is stored: the bridge turns `Value::Bool(false)` into `PropValue::Bool(false)` and emits `SetProp` (`runner/src/bridge.rs:195`, `runner/src/instance.rs:1046`). Nothing drops a false bool as a default. `PropList::contains` (`kernel/src/props.rs:156`) is the right test. The ruling is presence, and `== Some(true)` would leave an unselected row (`false`) an action. The new test's `aria-selected=dark` starts false (`boot` sets `state dark = false`) and still has `action == false`, with no checkmark.

2. **Sheet.** `styled` reads the row element it was given (`contract/lower/src/grouped.rs:788`), after component inlining (`contract/lower/src/lib.rs:360`, `:731`) and after `-exact-row-look` is stripped (`row` at `:781`). Those attributes are copied onto the lowered node (`:1080`), and `aria-selected` becomes the prop binding. Contract has no attribute spread. A component prop is substituted only where the body names it; a prop named `aria-selected` reaches the row only as `aria-selected=<that name>` on the button, which both classifiers then see. A `style` cannot carry it (`accessibilitySelected` is not styleable; only `buttonStyle` is). A literal, a binding, and `aria-selected=(cond)` are all an attribute whose value must be a bool (`contract/lower/src/values.rs:1057`), so the prop stays set for every evaluation. The two classifiers agree on those forms.

3. **Other uses of `action`.** The flag is serialized on the grouped-list record (`host/apple/src/abi/commands.rs:504`) and read in one place: the iOS/tvOS title tint (`host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1084`). The VoiceOver button trait follows `pressable` (`:1112`), which this change leaves set. Web, Mac, and Linux paint the sheet colour. A choice row keeps its press, its button trait, and its checkmark; it loses the accent title.

4. **Existing look.** No grouped-list row button in `apps/`, `examples/`, `packages/`, `scripts/fixtures/`, or `host/web-js/conformance/` carries `aria-selected`. The attribute shows up on tab bars and segmented controls (`apps/shelf/app.contract`, `apps/native-fixture/app.contract`, `scripts/fixtures/grouped-list.contract`'s style picker, the conformance tab fixtures). Those rows are not action cells. The checkmark rows in `scripts/fixtures/grouped-list.contract` (`dark`, `system`) do not set `aria-selected`, so their colour stays as it was.

5. **Tests and style.** `a_row_that_says_it_is_selected_is_a_choice_not_an_action` matches the file: the shared `boot` harness, kernel `action`, accessory, and sheet colour, for a false binding with the checkmark held back, a literal `false`, a literal `true`, a real action row, and the same row after the binding flips. The new conditions sit in the existing `&&` chain and the existing `attrs.iter().any` shape, and the comments match the surrounding voice.
