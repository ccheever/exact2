# Code review: a disabled pressable its author styles is not dimmed (LLP 1115.000 D6), 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4b77f6bf525fd774b62d42595b1ad2a278ac52f15f3538057e95331b54d3f2c5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both reviewers' MAJOR (bound names counted as reads) and Grok's (the border shorthands) taken in round 2's code.

---

## MAJOR

**A `border` shorthand that carries the disabled colour never marks the node — `contract/lower/src/disabled_look.rs:22`**

`PAINT` is the longhands `border-color` and `border-*-color`. The attributes authors write for that colour are the shorthands `border`, `border-top`, `border-right`, `border-bottom`, and `border-left` (`contract/lower/src/shorthands.rs:8`). `styled` runs on the attribute list in `prepare_element` before `bind_shorthand` splits those into `BorderColor*` rows (`contract/lower/src/lib.rs:945`). So this stays unmarked and still gets the host dim:

```
button press=go disabled=busy border=(busy ? "1px solid #ccc" : "1px solid #06f")
  text "Save"
```

The expression does read `busy`. The same is true of a side shorthand. `background` is in the list, so that shorthand is fine. Add the five border shorthands to the paint set, or decide after the shorthand has been split.

**A binder spelled like another name suppresses the dim — `contract/lower/src/disabled_look.rs:51`**

The intersection uses `idents` (`contract/lower/src/warnings/screens.rs:320`). That walk collects every identifier in a `match` arm (`:339`), an arrow body (`:329`), and a `let` body (`:349`), and it does not drop the binder. After inlining, a component prop is the same argument expression at every read, so a real prop, slot, or derive still matches. A shadowed spelling matches too:

```
button press=go disabled=(match user { some(u) => u.locked, none => true })
             background-color=(match theme { some(u) => u.accent, none => "#eee" })
  text "Save"
```

Both sides "read" `u`. The node is marked, the Mac and the web skip the dim, and the colour does not follow `locked`. The same happens for `map` / `filter` parameters (`item => …`) against a state named `item`. Freshening (`x@bN`) applies only inside derive bodies, not to these view expressions. Free names are still a static decision: subtract binders while walking, which is what "a slot, a derive, a prop, a local" already says.

## Checked, and holding

- **Hosts that dim a custom pressable.** The web sheet (`host/web/index.html:84`) and the Mac (`PressLookMac.swift:102`). iOS sets `alpha` from the presented opacity only. Linux's 0.45 fade is the native button face (`NodeType::Control`), which the web already excludes with `data-button-style` / `data-native`. Android, Windows, and the terminal painter do not fade a custom pressable. `neutral.css` fades native inputs and switches, the control's own look.
- **Web cascade.** A marked button fails `:not([data-disabledstyled])`, so the sheet rule does not compete with an inline opacity or a JS-target class. Bool props are the string `"true"`, and the wasm host writes `data-disabledstyled`.
- **Mac alpha.** `syncDisabledLook` stores the dim in `disabledShown` and later `opacity` presents multiply only while that flag is set (`PresenterMac.swift:1207`). A mark already applied leaves the flag clear, so an authored opacity that arrives afterwards is stored as itself. Setting the mark later restores `presentedAlpha`.
- **Schema.** `disabledStyled` is id 265, unused before. `SCHEMA_DIGEST` moved, and the only embedded plans (`InlineFrameMacTests`, `NativeFieldsIOSTests`, `WebDocumentsTests`) moved with it. The JS target learns the DOM name through `prop_name`, which reads the same `data-disabledstyled` fallback. `disabledstyled` is reserved in `dataset.rs`.
- **`style=` and pseudo-states.** There is no element `style=` bag, and Contract has no `:hover` or pressed selectors. `class=` is expanded before `styled`. A colour that reads only a hover or pressed name does not share `disabled`'s names, which is the right call.
- **`aria-disabled`.** The web dim is `:disabled` and the Mac dim is `props["disabled"]`. `aria-disabled` does not fade the control, so leaving it unmarked matches the dim.
- **LLP 1115.000 D6.** The amended paragraph matches the rule that was implemented, including literal `disabled` and which hosts read the mark.

NOT READY
