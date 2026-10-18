# Code review: The params section (LLP 1121 stage 7 A), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `559f4da32..04503504b`, the params section after the first round.
- **Method:** one brief (sha256 `45c262e7464c6e2154f5beb5e74fe935d3d27f6f275cd523deb839087c4a6f4e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P0: motion params inside calls; P1s: graph not rewritten, hashes in options, synthetic lines and warnings, eval's names, uses blind to params; P2s: text compile keeps one error, unnamed package by alias, stale module comment).
- **Disposition:** the last round, applied without re-review in `8d13da213`. 1: a param in a function's argument is refused (a row's value only). 2: the graph rewrites before folding, tested. 3: options by full name. 4: refusals and warnings at the param's default, names in full, CLI and `describe`. 5: eval lists no param; check counts them apart. 6: `uses` folds with params. 7: declined: the text entries (`compile`, `compile_terminal`, lean) return one error by their signature, as every other pass of theirs does; `compile_path_all` returns all. 8: identity no longer reads the alias: a package's file is named by its manifest name, else by its place under the app. 9: the comment is rewritten.

---

Round 1 is fixed in this tree. A param in a `style`, `keyframes`, another component’s argument, or a state or derive is refused as `tamber-param-scope`. Calls, branches, templates, image sources, select options, quotes, `url(#…)`, hole glue, `testId`, and text are handled as the disposition says. Identity, icon aliases on the compile path, `describe`’s fold, the `components` block, full-name refusals on the compiler path, `tamber/1.10`, and unused-before-motion all match the stage note. What follows is what is still wrong.

1. **P0** — A duration or easing param inside a call in a motion row is left as the internal token. `contract/lower/src/motion.rs:210`

`params::texts` rewrites the argument (`contract/lower/src/params/rewrite.rs:137`), then `motion::rewrite` only walks strings, templates, ternary branches, match arms, `let`, and `+`. A call, a ternary condition, and a match subject fall through to `_ => {}`. The transition checker only parses a string literal (`contract/lower/src/values.rs:562`), so the call compiles, the same way `id("param.fill")` does. The duration is never written in.

```
fn id(x: string): string = x
component App
  params
    fade: duration = "200ms"
  view
    column transition=(id("opacity param.fade"))
      text "x"
```

The argument stays `opacity param.f<hash>.App.fade` instead of `opacity 200ms`. Walk calls, ternary conditions, and match subjects in `motion::rewrite` the way `params::texts` does.

2. **P1** — `sources::graph` never rewrites, so an icon param that names another icon drops every SVG dependency. `contract/cli/src/sources.rs:428`

`load()` rewrites before `add_to` (`sources.rs:149`). `graph()` sets identity in `load_source`, then calls `tokens()` and `add_to` on the author spelling. The alias copy looks up an icon named exactly `param.base`, does not find `param.f<hash>.App.base`, and `check_design(...).ok()` (`sources.rs:466`) throws away the whole fold, including a sibling literal SVG.

```
component App
  params
    base: icon = "icon(svg \"icons/mark.svg\")"
    glyph: icon = "param.base"
  view
    image "symbol:param.glyph"
```

`compile_path` succeeds and reads `assets/icons/mark.svg`. `source_graph` / `rerun_if_changed` does not list it, so editing the file does not rebuild. Run the same `rewrite` before `add_to` in `graph()`.

3. **P1** — `describe` and `contract tamber ir` put the hash in `options`. `contract/cli/src/tamber_params.rs:126`

`default` goes through `params::shown` (line 121). `options` is the rewritten attribute, split on `|`. The protocol says the hash lives only in `token`, and that no tool writes it.

```
params
  quiet: color = "#ffffff"
  fill: color = "accent.fill"  control="select"  options="param.quiet|accent.fill"
```

`options` is `["param.f<hash>.App.quiet", "accent.fill"]`. Run `shown` on the options string before splitting. The scope test builds this program and never reads the field.

4. **P1** — `contract tamber check` and `describe` charge param diagnostics to line 536870912 and, for warnings, print the hash. `contract/cli/src/tamber.rs:158`, `tamber/src/lib.rs:226`, `contract/cli/src/tamber_serve.rs:1388`

Param tokens are added at `params::LINE + index` (`params.rs:96`, `1 << 29`). The compiler remaps that through `token_span`. These paths do not. `fold_with` runs `shown` on error messages only. Warnings are printed raw (`tamber.rs:158`) and serialized with `d.line` and `d.message` (`tamber_serve.rs:1388`). `type_fixed` formats `` `{token}`'s size is px `` using `t.name` and `declared.name.line`.

```
component App
  params
    body: type = "16px/1.4 system-ui"
  view
    column
      text "x" font="param.body"
```

`contract tamber check` prints `app.tamber:536870912:1: tamber-type-fixed: \`param.f<hash>.App.body\`'s size is px…`. A bad default takes the same line on the error path (`tamber.rs:122`). Map `line >= params::LINE` to the param’s `default_span`, and run warnings through `shown`.

5. **P1** — `contract tamber eval` lists param tokens under their hash names. `contract/cli/src/tamber.rs:272`

The stage note says checks fold params with the design and list them only under `components`. `eval` prints every `folding.tokens` name. With `fill: color = "#ffffff"` read by a row, the line is `param.f<hash>.App.fill = #ffffff`. Skip names that start with `param.`. The same fold makes `check`’s token count include params.

6. **P1** — `uses` cannot see a param token. `contract/cli/src/tamber_serve.rs:912`

`uses` calls `design()`, which folds the design file alone, then returns `no token` when the name is absent (line 914). The stage note puts `uses` below the IR, on the dotted token. A client holding `components[].token` gets `no token \`param.f<hash>.App.fill\``. A duration param never reaches `motion_sites` either. Fold with `design_with` for this lookup. Wire `resolve` of a full or short name is commit B; this one is the token `uses` already claims to speak.

7. **P2** — A text compile keeps only the first param error. `contract/cli/src/lib.rs:242`

`params()` does `refused.into_iter().next()`. The loader returns every rewrite and unused error. `compile`, `compile_terminal`, and the lean text path drop the rest. Return the whole vec, as `load` does.

8. **P2** — A package with no `package.json` `name` is still identified by the first import alias. `contract/cli/src/sources.rs:215`

`declared.as_deref().unwrap_or(name)` falls back to the specifier stored on the first `Origin::Package`. Two aliases of one unnamed package (`use Card from "a"` and `use Card as Other from "b"`) produce two identities and two hashes. Refuse a package whose manifest has no `name`, or key the identity by the canonical package root.

9. **P2** — The `params` module comment still says use-arguments are rewritten. `contract/lower/src/params.rs:7`

They are refused as `tamber-param-scope`. The stage note and `rewrite.rs` say so. The comment tells the next edit to follow them.

RIGHT WITH CHANGES
