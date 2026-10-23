# Code review round 1: package icon tokens (d8d608fb32), Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — A package param cannot name its own package icon.** [contract/lower/src/theme/packages.rs:380](/tmp/rv-pkgicons-astra/contract/lower/src/theme/packages.rs:380)  
   Repro: declare `reading.star` under `icons` and `component Map → params → glyph: icon = reading.star` in `reading.tamber`. Compilation reports `tamber-unknown-token`: `foreign()` uses `own()`, whose declarations contain only value tokens. Include the package’s icons in this ownership check.

2. **BLOCKING — Valid package-icon references prevent design edits.** [contract/cli/src/tamber_params.rs:233](/tmp/rv-pkgicons-astra/contract/cli/src/tamber_params.rs:233)  
   Repro: an app declares `icon.mine = reading.star` and an ordinary `ink` token. Building succeeds, but proposing or transacting `set-token ink #222222` fails with `tamber-kind`. The edit planner’s `joined()` returns only `d.tokens`; package icons disappear before `apply_all_with` checks the edited design. The later complete fold cannot repair this earlier refusal.

3. **BLOCKING — The `.contract` default is an asset-ownership hole.** [contract/lower/src/icons.rs:102](/tmp/rv-pkgicons-astra/contract/lower/src/icons.rs:102)  
   Repro: a package Contract component declares `glyph: icon = "icon(svg \"pin.svg\")"` and draws `symbol:param.glyph`. Give the package and app different `assets/pin.svg` files. `written_in()` matches only package **design-file** identities, so the Contract identity falls through to the app root and draws the app’s file. Without that app file, compilation fails despite the package asset existing. Carry package ownership for Contract defaults too.

4. **BLOCKING — SVG symlinks escape the owning package.** [contract/lower/src/icons.rs:170](/tmp/rv-pkgicons-astra/contract/lower/src/icons.rs:170)  
   Repro: make `<package>/assets/star.svg` a symlink to the app’s distinct `assets/star.svg`, then declare `reading.star = icon(svg "star.svg")`. The compiler follows the link and accepts the app’s bytes as the package icon. The same escape works through a linked assets directory or into another package. Lexical path validation rejects `..` but supplies no real-path confinement. Validate assets against the resolved owning root across readers.

5. **BLOCKING — Package-only designs lose their SVG build dependencies.** [contract/cli/src/sources.rs:626](/tmp/rv-pkgicons-astra/contract/cli/src/sources.rs:626)  
   Repro: remove the app’s `.tamber`; import a package component with no params that draws its package icon. Compilation merges the package design, but `source_graph` starts folding only when app design text or params exist. Its output omits the SVG, so `rerun_if_changed` and the resident file watcher do not track that asset. Seed the fold when package designs exist too.

6. **SHOULD-FIX — SVG edits produce no design-change notification.** [scripts/tamber-package-designs.mjs:43](/tmp/rv-pkgicons-astra/scripts/tamber-package-designs.mjs:43)  
   Repro: subscribe to the design server, then change a package SVG from a valid template to a stroked drawing. The directory watcher triggers a rescan, but notification compares only package design-text digests. Those remain identical, so no `changed` event announces the changed `draws` result. Include asset changes in invalidation.

7. **SHOULD-FIX — The template check admits multicolour `currentColor` drawings.** [contract/lower/src/icon_svg.rs:284](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:284)  
   Repro: use two filled paths with `fill="currentColor"`, one with `color="red"` and the other `color="blue"`. The existing classifier compares the identical string `currentcolor` and ignores `color`, returning a template. The new package guard therefore accepts a multicolour SVG and collapses its colours. Resolve this paint dependency or conservatively refuse it.

8. **SHOULD-FIX — `resolve` has a provenance consistency hole.** [contract/cli/src/tamber_base.rs:435](/tmp/rv-pkgicons-astra/contract/cli/src/tamber_base.rs:435)  
   Repro: resolve `reading.star`, then explain it. Resolve returns before `tamber_packages::fields`, omitting `package`, `file`, and `identity`. Explain includes them. This is nonblocking relative to the approved 1.16 clause, which explicitly covers describe/explain, but package-icon resolve should match package-token resolve.

Read-only review; repros above were traced, not executed. D7 retention and D15 release identities appear preserved. Externally linked package roots resolve correctly. The docs add D11’s SVG rule and one 1.16 clause. The claimed all-app byte-identical gate remains unverified here.
