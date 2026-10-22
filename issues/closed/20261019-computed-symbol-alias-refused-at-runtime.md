# Computed symbol sources are refused at runtime: unknown symbol role

**Status:** Closed
**Resolution:** fixed by bbd846b41: a runtime symbol source resolves by any spelling, and a tab takes a bundled raster glyph
**Systems:** kernel (replaced.rs symbol lookup), contract lowering (symbols), Apple host tabs
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

A tab or image whose source is `symbol:<name>` logs `image symbol:home refused: unknown symbol role` on iOS and draws nothing; the build passed. There is also no way to give a tab a raster icon.

## Cause

The compiler checks and rewrites only literal sources, through `symbol_role` (`contract/lower/src/symbols.rs:44-80, 208-233`), which accepts a role, Apple's name with `.`/`_` read as `-`, or an alias. A computed source (`` `symbol:${tab.icon}` ``, a value from data) is neither checked nor rewritten, and the runtime's `apple_symbol_name` (`kernel/src/replaced.rs:123-130`) accepts exact roles only. So a computed `symbol:house`, `symbol:gear` or `symbol:cart` builds and is refused when drawn. `docs/recipes/symbols.md` says computed sources are checked at runtime, which holds only for exact roles.

Tab items take their image from the child image's `symbolName` or `symbolPath` (`NavigationTabsIOS.swift:280-285`, `FaceSymbol.swift:117-121`); a raster child image gives neither, so `item.image` is nil.

## Fix

Resolve a runtime symbol source through the same spelling table the compiler uses (`symbol_role`, generated in `kernel/build/names.rs`), so a computed alias or SF name draws as the role does. Let a tab's child `image` with a bundled raster source become the tab bar item's image (template-rendered, as UIKit does).

## Fixed (2026-10-19)

`apple_symbol_name` and `symbol_path` read a role by any of its spellings through the generated `symbol_role` (Apple's name with `.`/`_` as `-`, a table alias), as the compiler reads a literal, so a computed `symbol:house` draws as `symbol:home`; `replaced::tests::a_role_by_another_spelling_draws_as_the_role`. A tab's child `image` with a bundled raster source (`app:/…` or an asset name) becomes the tab bar item's template glyph at the bar's glyph size (`TabFace.raster`, `TabFace.rasterImage`, `NavigationTabsIOS.syncItems`); a symbol still wins. Not driven on a simulator here (this Mac's SDK cannot build the iOS sound arm).
