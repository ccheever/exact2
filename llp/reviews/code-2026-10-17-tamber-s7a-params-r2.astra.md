# Code review: The params section (LLP 1121 stage 7 A), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `559f4da32..04503504b`, the params section after the first round.
- **Method:** one brief (sha256 `45c262e7464c6e2154f5beb5e74fe935d3d27f6f275cd523deb839087c4a6f4e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: predicates rewritten, image URLs rewritten, quotes lost across template parts, arrow bodies unresolved, motion params inside calls left as tokens; P1s: calls skip the kind check, identity by import route, alias defaults drop SVG sources, CLI admission without params, uses/resolve/explain on the standalone fold, the planner's synthetic fold, param-less components in ambiguity, hashes in options and eval, synthetic line numbers, hashes in lint diagnostics).
- **Disposition:** the last round, applied without re-review in `8d13da213`, each with a test (`tamber_params_scope.rs`, `tamber_params_tools.rs`, `params/rewrite.rs`). 1–6: a param is a row's value only (a string or template through `?:` branches, `match` arms, `let` and `+`, motion's set); one in a function's argument, a condition, a comparison or a callback is refused (`tamber-param-scope`), which closes 1, 4, 5 and 6; the template's quote is carried across holes (3); an image source is a token's only as `symbol:` (2). 7: a package file's identity is its package's however reached (`node_modules` or the named root), tested both orders. 8: the source graph rewrites before folding. 9: `check` admits by building an app with params. 10: `uses` folds with params; `resolve`, `explain` and export by param are B's (`x2/s7b`, next). 11: B's `Synthetic.params` (built on `x2/s7b`); in A alone such a change is classed rebuild, which is safe. 12: ambiguity reads every component's identity. 13: options and eval by full name. 14: a param's refusal or warning is charged to its default in its declaring file (CLI and `describe`). 15: lint's messages pass through `params::shown`. Design change: an argument to another component passes the param; after inlining a token anywhere but a row's value is refused.

---

R1’s basic rewrites, icon handling, FIELD grammar, text/Lean identity, namespace reservation and original diagnostic cases are fixed or answered by the scope disposition. Astra 7 remains partly unfixed; Astra 13–14 have remaining variants. The expanded tests still miss the failures below.

1. **P0 — Rewriting predicates changes program behavior.** [params/rewrite.rs:142](/tmp/rv-s7a-r2/contract/lower/src/params/rewrite.rs:142)

   ```contract
   fn is_fill(s: string): bool = s == "param.fill"
   component App
     params
       fill: color = "#ffffff"
     view
       column background-color=(is_fill("param.fill") ? "param.fill" : "#000000")
         text "x"
   ```

   The predicate’s argument becomes the internal token, while the function’s comparison remains `"param.fill"`. The condition becomes false and selects black. `match` subjects have the same indiscriminate traversal. **Fix:** distinguish strings supplying row values from predicate data; preserve comparisons and add a behavioral regression.

2. **P0 — Image URLs are rewritten as token references.** [params/rewrite.rs:185](/tmp/rv-s7a-r2/contract/lower/src/params/rewrite.rs:185)

   Declare `fill: color = "#ffffff"`, read it in `background-color="param.fill"`, and add `image "https://example.com/param.fill"`. Compilation changes the URL to contain the internal token. Named `src=` is affected too. **Fix:** recognize an image’s token-bearing source syntax, such as `symbol:param.glyph`, instead of rewriting words inside arbitrary URLs and asset paths.

3. **P0 — Quoting is lost across template parts.** [params/rewrite.rs:123](/tmp/rv-s7a-r2/contract/lower/src/params/rewrite.rs:123)

   With `state face = "Example"` and a separately used `fill` param, write:

   ```contract
   text "x" font-family=`"${face} param.fill", system-ui`
   ```

   The second text part is scanned with a fresh quote state, so the quoted family name becomes `"Example param.<hash>.App.fill"`. **Fix:** carry quote and escape state across the complete template, including whether a hole occurs inside quotes, as the motion scanner already does.

4. **P0 — Arrow bodies still leave accepted references unresolved.** [params/rewrite.rs:161](/tmp/rv-s7a-r2/contract/lower/src/params/rewrite.rs:161)

   Declare and directly read `fill`, then use `text "x" color=(["x"].map(x => "param.fill").join(""))`. Compilation succeeds, but the arrow body retains `"param.fill"` while the table contains only its internal name. Without the separate direct read, it incorrectly reports unused. **Fix:** include arrow bodies supplying row values in rewriting and reachability; test the resulting binding, not merely compilation.

5. **P0 — Motion params inside calls survive into runtime as internal names.** [motion.rs:210](/tmp/rv-s7a-r2/contract/lower/src/motion.rs:210)

   ```contract
   fn id(s: string): string = s
   component App
     params
       fade: duration = "200ms"
     view
       column transition=(id("opacity param.fade"))
         text "x"
   ```

   Param rewriting accepts and counts the call, but motion substitution skips it. The emitted expression contains `opacity param.<hash>.App.fade`, rather than `opacity 200ms`; runtime has no duration-token lookup to repair it. **Fix:** perform motion substitution through the supported value flow or after function expansion. Cover easing as well.

6. **P1 — Calls bypass the required consumer-kind check.** [theme.rs:1208](/tmp/rv-s7a-r2/contract/lower/src/theme.rs:1208)

   Using the same `id` function, declare `inset: length = "8px"` and write `background-color=(id("param.inset"))`. This compiles, whereas the direct literal is correctly refused with `tamber-kind`. `leaves()` leaves calls opaque and this check examines only literal leaves. **Fix:** validate the param against every consuming row through supported expression flows.

7. **P1 — Identity still depends on the first import route.** [sources.rs:202](/tmp/rv-s7a-r2/contract/cli/src/sources.rs:202), [sources.rs:596](/tmp/rv-s7a-r2/contract/cli/src/sources.rs:596)

   Put `Card.radius` in `node_modules/pkg/index.contract`, with package name `pkg`, then import it both ways:

   ```contract
   use Card as A from "./node_modules/pkg/index.contract"
   use Card as B from "pkg"
   ```

   Relative-first loading gives `node_modules/pkg/index.contract#Card.radius`; package-first gives `pkg/index.contract#Card.radius`. The canonical-file cache preserves the first origin, changing identity and hash when imports are reordered. **Fix:** determine declaring ownership independently of access route, or explicitly reject conflicting ownership. Add mixed-route and symlink regressions.

8. **P1 — An alias default makes SVG dependencies disappear again.** [sources.rs:428](/tmp/rv-s7a-r2/contract/cli/src/sources.rs:428)

   ```contract
   component App
     params
       base: icon = "icon(svg \"icons/mark.svg\")"
       glyph: icon = "param.base"
     view
       image "symbol:param.glyph"
   ```

   Source discovery collects params before rewriting their defaults. Its fold cannot resolve raw `param.base`, and `.ok()` discards the complete icon dependency list. Consequently `assets/icons/mark.svg` is absent from both existing sources and missing-file consultations. **Fix:** prepare identities and references before collecting dependencies; test both existing and missing SVGs through aliases.

9. **P1 — CLI admission still rejects designs whose dials are read only by params.** [tamber.rs:142](/tmp/rv-s7a-r2/contract/cli/src/tamber.rs:142)

   Define dial `roundness: number = 1 in 0..2`, then a component param `radius: length = "calc(12px * roundness)"` read by `border-radius`. The combined fold succeeds, but `tamber ir` subsequently calls `admission(&src)`, which compiles a dummy component without params and rejects the dial as inert. **Fix:** validate the same combined application throughout admission. Add an actual CLI regression; testing the components helper does not exercise this second check.

10. **P1 — `uses` still cannot follow a design token through a param.** [tamber_serve.rs:912](/tmp/rv-s7a-r2/contract/cli/src/tamber_serve.rs:912)

    Let `App.fill` default to design token `accent.fill` and read `param.fill` in a background row. A `uses` request for `accent.fill` builds its alias graph from the standalone design, so it misses the param and returns no matching site. `resolve`, `explain` and export also retain the standalone fold and reject the dial-only design from finding 9. **Fix:** share the combined fold across these consumers, translating internal names at public response boundaries.

11. **P1 — The transaction planner cannot fold synthetic expressions containing params.** [tamber_serve.rs:1060](/tmp/rv-s7a-r2/contract/cli/src/tamber_serve.rs:1060)

    Let `fill` default to `accent.fill`, and use `background-color="color-mix(in srgb, param.fill 50%, transparent)"`. Propose a color-value change to `accent.fill`. The planner receives the rewritten synthetic expression but no param declarations. Both synthetic folds fail, and [session.rs:213](/tmp/rv-s7a-r2/tamber/src/session.rs:213) classifies the change as `rebuild` instead of `live`. **Fix:** include params and their dependencies in both planner folds and classification; test color versus duration/easing changes through the protocol.

12. **P1 — A same-named component with no params is invisible to ambiguity checks.** [params.rs:362](/tmp/rv-s7a-r2/contract/lower/src/params.rs:362)

    Import `a.contract#Card`, which declares `radius`, and `b.contract#Card`, which declares no params. `resolve("Card.radius")` succeeds and the IR advertises that short name. Both implementations inspect only param owners, contradicting the rule that two components named `Card` make the short form ambiguous. **Fix:** use the complete component-identity set for resolution and IR generation; correct the narrower protocol wording too.

13. **P1 — Public options and CLI evaluation expose hashes.** [tamber_params.rs:126](/tmp/rv-s7a-r2/contract/cli/src/tamber_params.rs:126), [tamber.rs:274](/tmp/rv-s7a-r2/contract/cli/src/tamber.rs:274)

    Declare `quiet: color = "#ffffff"` and `fill: color = "accent.fill" control="select" options="param.quiet|accent.fill"`, with both params read. The IR’s `options` contains the internal token, although `default` is translated correctly. `contract tamber eval` likewise prints internal param names directly. **Fix:** translate public option and evaluation names to full names; keep the hash only in the intentional IR token mapping.

14. **P1 — Tool diagnostics report synthetic line numbers in the wrong file.** [tamber_params.rs:42](/tmp/rv-s7a-r2/contract/cli/src/tamber_params.rs:42), [tamber.rs:123](/tmp/rv-s7a-r2/contract/cli/src/tamber.rs:123)

    Declare `a: colour = "#ffffff"` and read `param.a`, then run `contract tamber ir`. The error is attributed to `app.tamber:536870912:1`, rather than the Contract declaration. Message sanitization leaves `params::LINE` coordinates untouched; `describe` has the same mapping problem. **Fix:** map synthetic indices back to each declaring file and default span before emitting diagnostics, including imported and invalid-kind params.

15. **P1 — Diagnostics still leak hashes when an earlier pass fails.** [theme.rs:873](/tmp/rv-s7a-r2/contract/lower/src/theme.rs:873), [lib.rs:833](/tmp/rv-s7a-r2/contract/cli/src/lib.rs:833)

    ```contract
    component App
      params
        gap: length = "8px"
      view
        column background-color="param.gap"
          text missing
    ```

    The unknown identifier sends compilation through `with_lint`. Its supplementary kind diagnostic contains the internal param name because lint errors bypass `shown_errors`. The original invalid-kind regression does not cover this path. **Fix:** sanitize lint and aggregate diagnostics using the file’s complete param mapping; assert that every returned diagnostic is hash-free.

No files changed. No exact-HEAD build or test suite was run; read-only probes used an existing compiler with matching rewrite, motion and source-graph implementations.

WRONG
