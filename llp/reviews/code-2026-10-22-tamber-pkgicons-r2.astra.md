# Code review round 2: package icon tokens (5670ec48df), Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — Vendored Contract defaults still read the wrong package’s assets.** [contract/cli/src/sources.rs:789](/tmp/rv-pkgicons-astra/contract/cli/src/sources.rs:789)  
   Asset ownership comes from `Origin`, while component identity comes from the nearest named package. Repro: `@acme/shelf/index.contract` imports `./vendor/reading/index.contract`; that directory has its own `package.json` and a component with literal default `icon(svg "pin.svg")`. Put distinct `assets/pin.svg` files in shelf and reading. The default reads **shelf’s** file; without it, compilation fails despite reading’s file existing. Resolve ownership alongside `home()`. App-vendored packages reached by relative imports similarly fall through to app assets.

2. **BLOCKING — A7 rejects previously accepted, genuinely one-colour app icons.** [contract/lower/src/icon_svg.rs:259](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:259)  
   Repro: a Linux-targeted app declares `icon.star = icon(svg "star.svg")`, containing `<svg viewBox="0 0 24 24" color="red"><path fill="black" d="M0 0h4v4H0Z"/></svg>`. Main accepts this template; this branch reports `tamber-icon-svg` because any `color` disqualifies it, although that colour affects no paint. This changes app admission without package icons. Track effective paint, or narrow the conservative restriction. Unchanged in-repo plans would not establish unchanged admission.

3. **SHOULD-FIX — Confinement still has a check/read race.** [contract/lower/src/icons.rs:192](/tmp/rv-pkgicons-astra/contract/lower/src/icons.rs:192)  
   `leaves()` canonicalizes and checks the SVG, but the subsequent read reopens the original pathname. Repro: pause immediately before line 199, replace the checked package SVG with a symlink to the app’s distinct SVG, then continue. The outside bytes are accepted and inlined. The asset-digest reader similarly reopens paths after graph validation. Use a read tied to the validated file and owning directory; static canonicalization alone does not close this.

4. **SHOULD-FIX — Contract-only packages still miss asset-change notifications.** [contract/cli/src/tamber_packages.rs:51](/tmp/rv-pkgicons-astra/contract/cli/src/tamber_packages.rs:51)  
   Digests are attached only to `app.packages`, which lists package **design files**. Repro: use a package with no `.tamber`, a Contract icon-param default, and its own SVG. Subscribe to the standalone design server, then replace the template with a stroked SVG. The directory watcher rescans, but neither the app-file digests nor package-design entries change, so no `changed` event announces the changed `draws`. Include asset dependencies belonging to packages without designs.

5. **SHOULD-FIX — A7 still admits multicolour SVGs through valid CSS spelling.** [contract/lower/src/icon_svg.rs:128](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:128)  
   Style-property matching is case-sensitive. Repro: two filled paths both use `fill="currentColor"`, with `style="COLOR:red"` and `style="COLOR:blue"` respectively. CSS property names are case-insensitive, but the new `property(..., "color")` check misses both; the classifier sees two identical `currentcolor` fills and accepts a package template. Normalize CSS property names before checking.

6. **NIT — Document roots rather than file-adjacent assets.** [docs/contract-for-agents.md:3715](/tmp/rv-pkgicons-astra/docs/contract-for-agents.md:3715)  
   “`assets/` beside the design file” conflicts with the implemented root rule. Repro: a package sets `"tamber":"theme/palette.tamber"`; its icon reads `<package>/assets/star.svg`, not `<package>/theme/assets/star.svg`. State package-root/app-root ownership directly.

Round 1 audit: **A1, A2, A5 and A8 are fixed**. A3/G1 is fixed for directly resolved packages, with finding 1 remaining. A4/G2 handles static escaping symlinks across the requested readers, with finding 3 remaining. A6 covers packages with designs; A7 covers lowercase colour declarations, with the limitations above.

Externally linked and symlinked package roots resolve canonically. HTTP hello requests force `paths:false`, and change events strip internal paths and asset digests. D11 and the single `tamber/1.16` clause are updated.

Read-only review; repros were traced, not executed. No builds or tests ran; the claimed app-plan equality remains unverified.
