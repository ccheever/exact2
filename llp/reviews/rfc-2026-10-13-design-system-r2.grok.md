I'll review the revised design-system RFC against the round-1 findings and the architecture it cites, starting with the RFC, the prior reviews, and the decisions it claims to follow.The draft and both round-1 reviews are in the tree. I'll read the revised RFC and the prior findings next, then check them against the cited decisions.The revision claims every round-1 finding is taken. I'll check those claims against the colour, motion, schema, and host code they cite.The round-1 resolution split is fixed: the kernel picks an app-axis slice, and scheme stays inside the value. The web rules that are supposed to implement that slice still don't.

**RIGHT WITH CHANGES**

Checked against LLP 1095 D1, D4, D10, D11 and §5 item 6, LLP 1034 D2 and §8, LLP 1115 and LLP 1115.000 D6 and D12.3, LLP 1061 D1, `ColorValue`, `relative.rs`, `roles.rs`, `disabled_look.rs`, `schema.json`, and `host/web/dev.mjs`. These round-1 items are actually taken: precedence, closed residuals, `setMode` versus `setScheme`, disjoint tuples, per-use invalidation, separate shadow kinds, pruning, `disabledStyled`, `accent-color` before new roles, Android Views, the 76 symbol roles, no Facet spacing scale, bindings unblocked from the editor, DTCG 2025.10, `brand`/`card`, the grammar header, and a colour-only stage 2. What follows is still wrong.

## BLOCKER

**1. D5/D7. Subtree modes still don't work on the web, and inherited colour ignores the pin on every host.** `llp/1121-design-system.rfc.md:411-415`

The heading says a boundary carries its whole slice. The next sentence emits one rule per tuple that sets only tokens whose value differs from the default. Custom properties inherit. A nested pin whose slice value equals the default does not redeclare that property, so it keeps the ancestor's override. That is the settings-screen gate in §5 stage 2 (`:748-749`): root in Dim, inner pin back to Black, `--x-surface-page` stays Dim.

Separately, `color` inherits as a computed value. A pin that only changes `--x-text` does not recolor text whose `color: var(--x-text)` was specified outside the pin. The same hole is on the kernel path: slice selection writes the parent's chosen `ColorValue` into the parent's row (`:379-382`), and `computed_inherited` copies those bytes (`kernel/src/arena.rs:590-594`). A child under another `-exact-mode` does not re-select. Scheme still works, because a `LightDark` pair has both sides (`llp/1034-scheme-aware-colour.rfc.md:382-383`). App axes do not.

`var()` inside an unregistered custom property is substituted when that custom property is computed (`--x-text: var(--exact-label)` snapshots the role on the boundary element). Registering `--x-*` as `<color>` would also resolve `light-dark()` there. "Folded value" (`:414`) can be read as scheme-flattened, which would throw away the `light-dark()` encoding D5 just specified (`:319-322`).

**Fix:** one canonical tuple serialization, declaration order, exact-match selectors. Every boundary rule sets the whole slice, each value still a `ColorValue` (`light-dark()`, `var(--exact-<role>, …)`), properties unregistered. Inherited token colours stay references and are slice-selected at the using node; on the JS target, every such node specifies `var(--x-…)`, including text that inherited it. Do not describe that write as the `rem` pixel write (`kernel/src/style/relative.rs:5-11`).

## MAJOR

**2. D5. `under scheme=dark` is only defined for colours.** `llp/1121-design-system.rfc.md:319-322`

Those rows are rewritten to `light-dark(default light, axis dark)`. A length, number, or type `when` on `dark-style` has no value of that form. The kernel then picks the slice without knowing the scheme (`:379-380`), so a non-colour would change in light mode too. Stage 4 allows every kind to vary by any axis (`:758-761`).

**Fix:** refuse a non-colour `when` on an `under scheme=dark` axis in r2 (`design-under-kind`).

**3. D6. Bold Text is tied to an unsaid weight.** `llp/1121-design-system.rfc.md:344-346`

LLP 1115.000 D12.3 weights the system font even under an authored weight, and leaves a shipped font file alone (`llp/1115.000-no-tells-on-every-host.rfc.md:567-571`). A type token that writes `600` on `-exact-body` must still bold. The same weight on `"InterVariable"` must not. The stage 4 gate only checks the unsaid case (`:760`).

**Fix:** say that. Weight stays unsaid so the text style's own weight applies; Bold Text is the accessibility pass on the system font, not a consequence of omitting the weight.

**4. D3. Role-override names don't match the lexer, and `AccentColor` is not how switches are tinted.** `llp/1121-design-system.rfc.md:236-237`, `:488-491`, `:702`

`role()` accepts `-exact-<name>`, CSS system colours, and `-apple-system-*`. An Exact role's bare name is not a colour (`kernel/src/style/roles.rs:13-17`). `secondary-label` will not override `-exact-secondary-label`. `AccentColor` is also a CSS colour keyword, which the same paragraph refuses.

