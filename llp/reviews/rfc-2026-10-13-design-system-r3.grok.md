I'll review the round-3 RFC against the architecture it cites and the round-2 findings, without editing anything. First I'm reading the draft, its disposition of earlier findings, and the documents it depends on.The round-2 nested-pin fix holds on the JS target, and the scheme split, state order, local write capability, and DTCG tuple export are in the text. The kernel rule those pins depend on still selects the wrong slice, and several cited mechanisms do not do what this draft says.

**RIGHT WITH CHANGES**

## BLOCKER

**1. D7. Inherited text under a pin still keeps the ancestor's slice.** `llp/1121-design-system.rfc.md:432-439`, `:482-483`

The JS-target rule is the right one: the boundary element re-specifies `color: var(--x-…)`, and the tuple rule sets the whole slice (`:468-479`). The kernel rule does not match it. It re-selects only when a node's tuple differs from its parent's. Inheritance does not stop at the parent. `copy_inherited` walks through ancestors that do not have the row and copies the ancestor that does (`kernel/src/arena.rs:598-614`).

Page Black, pin Dim, text inside the pin: the text node's parent is the pin, so the tuples match and nothing re-selects. The walk copies the page's Black value. That is the stage 2 gate (Black inside Dim, inherited text, on iOS as well as the web).

The wasm host is the same hole. `css_style_of` emits the node's specified style (`host/web/src/element.rs:633`), and the browser then inherits a computed colour. A selection that exists only in a transient computed snapshot never reaches CSS. "The wasm host gets the kernel's path" (`:482-483`) is not true of that emitter. Stage 2 names that host.

**Fix:** re-select when the node's app tuple differs from the tuple of the ancestor that specified the token, and write that value onto the mode-boundary element so CSS emitters and descendants both see it. Use that one rule on the kernel, the wasm host, and the JS target.

## MAJOR

**2. D6. The font path cited does not leave weight and leading unsaid.** `llp/1121-design-system.rfc.md:386-390`

`font()` writes a numeric weight and `line-height: normal` (`contract/lower/src/shorthands.rs:303-308`). LLP 1115.000 D12.2 says that is the current behaviour, and that an unsaid weight and leading stay with the text style so the kernel can resolve them per platform (`llp/1115.000-no-tells-on-every-host.rfc.md:562-565`). `relative.rs` resolves size only. A type token lowered through this function snapshots one weight and marks it written. The stage 4 gate ("Bold Text still weights a text-style type token", `:861`) also misses D12.3: a bundled family such as the example's `InterVariable` is left alone (`:391-393`).

**Fix:** D12.2 lands first. Lowering sets only the longhands the token writes; weight and leading it omits stay unsaid. The gate bolds a system-font token, including one with an authored weight, and does not bold a shipped family.

**3. D5/D7. A token that does not vary has no lookup key.** `llp/1121-design-system.rfc.md:355-358`, `:407-408`

Rows are "one per distinct value", keyed by the axes that value holds for. The kernel selects by the node's full tuple. In the example, `text.primary` has no `when`, so its only row is the default tuple; a Dim node will not find it. The CSS side avoids this by emitting every full tuple. The kernel side does not say how a partial key matches.

**Fix:** store every full tuple, repeating a value axes do not change, and look up by exact tuple. The 64-tuple cap is already the bound.

**4. D5. Three appearance previews are not three `-exact-mode` pins.** `llp/1121-design-system.rfc.md:369-375`, `:847-849`

`dark-style` is `black | dim` under dark. Light is `color-scheme`, not a value of that axis. A Dim pin on a light page selects the dim row, whose value is `light-dark(shared light, dim dark)`, so the preview stays light. The gate calls these "three `-exact-mode` previews".

**Fix:** a light preview sets `color-scheme="light"`; Dim and Black set `color-scheme="dark"` and the mode. Say a mode pin does not change scheme.

**5. D12. A Swift colour does not follow `setMode` or a pin.** `llp/1121-design-system.rfc.md:657-664`

Scheme and contrast are traits, so a dynamic `UIColor` re-resolves. The app tuple is not a trait. `observe` is specified for fonts, symbols, and the Kotlin `Color` int, not for that `UIColor`. The stage 3 gate says `ExactDesign(view).color(…)` follows `setMode` and a pin above the view (`:853-855`).

**Fix:** the provider reads the place's tuple on each resolve, and the binding invalidates the view on `setMode`, a pin change, and an editor swap. Trait changes alone do not.

