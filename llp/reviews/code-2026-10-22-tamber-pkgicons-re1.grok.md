# Narrow re-check after round 3: package icon tokens (cc3beef2b6), Grok

**RIGHT.** Round 3's four holes are fixed, and the checks that were already holding still line up.

The one-colour rule in `icon_svg.rs` now keeps the nearest `color` (style, any case, else the attribute) and, before the existing fill comparison, rewrites a `currentColor` paint to that value unless it is `currentcolor` or `inherit`. `<svg color="red" fill="currentColor"><path/></svg>` is therefore one colour and a template, as on main. `currentColor` under red and under blue becomes red versus blue. `currentColor` under red beside an explicit `fill="red"` stays one colour. A `color` nothing paints in still does not reject.

`importance` lowercases first, then strips a trailing `important` and the `!` before it, with whitespace allowed after the `!`. `!IMPORTANT` and `! Important` both normalize, so the round 3 pair is red versus blue, and `!important` still wins inside one `style`.

A relative import whose directory has its own `package.json` gets `package` and `root` from `named_home`. The source graph stores that SVG as `Origin::Package` (`packages::named` when `packages()` has no entry) and puts the manifest in `consulted`, which `rerun_if_changed` prints. `hello {paths:true}` adds that root to `watch.roots` and, when the package has no design, a `watch.assets` row. The HTTP hello still forces `paths: false`.

`read_confined` lstats and returns `Missing` without opening; a dangling link fails `open` as `Leaves`. Otherwise it opens, fstats, requires the re-canonical path under `<root>/assets/` with the same device and inode, and reads that descriptor. `svg_of`, the assets digest, `tamber check`'s copy, and `Home::leaves` (the graph and the write check) all use it. App icons stay on `std::fs::read`.

Read-only. Repros were traced, not executed. No build ran.
