# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `26dd75647`.
- **Method:** one brief (sha256 `75613a682b11d4722324979e5c0fad55d6492f32e66fb4abeabb9bf23b0f7220`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** taken in most part.
  - The gate is the dev client's: a classic script that listens on the window in the capture phase before the app's deferred module runs (and before `capture.js` queues a replay). Events go to the module's `gate` once it loads. Before that, a page reloaded while inspecting takes every input.
  - A token's row takes its value from the page's current design when a live edit moved it from the table's (theme2's ruling): the dev loop's snapshot gives the one token's value in the tuple, else the computed CSS. This was in the tree before this round.
  - Each row's CSS property is the emitter's (`css_property`, in the table), for set-now, computed values and the token scan.
  - An expression's row is set only when it isn't a clear (`inherit`, `unset`, …). A pin counts only when the design has it (modes.js `$xbad`). On a boundary, a colour counts only when its binding wrote it (`$xi`), not modes.js's implied one.
  - A boundary below an expression's row that names a token gets that token as its implied row.
  - A style's row takes its value from the table, in the tuple, with `from` the node itself when its own style gives it.
  - `focusin` stops before an app handler hears it. A guest frame's focus returns to the close control inside the gate.
  - The selection, with its click session, is kept across the dev loop's reload and found again by site and instance path.
  - The trim keeps `computed`.
  - Limits, for §10: a control start below a row only an expression sets; an expression's token text when it mixes text with tokens (role spellings too); a dynamic circle's `r` written as an attribute. The test still compares value, source and token, not `from` (view ids differ by page).

---

I'll review this commit read-only: the inspect-mode JS page, how its report is built against the runner's `node` op, and whether input stays gated. First I'll read the commit and the wasm inspect path it ports.The new module is in place. Next I’ll compare its report and input gate with the wasm page and the runner’s `node` op.The fixture's literal rows match `node`. Expression rows, text-style rows, implied rows under a dynamic color, and the reload gate do not. The new test never leaves those seven literal nodes on an already-loaded page.

## High

**Expression rows are read off the wrong CSS property, so a set row is dropped or inherited.** `cssName` in `host/web-js/inspect-dev.js:61` only special-cases `text_color`. `setNow` (`:86`) and `computed` (`:71`, `:78`) use that name. The emitter writes the property from `css_property` in `host/web-js/src/style.rs:996`: `border_radius_top_left` is `border-top-left-radius`, `border_color_*` / `border_width_*` / `border_style_*` are `border-<side>-*`, `position_type` is `position`, `text_stroke_color` is `-webkit-text-stroke-color`, `tint_color` is `--exact-tint`, `placeholder_color` is `--exact-placeholder-color`, `svg_mask` is `mask`.

A `border-top-left-radius={n}` that is set inline makes `getPropertyValue("border-radius-top-left")` `""`. `setNow` is false, the row is not inherited, and `nodeOf` (`:117`) omits it. The runner's `node` includes it as `dynamic` with the pixel number. A set `text-stroke-color={…}` is inherited, so the report shows `inherited` or `initial` instead of `dynamic`.

Fix: carry each row's CSS property from `css_property` on the inspect table and use that for `setNow`, the computed value, and the `var()` token scan.

**A mode boundary's implied color is wrong when the nearest row above is an expression that reads a token.** `impliedRow` (`host/web-js/inspect-dev.js:153`) returns null as soon as that row has no `k` (`:157`). The table stores tokens only for literals (`host/web-js/src/inspect.rs:153`), so a dynamic row's `k` is null even when its inline value is `var(--x-…)`. `nodeOf` then walks to that ancestor (`:138`) and reports `source: "inherited"`, `implied: false`, and the ancestor's tuple. The runner re-parses that text in the boundary's tuple and reports the boundary's own row, `source: "authored"`, `token.implied: true`.

The same `setNow` check treats modes.js's implied write as the binding. `modes.js:84` writes the ancestor text onto the boundary's inline style. A cleared dynamic color on a boundary (`$xi` empty, `$xs` set) still has a non-empty inline property, so `nodeOf:118` keeps it as `dynamic` instead of the authored implied row.

