# Code review: disabled look round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `65e0054078a3d4bc211edfcc50d11abe4914e6ef8300ec7ce3aaaa7cfd7b40cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY (Astra); Grok's run ended without a verdict.
- **Disposition:** Astra's MINOR (the shadowed case not asserted) taken in round 3's code.

---

## MINOR

**A grouped-list sheet colour the author replaces still marks the node.** `contract/lower/src/disabled_look.rs:50` treats any paint attribute in the expanded list as the node's look. For a grouped row, `contract/lower/src/grouped.rs:916` writes `color` from `by_flag` (`grouped.rs:699`), which copies a dynamic `disabled` expression into a ternary. `grouped::split` (`grouped.rs:214`) renames that `ua:color` to `color`, and `contract/lower/src/nodes.rs:66` keeps the sheet row in front of the author's attributes. A class row the node itself sets is dropped first (`contract/lower/src/class.rs:33`); this sheet row is not. The later binding wins, so

```
button disabled=busy color="#111"
```

inside `listStyle="inset-grouped"` shows `#111` and is still marked, because the losing sheet colour reads `busy`. The web then skips the half-opacity dim. A dynamic `disabled` with no authored `color` is marked too, which is the right look (the sheet already paints the tertiary label). A literal `disabled=true` collapses to a static string and is not marked, so that same row stays at half opacity.

## NIT

- **`kernel/tables/schema.json:1544`** says "border colour" and stops. The rule and LLP D6 also count `border` and the four side shorthands, and they ignore a callback parameter and a `match` binding.
- **`host/apple/tests/ExactKitTests/PressFeedbackMacTests.swift:234`** never presents an opacity after the mark. The multiply is `PresenterMac.swift:1207`, and it uses full alpha while `disabledShown` is false (`PressLookMac.swift:101`). The unmarked test at line 212 covers the multiply; the marked path does not.
- **`host/web-js/conformance/disabled-styled.contract:4`** says the agent's tree reports each button's opacity. `host/web-js/conform.mjs:152` compares depth, type, test id, text, value, label, checked, handlers, and focus. The plan is wired (`synthetic-disabled-styled` from the file name). It does not measure the dim. Computed opacity is covered for the shared sheet by `host/web/tests/press.test.mjs:304`, and the JS emit carries `disabledstyled` once (`host/web-js/src/emit_tests.rs:316`).

## Checked, not findings

The rule is decidable after inlining. `contract_types::check_all` expands the root before lower, and a prop ident is replaced by the argument expression (`contract/syntax/src/inline/subst.rs:185`). Two attributes that both named the prop still meet on the argument's free names. A capture rename (`name@k`) stays a binder, so it does not count. A literal argument is the literal-`disabled` case: only an `opacity` marks it. There is no surface `let`; the compiler's `Expr::Let` reads its value outside the binder and its body under it.

There is no `style="…"` attribute and no `:hover` or `:disabled` style block. `style Name` is a class, expanded before this rule; an `opacity` in that class marks, and a conditional class keeps the condition in the ternary. The only pseudo-element is `::placeholder`, and its colour is not in the paint list. `aria-disabled` is `accessibilityDisabled`. The web's `:disabled` and the Mac's `props["disabled"]` do not treat it as disabled, and those hosts do not dim on it.

The half-opacity dim of a custom pressable is the web sheet and the Mac's `syncDisabledLook`. iOS dims while pressed (`platformPressDims`) and uses alpha 0.4 on native switch and radio chrome (`ControlsIOS.swift:80`), not on a custom pressable. Linux and Windows share `button_control` (`host/linux/src/paint/button.rs:381`), which fades a native button's accent (0.45) or uses `GrayText`; that face is `buttonStyle`, which the web rule already excludes. Android's canvas has no such multiply.

The web rule is `#exact-root button:disabled:not(…, [data-disabledstyled])` (`host/web/index.html:84`), specificity (1, 2, 1), and it sets only `opacity`. An authored colour does not compete with it; the mark removes the rule. An authored opacity wins without the mark: inline on the wasm host, and `#exact-root#exact-root .cN` at (2, 1, 0) on the JS target (`host/web-js/src/emit.rs:734`). Both targets share that sheet (`host/web-js/build.mjs` reads `host/web/index.html`). A marked button still matches `:disabled`, so it takes no press. `press.test.mjs:304` sees opacity 1.

On the Mac, `presentedAlpha` starts at 1 (`NodeViewMac.swift:201`). Bool props arrive as `"true"` (`host/apple/src/host.rs:1449`). Whether the opacity present runs before or after `syncDisabledLook`, a marked node keeps `disabledShown` false and a later opacity is stored whole; clearing the mark restores `presentedAlpha * 0.5`.

`disabledStyled` is prop 265. The digest pin is `0xcee3e61c678f8914` (`kernel/src/wire/codec.rs:715`). The three embedded Swift plans change only the digest prefix. Other plans use `SCHEMA_DIGEST`. The JS target gets `data-disabledstyled` through `style::prop_name`, which diffs the wasm template (`host/web-js/src/style.rs:313`); `rt.js` `P` sets it with `setAttribute`. `disabledstyled` is reserved in `dataset.rs`, and an author attribute `disabledStyled` fails as an unknown attribute, the same way `listRows` does. LLP D6's decision paragraph matches the code, including the border shorthand and the binder exception. The wave-1 as-built rows at `llp/1115.000-no-tells-on-every-host.rfc.md:732` are the 2026-10-11 measurement log.

A name is the root ident, so `account.locked` and `account.tint` meet on `account`. A call contributes its arguments, not its name; a `fn` body cannot read state except through those arguments (`ast.rs:501`). A descendant's paint is invisible. D6 states the rule that way, and it is the smallest static one.

READY
