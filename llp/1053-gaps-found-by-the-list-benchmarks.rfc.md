# LLP 1053: What the list benchmarks found missing in exact2, and which to add

**Type:** RFC
**Status:** Draft (r2: dispositions after two reviews, §0)
**Systems:** Contract (style attribute names, `tags.rs`), Kernel (`kernel/tables/schema.json` styles and enums, text preparation), Apple / web / Linux text engines (no-wrap lines, case mapping, tabular figures), Runner and DataSource (large keyed answers, launch-time values), Bake (input paths)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-26 (r1 and r2)
**Related:** LLP 1050 / 1050.000 (the fill policy; the benchmarks that exposed these), LLP 1001 (kernel; the web is the standard), LLP 1035.004 (symbol roles), LLP 1011 (images), LLP 1041 (graceful overload; keyed record sharing), `CLAUDE.md` ("a semantic that could follow CSS follows CSS"). Evidence outside the repo: `~/bench/listbench/` (easy benchmark: Expo PR 49975's demo ported to exact2 and SwiftUI) and `~/bench/heavybench/` (heavy benchmark: rich runs, photo grids, link cards, quotes, reaction chips, live inserts), each with a per-port README of gaps.

## Summary

Charlie asked, 2026-09-26: list the features the benchmark ports found missing in exact2, judge which to add, have Astra and Grok review the judgement, then build what is worth it ("it's ok to modify exact2 and even taffy").

Three ports of the same screens were built: the ordinary SwiftUI app, the ordinary Expo `@expo/ui` app, and the ordinary exact2 app. Each recorded what it could not express rather than substituting. This document covers exact2's gaps.

Some gaps were already fixed while benchmarking, and some are rendering bugs in flight. The rest divide three ways:
- **CSS properties the kernel already carries but Contract does not name.** These are nearly free, and the web-is-the-standard rule says to add them.
- **CSS properties or values that need host text-engine work.** These are worth adding, and each gets a web-oracle parity case.
- **Runtime and data shapes.** Each needs a measurement or a design choice before code.

**Recommendation:**
- **Add now:** `aspect-ratio`, per-side `border-*-color`, `flex-grow` / `flex-basis` longhands, `font-variant-numeric`, `white-space: nowrap` and `pre-line`, `text-transform`, and expression-valued generic `font-family`.
- **Measure first:** keyed partial answers for large collections.
- **Add a small design:** launch-time values.
- **Leave as is:** bake inputs outside the app directory.

## 0. Disposition after review (r2)

Astra (`gpt-6-astra`, reasoning max) and Grok (served as `grok-4.6-build`, xhigh; Charlie accepted 4.6 as a sanity check) reviewed r1 blind (`llp/reviews/1053-gaps-found-by-the-list-benchmarks.{astra,grok}.md`). Both found r1's host claims too optimistic and several CSS stories incomplete. This section supersedes §4's recommendations and §5's plan; §3's table stands with the corrections below.

**Corrections to r1:**
- **G4:** no host applies `font-variant-numeric`. The web skips the row (`host/web/src/css.rs:121`), Apple's measure ABI and runs don't carry it, and Linux drops it building a run.
- **G2:** Apple's rounded path strokes all four sides in the top colour when the widths match, whatever the colours are. Linux's non-uniform path paints four rectangles, so rounded joins are wrong.
- **G3:** `flex-grow: 1` is not `flex: 1` (`1 1 0%`). The longhand completes the set; it does not replace the list idiom.
- **G5:** `nowrap` collapses whitespace. Native engines pass raw strings with no CSS collapsing step. Apple and Linux ellipsis runs only through `line-clamp`. The shared text-flow walker and its wasm protocol have their own two-value enum. `white-space` is paragraph-level in the measure interface.
- **G8:** "shares unchanged records" is the producer's job (`Rc` identity, as Messages stress does). The runner still iterates the list. LLP 1027.004 already chose bounded answers over a delta protocol.
- **G9:** "resources are baked" is too broad. Deferred and store resources already answer on the device, and launch facts belong beside `exactViewport`.
- **G11:** the kernel has `Toggle` / `toggleValue`, and the web maps them.
- **Missing from r1:** Contract renames `direction` to `flex-direction` (`contract/lower/src/tags.rs:532`), so CSS `direction` cannot be authored. Inline run backgrounds are missing on macOS and Linux too, not only iOS.

**Build now (this tranche):**

| Item | Scope |
|---|---|
| G1 `aspect-ratio` | `<ratio>` = `a` or `a / b`, `auto`, `auto <ratio>` (intrinsic first, the ratio as fallback); degenerate ratios per CSS; a preferred ratio never overrides two definite sizes; fix the Taffy leaf patch that treats any leaf with a ratio as replaced (`vendor/taffy/src/compute/leaf.rs:150`, `:207`); browser differential cases |
| G2 `border-*-color` longhands | keep `currentcolor`; Apple rounded multicolour borders (iOS and macOS); Linux rounded corner joins; parity with four colours, unequal widths and radii |
| G3 `flex-grow` | longhand; non-negative validation for grow and shrink; `flex: 1` versus `flex-grow: 1` cases |
| G4 `font-variant-numeric` | `normal` and `tabular-nums` only, other keywords refused by name; web emits CSS; Apple carries it through the measure ABI into a CoreText feature; Linux passes `tnum` to cosmic-text; cache identities include it |
| G5 `white-space: nowrap` | internally `white-space-collapse` × `text-wrap-mode`; CSS collapsing preparation on native; min-content = max-content for nowrap; real inline ellipsis (`overflow: hidden` + `text-overflow: ellipsis`) without `line-clamp` on Apple and Linux; the text-flow walker and protocol; `pre-line` later |
| G7 `font-family` choices | ternary / `match` over literal family names, each arm resolved to a stack id at compile time; declared-face validation preserved; runtime strings refused |
| `direction` | stop renaming it to `flex-direction`; bind `StyleId::Direction` |

**Deferred:**
- **G6 `text-transform`:** needs source-preserving offsets, language, contextual casing, and web emitting CSS rather than a kernel rewrite.
- **G8:** measure end to end; the benchmark's DataSource shares unchanged records.
- **G9:** a runner-owned launch fact if a real need appears.

**Kept:** G10's refusal; the documentation says "copy or materialize", not "link".

**Dropped:** G11 (LLP 1035.006's settings slice, through `Toggle`).

## 1. Already fixed while benchmarking (context, not proposals)

| Gap | Landed |
|---|---|
| SF Symbol roles for a list's glyphs (bookmark, document, select, reorder, sort, filter; `-fill` convention) | `a96e8094` |
| Drag reorder on iOS and macOS (`reorderFor` / `reorderdrop`) | `b58430d2` |
| iOS swipe rows cost a table view each | `80223cee` |
| iOS node views all drew into bitmaps; spacers asked for gigabytes | `0e047888` |
| Taffy cache key omitted three layout inputs | `43708204` |

## 2. Rendering bugs in flight (branch `fix/ios-text-render`)

- **`line-clamp` text painting blank on iOS** in rows built during a later scroll.
- **The last line of a mixed-script paragraph (Arabic via font fallback) clipped on iOS**: measure and draw disagree on line height.
- **Inline `background-color` on text runs never painting on iOS**; the Apple run type has no background attribute.

These are bugs against CSS semantics exact2 already claims, so they need no decision here.

## 3. The gaps, with what exists today

Verified against origin/main `e992cefe` by reading `kernel/tables/schema.json`, `contract/lower/src/tags.rs` and the hosts.

| # | Gap (CSS name) | Kernel row | Contract name | Hosts | Found by |
|---|---|---|---|---|---|
| G1 | `aspect-ratio` | yes (`aspect_ratio`, f32, fed to Taffy in `layout.rs:731`) | **no** | layout only; nothing to paint | heavy port: photos sized from viewport width − 84 instead |
| G2 | `border-top/right/bottom/left-color` | yes (`border_color_*`) | only the `border-color` shorthand | iOS draws per-side colours (`BoxLayerIOS` falls back to `draw(_:)`); web and Linux to verify | heavy port: the quote's left bar became a separate clipped box |
| G3 | `flex-grow`, `flex-basis` longhands | yes | `flex-basis` yes; `flex-grow` only through `flex` | layout only | easy port: "no flex-grow", used `flex=1` |
| G4 | `font-variant-numeric: tabular-nums` | yes (`font_variant_numeric`, u8) | **no** | web and Apple partially read it | easy port: header count without tabular digits |
| G5 | `white-space: nowrap` (and `pre-line`) | enum `WhiteSpace` = `normal`, `pre-wrap` only | the property yes; these values no | every text engine needs a no-wrap line mode | heavy port: author name had to use `line-clamp=1`, which then hit the blank-text bug |
| G6 | `text-transform: uppercase / lowercase / capitalize` | **no row** | no | text preparation on every host | heavy port: link site upper-cased in the DataSource |
| G7 | `font-family` from an expression (a generic like `ui-monospace` chosen per run) | yes (u16 index into declared fonts) | literal only | the plan's font table resolves at bake | heavy port: code runs needed a separate `when` branch |
| G8 | Updating one record in a large keyed answer | — | — | Runner re-evaluates by key and shares unchanged records (LLP 1041) | heavy port: each tap and each 250 ms live tick re-sends all 10,000+ rows |
| G9 | A value read once at launch (e.g. an environment flag) | — | resources are baked at build time; no mount action | — | heavy port: "Live: on" appears about 250 ms late, read on the first timer tick |
| G10 | Bake inputs outside the app directory | — | bake refuses them | — | heavy port: the 6 MB JSON copied into the app |
| G11 | A native switch (`<input type="checkbox" switch>`) | — | no | — | easy port: toggle drawn with shapes |

## 4. Analysis and recommendation

**G1 `aspect-ratio`: add.** It is CSS, the kernel already stores it and Taffy honours it, so this is one entry in `tags.rs` plus value checking (a number, or `a / b` per CSS; `auto` and `auto && ratio` can wait). Cost: trivial. Risk: none beyond the parity case.

**G2 per-side border colours: add.** CSS longhands for rows that exist. Each host must paint differing sides. iOS already can (the `draw(_:)` path in `BoxLayerIOS`); verify web (the glue emits colours per side?) and Linux. Cost: small.

**G3 `flex-grow` longhand: add.** A CSS longhand for an existing row. Trivial. `flex-shrink` and `flex-basis` are already named, so the set becomes complete.

**G4 `font-variant-numeric`: add `tabular-nums` (and `normal`).** The row exists. Apple maps it to the monospaced-digits feature, the web passes CSS through, and Linux sets the `tnum` feature in its shaper. Other values (`oldstyle-nums`, `slashed-zero`, fractions) can follow on demand. Cost: small.

**G5 `white-space`: add `nowrap` and `pre-line`.** Every list app wants a single-line label without clamping, and `nowrap` + `overflow: hidden` + `text-overflow: ellipsis` is the CSS idiom. `pre-line` (collapse spaces, keep newlines) is cheap once the enum grows. `pre` and `break-spaces` can wait.
- Each text engine needs a mode that does not break lines.
- Measurement must report the unwrapped width as both the min-content and max-content width.

Cost: moderate (three engines). Parity cases against the browser are required.

**G6 `text-transform`: add `uppercase`, `lowercase`, `capitalize`, `none`.** It is CSS and inherited, and apps otherwise transform data, which breaks copy (CSS copies the transformed text in Chrome and the source text in Safari; follow Chrome's rendering and the engine's copy path, and note it).
- Apply it in text preparation, before shaping, with Unicode full case mapping: `ß` → `SS`, locale-insensitive at first.
- `capitalize` uses word boundaries (UAX #29).

Cost: moderate (a new row, an enum, and one shared Rust transform so hosts do not diverge). It is a kernel-side transform, so hosts shape already-transformed strings.

**G7 expression-valued `font-family`: add, limited to declared fonts and CSS generic families.** Today a per-run choice of monospace needs a `when` branch per run, which is clumsy and doubles row nodes. Allow an expression whose values are declared families or generics (`ui-monospace`, `ui-serif`, `system-ui`), resolved through the plan's font table at runtime. Refuse unknown names at compile time when they are literal. Cost: small to moderate.

**G8 keyed partial answers: measure first, then decide.** The runner already re-evaluates by key and shares unchanged records. What a 10,000-row re-send costs per tick is unknown: a runner benchmark of one 10,000-row answer replaced with one field changed should report the time.
- If it is under about 1 ms per tick on the phone, document the pattern and stop.
- If it is over, add a DataSource answer form that patches a previous answer by key (insert / update / remove), which the runner applies without re-reading the whole value.

This is the biggest design in the list and must not be built on a guess.

**G9 launch-time values: add a small form.** The need is a value read on the device at mount, not at bake: an environment flag, the locale at launch, a first-run marker. Two candidates:
- a DataSource resource marked not-bakeable (answered at mount);
- a `mount` action.

The first fits the existing model (resources, not lifecycle hooks) and keeps effects declarative. Recommend it, named in Contract by the existing resource syntax plus a flag.

**G10 bake inputs outside the app: keep the refusal.** Hermetic, reproducible bakes are worth more than convenience. Copying or linking the data into the app directory is the right answer. Document it in the app guide.

**G11 native switch: add as `<input type="checkbox" switch>`.** That is the HTML spelling (Safari 17.4+); the other browsers render a checkbox, which is acceptable. It renders `UISwitch` / `NSSwitch` on Apple. Cost: small to moderate (one input kind per host). It was a cosmetic gap in the easy benchmark, so it is lower priority than G1–G7.

## 5. Plan

In priority order. Each item is one commit or more, with a web-oracle parity case where layout or text changes:

1. **Trivial names: G1, G3, G4.** Contract names plus parity cases.
2. **Per-side border colours: G2**, including the host paint check on web and Linux.
3. **`white-space`: G5**, in all three engines, with parity cases for no-wrap measurement and ellipsis.
4. **`text-transform`: G6**, a kernel row and a shared Rust case mapping.
5. **Expression-valued `font-family`: G7.**
6. **Measure G8**, and report before any design.
7. **Launch-time values: G9.**
8. **Native switch: G11.**

After items 1–5, the heavy benchmark's exact2 app drops its workarounds (the viewport-derived photo sizes, the clipped quote bar, the DataSource upper-casing, the per-run `when` branches) before its phone run.

## 6. Questions

1. **G8:** keyed patches if the measurement says so, or accept whole re-sends?
2. **G9:** a not-bakeable resource, or a mount action?
3. **G11:** now, or after the benchmark work?
4. **G6 copy:** does copying transformed text copy the transform (Chrome) or the source (Safari)?