Fix: if the nearest set row's inline value names a token, emit the boundary's implied row from that text in the boundary's tuple (`computed` on the boundary, `implied: true`). Treat a dynamic row as set only when the binding wrote it (`$xi`), not when only `$xs` is set.

**Input is not gated across the reload this code claims to hold.** The gate is registered in `install` (`host/web-js/inspect-dev.js:346`), which runs only after `import('/__dev/inspect.js')` (`host/web-js/dev.mjs:466`). That response is `cache-control: no-store` (`dev.mjs:563`). `app.js` is a deferred module and is cacheable, so on reload it evaluates first. `capture.js:1` has already queued `click` / `keydown` / `input` / `change` on `window` in the capture phase. `rt.js:1433` takes that queue and `rt.js:1459` replays it with `target.click()` during `mount`, before `install`. `sessionStorage['exact.dev.inspect']` is read only inside `install` (`inspect-dev.js:354`), so the cover and the swallow are not up yet. A press on Swap pins in that window changes the app. After `install`, a `window` capture swallow does stop later document listeners. The hole is boot and the replay.

Fix: in the inline dev client, before `import()`, if `exact.dev.inspect` is set, register the capture swallow and the cover synchronously. That classic script runs before the deferred `app.js`, so the replay is swallowed.

## Medium

**A text style's weight and leading are not `node`'s value, and `from` is missing when the style is on the selected node.** `host/web-js/inspect-dev.js:131` reads `getComputedStyle`. In Chrome that is a string: font-weight `"600"` against the runner's number `600`, line-height `"22.4px"` against the unitless number. `computed: true` is set, which the protocol adds for expression rows, not for `source: "style"`. `from` starts at `above(el)` (`:33`, `:132`), so a `font` shorthand on the selected node never matches `entry.s[name]` and `from` is omitted. The runner's `from` is that node.

Fix: store the style row's resolved value per tuple in the table (the static tree already computed it) and set `from` to `viewOf(el)` when `entry.s[name] === siteOf(el)`.

**An unset or invalid dynamic `-exact-mode` is reported as the boundary's own mode row.** `mode` is inherited (`kernel/tables/schema.json:3988`) and its initial is `0` (the window). `setNow` (`inspect-dev.js:86`) is `hasAttribute("data-x-pin")`, which stays true for `unset` and for a pin the runner refuses. `ownRow` (`:70`) then returns that surrounding tuple plus one with `source: "dynamic"`. The runner does not set the row, so the value is the ancestor's or `0`, and descendants inherit that. A live `data-x-pin="dark-style=dim"` is fine. The test's Swap pins literal does not hit this.

Fix: treat the pin as set only when `pinned` accepts it, the same rule as `modes.js:78`. Otherwise leave the row to inheritance or `t.initial.mode`.

## Low

**The 60 KiB trim erases `computed`.** `host/web-js/inspect-dev.js:174` keeps `name`, `value`, `source`, `from`, and `token` only. An expression row that pushed the report over 60 KiB keeps its CSS used value (`rgb(…)`, `"16px"`) and loses `computed: true`, so it is no longer distinguishable from a kernel value.

Fix: keep `computed` on the trimmed row.

## Security and what the test shows

`/__dev/inspect.js` is served only by the dev loop, after `gate.check` (`host/web-js/dev.mjs:551`, `:562`). `build.mjs` does not copy `inspect-dev.js`. `dev-inspect.json` is copied only when `devReload` (`host/web-js/build.mjs:177`). `$exactViewId` sits in a `// dev-only` block (`host/web-js/rt.js:744`) that `withoutDev` strips for a release `rt.js`. Nothing here is on a release page.

`scripts/tamber-server.test.mjs` "inspect on the JS page" checks seven theme-fixture nodes (literals, two of them boundaries) for site, type, instance, tuple, and each row's value, source, and token, plus the join, Swap pins, outlines, and Escape. It drops `from` and `computed`. It does not build an expression row, a text style, a dynamic color above a boundary, an `each` or virtualized path, an unset pin, a report over 60 KiB, or a reload. "No web build ships inspect" checks `build.mjs` text, not a built release tree. A green run does not prove row-for-row equality with `node`.

Verdict: The JS page does not equal `node` for every node, and a reload lets a press through before the gate exists.

