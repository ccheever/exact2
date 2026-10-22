# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `fad694e0ee`.
- **Method:** one brief (sha256 `9b0b93a7a8ce9347ada6b12ec14b6d3884ad48172a4182030507d24db685aabf`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken: derives, bindings, hold (theme2 06:40); the message names role=button nodes; HeaderButton's id as its testId declined (navigationBack)

**VERDICT: RIGHT WITH NITS**

The direct look paths hold, and both components meet D0–D9. `aria-selected` on BarItem’s button is what theme2 accepted: iOS applies the selected trait from it on any button (`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1171`). Nothing in today’s exported packages is refused. The adopting screens are not in `27c1f461f4..fad694e0ee`, so this review does not confirm the pixel claim.

Checked by reading the walker (tests were not run). A state read through `class=(selected ? A : B)`, an expression `match`, or a `?:` / `match` inside a used component’s arguments is recorded (`library.rs:724-733` calls `reads`, and `anatomy.rs:799-811` sets `inside` for both). Buttons inside `for` / `if` / `match` are visited (`library.rs:652-663`). `role="button"` written with a `?:` is detected (`library.rs:645-646` via `leaf_texts` / `theme.rs:939-941`). HeaderButton’s `small` and `lead` are outside `STATES` (`library.rs:616-618`). BarItem’s `children` sits once, outside `if d != ""` (`bar-item.contract:53-57`). `selected` is read only on the path `d`, so it is not a D5 variant; HeaderButton’s two bools are three cells; 20 + 4 = 24.

1. **should-fix** — A label or a look that reaches a state prop only through a component `derive` is invisible. `control` has the component (`library.rs:599`) and never passes `c.derives`. `mentions` (`library.rs:751-754`) and `looks` (`library.rs:726`) only scan the attribute expression. Anatomy runs on the parsed file (`contract/lower/src/lib.rs:535`), before derives are inlined (`lib.rs:552`), and there is no surface `let`. So `derive name = selected ? "On" : "Off"` with `aria-label=name` is refused when the look also reads `selected` (false positive), and `derive on = selected` with `opacity=(on ? 1 : 0.5)` is accepted (false negative). The docs say “a label that reads it” (`docs/contract-for-agents.md:3758-3761`). The test (`contract/cli/tests/it/library_rules.rs:668-704`) only uses a direct `aria-label` ternary.

2. **should-fix** — `looks` does not carry `for` / `match` bindings. It calls `reads` with an empty bound set (`library.rs:726`) and walks `Each` / `Match` arms without pushing the loop variable or the `some` name (`library.rs:734-743`). `walk_conditions` does push them (`anatomy.rs:870-890`). A bool prop named `on`, `current`, `active`, or `selected` is then blamed for a `?:` that only reads the binding of the same name. No package hits this today.

3. **should-fix** — `hold` is a required prop and is always `contextmenu=hold` (`packages/actions/bar-item.contract:35`, `:47`). The web host attaches a `contextmenu` listener whenever that handler exists (`host/web/glue.js:568`) and then `preventDefault` plus `stopPropagation` before the action runs (`host/web/input-glue.js:706-709`), unless the target is a text-field edit menu. A no-op `hold` still swallows the browser right-click. iOS adds a long-press recognizer on the same condition (`NodeViewIOS.swift:241-246`). Four of five bar items typically have no menu; the comment describes `hold` as the Profile account switcher.

4. **nit** — The unnamed-control diagnostic names the element tag (`library.rs:686-688`), so `row role="button"` is reported as a `` `row` `` without a name. The test only asserts `"without a name"` (`library_rules.rs:676`).

5. **nit** — HeaderButton writes one prop to both attributes (`packages/actions/header-button.contract:28-29`). `id="back"` still reaches `navigationBack` (the web id attribute and the iOS `id` prop). A caller cannot also set a distinct `testId`, which ComposeFrame’s cancel does (`id="back"`, `testId="compose-cancel"`). On iOS the accessibility identifier becomes `"back"` as well.

A structural `if selected` (ListRow’s checkmark) and a bare argument `Icon(filled=selected)` are also unseen. Both sit outside the stated `?:` / `match` scope. The badge string is `aria-hidden` under an `aria-label` (`bar-item.contract:44`, `:59-60`), so the count is announced only when the caller puts it in `label`.
