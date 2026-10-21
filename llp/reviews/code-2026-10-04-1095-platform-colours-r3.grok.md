# Code review: LLP 1095 (reviewed as 1091), platform colours, round 3 (grok)

- **Family:** xAI, `grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --max-turns 400 --output-format streaming-json`, in a detached checkout (`/tmp/grok-pc3`) (76 turns, `stopReason: end_turn`).
- **Date:** 2026-10-04.
- **Commits:** the reviewed SHA is the squashed commit before its final rebase; it is not on `origin/main`.
- **Reviewed:** the squashed commit `65ba2e12b` over `3d76ccdb7`.
- **Method:** requested by Charlie. Mutually blind, identical brief (rounds 1-2 plus a round-3 section; sha256 `5c659fa0be777462d574ffdcbc241a47e925bd2b210a80fa83ccc91b7961b742`). Static and read-only. The landing agent (Claude) is not a reviewer.
- **Transcription:** the joined `text` events, unedited; the opening sentences are progress narration and the review starts at "**SHIP WITH FIXES.**"

---

I'll review the squashed platform-colours commit against origin/main, starting with the LLP and both round-2 reviews so I can check each prior finding against the code.The checkout is in place. Next I’ll read the LLP sections, both round-2 reviews, and the squashed commit so I can check each prior finding against the current code.Round 2 left a shared-tint table, a cache race, missed view tints, raw web filters, and light-only box filters. I’ll read the LLP’s binding sections and those paths in the squashed commit.The generation and tint-keep logic is in the kernel. I’ll read the Swift resolvers, the box-filter programs, and the rebase-sensitive call sites next.The review is still open on the rebase call sites and a few round-2 fixes. I’ll confirm those in the current squash, then write the disposition.**SHIP WITH FIXES.** The squashed commit on `3d76ccdb7` keeps the round-2 behaviour that was supposed to survive the rebase, and the shared-tint overwrite is gone in the way LLP 1091 §9 now describes. One tint-role path the new symbol key does not cover still paints a stale glyph.

## Round 2

### Grok

**A. High — one session’s tint becomes every session’s `AccentColor`. Fixed** for the overwrite §9 now forbids. The kernel tint is the key window’s, not the view’s (`SystemColor.appTint`, `SystemColor.swift:169`), and `Session.reportColors` reports that (`Session.swift:1216`). `set_reported` keeps a tint role when the new report has no row for that id (`kernel/src/style/roles.rs:130`). Presenter paint uses `ownTint` / `viewTint`. A session still repaints only when it next calls `set_colors` (`host/apple/src/colors.rs:54`); contrast and a macOS accent change do that for every session, and an iOS tint change does it for the `ExactView` that got `tintColorDidChange`.

**B. Medium — in-flight resolution refills the cache after `invalidate`. Fixed.** The generation is captured before the lookup and the cache is written only if it is unchanged (`SystemColor.swift:73` and `:86`).

**C. Medium — bake sites ignore the view tint, and a tint change does not rebuild a symbol. Fixed** for the paths named in that review. `ownTint` is passed from `NodeViewIOS.channels` (`NodeViewIOS.swift:782`), plain and inline text (`InlineText.swift:40`, `NodeText.swift:94`), text shadow and stroke (`TextRunPaint.swift:51`), box shadow (`BoxShadow.swift:181`), and palette colours (`Affordances.swift:64`). `symbolLookKey` includes `bakedTintKey` for an inherited hierarchical glyph and for a palette that names `@tint` (`Affordances.swift:48`). `tintColorDidChange` reapplies when a symbol view exists (`SystemColor.swift:240`). Monochrome still assigns `symbolView.tintColor` on every update (`NodeViewIOS.swift:438`). The `@tint/0.2` hole is the new finding below.

**D. Medium — a bound JS `filter` is raw CSS. Fixed** in the wiring. `StyleCodec::Filter` is a colour codec (`host/web-js/src/rows.rs:251`), and `SYSTEM_COLOR_MAP` rewrites whole ident tokens inside the value (`host/web-js/src/style.rs:346`). A value that merely contains `platform-color(` and is not an exact plan literal becomes `null`, which is D3.