Switches, ranges, and progress read the `accent-color` row (`host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:901`, `RangeIOS.swift:23`, `host/linux/src/paint/control.rs:155`). `AccentColor` follows that row (`follows_accent`, `roles.rs:178-186`), not the reverse. LLP 1095 D4's theme entries are colours (`llp/1095-platform-colours.rfc.md:357-360`); calling the inline block "the same grammar" (`:199-201`) quietly adds `when` lines and other kinds.

**Fix:** override keys are the author-facing names (`-exact-secondary-label`, `AccentColor`). A role name is the exception to the keyword refusal. Switches are coloured by a root `accent-color` token. A role override only changes what paints that role, within the reach ruling (`llp/1095-platform-colours.rfc.md:812-816`). Inline `theme` stays D4's colour entries until the stage that adds the other kinds.

**5. D8. `:disabled` and `:checked` still have no winner, and the web can implement the wrong `:active`.** `llp/1121-design-system.rfc.md:449-467`

Host overlays have an order. Both runner-known states are "folded into the base," and both can set `background-color`. A checked disabled control is the normal case. Class versus node, for the same state and row, is also unstated. "Has `checked`" is true of `checked=false`.

`:active` is defined as LLP 1061's press (`llp/1061-press-feel-and-reduced-motion.rfc.md:118-127`). A stylesheet `:active` matches ancestors and nodes with no `press` handler. The web already tracks `data-pressed`.

**Fix:** per row, node beats class; among runner states, `:disabled` then `:checked` then the base. `checked` means the boolean is true, and the aria values mean `"true"`. On the web, `:active` is the existing press glue, not a CSS `:active` rule. Say whether a disabled node still shows `:focus-visible`.

**6. D3. `duration` and `easing` have no row.** `llp/1121-design-system.rfc.md:252`

`transition` and `animation` are composite codecs (`kernel/tables/schema.json:3925-3926`): property, time, delay, easing, and the rest. There is no duration row to lower into. Keyframes still refuse colour references (`:396`).

**Fix:** a token replaces one component inside the shorthand (`transition="background-color duration.fast easing.standard"`), substituted before that codec runs. Until that substitution exists, don't list the kinds as rows.

**7. D12. Returned fonts, symbols, and Kotlin colours go stale.** `llp/1121-design-system.rfc.md:577-579`

`observe` fires on `setMode`, update, reload, and editor swap. It does not fire on scheme, contrast, Dynamic Type, or Bold Text. A Swift colour is a dynamic provider, so it is fine. A font "for now" and an Android `Color` int are not. Trait changes are already how LLP 1095 D5 re-resolves.

**Fix:** the same listener fires on those trait changes. Kotlin's listener too, not only `setMode`.

**8. D13. The write capability is still deliverable to a LAN client, and `set-state` is outside the digest.** `llp/1121-design-system.rfc.md:596-611`, `:613-618`

The dev server binds loopback unless `--lan`, and a LAN or `--allow-host` peer may read what the server sends. The installer token is deliberately not among that (`host/web/dev.mjs:8-15`). A capability "given to the editor it serves" is given to whoever can load that page. The gate only checks Host and cross-site Origin (`scripts/install-page.mjs:274-294`).

`set-state` edits a Contract `style` through `contract fmt`, but the precondition is the design digest. Two state edits do not conflict. The batch description rewrites "the changed custom properties" (`:611`), not every tuple rule from finding 1, so another mode's preview stays stale.

**Fix:** deliver the capability only on a loopback response, mode `0600` under `.exact/`, never in a `--lan` or `--allow-host` body. `set-state` preconditions the Contract file's digest. A value edit rewrites every tuple rule that contains the token, then one style update.

## MINOR

**9. D10. Android does not draw arbitrary Material Symbols.** `llp/1121-design-system.rfc.md:523`

The reader paints the symbol table's SVG paths. The Up arrow is a special case (`host/android/README.md:824`, `:733`). A `material "…"` source has no consumer. The SVG fallback is what Android will draw.

**Fix:** say that. Keep the `material` name as an optional annotation, not as a source the host already resolves.

**10. D14. The export is not valid DTCG for icons, and `em` is not a root length.** `llp/1121-design-system.rfc.md:645-652`

Format 2025.10 requires `$value` and `$type`. "Extensions only" will not validate. An `em` depends on the element, so "px at the default root size" is the wrong loss. Re-import has to compare the `$extensions` source, not the resolved `$value`, or the stage 3 gate (`:757`) fails as soon as a fold is lossy. State the modifier cross product (each app axis × scheme).

**11. D5. The worked example's default mode is Dim.** `llp/1121-design-system.rfc.md:300`, `:696-704`

The first axis value is the default, and the example writes `dim | black` while the base colour is the black palette. It only comes out right if a `when` beats the base. `setMode` of a colour-only slice is also "laid out and painted" (`:325-326`), which contradicts D7's per-use domains and the stage 2 gate that a Dim switch does not relayout (`:749`).

**Fix:** `black | dim`, base value = default tuple, and `setMode` lays out only when a selected value is in a layout row.
