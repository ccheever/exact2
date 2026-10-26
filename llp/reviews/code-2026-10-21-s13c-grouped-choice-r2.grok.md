# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9bded1c9a5`.
- **Method:** one brief (sha256 `e6b32eb899f8116996253da403e95d39f20396b0147cb0de9c8dc17826826d9c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

The two round-1 follow-ups are in this commit. The test lives in `contract/cli/tests/it/grouped_action.rs` (214 lines; `grouped_list.rs` stays at 1496). `unmarked` is an `aria-selected=true` row whose only child is the title, and the test asserts it is not an action and that its title is the label colour.

1. **Kernel.** `row` in `grouped_row` is the section group's child, so `row.props` is that element's own list. A grouped `button` is that element. `aria-selected` lowers only to `accessibilitySelected` (`contract/lower/src/tags.rs:695`), and a bool binding is `SetProp` of `PropValue::Bool`, including `false` (`runner/src/bridge.rs:195`, `kernel/src/txn.rs:708`). Nothing strips a false bool, so `contains` at `kernel/src/grouped.rs:500` is true for a literal, a binding, and `aria-selected=(cond)` while the value is false. Presence is the right test: the ruling is that any value makes the row a choice. `== Some(true)` would leave an unselected row as an action cell.

2. **Sheet.** `styled` runs on the expanded tree, and `attrs` there are that row element's own attributes (`contract/lower/src/grouped.rs:799`, `850`). Contract has no attribute spread, and `accessibilitySelected` is not a styleable prop, so a class cannot carry it. A component prop named `aria-selected` is an attribute on the button only when the component body writes it; inlining leaves that name in place, and both classifiers see it. A literal, a binding, and `aria-selected=(cond)` all keep the attribute name in the source and the prop on the node at runtime. I did not find a legal form where one classifier now treats the row as a choice and the other still treats it as an action.

3. **Other uses of `action`.** The flag is serialized on the grouped-list model and read in one place: `GroupedListIOS.swift:1084`, which tints the title only when `action` is true and the author set no title colour. The VoiceOver button trait uses `pressable` (`GroupedListIOS.swift:1113`), which this commit leaves set, so a choice row stays a button. Android, web, and Linux do not read `GroupedRow.action`; the web title colour is the sheet's `color`.

4. **Existing look.** No grouped-list row button in `apps/`, `examples/`, `packages/`, or the conformance fixtures carries `aria-selected`. The `aria-selected` buttons in those trees are tabs or segmented controls (headers, tablists), including Shelf's `StatusControl`, whose grouped-list child is the `tablist` row and whose buttons sit inside it. `scripts/fixtures/grouped-list.contract` draws its checkmark rows with `if` and no `aria-selected`, so this commit does not recolour them.

5. **Tests and style.** `a_row_that_says_it_is_selected_is_a_choice_not_an_action` covers a false binding with no mark, a literal `false`, a checked row, the unmarked selected row, a plain action row that stays an action, and the same row after it becomes checked. It checks both `GroupedRow.action` and the title colour. The wording matches the surrounding tests.