**E. High — branch does not merge, and the LLP number clashes. Out of scope** as “behind `origin/main`”. The conflict pieces in this squash are intact: `_styles` has viewport kinds 14–23 and the colour-role default sentence (`kernel/tables/schema.json`, the `_styles` string ends with the `CanvasText` sentence); Contract’s colour error lists both the old grammar and the `platform-color(…)` literal (`contract/lower/src/values.rs:134`, matched by `contract/cli/tests/it/lint.rs:322`); `tellPreferences` still calls `setPreferences` then `reportColors` (`Session.swift:1157`); `abi.rs:1462` declares `mod colors`. `SvgHost.reappear` is main’s (`SvgScene.swift:611`, file unchanged vs `HEAD~1`); both appearance call sites pass `dark` (`NodeViewIOS.swift:498`, `NodeViewMac.swift:762`). The LLP is 1091.

### Astra

1. **Medium — session tint reports overwrite other sessions. Fixed.** Same evidence as Grok A. The two-session test still shares one non-tint report (`host/apple/src/colors_tests.rs:123`); it does not claim two different view tints.
2. **Medium — Swift resolvers use the key window’s tint. Fixed** for the cited sites (Grok C). `NodeViewMac.channels` (`NodeViewMac.swift:737`) still omits a tint argument; on macOS the accent name is `controlAccentColor` and `viewTint` is nil.
3. **Medium — palette symbols stay stale after an inherited tint change. Fixed** for a palette that names `@tint`, and for hierarchical with no resolved tint (Grok C). Covered by `testEveryResolverTakesTheViewsTint` (`SystemColorIOSTests.swift:120`), which checks the key, not a drawn image.
4. **Medium — JS filters skip role and platform-literal conversion. Fixed.** Same evidence as Grok D.
5. **Medium — box-filter colours always resolve in light. Fixed** for box filters. The style JSON carries `p` and `pd` when the dark encode differs (`host/apple/src/style.rs:355`), `BoxFilter.render` picks the program and its region by `dark` (`BoxFilter.swift:50`), and `box_filter` resolves with that appearance (`kernel/src/svg/scene/filter.rs:173`). Tests state both colours (`colors_tests.rs:168`, `kernel/src/svg/tests.rs:384`). SVG scene filters still call `resolve(false)` (`filter.rs:370`); §9 says that case stays light-only.
6. **Low — off-window boot animates from the fallback tint. Fixed.** iOS `reportColors` returns when there is no window (`Session.swift:1214`). The first report that does run snaps because `colors_seen` is still `None` (`colors.rs:58`), even if `scheme` already set `PaintMotion.dark`. Boot reports colours in `tellTime` before `rebooted` (`Session.swift:801`). `testNoColourReportIsMadeOffAWindow` covers the empty report (`SystemColorIOSTests.swift:153`).

## New

**Medium — a hierarchical symbol tinted with `Highlight` stays on the old tint.** `host/apple/Sources/ExactKit/Affordances.swift:50`, `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:426`.

`Highlight` is `@tint/0.2`. `symbolTint` returns nil only for a row whose `sys` is exactly `@tint` (`Affordances.swift:25`), so `Highlight` is resolved to a fixed colour and baked into `SymbolConfiguration(hierarchicalColor:)`. `bakedTintKey` is filled only when `symbolTint == nil` or the palette names `@tint`, so this key does not include the tint. Change the view tint from red to green: `tintColorDidChange` calls `applyStyle`, `updateSymbol` runs, the key is unchanged, and the image is not built again. The later `symbolView.tintColor = symbolTint` does not recolor a hierarchical image. A palette that names `Highlight`, and a hierarchical symbol left on `AccentColor`, do rebuild.

Include a hierarchical tint in `bakedTintKey` whenever that row `namesTint`, not only when `symbolTint` is nil.

## Not verified

No build, tests, simulator, or browser. `SCHEMA_DIGEST` (`kernel/src/wire/codec.rs:664`, `0xe4af_d57b_5de6_994a`) was not recomputed. `@property` accepting the literal `AccentColor` in `host/web-js/symbols.js:22` and `host/web/glue.js:343`, the ObjC return encoding `"@"` (`SystemColor.swift:115`), and `JSONSerialization`’s `as? Int` (`Bridge.swift:479`) were not executed. `appTint` takes any key window in `connectedScenes`, and `windows.first` when none is key (`SystemColor.swift:171`); that path was not run, so a system window (the keyboard window is often key) stealing the process tint is unconfirmed.