# Code review round 3: package icon tokens, Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — A7 still rejects valid, previously accepted app icons.** [contract/lower/src/icon_svg.rs:352](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:352)  
   Repro: a Linux-targeted app declares `icon.star = icon(svg "star.svg")`, with `<svg viewBox="0 0 24 24" color="red" fill="currentColor"><path d="M0 0h4v4H0Z"/></svg>`. This is one opaque colour. Main accepts it; this branch marks it non-template and `declare` reports `tamber-icon-svg`. Round 2’s black-fill example is fixed, but the admission regression remains for constant `currentColor` paint. The classifier must distinguish one effective colour from several.

2. **SHOULD-FIX — Reading the canonical pathname does not close the check/read race.** [contract/lower/src/icons.rs:204](/tmp/rv-pkgicons-astra/contract/lower/src/icons.rs:204)  
   Repro: start with a regular `<package>/assets/star.svg`; pause after `Home::resolve` returns, replace that file with a symlink to the app’s distinct SVG, then continue. The canonical pathname equals the original pathname in this case, and `read_to_string` follows the replacement link. Outside bytes are still accepted and inlined. Replacing a parent directory also works. The asset digest’s `std::fs::read(real)` has the same issue ([tamber_packages.rs:102](/tmp/rv-pkgicons-astra/contract/cli/src/tamber_packages.rs:102)). Bind validation and reading to opened files/directories; canonicalization alone is insufficient. Round 2’s race finding remains open.

3. **SHOULD-FIX — App-vendored packages still miss SVG-change notifications.** [contract/cli/src/sources.rs:593](/tmp/rv-pkgicons-astra/contract/cli/src/sources.rs:593)  
   Repro: import `./vendor/reading/index.contract` from the app; give that directory a named `package.json`, no `.tamber`, and a component with literal default `icon(svg "pin.svg")`. Compilation now correctly reads `vendor/reading/assets/pin.svg`. However, `packages::packages` skips sources with `Origin::App`, so this SVG falls back to that origin. `tamber_packages::assets` consequently omits it from `watch.assets`. Change the SVG from a template to a stroked drawing: the standalone design server announces no asset change. Carry the named-package ownership into the graph and watcher inventory too. Round 2’s design-less-package fix is incomplete.

4. **SHOULD-FIX — Valid `!IMPORTANT` spelling bypasses the multicolour guard.** [contract/lower/src/icon_svg.rs:292](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:292)  
   Repro: two filled paths use `style="fill:currentColor !IMPORTANT;color:red"` and `style="fill:currentColor !IMPORTANT;color:blue"`. `clean` strips only lowercase `!important` before lowercasing, leaving both paints as `currentcolor !important`. They compare equal, but neither sets `current`; the package SVG passes as a template and loses its two colours. Normalize importance syntax before comparing paint values. Case-insensitive property names are fixed; A7’s broader correctness issue remains.

The remaining prior findings are fixed by inspection: package params naming their own icons; `joined_all`/`joined_icons` in write planning; `.contract` default ownership, including vendored packages; package-only design dependencies; resolve provenance; ordinary package/design-less asset digests; and the root-ownership wording plus D11’s superseded refusal. Static escaping symlinks are refused, dangling final links fail closed, and app icons remain unconfined. Whole overrides and D7 retention remain intact.

Externally linked and symlinked package roots resolve canonically. HTTP hello forces `paths:false`; change events omit internal paths and asset digests. The single `tamber/1.16` clause is present.

Read-only review: repros were traced, not executed. No builds or tests ran; the claimed all-app plan equality remains unverified.
