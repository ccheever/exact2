# Code review: The params section (LLP 1121 stage 7 A), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD (`559f4da32`), the params section.
- **Method:** one brief (sha256 `90262a03f8b9ef133618bc5f00603dc0db93317d0aff48e289771f05dc4a8b5d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: text arguments and non-row props rewritten, calls not followed, quoted names and fragments rewritten; P1s: package aliases, positional icons, icon aliases, describe's fold, IR discovery, TS types, refusals naming the hash, icon names reserved, lean, SVG sources, the short-name rule, FIELD).
- **Disposition:** in the first-round commit, each with a test (`contract/cli/tests/it/tamber_params_scope.rs`, `tamber_params.rs`, `params/rewrite.rs`). A param in a style, keyframes, another component's argument or a state is refused (`tamber-param-scope`) rather than followed to its consumers; a param in a package is named by the package's declared name.

---

1. **P0 — Component arguments corrupt ordinary text.** [params.rs:139](/tmp/rv-s7a-r1/contract/lower/src/params.rs:139)  
   Declare `fill: color = "#fff"`, use it in `background-color="param.fill"`, and call `Label(label="param.fill")`, where `Label` renders its string prop with `text label`. The emitted text becomes `param.<hash>.App.fill`. Confirmed with the matching compiler. Non-style attributes such as `placeholder="param.name"` also trigger unwanted rewriting or refusal. **Fix:** identify actual token-consuming rows, following props to their consumers, and preserve strings consumed as text.

2. **P0 — References inside function calls remain unresolved in accepted programs.** [params.rs:101](/tmp/rv-s7a-r1/contract/lower/src/params.rs:101)  
   Define `fn id(x: string): string = x`, declare `fill: color = "#fff"`, and render a column with `background-color="param.fill"` containing `text "x" color=id("param.fill")`. Compilation succeeds, but the function receives the unchanged `"param.fill"` while the table contains only the hashed token. Without the direct background read, compilation incorrectly reports `tamber-param-unused`. State/derive initializers have the same omission. **Fix:** traverse the complete value flow into rows, and use that traversal for reachability too.

3. **P0 — The replacement changes quoted CSS names and fragment references.** [params.rs:75](/tmp/rv-s7a-r1/contract/lower/src/params.rs:75)  
   With a declared `fill` param, `font-family="\"param.fill\", system-ui"` becomes a different font-family name; `fill="url(#param.fill)"` becomes a different fragment reference. Both transformations were confirmed. Template text is also rewritten without respecting adjacent interpolation holes. **Fix:** recognize CSS token references lexically, preserving quoted strings, fragment identifiers, and partial words across template boundaries.

4. **P1 — Package aliases make identity depend on import order.** [sources.rs:202](/tmp/rv-s7a-r1/contract/cli/src/sources.rs:202)  
   Link `node_modules/a` and `node_modules/b` to the same package declaring `Card.radius`. Import it as `use Card as A from "a"` and `use Card as B from "b"`. The canonical-file cache loads it once, but `home()` uses the first import’s package name. Reordering imports changes `a/index.contract#Card.radius` to `b/index.contract#Card.radius`, including its hash. **Fix:** derive identity from stable declaring-package ownership, independently of the first resolution alias.

5. **P1 — Positional icon sources never get rewritten or counted.** [params.rs:147](/tmp/rv-s7a-r1/contract/lower/src/params.rs:147)  
   `glyph: icon = "icon(sf \"heart\")"` followed by `image "symbol:param.glyph"` incorrectly fails with `tamber-param-unused`. The equivalent named `src=` reaches the param. Confirmed; positional image sources are established syntax. **Fix:** traverse token-bearing positional arguments according to their tag and meaning, while continuing to preserve positional text.

6. **P1 — Icon params cannot default to another icon token.** [params.rs:356](/tmp/rv-s7a-r1/contract/lower/src/params.rs:356)  
   Declare `base: icon = "icon(sf \"heart\")"` and `glyph: icon = "param.base"`, then use `image src="symbol:param.glyph"`. The compiler refuses the default because the icon checker accepts only literal `icon(...)`. This violates “any token value.” Also, [tamber_params.rs:69](/tmp/rv-s7a-r1/contract/cli/src/tamber_params.rs:69) searches only `folding.tokens`, so even valid literal icon params receive `values: null`. **Fix:** resolve icon aliases and provide icon-aware resolved values in the IR.

7. **P1 — `describe` validates the design before adding its params.** [tamber_serve.rs:714](/tmp/rv-s7a-r1/contract/cli/src/tamber_serve.rs:714)  
   Put `roundness: number = 1 in 0..2` under `dials`, and let a component’s `radius` default be `"calc(12px * roundness)"`, consumed by `border-radius`. The compiler’s combined design accepts this; `describe` and `tamber ir` first check the standalone `.tamber` and reject the dial as inert. The same split means `uses` cannot follow a design token through a param alias. **Fix:** share the combined design fold with these consumers before validation and dependency queries.

8. **P1 — IR discovery loses the app root and excludes params-only apps.** [tamber.rs:293](/tmp/rv-s7a-r1/contract/cli/src/tamber.rs:293)  
   With `app.json` containing `"tamber":"themes/app.tamber"`, `contract tamber ir <app-dir>` looks for `themes/app.contract` and silently omits the real app’s components. An app declaring params without any `.tamber` is refused outright; that refusal was confirmed against the commit’s existing fixture. **Fix:** retain the original app entry/root and allow the synthesized design already supported by compilation.

9. **P1 — Generated public types expose internal hashes.** [typescript.rs:196](/tmp/rv-s7a-r1/contract/cli/src/typescript.rs:196)  
   Running `contract types` on the new owned-param fixture emits:
   `export type ColorToken = "param.f63ddb2d5.PostButton.pressed";`
   Length params leak similarly. This directly violates the binding naming ruling. **Fix:** carry the full-name/internal-token mapping into binding generation and translate at the binding boundary.

10. **P1 — Diagnostic sanitization drops precisely the invalid params.** [theme.rs:249](/tmp/rv-s7a-r1/contract/lower/src/theme.rs:249)  
    For `a: colour = "#fff"` and `background-color="param.a"`, `build --json` emits the proper first diagnostic, followed by a `tamber-unknown-token` containing `param.fbc96c5e4.App.a`. Confirmed. Separately, [params.rs:225](/tmp/rv-s7a-r1/contract/lower/src/params.rs:225) explicitly prints the collision hash; `f6059.contract` and `f264602.contract` reproduce that collision. **Fix:** retain all params for display-name translation, separately from the filtered line-index table, and omit internal hashes from collision messages. Test every diagnostic, not just the first.

11. **P1 — The reserved namespace check excludes icon tokens.** [theme.rs:227](/tmp/rv-s7a-r1/contract/lower/src/theme.rs:227)  
    A design containing `icons` → `param.fake = icon(sf "heart")`, consumed by `image "symbol:param.fake"`, bypasses the reservation because only `design.tokens` is checked. **Fix:** reject `param.*` across both ordinary and icon declarations before adding compiler-owned params.

12. **P1 — Text-based Lean APIs discard param preprocessing.** [lean.rs:46](/tmp/rv-s7a-r1/contract/cli/src/lean.rs:46), [components.rs:22](/tmp/rv-s7a-r1/contract/cli/src/lean/components.rs:22)  
    Pass source declaring `fill: color = "#fff"` and `background-color="param.fill"` to `contract::lean` or `lean_components`. Their preliminary `compile(src)` rewrites the param, but they then reparse the original source and emit `"param.fill"` unchanged. The plan and semantic program therefore use different names. **Fix:** reuse the prepared AST, or run the same preprocessing in every text entry point.

13. **P1 — Param-owned SVGs disappear from source dependencies.** [sources.rs:434](/tmp/rv-s7a-r1/contract/cli/src/sources.rs:434)  
    Declare `glyph: icon = "icon(svg \"icons/mark.svg\")"` and consume it through named `src=`. Compilation reads `assets/icons/mark.svg`, but `contract sources` omits it—and omits it from `consulted` when missing. Confirmed. Consequently, `rerun_if_changed` cannot register this input. **Fix:** collect icon dependencies from the combined design, including param defaults and missing assets.

14. **P1 — Short-name resolution contradicts the component ambiguity rule.** [params.rs:580](/tmp/rv-s7a-r1/contract/lower/src/params.rs:580)  
    Let `a.contract#Card` declare `radius` and `b.contract#Card` declare `padding`. `resolve("Card.radius")` succeeds because it counts matching params; the IR correctly reports `short: null` because two components share `Card`. **Fix:** use one component-name ambiguity rule for both resolution and IR generation, with a regression covering disjoint param sets.

15. **P1 — The parser implements `IDENT`, not the specified `FIELD`.** [parser/params.rs:26](/tmp/rv-s7a-r1/contract/syntax/src/parser/params.rs:26)  
    `if: color = "#fff"` with `background-color="param.if"` is refused as a reserved binding name. Confirmed; `FIELD` explicitly permits reserved words in this position. **Fix:** parse the param name with `field_name()` and add a reserved-word regression.

WRONG
