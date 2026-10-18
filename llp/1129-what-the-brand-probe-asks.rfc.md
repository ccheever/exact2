# LLP 1129: What the brand probe asks

**Type:** RFC
**Status:** Accepted as built, 2026-10-18, after four review rounds (Astra and Grok; Grok's first stopped on its turn budget, its notes read). D5's warning is set aside (D5.2). Charlie asked for it ("write up an LLP with your plan of how to fix these things … then when you are happy with the plan, go ahead and implement").
**Systems:** the web's neutral look (`host/web/neutral.css`), the web role sheet (`kernel/src/style/roles.rs` `theme_css`), the web element marks (`host/web/src/element.rs`), the `list-detail` pattern, four apps' fields, `docs/recipes/tamber.md`, `docs/contract-for-agents.md`.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** the author, from 2026-10-18
**Related:**
- The probe: `QUEUE.md`'s theme entry ("The probe ran 2026-10-16") and its record, `~/audits/exact2-no-tells-2026-10-11/theme-proof/probe-20261016/` (`grade/grade.md`, `fern-fig/DIARY.md`).
- LLP 1115 (write the web, ship the platform); 1115.000 D4 part B (the neutral look), D6 (press feedback), D4.2 (the browser's own focus ring); 1116.005 D8 (an invalid field's solid red ring).
- LLP 1121 (Tamber), D8 (state parts) and stage 8; LLP 1095 D1 (a theme's accent is the app's tint) and D4 (the web's reach).
- [CSS Basic User Interface 4, `outline-style: auto`](https://www.w3.org/TR/css-ui-4/#outline-style) (an engine may draw `auto` as it likes); [CSS Pseudo-Elements 4, highlight styling](https://www.w3.org/TR/css-pseudo-4/#highlight-styling); [HTML, implicit submission](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#implicit-submission); Apple, [`UIFocusHaloEffect`](https://developer.apple.com/documentation/uikit/uifocushaloeffect) (drawn in the view's tint, outside it).

## Summary

One model built the same branded app, Fern & Fig, in Exact (with Tamber) and in React; a
blind grader preferred React, 8 to 6 overall and 9 to 8 on the brand. Tamber held every
colour in both schemes. Exact lost points in two places: the brand did not reach the
states Exact draws itself, and a form did not save on Enter. This RFC fixes the parts that
are Exact's own defaults, says what the builder could not find, and names what waits on
Tamber's stage 8. Then the probe runs again.

## 1. What lost points, and whose it is

| Grader's finding | Cause | Owner |
|---|---|---|
| A button the author filled has no hover | The unsaid hover of a custom pressable is a 4% wash of the look's fill (`--exact-quaternary-fill`, black or white); over a dark brand fill it cannot be seen | Exact: D1 |
| The focus ring is the browser's blue | The theme's accent reaches controls but not the ring | Exact: D2 |
| Selected text is the browser's blue | The theme said `Highlight` and `HighlightText`; the role sheet declares them and nothing draws `::selection` with them | Exact: D3 |
| The sheet's corners are 12 px, not the brand's 14 | Unsaid on the route. Said, both hosts take it: iOS (`ModalIOS.swift`, `preferredCornerRadius`), and the web, whose author classes (two ids) outrank the look's card rule (one); checked 2026-10-18. The builder did not find where to say it | Docs: D4 |
| Enter in the add sheet does not save | `submit` exists; the builder copied the `list-detail` starter, whose title field does not use it, and nothing said so | Exact: D5 |
| `verify` reported stored data lost after a relaunch | Its relaunch opened another origin | Fixed on main before this RFC (`c0a0edc11`): D6 |
| A row's hover and press are a neutral grey | The wash is the brand's card darkened 4% (`#FFFDF8` to `#F5F3EE`); a wash of the brand's ink moves it by under 1%. What the grader wanted is the author's own hover | Tamber stage 8 B (§3) |
| Generic validation; mixed date formats; a new plant shown overdue; the name twice | The builder's code | Not Exact's |

## 2. Decisions

### D1 — A button the author filled hovers as the filled button does

A `button` (no `href`, no `buttonStyle`) whose author wrote a `background-color` that is
not transparent takes the filled look's hover where a pointer hovers: a wash of its own ink
(`currentColor`) at the filled hover's share (12%, the `button-hover-filled` metric), as an
image over the author's fill. Any other custom pressable keeps the 4% wash. Press stays
LLP 1115.000 D6's dim; a disabled button takes no hover.

- **Why:** the look's states are "a wash of the control's ink" (`neutral.css`'s header); a
  filled control's ink is its text. On the accent fill (`#2F6B3F`, white ink) 4% black is
  invisible; 12% of the ink is the step Exact's own filled button shows.
- **Where:** `element.rs` adds `background-color` to the button's `data-exact-said` words
  (merged with the words already there). `neutral.css`'s rule is the unsaid hover's
  selector with that word added, so it outranks the 4% rule by one attribute.
- **Limit:** a fill computed at run time (`background-color=(on ? "accent" : …)`) is not
  seen when the mark is made and keeps the 4% wash, as today. The JS target writes such a
  row at run time with no mark; carrying one is a follow-up if a probe shows it matters.
- **Stage 8 B's handoff:** the unsaid washes are `background-image`s, which an authored
  `:hover { background-color }` does not replace. Tamber D8 already says a declared state
  replaces the host's default for that state; on the web that means stage 8 B's `:hover`
  look clears `background-image` (or marks the node so the sheet's two hover rules step
  aside), the way an authored press replaces the dim.
- The web only: an iPhone has no hover, and an iPad's pointer effect is UIKit's.

### D2 — The focus ring is drawn in the theme's accent

When an app's theme replaces `AccentColor`, the web's keyboard ring is a solid 2 px line in
the accent, at the offset the shell already gives each control (2 px outside a button or a
field, inside a list's row). An invalid field keeps its red ring; an authored `outline`
(`none` on a field) still wins.

- **Why:** a theme's accent is the app's tint (LLP 1095 D1), and iOS draws its focus halo
  in the view's tint, outside the view, so an iPad with a keyboard already shows the
  brand's ring. Solid, not the browser's `auto`: Chrome draws `auto` in an `outline-color`
  but ignores its offset and hugs the control, so in the accent it vanished on a control
  filled with the accent (checked 2026-10-18); Firefox and WebKit draw `auto` in their own
  colour whatever is said. A solid line looks the same in all three, as the invalid ring
  already does (LLP 1116.005 D8).
- **Where:** `theme_css` writes `outline: 2px solid var(--exact-accent)` on
  `#exact-root#exact-root :where(:focus-visible:not([aria-invalid=true]))`: two ids and
  nothing else, so above the shell's `outline: revert` (one id) and below an author's class
  (two ids and a class), whose `outline="none"` still wins (checked in Chromium and
  Firefox). The bar's Back part takes the same ring. A platform button the accent fills
  (`filled`, `bordered-prominent`), to which the shell gives no offset, is given 2 px, so
  its ring does not sit on its own fill.
- Without a theme the ring stays the browser's, and so it does under an author's own
  `accent-color`, as CSS has it.

### D3 — Selected text is drawn in the theme's `Highlight`

When a theme replaces `Highlight` or `HighlightText`, the web draws `#exact-root
::selection` with `var(--exact-highlight, Highlight)` and `var(--exact-highlight-text,
HighlightText)`. Unsaid, no `::selection` rule is written, since one changes how an engine
draws an inactive selection.

- **Why:** they are CSS's names for the selection's colours, Tamber admits them, and a
  theme that says them means them.
- **The web only.** On iOS the selection is drawn in the tint, which `AccentColor` sets;
  `Highlight` alone does not reach it.

### D4 — Where a sheet's corners are said

No code change. `docs/recipes/tamber.md` says that a sheet's corners are `border-radius`
on its route, on iOS and the web.

### D5 — A field whose Enter should act, and does not, is named

1. **The starter teaches it.** The `list-detail` pattern's title field says `submit=save`
   in place of `enterkeyhint="next"`: Return and Enter save, as a form does on the web (Save is still guarded by the title and by `pending`). The trade:
   on iOS Return no longer moves to Notes, which is optional; the notes `textarea` keeps
   Return as a new line. A field cannot do both: on iOS `submit` runs before Next.
2. **Not built: a warning.** `lower-field-no-submit` was to name a one-line field whose
   Enter does nothing beside a button whose action uses its value. It was built and
   reviewed for four rounds and set aside at the fix-loop limit (`rules/RULES.md`): its
   read set came from printed expression trees, and each round found another scope it
   misread (a binding that hides a state, a parameter an action only tests, a route arm
   inside a route box). Built again, it reads uses from the checker's scoped analysis
   (the analyzer's slot reads). The patch and the four rounds are in
   `~/audits/exact2-no-tells-2026-10-11/theme-proof/llp1129-field-no-submit/`; `QUEUE.md`
   holds it. Before it was set aside it found the apps below.
3. **The apps it found.** Shelf's Pages, Exact Live's crew note, Messages Stress's
   composer and Markdown Stress's link field say `submit` with their button's action,
   each of which guards itself. The design tool's two forms are left to its lane: their
   actions rely on the button's `disabled`, which `submit` does not consult; and
   RealWorld's sign-in, whose one button is Sign up or Sign in by a state, wants an action
   of its own for Enter (`QUEUE.md`).

- **Why:** on the web, a form does its button's work on Enter, and React's builder got it
  from `<form>` without asking. Exact has no `form`, so the default cannot be inferred:
  the starter teaches it, and a warning would name it at build.
- **Not:** inferring the action, a `form` element, or Enter moving to the next field on
  the web.

### D6 — `verify`'s relaunch opens its test's origin

Already on main (`c0a0edc11`). The re-probe (§4) shows whether the warning is gone.

### D7 — The Tamber recipe says how a brand reaches states and controls

`docs/recipes/tamber.md` gains one paragraph: `AccentColor` for the tint and the ring
(D2), `Highlight` and `HighlightText` for the selection (D3), a filled button's hover
(D1), and a sheet's `border-radius` (D4). And the builder's first stumble: a button in the
brand's own shape is a face the author writes on a `button` without `buttonStyle`; a
`buttonStyle` is the platform's control, which takes the accent and refuses a radius
(`lower-button-style`), and `ButtonText` and `LinkText` are the tint's, which a theme
cannot replace (`lower-theme-tint`).

## 3. What this does not do

- **Authored state parts on the hosts.** `:hover`, `:active` and `:focus-visible` compile
  (LLP 1121 stage 8 A) and no host draws them yet; that is stage 8 B, in Tamber's lane,
  with D1's handoff above. It is the lever for the rows' hover, and the re-probe will say
  how much is left for it. The recipe does not mention them until they draw.
- **`outline-color` and a `::selection` part in Contract.** The theme's accent and
  `Highlight` say the brand's ring and selection (D2, D3); a per-node colour has no
  counterpart on iOS, where both are the tint.
- **Per-field validation messages.** The builder's choice; the recipe's pattern (Save
  disabled with its reason) is the platform's.

## 4. Gate

1. Tests: the neutral look's test hovers a filled button in Chromium, Firefox and WebKit
   and finds its own ink at 12%; a `theme_css` unit test for D2 and D3 (nothing without a
   theme's word); the pattern's own tests on the web, Enter included.
2. Driven: the probe's app, copied without its web hatch and built on this change, in
   Chromium, Firefox and WebKit: the filled buttons hover in their ink, the ring and the
   selection are the brand's green (WebKit's Tab skips buttons, as Safari does), a field's
   `outline="none"` still wins, a `filled` platform button's ring stands 2 px off its fill,
   and a route's `border-radius=14` gives 14 px corners.
3. The six checks; `cargo test -p exact-web-js --bins`; `bun host/web-js/conform.mjs`.
4. **The probe again**, by its protocol (same prompts, `gpt-6.1-sol` at high, a blind
   grader, Exact at the commit that lands this), beside the first in
   `~/audits/exact2-no-tells-2026-10-11/theme-proof/`. Success is no point lost to D1–D6's
   causes; what remains goes into `QUEUE.md`, with what stage 8 B would take.
