# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `4f4e76812b`.
- **Method:** one brief (sha256 `a400a580cc7cc4ab304e34ae506556a07f2d2f0787037a55f46a39a3992d0565`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in db94c10a4d (derive scope, text_says, native iOS button's aria-current)

VERDICT: WRONG

1. **should-fix** — A surrounding binding drops a state the look only reaches through a derive. `contract/lower/src/anatomy/library.rs:756`

`expand` (`library.rs:651`) follows derives with an empty binding list, so `derive lit = selected` still means the prop when a `for` or `match` binds the name `selected`. `reads` already skips a direct use of a bound name. The filter on line 756 then throws away that expanded prop whenever the button sits inside `for selected` / `match … some selected`. This passes, and it should not:

```contract
derive lit = selected
for selected in items
  button aria-label="Row" opacity=(lit ? 1 : 0.5)
    box width=4 height=4
```

The opacity tracks the prop. The loop variable never hides `lit`. Cycles are fine: a name is pushed once, and a derive cycle is already `type-derive-cycle` before this lint.

2. **should-fix** — `text_says` does not apply the bindings, the one-arm rule, or `aria-hidden`. `contract/lower/src/anatomy/library.rs:802`

`mentions` honors the `bound` it is given. `text_says` never pushes a `for` variable or a `match` arm (`:811` and `:814`), and it accepts a text in either arm (`||`) or an `aria-hidden` text (`:809` checks only the positional). A button whose look reads the prop `selected` is treated as having said it when the only text reads a loop variable of that name, or when that text is in one arm or hidden. `texted` (`:789`) is the stricter rule and does not have this hole: a text in one arm names nothing, and a sibling text outside an `if`/`else` still names the button, because `texted` is `any` over the children and the `&&` applies only to the two arms.

3. **should-fix** — A native iOS button never takes the selected trait from `aria-current`. `host/apple/Sources/ExactKit/IOS/NativeButtonsIOS.swift:259`

`NodeViewIOS.swift:1173` maps a non-false `accessibilityCurrent` only inside `actsAsButton`. A native button is `kind == "control"` (`NativeButtonsIOS.swift:153`), so it never enters that branch. Its trait is `accessibilitySelected == "true"` alone, then written at `:276`. A plain `button` with `aria-current="page"` lowers as that native control (the fixture in `contract/cli/tests/it/aria.rs`). VoiceOver does not hear it as selected. `BarItem` and `HeaderButton` are custom (`svg` face, and `BarItem`'s press scale), so they do go through `NodeView`.

No separate finding for the two host cases you named. A `link` is `kind == "button"` with `accessibilityRole == "link"`: it keeps `.link`, loses `.button`, and gains `.selected` when `aria-current` is not `""` or `"false"`. A node that is not `actsAsButton` never touches `.selected`. A checked radio still gets `.selected` from `setAccessibilityChecked` after this line. Mac is unchanged and still serves `AXARIACurrent`, with `"false"` as none.

What holds: `BarItem` sets `aria-current=(selected ? "page" : "false")` on every item (`bar-item.contract:45`), drops `hold` / `contextmenu`, and says `selected` through that attribute (the path `d` is the look). `"false"` stays the string `"false"` and is not the selected trait (`CurrentTraitIOSTests`). `HeaderButton` keeps `id` and `testId` both bound to `id`. Dot corner is `radius.full`; the badge corner stays `6px`. Against D0–D9 both are exported authored buttons (D9's exception, not `tabs` / `header`): token knobs, parts matching their classes, `BarItem`'s slot once and not inside `if`/`for`/`match`, no platform read. The agents doc matches the hold and `aria-current="page"` notes. It does not say a derive counts, which is the same hole as finding 1.