**6. D3/D7. A token inside a row expression is not slice-selected.** `llp/1121-design-system.rfc.md:297-302`, `:432-435`

The kernel model is one token beside the row. `color="color-mix(in srgb, brand 60%, transparent)"` is allowed, and `brand` may differ by tuple. Nothing says that mix is re-folded per node, and a device must not gain an evaluator.

**Fix:** lower a row expression to a synthetic token, folded per tuple at build, so the row stays one reference.

**7. D3. A D4 role override copied into `app.design` stops being an override.** `llp/1121-design-system.rfc.md:271-274`, `:235-238`

`roles.rs` does not treat a bare Exact role as a colour (`kernel/src/style/roles.rs:13-17`). This draft overrides `-exact-secondary-label`. LLP 1095 D4's key is `secondary-label` (`llp/1095-platform-colours.rfc.md:350-360`). The inline block stays D4. In `.design`, that same key is an ordinary token.

**Fix:** accept D4's bare key and the spelled form as the same override, in both files.

**8. D2. The shared lexer cannot tokenize the example.** `llp/1121-design-system.rfc.md:232-234`, `:797`, `:813`

D2 says the grammar is shared. The contract lexer has no `|` (`contract/syntax/src/lexer.rs:55-56`), so `black | dim` is `syntax-unexpected-char`. `-exact-` is an identifier only as an attribute name followed by `=` (`:226-245`); `-exact-body/1.35` in a value is a negation.

**Fix:** a design lexer, stated as an extension: `|`, `..`, and vendor names in value position. Do not call the contract lexer unchanged.

**9. D11 contradicts D7 on what a build keeps.** `llp/1121-design-system.rfc.md:634-635` against `:426-430`

D7 keeps every token when bindings exist, unless `exports` bounds them, and keeps a whole kind when a computed string may name one. D11 says a package keeps only tokens a plan references, and cites D7.

**Fix:** one retention rule. Package tokens used from a binding or a computed name stay, under the same `exports` bound.

**10. D7. A scheme pair with a modern half is not `LightDark`.** `llp/1121-design-system.rfc.md:359-361`, `:415-420`, `:810`

`LightDark` holds two 8-bit colours (`kernel/src/style.rs:700`). A pair with a wide half is `Wide` (`kernel/src/style/wide.rs:8-14`, `llp/1100-color-spaces-and-hdr.rfc.md:160-162`). The example's `accent.fill-pressed` is `light-dark` of two `oklab` mixes. Folding that into `LightDark` clips it and changes the transition (`kernel/src/motion.rs:267-278`). The quantize sentence (`:787`) can be read as doing that clip.

**Fix:** both halves legacy means `LightDark`; otherwise one `Wide` with `dark: Some` and the `modern` flags parsing that CSS would set. Quantize only the legacy form.

**11. D13. The watcher cannot see the "own write" mark.** `llp/1121-design-system.rfc.md:690-693`

`host/web-js/dev.mjs` rebuilds and reloads on any non-skipped write (`:109-127`). `app.design` is not skipped (`:26`). The JS loop is a separate process from the design server. An in-memory mark does not cross that boundary, so a value edit still reloads. The stage 7 gate says it must not (`:877`).

**Fix:** a signal that process already reads: a digest it skips, or the write and the watch in one process. Ignore only that write.

## MINOR

**12. D5. `design-under-light` has no split for a role or a `platform-color`.** `llp/1121-design-system.rfc.md:358-361`

Those values have no author-written light half (`llp/1095-platform-colours.rfc.md:186-187`). Refuse them on an `under scheme=dark` axis, or define the rewrite. A `when` value that is a single colour, meaning the dark half only, needs the same rule.

**13. D8. `border-color` in a state part is a shorthand.** `llp/1121-design-system.rfc.md:516-519`

The kernel stores four sides; `border-color` is the shorthand (`kernel/tables/schema.json:3925`). Expand it before the per-row order, and say how a state shorthand meets a base longhand.

**14. D3. A `when` on `duration` or `easing` is not a named refusal.** `llp/1121-design-system.rfc.md:291`

The grammar allows `when` on every token. Substitution has no tuple to pick. Refuse it (`design-motion-axis`).

**15. Header. Stage 2 still has no date.** `llp/1121-design-system.rfc.md:16`

`rules/RULES.md:58` wants an implementer and a date. "When accepted" is the gate, not the date.
