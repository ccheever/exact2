# Code review: The params section (LLP 1121 stage 7 A), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** HEAD (`559f4da32`), the params section.
- **Method:** one brief (sha256 `90262a03f8b9ef133618bc5f00603dc0db93317d0aff48e289771f05dc4a8b5d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P0s: styles and keyframes, positional sources, non-row text; P1s: calls, select options, the hash in refusals, text compiles and lean, icon values; P2: tests).
- **Disposition:** in the first-round commit, each with a test (`contract/cli/tests/it/tamber_params_scope.rs`, `tamber_params.rs`, `params/rewrite.rs`). A param in a style, keyframes, another component's argument or a state is refused (`tamber-param-scope`) rather than followed to its consumers; a param in a package is named by the package's declared name.

---

1. **P0.** `rewrite` and `unused` never see a `style` or a `keyframes` row. `contract/lower/src/params.rs:143` (`nodes`) and `:236` (`read`) walk the component view only; `class.rs:31` copies a style's attributes onto the element later, and `motion.rs:269` and `theme/scan.rs:110` already treat those attributes as rows.

```
style Card
  padding = "param.inset"
component App
  params
    inset: length = "8px"
  view
    column class=Card margin="param.inset"
      text "x"
```

`unused` accepts `inset` because of `margin`. The copied `padding` stays `param.inset`, and lowering then reports `tamber-unknown-token` for a row that names the param. A param that appears only on the style, or only as `keyframes wash` / `from background-color = "param.fill"`, is refused `tamber-param-unused`. A duration param in a style hits the same hole: the loader runs `unused` before `motion::substitute`, and the style is never scanned.

Fix: rewrite `param.<name>` when a style is applied to a component and when that component's animation reads a keyframe, using that component's identity, and count those reads in `unused`. One style is shared, so rewrite at application.

2. **P0.** Every positional is left untouched, including an image source. `params.rs:147` keeps `positional` out of both walks so a `text` literal stays as written. An icon is named in that positional (`symbols.rs:36`, `image "symbol:icon.reply"`).

```
component App
  params
    glyph: icon = "icon(sf \"bubble.left\", svg \"icons/reply.svg\")"
  view
    column
      image "symbol:param.glyph"
```

This is `tamber-param-unused` for `app.contract#App.glyph`. Name `glyph` on a real row as well and the image source is still `symbol:param.glyph`, so `icons::named` does not see the param token.

Fix: rewrite and count positionals on tags whose positional is a source (`image`). Leave a `text` element's content as written. Scan those same sources in `Said::of`.

3. **P0.** The rewrite hits text that is not a row. `params.rs:137` rewrites every attribute, and `words` (`:71`) rewrites inside quotes. `motion::text` (`motion.rs:100`) leaves quoted words alone. `testId` is a prop (`tags.rs:541`).

```
component App
  params
    fill: color = "#0a7aff"
    fade: duration = "200ms"
  view
    column background-color="param.fill" testId="param.fill" animation="'param.fade' 200ms"
      text "x"
```

The plan's `testId` becomes `param.f<hash>.App.fill`. The quoted keyframes name is rewritten the same way, so the animation no longer names its keyframes. A param that occurs only in `testId` is kept: `Said::of` counts every attribute, and `unused` then treats it as read.

Fix: rewrite style rows, motion rows, use arguments, and defaults. Leave other props, and any quoted word, as written.

4. **P1.** `expr` (`params.rs:133`) and `unused`'s `text` (`:259`) skip `Call` and the subject of `?:` / `match`. `scan::exprs` (`theme/scan.rs:307`) enters calls. A keyframe may be `color=tone("strong", 0.4)` (`parser/keyframes.rs:66`).

```
fn tone(name: string) -> string = name
component App
  params
    fill: color = "#0a7aff"
  view
    column background-color=(tone("param.fill"))
      text "x"
```

`param.fill` stays the author spelling. When that call is the only read, the param is `tamber-param-unused`.

Fix: walk calls and subjects the way `scan::exprs` does.

5. **P1.** A `select` option that names a param is checked under the author spelling. Options are not rewritten (`rewrite` touches defaults only, `params.rs:190`). `folded` (`:531`) looks the string up as a design token, or for an icon as `folding.icons` by exact name.

```
component App
  params
    quiet: color = "#ffffff"
    fill: color = "accent.fill"  control="select"  options="param.quiet|accent.fill"
  view
    column background-color="param.fill" border-color="param.quiet"
      text "x"
```

`tamber-param-options` says `param.quiet` is no token. The token that exists is `param.f<hash>.App.quiet`. The same lookup rejects `options="param.reply"` on an icon param. `options="icon.reply"` against a design icon is accepted; that case is untested.

Fix: rewrite `param.<name>` inside `options` before the lookup.

6. **P1.** Two refusals print the hash. `collisions` (`params.rs:225`) emits `` `param.f{hash}…` ``. Loader errors do not go through `shown`. `theme.rs:249` then drops every unkinded param from `PARAMS`, so `shown_errors` (`:714`) cannot repair the follow-on.

```
component App
  params
    a: colour = "#fff"
  view
    column background-color="param.a"
      text "x"
```

`compile_path` returns `tamber-param-kind` with the full name. `compile_path_all` also returns `tamber-unknown-token` whose message contains `param.f<hash>.App.a` (`theme.rs:1358`).

Fix: name both files in the collision and leave the hash out. Keep every param, including a bad kind, in the `shown` list.

7. **P1.** A text compile rewrites with no file identity, and lean emits the original spelling. `lib.rs:233` never sets `params.identity`, so `token_name` (`params.rs:30`) hashes `""` and the full name is `App.fill`. `lean.rs:45` and `lean/components.rs:22` parse the source again after `compile` has accepted the rewritten plan, and emit `param.fill`. `compile_path`, `lean_path`, and `symbols_json` go through the loader.

Fix: give a pathless compile a declared identity, or refuse `params` until the source has a path, and emit lean from the rewritten file.

8. **P1.** An icon param's `values` in the `components` block are always null. `tamber_params.rs:68` reads `folding.tokens`. `add_to` (`params.rs:365`) puts an icon param on `design.icons`. `describe` and `contract tamber ir` therefore report `"values": null` for a param that folded. Protocol `tamber/1.10` and the `components` field are otherwise in place.

Fix: fill `values` from `folding.icons` for kind `icon`.

9. **P2.** The new tests never exercise styles, keyframes, image sources, `testId`, quoted motion names, call arguments, option spellings of `param.*`, the collision text, or `compile` of a text. `tamber-param-range` on a colour and on a number literal are implemented (`params.rs:456` and `:512`) and untested. The varying length `space.row` case is tested and holds.

RIGHT WITH CHANGES
