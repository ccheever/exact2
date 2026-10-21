# Code review: LLP 1095 (reviewed as 1086/1087), platform colours, round 2 (grok)

- **Family:** xAI, `grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --max-turns 400 --output-format streaming-json`, in a detached checkout at `83834f926` (`/tmp/grok-pc2`) (37 turns, `stopReason: end_turn`).
- **Date:** 2026-10-04.
- **Commits:** the reviewed SHAs are the branch before it was squashed into one commit for landing; they are not on `origin/main`.
- **Reviewed:** `feat/platform-colours-rebase` at `83834f926`.
- **Method:** requested by Charlie. Mutually blind to Astra xhigh, identical brief (sha256 `9d6196b19bf5f8435be655b9727a49f5bec3ebad2e3493279b24fa1d20029fa5`). The landing agent (Claude) is not a reviewer.
- **Transcription:** the joined `text` events, unedited. The opening sentences are progress narration; the review starts at "**NOT READY.**"

---

I'll review the rebased platform-colours branch against `origin/main`, starting with the LLP, the round-1 reviews, and the conflict resolutions that landed after them.Round 1 left a list of colour-staleness and wire bugs. I'll read the LLP's binding sections and the fold that claims to fix them.The fold claims to close every round-1 finding. I'll check those paths in the current tree, then the four areas the brief calls out.The generation counter and the plan-literal gate are the riskiest fixes. I'll read those paths and the paint sites that were supposed to keep live references.The round-1 fold and the second rebase are the parts I still need to pin to current lines before the verdict. I’ll re-check the reported-colour table, the Swift tint paths, and whether this branch actually contains current `origin/main`.**NOT READY.** The round-1 colour bugs that were only about a missing refresh or a lost wire reference are fixed. What the fold added is worse in a multi-session app: `AccentColor` and `Highlight` live in one process-wide table, and each session writes its own tint over that table. The branch is also 106 commits behind current `origin/main` (`83834f926` vs `6cfb736a8`, merge-base `3394b529`), and a merge conflicts in the schema, the digest, and the SVG appearance call.

## Round 1

### Astra

1. **Fixed.** tvOS uses UIKit. `SystemColor.swift:9` and `Affordances.swift:7` are `#if os(iOS) || os(tvOS)`. The remaining `#if os(iOS)` in `SystemColor.swift` is `userInterfaceLevel`, which tvOS does not have. Not compiled.
2. **Fixed.** Composite codecs encode with `.wire()` (`kernel/build/codec.rs:301`). Tag 3 writes `p.text` (`kernel/src/wire/codec.rs:539`). Roles still round-trip through `var(--exact-…)`. The round-trip test was not run.
3. **Partly fixed.** A generation counter (`kernel/src/style/roles.rs:113`) makes a second session repaint on an identical report (`host/apple/src/colors.rs:49`). A later session with a different tint still overwrites the first. That is finding A.
4. **Partly fixed.** `reportColors` runs on main and passes `tintPair(view?.tintColor)` (`Session.swift:1211`, `Bridge.swift:470`). Direct iOS `NodeView.channels` passes `ownTint` (`NodeViewIOS.swift:769`). Palette, shadows, plain text, and the `tint()` fallback still ignore that view. Finding C.
5. **Fixed in source.** Both web stylesheets set `:where(img[data-symbol-path]){--exact-tint:AccentColor}` (`host/web-js/symbols.js:21`). Whether `@property` syntax `"<color>"` accepts `AccentColor`, or drops the declaration and stays at `#000`, was not run in a browser. Rasters still tint only when a tint row is set.
6. **Partly fixed.** Bound gradients, masks, paint, and shadows go through `color_map` (`host/web-js/src/rows.rs:213`). `filter` does not. Finding D.
7. **Partly fixed.** Inline runs keep `colorRef` / `backgroundRef` and resolve them in `InlineText.run` (`InlineText.swift:40`). Plain text and shadow/stroke still omit the view tint. Finding C.
8. **Fixed for contrast and elevation.** `symbolLookKey` includes both and `SystemColor.generation` (`Affordances.swift:41`). It does not include the resolved tint, so a tint change does not rebuild a hierarchical or palette glyph. Finding C.
9. **Partly fixed.** macOS observes `NSColor.systemColorsDidChangeNotification`, then invalidates, reports, and reapplies (`Session.swift:525`). The cache can be filled again with the colour sampled before that invalidate. Finding B. An iOS tint change does not invalidate, so the symbol key does not move.
10. **Partly fixed, and the rest matches §9.** Direct iOS paint passes `drawsHighContrast`. The kernel report is still `DisplayPreferences.contrast` at base level, which §9 states is process-wide. A subtree `.accessibilityContrast = .high` still does not recolour kernel gradients. Not re-scored.
11. **Fixed.** `filter_refers` restyles a drop-shadow whose colour is a reference, or `currentcolor` under a referenced text colour (`host/apple/src/colors.rs:36`).
12. **Fixed on the wire.** `drop-shadow` carries `ColorValue` and encodes with `wire()`. The JS host still emits that colour as raw CSS. Finding D.
13. **Fixed.** `kernel/src/style.rs` is 1,493 lines.

