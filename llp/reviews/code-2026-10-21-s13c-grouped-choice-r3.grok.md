# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 3, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9636e63edd`.
- **Method:** one brief (sha256 `fb39af4d9526eba5ab0ccbbc7e421c7cdf9631af6e93fa251f01eb721d791c1a`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

1. **Kernel.** `accessibilitySelected` is a bool prop with no default, and the only contract attribute that lowers to it is `aria-selected` (`contract/lower/src/tags.rs:695`). A literal, a name, or `aria-selected=(cond)` all become a prop binding. Evaluating it, including to false, is `SetProp` with `Bool(false)` (`runner/src/bridge.rs:195`, `runner/src/instance.rs:1046`). `ClearProp` is only for a kind mismatch or an explicit unset, and a non-bool does not type-check. `contains` (`kernel/src/props.rs:157`) is the right test: `== Some(true)` would treat an unselected choice as an action again. `row` in `grouped_row` is the section group's direct child (`kernel/src/grouped.rs:335`, `kernel/src/grouped.rs:353`), so `row.props` is that button's own props.

2. **Sheet.** `styled` runs on the expanded tree (components already inlined, props substituted by name). There is no attribute spread. A prop named `aria-selected` reaches the button only if the component writes that attribute, and after substitution the name is still `aria-selected`. Classes cannot set it (`accessibilitySelected` is not styleable). So attribute presence and prop presence match for a literal, a binding, and `aria-selected=(cond)`. A `for` body is styled by `over` (`contract/lower/src/grouped.rs:283`) and is the row at runtime, with no wrapper. A swipe styles the inner row through `row(content, true)` (`contract/lower/src/swipe_rows.rs:374`) while the kernel's row is the scroll, which is custom and already `action: false`. Both stay off the accent.

3. **Other readers of `action`.** The flag is only the title tint in `GroupedListIOS.swift:1084` (iOS and tvOS). The VoiceOver button trait is `pressable`, not `action` (`GroupedListIOS.swift:1113`); a choice row stays a button. Mac, web, and Android paint the sheet color, which this commit updates the same way. Nothing else in the compiler writes `accessibilitySelected`.

4. **Existing apps and fixtures.** None change. `aria-selected` in apps, examples, packages, and conform fixtures is on tabs, segments, or calendar cells, not on a grouped-list row. `scripts/fixtures/grouped-list.contract:90` still uses a conditional checkmark with no `aria-selected`, so those rows stay action cells when the mark is hidden. That is outside this ruling.

5. **Tests.** `a_row_that_says_it_is_selected_is_a_choice_not_an_action` covers a false binding, a literal false, a checked row, an `aria-selected=true` row with no checkmark, a real action row, and the same row after it becomes selected. It checks both `GroupedRow.action` and the title color. The wording matches the rest of `grouped_action.rs`.