### Grok

1. **Fixed in source.** Same as Astra 5, with the same untested `@property` caveat.
2. **Fixed in source.** An untinted Linux symbol uses `style.tint_color.resolve` (`host/linux/src/paint/svg.rs:101`). `image_tint` is still mask-gated (`host/linux/src/paint.rs:53`), so rasters stay untinted. The pinned test expects `#007aff`. Not executed.
3. **Partly fixed.** Same split as Astra 4 and 9. The new failure is the shared table (A), not a missing observer.
4. **Fixed for contrast.** Same residual as Astra 8.
5. **Fixed in source.** `lookup` requires `method_copyReturnType == "@"` before `perform` (`SystemColor.swift:111`). Not executed, and the encoding was not dumped. A return type of `@"UIColor"` would refuse every colour; UIKit's usual encoding is bare `@`.
6. **Fixed at the runtime gate.** `set_plan_style` rejects a `platform-color(` string that is not an exact `plan.strings` entry (`runner/src/bridge.rs:66`). The JS map does the same with `Object.hasOwn` (`host/web-js/src/style.rs:514`). Contract has `lower-platform-color-literal`. The kernel parser still accepts the function, which D3 requires. Style sites call `set_plan_style`. A `+` that splits the token `platform-color(` slips the lint; the runner still rejects the computed string.
7. **Fixed.** `refused` is filled only when lookup fails, and `invalidate` clears it (`SystemColor.swift:50`, `:75`). A failed `rgba` does not blacklist the name.
8. **Fixed in source.** No report until `view?.window != nil` (`Session.swift:1210`). `didMoveToWindow` calls `reportScheme` only with a window (`ExactViewIOS.swift:145`), and `scheme` reports colours before `runtime.scheme` (`Session.swift:1200`). Not executed.
9. **Fixed.** Wire tag 2 rejects a role id past the table (`kernel/src/wire/codec.rs:216`). Reports drop unknown ids.
10. **Not a defect.** §12 keeps `-apple-system-fill` as an alias of `fill` until LLP 1081's sweep, and as the vibrancy key (`llp/1087-platform-colours.rfc.md:1084`). 1081 is unbuilt.

## New findings

**High — the last session's tint becomes every session's `AccentColor`.** `kernel/src/style/roles.rs:113`, `host/apple/src/colors.rs:48`, `Session.swift:1203`, `ExactViewIOS.swift:95`.

`set_reported` replaces the whole map. Each iOS session reports `@tint` and `@tint/0.2` from its own `tintColor`. Window A is red, window B is blue: B's report stores blue for `AccentColor` and `Highlight`. Only B repaints. A's gradients, SVG scenes, and paint-motion targets stay red until A reports again, while a new node or a later `ColorValue::resolve` in A reads blue. The next report from A swaps them back.

`colors_seen` is stored before `repaint_colors` reads the table. If the other session replaces the table in that gap, this session paints the other tint and records the older generation, so it will not correct itself until it reports again.

A report whose `tintPair` is nil still sends every other name (`Bridge.swift:468`). Because the table is replaced whole, that withdraws `AccentColor` for every session. The no-window early return avoids this only when there is no window.

Do not put a per-view `@tint` in the process-wide table. Resolve it on the view, which `channels(tint:)` already can. If a report has no tint, leave the previous accent entries in place. Paint from the same snapshot whose generation was recorded.

**Medium — a resolution in flight can refill the cache after `invalidate`.** `SystemColor.swift:71` and `:49`.

`channels` drops the lock, resolves, then writes `resolved[key]`. `systemColorsDidChange` clears the cache and bumps `resolutions` on the main thread. A lookup that sampled the old accent can store it afterwards. `symbolLookKey` then rebuilds symbols from that stale entry, and it sticks until the next invalidate.

Capture `resolutions` before the lookup and store only if it is unchanged.

**Medium — several bake sites still ignore the view tint, and a tint change does not rebuild a symbol.** `Affordances.swift:56`, `Affordances.swift:41`, `SystemColor.swift:223`, `NodeViewIOS.swift:418`, `BoxShadow.swift:36`, `TextRunPaint.swift:50`, `InlineText.swift:50`, `NodeText.swift:101`.

Palette colours call `channels` without `tint`, so `@tint` uses `tint()`: the key window, else `lastTint`, else `systemBlue`. Hierarchical and palette images bake that colour into `SymbolConfiguration`. `tintColorDidChange` calls `applyStyle` but does not invalidate, and `symbolLookKey` has no tint identity, so `updateSymbol` returns early. Monochrome is fine: it assigns `symbolView?.tintColor` on every call, and `nil` inherits the live tint.

Box shadows, text-shadow, and text stroke also omit the tint. Plain text uses `InlineText.run`, which has no tint parameter. `namesTint` looks only at the run's colour and background, so a shadow that is the only `@tint` does not even schedule a refresh. Markdown text colour does go through `NodeView.channels` and gets `ownTint`; its shadow does not.

This is iOS `@tint`. macOS `AccentColor` is `controlAccentColor` by name.

Pass `ownTint` into those `channels` calls, and put a tint identity in `symbolLookKey` (or rebuild when `namesTint` on a tint change).

**Medium — a bound `filter` on the JS host is still raw CSS.** `host/web-js/src/rows.rs:213`, `host/web-js/src/style.rs:716`.

`color_map` covers colour, paint, gradients, masks, and both shadow codecs. `Filter` is emitted as the author wrote it and is not in that set. `filter="drop-shadow(0 2px 4px system-orange)"`, or a plan literal containing `platform-color()`, reaches the browser unchanged and is dropped. The wasm host uses kernel `css()` and translates. Symbol palettes are not emitted on the web, which is right.

Add `StyleCodec::Filter` to that set.

The wire/CSS split itself is doing what §9 describes. `wire()` keeps `platform-color()` for Paint, Filter, BackgroundImage, MaskImage, BoxShadow, TextShadow, and SymbolPalette; `.css()` is what the browser sees. The plan-literal gate matches D3: the whole string must be a `plan.strings` entry, in both `set_plan_style` and the JS map.

**High — this branch does not merge onto current `origin/main`.** Merge-base `3394b529`; `origin/main` is `6cfb736a8`, 106 commits ahead. `git merge-tree` conflicts in five files. The ones that lose behaviour:

- `kernel/src/wire/codec.rs:657` is `0xcea1_4a1e_2bab_0343`. Main's snapshot is `0x1b94_d1a6_08e1_8ef6`, and main added dimension kinds 14–23. Neither literal is the combined digest.
- `kernel/tables/schema.json` conflicts in the `_styles` codec comment. This side says a colour default may name a role. Main says kinds 14–23 are viewport units. Taking one side drops the other, and that comment is the declaration authority.
- `NodeViewIOS.swift` and `NodeViewMac.swift` call `svg.appearance`. Main renamed it to `reappear` (`SvgScene.swift` on main, line 611) and removed `appearance`. Keeping this side does not compile.

`host/web-js/symbols.js` auto-merges. The merged file has both `:where(img[data-symbol-path]){--exact-tint:AccentColor}` and main's `app:/` path (`appSource`, `unsymbol`, the `data:` size limit). Resolving that file by keeping this branch's copy would drop those.

The number 1087 is also taken. This branch adds `llp/1087-platform-colours.rfc.md`. Main already has `llp/1087-authoring-bench.plan.md`, and `9bb22b3f3` moved the app-diaries LLP to 1088 because main had taken 1086 and 1087. The status line in this document says 1087 was free.

Rebase onto current `origin/main`. Keep main's viewport kinds and `reappear`. Recompute `SCHEMA_DIGEST` from the merged schema and the production codec sources. Renumber the platform-colours LLP.

## Not verified

No build, tests, simulator, or browser. The digest was not recomputed. `@property` accepting `AccentColor`, the ObjC return encoding, and `JSONSerialization`'s `as? Int` were not executed. The SVG scene's own filter `resolve` was not re-read; the restyle decision in `colors.rs` was. `git merge-tree` predicts the conflicts; it is not a finished rebase.