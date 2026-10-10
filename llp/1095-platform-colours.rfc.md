# LLP 1095: Platform colours — the OS decides, by default

**Type:** RFC
**Status:** Draft r3 (comprehensive), 2026-10-03. Stage 1 (D1–D3, D5–D7) and stage 2 (D8; D9 except keyframes) are built on `feat/platform-colours`. D4 (`theme`), D10 (relative colour) and D11 (the web role sheet) are proposed, not built. §10 reviews colour fidelity and proposes fixes. r1 (2026-10-02) was reviewed adversarially and found not ready; §8 folds every finding. Written on the `ide` fork as LLP 1078 and ported to `origin/main` as LLP 1081; renumbered to 1095 on landing (2026-10-04) because `origin/main` took 1078 (foldables), 1081 (the `-exact-` naming rule) and 1086–1094 while it landed. §12 reconciles the two.
**Systems:**
- Kernel: the colour value, an interned reference; the role table; resolutions the host reports
- Contract: role keywords, `platform-color()`, `theme`
- Apple host: trait-set resolution in every paint path
- Web host and the web JS target: system colours, the role sheet
- Linux host: fallbacks
- Agent API: resolution report; contrast and elevation in `prefer`
**Author:** Claude (Opus 5.5) for ide@expo.io
**Implementer:** Claude (Opus 5.5), when accepted
**Date:** 2026-10-03
**Related:**
- LLP 1034: scheme-aware colour, the `light-dark()` pair.
- LLP 1062: paint motion. D4: an appearance change transitions. D9: keyframe colours known at compile time.
- LLP 1066: gradients.
- LLP 1077 D13: the `-apple-system-*` colours, which this replaces.
- LLP 1053.000: materials, the table shape this reuses.
- LLP 1001: the style table and its declared deviations.
- LLP 1030 D3a: the compatibility id.
- LLP 1076: Android rendering research.
- CSS Color 4 §6.2 (system colours); CSS Color 5 `light-dark()`; WebKit's `-apple-system-*` keywords.


> **Spelling (2026-10-06):** Exact's roles and `platform-color()` are spelled `-exact-<role>` and `-exact-platform-color()`, as §12 says since [LLP 1081](1081-names-exact-invents.rfc.md). This document keeps the spelling it was written with, as the record.

## Summary

**The principle (the owner's ruling, 2026-10-03): on native platforms the OS
decides.** A colour that means something to the platform — a label, a fill,
a system hue, the app's tint — is the platform's, resolved by the platform
for appearance, Increased Contrast, the user-interface level, vibrancy and
whatever it adds next. Exact never ships a snapshot of it by default. A
fallback pair exists only so the kernel, Linux and headless runs always have
a deterministic value.

> On native platforms we should let the OS have the ability to handle
> contrast and really anything else that relies on semantic, symbolic
> colors. I don't want to use hardcoded snapshotted values by default.

The whole design:

| | Decision | Status |
|---|---|---|
| D1 | A colour can be a **reference** (interned, with a fallback pair the kernel always has) | built |
| D2 | **Semantic roles**: one table, CSS's names where CSS has them (`CanvasText`, `AccentColor`), Apple's where it doesn't (`secondary-label`, `system-orange`); WebKit's `-apple-system-*` names are aliases | built |
| D3 | **`platform-color()`**: any native colour by name, with a CSS fallback | built |
| D4 | **`theme`**: an app's own named colours, able to override a role | proposed |
| D5 | The **host resolves per view**, against its trait set, everywhere it paints | built |
| D6 | **Motion** follows LLP 1062: a trait change transitions | built (keyframes: D9) |
| D7 | The **agent pins traits** so tests are deterministic | built |
| D8 | **Initial values are system colours**: `color` is `CanvasText`, `tint-color` is the platform's tint (`AccentColor`) | built |
| D9 | **Host-reported resolutions**: paint motion, gradients and SVG follow the live platform colour | built, except keyframes |
| D10 | **Relative colour and `color-mix()`** over references (`rgb(from system-orange r g b / 15%)`), resolved by the host | proposed |
| D11 | **The web role sheet**: `--exact-<role>` per scheme, `prefers-contrast` and `forced-colors` | proposed |

§10 reviews colour fidelity end to end: resolved roles clamped to sRGB (P3
lost), 8-bit sRGB colour rows, CSS Color 4 syntax refused, images flattened
to sRGB, and two unmanaged device-RGB fills, each with a fix. §6 lists the
questions for the maintainer.

## 1. Motivation

**Native apps do not write hex for system colours.** UIKit's
`secondaryLabel` is one name whose value depends on the trait collection:
- appearance
- Increased Contrast
- the user-interface level (base or elevated)
- a material's vibrancy

Copied values drift from the platform:
- in accessibility modes
- on elevated surfaces
- whenever the OS retunes its palette, as iOS 26 did

**The web is the standard (CLAUDE.md), and CSS has system colours.**
`Canvas`, `CanvasText`, `LinkText`, `GrayText`, `Highlight`, … are resolved
by the browser against the OS and `color-scheme`. CSS's list is small and
form-oriented. It has no "secondary label" or "grouped background", so Exact
needs more roles than CSS has, and a way past any fixed list.

**Three requirements from the people who will write Exact apps:**
1. the semantically correct thing must be the easiest to write
2. every colour a platform provides must be reachable, including future ones
3. a new OS must not need a new Exact to use its colours

### The principle, in full

**A colour that means something to the platform is the platform's, on that
platform, at the moment it draws.** Exact names it; the OS resolves it
against everything it knows: appearance, Increased Contrast, the
user-interface level, a material's vibrancy, Reduce Transparency, and
whatever it adds next. A fallback pair exists so the kernel, Linux and
headless runs always have a deterministic value, never so a native host can
skip asking (D1).

The web is the standard for *semantics*: which role, inheritance,
`currentcolor`, motion. It is not the standard for a native platform's
palette values (§6.5's recommendation). This RFC keeps that line.

## 2. Decisions

### D1 — A colour can be a reference: interned, with a fallback pair the kernel always has

`ColorValue` today is `Copy + Eq`, with `const fn resolve(dark)`
(`kernel/src/style.rs:716-746`). It is embedded in `Paint`, gradients,
shadows, strokes, `symbol-palette` and filter primitives, and it is
dereferenced throughout the kernel. A boxed, recursive reference would break
all of that.

So a reference is **interned**. The plan carries a reference table, and a
colour holds an index:

```rust
enum ColorValue {
    Fixed(Color),
    Pair(Color, Color),                        // light-dark(); LLP 1034
    Ref(RefId),                                // this RFC: u16 into the plan's table
}
struct Reference {                             // the plan's table, one row per distinct reference
    kind: RefKind,                             // Role(RoleId) | Platform { ios, macos, web: Option<Name> }
    fallback: (Color, Color),                  // light and dark: what the kernel and fallback hosts use
}
```

**`ColorValue` stays `Copy + Eq`**, the wire grows by one discriminant and a
`u16`, and D5's per-host cache has a natural key: `(RefId, trait set)`.

**Every reference carries a fallback pair**:
- a role's comes from the role table (D2)
- a `platform-color`'s comes from its CSS fallback, itself a colour or pair,
  never another reference (D3)

So **every reference has a deterministic RGBA for any appearance without
asking a host**. That is the property the kernel needs, because the kernel
does resolve colours today:
- **Native paint motion** seeds its engine from resolved colours in Rust
  (`Kernel::paint_sync`/`paint_resync`, `color_targets(node, dark)`,
  `kernel/src/motion.rs:207, 388-421`).
- **SVG filter primitives** resolve flood, lighting and drop-shadow colours
  (`kernel/src/svg/scene/filter.rs:355, 732, 742`), as does
  `Paint::resolve` (`kernel/src/svg/mod.rs:266`).
- **The Linux painter** resolves through `ColorValue::resolve(dark)`
  (`host/linux/src/paint.rs`).

**The kernel resolves a reference to what the host reported, else the
fallback.** A host that resolves dynamic colours reports, per reference and
per trait set it is showing, the RGBA it resolved. This is the same shape
as `exact_scheme`/`exact_view_scheme`, which report the scheme today
(`host/apple/src/abi/exports.rs:459-466`). Concretely:
- the call is `exact_color_resolved(ref, traits, rgba)`
- the kernel keeps a small map, `(RefId, traits) → Color`
- a new report for a reference that motion or a filter is using re-seeds it,
  as `paint_resync` does on a scheme change

So paint motion animates a role toward the colour the platform actually
shows, and an SVG filter's flood follows the system colour. Where no host
reports (Linux, the agent's headless runs), the fallback pair is used, and
it is the documented value.

`currentcolor` keeps its meaning and inherits the reference, not a resolved
value. A child under a different trait context (an elevated sheet) resolves
it itself.

A reference is valid in **every colour row** (the `color`, `color2`,
`current-color`, `auto-color` and `paint` codecs in
`kernel/tables/schema.json`) and in every embedded colour:
- `box-shadow` and `text-shadow` colours
- `-webkit-text-stroke-color`
- `stop-color`, `flood-color`, `lighting-color`, the filter `drop-shadow`
  colour
- `symbol-palette`
- gradient and `mask-image` stops

The parsers that learn the new form are:
- `shadow.rs:115, 205`
- `stroke.rs:45`
- `gradient.rs:750`
- `svg/mod.rs:225`
- `style/symbols.rs:42`
- the filter `drop-shadow` parser
- `parse_light_dark`

A reference is **not** valid inside a `light-dark()` half. A pair of
references is a reference whose fallback is a pair, so nothing is lost.

### D2 — Semantic roles: one table, CSS's names where CSS has them

Roles live in **one table in `kernel/tables/schema.json`**, shaped like the
`materials` table (LLP 1053.000). Each row has:
- the role's name
- its Apple names (UIKit, AppKit; Objective-C spellings)
- its web form
- its fallback light and dark colours

`kernel/build.rs` generates the role ids. The table **replaces**
`SYSTEM_COLORS` (`kernel/src/style/symbols.rs:63-72`), which is deleted
(RULES: delete, don't deprecate). Its six WebKit names become aliases of the
roles below, so `-apple-system-secondary-label` is now a reference, not a
copied pair. LLP 1077 D13 is amended.

**Roles are colour keywords, written as strings.** They are not bare
identifiers:
- `color=secondary-label` lexes as a variable reference in Contract
  (`contract/syntax/src/lexer.rs:213-222`)
- `label`, `fill` and `background` are common local names
- `theme` is the most-used local in the repo's apps

So a role is written as CSS writes a keyword: `color="secondary-label"`,
the same as `color="#ffffff"` today. Contract checks every literal colour
string, so a misspelt role is a compile error.

**CSS system colours** (CSS Color 4 §6.2) are roles, case-insensitive as
CSS keywords are. Each has an Apple mapping, because the web is the oracle
and native must answer too:

| Keyword | iOS (UIKit) | macOS (AppKit) | Web |
|---|---|---|---|
| `Canvas` | `systemBackgroundColor` | `textBackgroundColor` | as is |
| `CanvasText` | `labelColor` | `textColor` | as is |
| `LinkText` | the inherited `tintColor` (LLP 1115 §3: UIKit draws text links in the tint; was `linkColor`) | `linkColor` | as is |
| `GrayText` | `tertiaryLabelColor` | `disabledControlTextColor` | as is |
| `Highlight`, `HighlightText` | the tint at 0.2 alpha (stand-in), `labelColor` | `selectedTextBackgroundColor`, `selectedTextColor` | as is |
| `AccentColor`, `AccentColorText` | the inherited `tintColor`, white | `controlAccentColor`, `alternateSelectedControlTextColor` | as is |
| `Field`, `FieldText` | `secondarySystemBackgroundColor`, `labelColor` | `textBackgroundColor`, `textColor` | as is |
| `ButtonFace`, `ButtonText` | `secondarySystemFillColor`, `linkColor` | `controlColor`, `controlTextColor` | as is |

**Declared deviation:** dark `Canvas` is `#000000` on iOS (`systemBackground`)
and `#121212` in Chrome. Native follows the platform; LLP 1001 records it.

CSS's remaining system colours are **refused** in r1 as having no
meaningful Apple counterpart:
- `ButtonBorder`, `Mark`, `MarkText`
- `SelectedItem`, `SelectedItemText`
- `ActiveText`, `VisitedText`

CSS's deprecated system colours (`Background`, `Window`, …) are refused too.

**Exact roles**, where CSS has no name: lowercase, hyphenated, and closed.

| Role | iOS (UIKit) | macOS (AppKit) | Web (Safari: WebKit's own keyword) | Fallback light / dark |
|---|---|---|---|---|
| `label` | `labelColor` | `labelColor` | `-apple-system-label` | `#000000` / `#ffffff` |
| `secondary-label` | `secondaryLabelColor` | `secondaryLabelColor` | `-apple-system-secondary-label` | `#3c3c4399` / `#ebebf599` |
| `tertiary-label` | `tertiaryLabelColor` | `tertiaryLabelColor` | `-apple-system-tertiary-label` | `#3c3c434d` / `#ebebf54d` |
| `quaternary-label` | `quaternaryLabelColor` | `quaternaryLabelColor` | `-apple-system-quaternary-label` | `#3c3c432e` / `#ebebf529` |
| `placeholder` | `placeholderTextColor` | `placeholderTextColor` | `-apple-system-placeholder-text` | `#3c3c434d` / `#ebebf54d` |
| `separator` | `separatorColor` | `separatorColor` | `-apple-system-separator` | `#3c3c434a` / `#54545899` |
| `background` | `systemBackgroundColor` | `windowBackgroundColor` | `-apple-system-background` | `#ffffff` / `#000000` |
| `secondary-background` | `secondarySystemBackgroundColor` | `controlBackgroundColor` (lighter than the window in light mode: declared) | `-apple-system-secondary-background` | `#f2f2f7` / `#1c1c1e` |
| `grouped-background` | `systemGroupedBackgroundColor` | `windowBackgroundColor` | `-apple-system-grouped-background` | `#f2f2f7` / `#000000` |
| `secondary-grouped-background` | `secondarySystemGroupedBackgroundColor` | `controlBackgroundColor` | `-apple-system-secondary-grouped-background` | `#ffffff` / `#1c1c1e` |
| `fill` | `systemFillColor` | `systemFillColor` | `-apple-system-fill` | `#78788033` / `#7878805c` |
| `secondary-fill` | `secondarySystemFillColor` | `secondarySystemFillColor` | none (WebKit has no such keyword; §12) | `#78788029` / `#78788052` |
| `system-red`, `-orange`, `-yellow`, `-green`, `-mint`, `-teal`, `-cyan`, `-blue`, `-indigo`, `-purple`, `-pink`, `-brown`, `-gray` | `systemRedColor`, … | `systemRedColor`, … | `-apple-system-red`, … | Apple's published light / dark values |

The macOS rows are AppKit's own semantics, which are not iOS's. Where they
differ visibly (`controlBackgroundColor`), the table says so. Open question
§6.3 asks for a side-by-side on macOS 26.

**There is no `accent` role.** `AccentColor` is the accent: on iOS it is
the view's inherited `tintColor`, which is what an app's `accent-color` row
already sets. So `accent-color` stays the way to *set* the accent, and
`AccentColor` the way to *use* it. A row cannot be both, so there is no
cycle (`accent-color: AccentColor` is refused).

**The web** emits a CSS system colour as is. For an Exact role it emits
`var(--exact-<role>)`, defined by a stylesheet Exact ships:
- **Safari:** WebKit's `-apple-system-*` keyword, which is the dynamic colour
  itself.
- **Elsewhere:** `light-dark(<fallback>)`, redefined under
  `@media (prefers-contrast: more)` with the higher-contrast values, so the
  web does not ignore Increased Contrast.

The web JS target, which writes bound values through JS maps
(`host/web-js/src/rows.rs:208-214`), uses the same names, generated from
the same table.

### D3 — `platform-color()`: every platform colour, safely, with a CSS fallback

```
platform-color(<platform> <name> [, <platform> <name>]* , <fallback>)
<platform> = ios | macos | web        (r1; android, windows in Appendix A)
<name>     = an Objective-C class colour property (ios, macos) | named:<Asset> | a CSS colour (web)
<fallback> = a literal colour or light-dark() pair; not a reference
```

**Literal only in r1.** `platform-color()` may not come from a data source
or a computed value. Contract refuses one built by a template, a
concatenation or a call (`lower-platform-color-literal`); state may still
choose between literals (a `?:` or `match` arm, a class). The runner admits
a `platform-color()` in a style value only when the value is one of the
plan's own string literals (`exact_runner::bridge::set_plan_style`); any
other is invalid at computed-value time, the row unset with a journal line.
A plan's names are fixed when the app compiles, so data can never choose
which selector is called. (The kernel's parser itself accepts the function:
it is also the compiler's check and the wire's decoder.)

**Safe runtime lookup on Apple:**
- **Spelling:** names are the Objective-C class properties, the
  `…Color` spelling, one canonical form on both platforms.
- **Refused by Contract:** a leading `_`, and any name that is not
  `[a-z][A-Za-z0-9]*Color`.
- **Checked at runtime:** the host resolves a name only if
  `class_getClassMethod` finds a zero-argument class method on
  `UIColor`/`NSColor` returning an object, and the result `isKindOfClass:` a
  colour. Anything else is the fallback, logged once.
- **What can and can't be reached:** private selectors and `+new`/`+alloc`
  are unreachable. A colour added in iOS N resolves on iOS N with no Exact
  change.
- **`named:<Asset>`:** `UIColor(named:)` / `NSColor(named:)`, from the app's
  asset catalogue.

A platform with no entry, a name the OS lacks, or a name that is not a
colour: **the fallback**. The fallback is **required**
(`lower-platform-color-fallback`), and it is also the reference's kernel
fallback pair (D1).

### D4 — `theme`: an app's own colours, named once, able to override a role

```
theme
  brand = platform-color(ios named:BrandColor, macos named:BrandColor, #c8102e)
  card = "secondary-grouped-background"
  defrost-on = light-dark(#d0e2f7, #27394f)
  secondary-label = platform-color(ios secondaryLabelColor, light-dark(#6b6b73, #a1a1aa))
```

- **Declared once**, at the app root, like `keyframes` and `style`. Theme
  names are colour keywords too, written as strings: `color="brand"`.
- **An entry may be any colour:** a role, a `platform-color`, a pair, hex.
- **An entry named like a role overrides that role for the whole app.**
- **Cycles are refused:** an entry that reaches itself through
  others (`a = "b"`, `b = "a"`), and a role override that names its own role
  (`secondary-label = platform-color(ios secondaryLabelColor, "secondary-label")`,
  which has a reference as a fallback and is refused there as well).

**The theme ships in the plan** as part of the reference table: each entry
is a reference row, and a role an app overrides has its row replaced. So
"the whole app" really is the whole app, on every target:
- the kernel's runtime parser resolves a dynamic `"brand"` or
  `"secondary-label"` from a data source against the plan's table
- the web JS target resolves against the same table, emitted as generated
  JS

A dynamic string that names neither a role nor a theme entry is an invalid
colour, as a malformed hex is today.

This replaces a per-app palette function (`c(name)` returning hex pairs)
with declarations Contract checks.

### D5 — The host resolves per view, against a trait set, everywhere it paints

A reference resolves against the **view that shows it**, and re-resolves
when that view's traits change. The trait set is:
- **style:** light or dark
- **contrast:** normal or increased
- **level:** base or elevated (iOS)
- **material:** under a material or not (vibrancy, LLP 1077 D13)

**The Apple host is built on `drawsDark: Bool` and RGBA arrays today.**
These sites change from `dark: Bool` to a trait set, and from RGBA to
"RGBA or reference":
- `BatchValue.channels(dark:)` / `isSchemeColor` (`Batch.swift:20-25`) and
  `TextEngine.color([Double])` (`Text.swift:717`)
- the text spec's `color: [Double]` (`Text.swift:83`) and the `TextPaint`
  cache key (`TextResidency.swift:116-123`)
- inline runs (`InlineText`) and their dark colours
- `BoxShadowSpec.list(dark:)`, `gradient.apply(…dark:)`, and the
  `BoxMaskState.Key(dark:)` cache
- `SvgScene.color(_:dark:)`
- the symbol look key (`Affordances.swift:22, 35`)
- `RegionController.appearance` / `RegionTextSource.capture(dark:)`
  (`RegionController.swift:164, 276`)
- on the Rust side, `style_json_presented` and `push_color_value`
  (`host/apple/src/style.rs:401, 647`), which carry a reference across the
  ABI as `{"ref": <RefId>}` beside its fallback pair

**On iOS:**
- **Resolution:** `UIColor(dynamicProvider:)`-backed system colours are
  resolved with `resolvedColor(with: traitCollection)`. Exact draws boxes as
  `CGColor` layers and never sets `UIView.backgroundColor`
  (`NodeViewIOS.swift:489-490`), so every site above resolves explicitly.
- **Trait changes:** registered with `registerForTraitChanges` on
  `userInterfaceStyle`, `accessibilityContrast` and `userInterfaceLevel`.
  These invalidate the text caches (`invalidateText`, as LLP 1034 D2 does
  for a scheme change) and the layer colours.

**On macOS:**
- **Resolution:** under the view's `effectiveAppearance`, using
  `NSAppearance.performAsCurrentDrawingAppearance`.
- **Increased Contrast:** changes `effectiveAppearance` to a high-contrast
  variant, so `drawsDark`'s `bestMatch([.aqua, .darkAqua])`
  (`NodeViewMac.swift:766`) becomes a trait set that keeps contrast.
- **Accent and selection colours:** changes post
  `NSColor.systemColorsDidChangeNotification`, which the host observes too.

**After resolving,** the host **reports** (D1) any reference that the kernel
uses for motion or a filter on that view, and caches resolutions per
`(RefId, trait set)`. A thousand rows of `secondary-label` resolve once per
trait set.

### D6 — Motion follows LLP 1062: a trait change transitions; keyframes keep their values

LLP 1062 settled colour motion, and r1 contradicted it:

- **A trait change transitions, as an appearance change does.** LLP 1062 D4:
  "An appearance change transitions, as in Chrome", implemented by
  `paint_resync`. A reported resolution (D1) for a trait set the view has
  just entered is the new target, so a `transition` on a role row eases from
  the old platform colour to the new one. With no `transition`, it changes
  at once.
- **Keyframe colours stay compile-time** (LLP 1062 D9). A reference is
  refused inside `keyframes` in r1 (`lower-keyframe-color-ref`). The engine
  folds keyframe colours to floats when the app compiles
  (`motion/src/color.rs`), and a reference has no value then. The natural
  follow-up is a keyframe holding a reference and seeded from its reported
  resolution; it is not needed for roles to be useful.
- **The web** transitions a system-colour change under `transition`, as
  LLP 1062 D4 records for appearance; nothing here widens that.

### D7 — The agent pins traits, so tests are deterministic

UIKit and AppKit dynamic colours resolve against real traits and ignore
Exact's display preferences (`DisplayPreferences.agentContrast`,
`Agent.swift:311, 329`). Under the agent:
- **iOS:** `prefer contrast more` applies
  `traitOverrides.accessibilityContrast` (iOS 17, the deployment target) to
  the session's window scene. `prefer elevation elevated` applies
  `traitOverrides.userInterfaceLevel`.
- **macOS:** `prefer contrast more` sets a high-contrast `NSAppearance` on
  the session's window.
- **The accent:** pinned to the system default under the agent, and reported.
- **`layout <node>`** reports each colour row's reference, what it resolved
  to and from where. For example:
  `{"ref": "secondary-label", "resolved": "#3c3c4399", "source": "secondaryLabelColor", "traits": "light/normal/base"}`,
  or `"source": "fallback"`.
- **Linux and headless runs only ever show fallbacks**, stated, so a Linux
  screenshot is the fallback palette by definition.

### D8 — Initial values are system colours where the platform has them

**Today** (`kernel/tables/schema.json`, ide/main `0e64493b`):
- **`color`** (`text_color`, l. 2436) defaults to `255`, opaque `#000000`.
  An unstyled `text` is black on native in dark mode too, so it is invisible
  on a dark background.
- **`tint-color`** (`tint_color`, l. 2498) defaults to `255`, opaque black.
  An untinted SF Symbol is black.
- **The Apple host repeats the snapshot** where a row is absent:
  - `NodeText.swift:119`: `channels("text_color", …) ?? [0, 0, 0, 255]`
  - `Affordances.swift:28`: `color("tint_color", .black)`

**CSS already says what these are.** The initial value of `color` is
`CanvasText` (CSS Color 4 §6.2). A browser resolves that against the OS and
`color-scheme`, so on the web the default was never black. Exact copied
CSS's light-mode result instead of CSS's value.

**Decision:**
- **`color`** starts as the `CanvasText` role. iOS resolves it as
  `labelColor`, macOS as `textColor`, and the web leaves it to the browser.
- **`tint-color`** starts as `AccentColor`, the platform's tint (ruling,
  2026-10-03). iOS lets an untinted symbol inherit the view hierarchy's
  `tintColor`, so an app or window tint and its dynamic variants apply with
  nothing resolved by Exact; macOS uses `controlAccentColor`; the web CSS's
  `AccentColor`. An untinted symbol is therefore the accent (blue by
  default), as a bare `UIImageView` is in UIKit; a symbol that should follow
  text sets `tint-color="CanvasText"`.
- **The schema can say so.** A `color` codec default may name a role
  (`"default": "CanvasText"`), and `build.rs` generates `ColorValue::Role(id)`
  for it. The table stays the single authority for initial values.
- **The hosts' hardcoded fallbacks go.** Every `?? black` site asks the role
  instead, so no native path invents its own default.
- **Where a platform has no system colour, the role's fallback pair is the
  initial value.** On Linux and headless runs that's `#000000` / `#ffffff`, so
  an unstyled `text` becomes legible in dark mode there too.
- **SVG paint keeps SVG's initial values.** `fill: black`, `stop-color: black`,
  `flood-color: black` and `lighting-color: white` are the SVG and Filter
  Effects specs' initial values, and a drawing's black is a drawing's black,
  not a text colour. `currentColor` stays the way to make an icon follow
  text.
- **Inheritance** is unchanged: `color` inherits the reference, not a
  resolved value, so a child in an elevated sheet resolves `CanvasText`
  itself (D1).

**Cost:** a schema change, so the compatibility id moves and native hosts
need a new binary (LLP 1030 D3a). Pre-1.0, that is accepted.

**Alternative rejected: `label` instead of `CanvasText`.** On Apple they
resolve to the same colour. But `CanvasText` is CSS's own initial value, so
the web and the kernel agree with a browser by construction.

### D9 — References are resolved by the host everywhere they paint (built, except keyframes)

§9 lists the paths where stage 1 still paints a role's fallback
pair:
- **paint motion:** colour transitions, and their targets from
  `color_targets(node, dark)` (`kernel/src/motion.rs:187, 207, 242`);
- **keyframe colours** (refused for references by D6 r1);
- **gradient stops:** `s.color.resolve(dark)` (`kernel/src/gradient.rs:322`);
- **SVG paint and filters:** `Paint::resolve` (`kernel/src/svg/mod.rs:274-275`)
  and the scene's `Resolved::Fallback` (`kernel/src/svg/scene.rs:673`);
- **shadow colours.**

The ruling agrees with 3. The approach is D1's report path:
- **Reporting:** the host resolves each reference per view and trait set and
  reports what it showed.
- **Kernel:** keeps `(reference, traits) → Color` and re-seeds motion,
  gradients, SVG scenes and filters from it.
- **Trait changes:** a change of style, contrast or level re-resolves and,
  where a `transition` applies, transitions (LLP 1062 D11).

**As built (2026-10-03):**
- **The table.** The kernel keeps one process-wide table of what the
  presenter reported for each reference, keyed by reference and appearance
  (`kernel/src/style/roles.rs`). Contrast is process-wide, so it isn't in
  the key: the presenter re-reports the whole set when it changes.
- **One resolution path.** `ColorValue::resolve` returns the reported colour
  if there is one, else the fallback pair. Paint motion endpoints, gradient
  stops, SVG scene paint and SVG filter colours all resolve through it, so
  they now show the platform's value.
- **The report.** On every appearance report and preference change, the
  Apple presenter asks for the references in use (`exact_color_references`),
  resolves each light and dark through UIKit/AppKit, and reports them
  (`exact_colors`). A name the platform doesn't know isn't reported and keeps
  its fallback. iOS's `ExactViewIOS` now also listens for contrast changes.
- **What a changed report does** (`host/apple/src/colors.rs`): paint motion
  re-targets and transitions to the new colour (LLP 1062 D11); every SVG
  scene already sent is rebuilt; nodes whose background or mask gradient
  names a reference, or whose box `filter` has a `drop-shadow` resolving
  one (its own colour, or `currentcolor` under a referenced `color`), are
  restyled. The table is the process's but what was resolved from it is
  each session's, so the table keeps a generation and each session
  re-presents when it moved since that session last did: with two embedded
  sessions, the second's identical report still refreshes it. Text colours, borders, shadows and the
  symbol palette already crossed the wire by name and were resolved per view
  in Swift, so they needed nothing.
- **Fixed on the way:** composite rows (gradients, masks, shadows, the
  symbol palette, SVG paint, `filter`) crossed the binary wire
  (`StyleProps::encode_patch`: `Op::SetStyle`, plan export, shared styles)
  as their browser CSS, which writes a `platform-color()` as its web colour
  or fallback, so the reference was lost. They now cross as their wire text
  (`wire()`, `ColorText::Wire`), which keeps the function as written; the
  browser's CSS is unchanged. The filter `drop-shadow` colour was a fixed
  `Color`, refusing a role, `platform-color()` and `light-dark()`; it is a
  `ColorValue` now.
- **Fixed on the way:** SVG scenes never redrew their light/dark pairs on an
  appearance change; `SvgHost.appearance` now does, on iOS and macOS.
- **Web and Linux:** unchanged. The web emits `var(--exact-<role>)` and the
  browser resolves it; Linux and headless runs report nothing and keep the
  fallback pair.
- **Tests:** a kernel test (a report replaces the fallback until withdrawn)
  and an Apple host test (a report restyles a gradient, rebuilds an SVG
  scene, starts a colour transition, and sends nothing when repeated). On
  the iOS simulator, a gradient, an SVG circle and a box sampled iOS's own
  `systemOrange`/`systemBlue` in all four states (light, dark, each with
  Increased Contrast), e.g. gradient 250,140,45 → 194,83,6 under Increased
  Contrast; a dark-mode colour flip transitioned between the platform's
  colours.

**Not built yet:**
- **Keyframes.** A keyframe colour may be a reference: its frames are seeded
  from the host's report when the animation starts and re-seeded on a trait
  change. This supersedes D6's refusal and amends LLP 1062 D9
  ("known at compile time" becomes "known when the animation starts").
  Contract still refuses it; the web needs the substitution too.
- **Content-region text rasters** resolve when built and don't redraw on a
  new report.
- **The elevated level** (sheets, popovers): the kernel's table resolves at
  the base level only.
- **SVG scene filter colours** still resolve in the light appearance (a
  box `filter` resolves in both).
- ~~**A `platform-color()` interned after the last report**~~ The runner
  interns every `platform-color()` in the plan's literals at boot (a branch
  not yet taken, a gradient's stop), so the first report resolves it.

### D10 — Relative colour and `color-mix()` over references (documented, not built)

**The need, from the first app.** A vehicle app draws 15% washes of the
system hues behind banners and icons, and an active chip as a blue wash over
a fill. With no way to say "system orange at 15%", each is a hand-written
pair (`light-dark(#ff950026, #ff9f0a26)`). That pair:
- ignores Increased Contrast, where `systemOrange` darkens;
- drifts when the OS retunes its palette, which iOS 26 did;
- doesn't match the solid `system-orange` beside it once either moves.

§4 left this out deliberately ("mixing needs resolution first on
every host"). Stage 1 built that resolution, so mixing can now be built.

**Authoring: CSS's own two forms, in a subset.**

```
background-color="rgb(from system-orange r g b / 15%)"     // CSS Color 5 §4
background-color="color-mix(in srgb, system-blue 15%, quaternary-fill)"   // §3
```

- **Relative colour:**
  - **Origin:** any colour, including a role, `platform-color()`,
    `currentcolor` or a pair.
  - **Function:** `rgb()` in r1.
  - **Channels:** each `r g b` channel keyword passed through unchanged.
  - **Alpha:** a number, a percentage, or `alpha` times a number
    (`calc(alpha * 0.5)`).
  - This covers every case the first app has. Channel arithmetic, `hsl()`,
    `oklch()` and `lab()` relative forms are refused in r1
    (`lower-relative-colour-form`); they follow D10's own wire form once §10's
    wide colour row exists.
- **`color-mix()`:**
  - **Inputs:** two colours, each optionally weighted.
  - **Spaces:** `in srgb` and `in oklab`. `oklab` is what CSS recommends for
    perceptual mixes; `srgb` is what a designer's 15%-over-a-fill means.
  - **Refused:** other spaces, and hue-interpolation keywords.
- **Nesting:** an origin may itself be relative, to a depth of 2. Deeper is
  refused, which keeps the wire form bounded.
- **Literal or computed:** both are allowed. The runtime parser takes them
  too, as it takes `light-dark()`, because a wash's percentage may come from
  data.

**Wire and kernel.**
- `ColorValue` stays `Copy + Eq` (D1). A mix becomes
  `ColorValue::Mix(u16)`, an index into an interned table of
  `{ op, inputs: [ColorValue; 2], weights, alpha }`, capped like
  `platform-color()`'s table.
- Its fallback pair is computed eagerly from the inputs' fallback pairs. So
  the kernel, Linux and the agent always have a value, by the same arithmetic
  a host would use, in the stated space.
- The Apple wire carries `{"mix": …, "c": <fallback pair>}`, beside stage 1's
  `{"sys": …, "c": …}`.

**Per host.**
- **iOS:** one dynamic colour,
  `UIColor { traits in base.resolvedColor(with: traits).withAlphaComponent(a) }`,
  or for a mix, both inputs resolved with the same `traits` and then mixed.
  The OS calls the provider for each trait set it draws in, so contrast,
  appearance, level and materials flow through without Exact re-resolving.
- **macOS:** `NSColor(name: nil, dynamicProvider: { appearance in … })`, the
  same way.
- **Web:** CSS relative colour and `color-mix()` are native in every
  evergreen browser. Exact emits the authored function, with the origin
  written as `var(--exact-<role>, <fallback>)` (D11), so the browser resolves
  the whole thing against `prefers-color-scheme` and `prefers-contrast`.
- **Linux and headless:** the eager fallback pair.
- **Interpolation:** paint motion between two mixes interpolates their
  resolved colours (LLP 1062), as D9 does for a role.

**Declared deviation:** none for the web, since the syntax is CSS's own.
The subset and the depth cap are declared in LLP 1001.

**Alternatives rejected:**
- **An alpha argument on roles** (`system-orange/0.15`). Stage 1's `@tint/0.2`
  is internal shorthand for `Highlight`; making it public invents syntax CSS
  already has.
- **`theme` entries with fixed alphas.** A theme names a colour once (LLP
  D4), but its value would still be a snapshot.
- **Resolving in the kernel and shipping RGBA.** That is today's snapshot,
  moved.

### D11 — The web role sheet (documented, not built)

**Today** the web writes an Exact role as `var(--exact-<role>, light-dark(<fallback>))`
(`host/web/src/css.rs:529`), and nothing defines `--exact-<role>`. So every
browser shows the fallback pair: it follows dark mode but ignores
`prefers-contrast`, and Safari doesn't use its own dynamic colours. CSS's
system colours (`CanvasText`, `AccentColor`, …) are written as is and are
already right.

**Decision.** Exact ships one stylesheet, generated from the schema's
`colors` table by `kernel/build.rs`, so the table stays the single source:

```css
:root {
  --exact-secondary-label: light-dark(#3c3c4399, #ebebf599);
  …
}
@media (prefers-contrast: more) {
  :root { --exact-secondary-label: light-dark(<high-contrast light>, <high-contrast dark>); … }
}
@media (forced-colors: active) {
  :root { --exact-secondary-label: GrayText; --exact-label: CanvasText; … }
}
@supports (color: -apple-system-secondary-label) {
  :root { --exact-secondary-label: -apple-system-secondary-label; … }
}
```

- **Values:** the high-contrast pairs are Apple's own Increased Contrast
  values, so they need two new columns in the table, measured from UIKit.
- **Order:** the `@supports` block comes last, so Safari uses WebKit's
  dynamic keywords, which follow the OS's Increased Contrast and accent
  themselves.
- **`forced-colors`:** maps each role to the nearest CSS system colour. In
  Windows High Contrast the user's palette wins, as the platform expects.
- **Loading** follows LLP 1047, pay for what you use:
  - The build knows which roles a plan names, from literals and the plan's
    string table, and emits only those properties, inline in the page's head
    CSS. That's a few hundred bytes for a typical app.
  - A plan whose computed colours may name any role takes the whole sheet,
    about 3 KB uncompressed.
  - The web JS target emits the same sheet from the same generator.
- **`light-dark()`:** roles stay `light-dark()` pairs inside the sheet, so
  they resolve against `color-scheme` exactly as authored pairs do (LLP 1034),
  and the page's `color-scheme` stays the one switch.
- **Relative colour (D10):** composes for free, because an origin written as
  `var(--exact-<role>, …)` follows whatever the sheet says.

**Alternative rejected: JavaScript that reads `matchMedia` and sets the
variables.** It runs before first paint and adds work at boot (RULES: the
boot path executes nothing), for what CSS does on its own.

## 3. The compatibility id and the wire

**The cost:**
- The colour codec gains a discriminant, and the plan gains its reference
  table.
- The kernel schema digest changes, so every native host needs a new
  compatibility id and a new binary (LLP 1030 D3a), as LLP 1034 D5 did.
  Pre-1.0, that is the cost.

**What needs a binary and what doesn't:**
- **A new role** is a schema change, so it needs a binary. That is
  deliberate: roles are curated.
- **A `platform-color` name or a theme entry** is plan data, which a bundle
  can carry and update over the air. That is the point: the open-ended set
  never needs a binary.

## 4. What this deliberately does not do

- **No per-element `color-scheme`** (LLP 1034 §3 stands).
- **No automatic contrast correction.** A role is what the platform says.
- **No asset-catalogue generation.** `named:` reads what the bundle already
  has.
- **No vibrancy beyond LLP 1077 D13's scope.** A role under a material
  resolves with the material in its trait set.
- **No change to SVG paint initial values** (D8): `fill` stays black, as the
  SVG spec and the web have it.

## 5. Landing order

1. **Stage 1, built:** D1 (references), D2 (roles), D3 (`platform-color()`),
   D5 (host resolution per view), D6 (trait changes transition), D7 (the
   agent pins traits).
2. **Stage 2, built:** D8 (initial values as system colours, the accent
   tint) and D9 (host-reported resolutions in paint motion, gradients and
   SVG).
3. **D9's keyframes:** a keyframe colour may name a reference, seeded when
   the animation starts (amends LLP 1062 D9).
4. **D11, the web role sheet,** with the two high-contrast columns and the
   `@supports` block. Tests: conformance fixtures under
   `prefers-contrast: more` and `forced-colors`, Chromium and WebKit.
5. **D10, relative colour and `color-mix()`,** and the `Mix` table. Tests:
   iOS dynamic providers compared with UIKit's `withAlphaComponent` in four
   trait sets; the web's native output.
6. **D4, `theme`,** when an app needs one.
7. **§10's fidelity fixes,** in their own order (§10.6).

## 6. Open questions for the maintainer

**The principle, from the app's owner (2026-10-03):** follow each platform's
own guidelines wherever they speak (Apple's Human Interface Guidelines, and
Material and Fluent when those hosts land). An Exact choice is made only
where a guideline is silent, and is then declared. Each question below has a
recommendation on that basis; the maintainer decides.

1. **What is `Highlight` on iOS?**
   - **Why it's open:** UIKit has no named selection colour. The HIG says
     selection follows the app's tint, and UIKit's text selection draws the
     tint at a reduced alpha.
   - **Recommendation:** follow UIKit. `Highlight` is the inherited
     `tintColor` at the alpha UIKit's own selection uses on the running OS,
     measured, not guessed. `HighlightText` is `labelColor`. Measure on iOS
     26 and 27 before fixing the alpha in the table.
2. **Should roles be vibrant on glass and other materials by default?**
   - **Why it's open:** the HIG's Liquid Glass and materials guidance says
     text and symbols on a material use vibrant label and fill colours, and
     UIKit does this for its own controls.
   - **Recommendation:** yes. A role resolved under a material
     (`backgroundMaterial`, a glass button or group) is the vibrant variant,
     as LLP 1077 D13 already intends for the `-apple-system-*` labels, and
     the trait set (D5) carries the material. A non-role colour (hex, a pair,
     a `platform-color`) is never made vibrant. Confirm by side-by-side with
     a native glass button and a sheet on iOS 26.
3. **Which AppKit colours are the macOS grouped backgrounds?**
   - **Why it's open:** the HIG's macOS guidance has grouped forms (System
     Settings) draw their groups as raised rounded boxes on the window
     background. AppKit has no `systemGroupedBackground`.
   - **Recommendation:** match what AppKit's own grouped forms draw.
     `grouped-background` is `windowBackgroundColor`;
     `secondary-grouped-background` is the colour a `.grouped` SwiftUI
     `Form`'s section uses on macOS 26, measured and named in the table (or
     declared as a stand-in if it is not a public colour). `controlBackgroundColor`
     only if the measurement says so.
4. **Should Contract warn when `platform-color` omits a platform the app
   ships?**
   - **Recommendation:** yes, a lint, not an error, since the fallback
     always renders. The lint's message points at the role table first: if
     a role covers the need, the guidelines' own semantic colour beats a
     hand-picked one on every platform.
5. **Where a platform's guideline and CSS disagree, which wins?**
   - **Example:** dark `Canvas`, `#000000` on iOS and `#121212` in Chrome.
   - **Recommendation:** the platform, on that platform. That's the
     principle above. The web stays the oracle for *semantics* (which role,
     inheritance, `currentcolor`, motion), not for a native platform's
     palette values, and each such difference is declared in LLP 1001. Confirm
     this reading of "the web is the standard" for colour values.

**From stage 2 (D8–D11, §10):**

1. **`tint-color`'s initial value (D8).** Built as `AccentColor`, as UIKit
   does: an untinted symbol is the app's tint, where it used to be black.
   Apps whose icons should read as text set `tint-color="CanvasText"`.
2. **Which system colours are P3?** Measure UIKit's system hues and fills in
   all four trait sets on iOS 26 and 27, and record which have components
   outside sRGB. That decides how urgent 10.1 is.
3. **The relative-colour subset (D10):** is `rgb(from X r g b / a)` plus
   `color-mix(in srgb|oklab, …)` enough for r1, or should `oklch()` relative
   forms come with 10.2?
4. **Keyframe references (D9)** amend LLP 1062 D9. Does the maintainer accept
   "known when the animation starts" in place of "known at compile time"?
5. **High-contrast columns (D11):** take Apple's Increased Contrast values for
   every browser, or only under `@supports (-apple-system-*)`'s absence?
   Recommended: everywhere, since the platform's own palette is the most
   considered one available.
6. **Wide images (10.3):** worth 2× memory for wide-gamut sources by default,
   or opt-in per image?

## 7. Scope of r1

**In:**
- the reference value and its table
- Apple and web roles, with CSS's admitted keywords
- `platform-color` for `ios`, `macos` and `web`, literal only
- `theme`, shipped in the plan
- the kernel report path
- the agent's trait pinning

**Out, until a host or a need exists:**
- Android and Windows columns, keywords and lookups (Appendix A)
- `platform-color` in dynamic values
- references in keyframes
- the Linux desktop accent

## 8. Review fold (r1 → r2)

One adversarial review of r1 against the source (NOT READY). Every finding
is taken:

| Finding | r2 |
|---|---|
| The kernel resolves colours (paint motion, SVG filters, Linux); "never resolves" was false (BLOCKER) | D1: every reference has a fallback pair; hosts report resolutions; motion and filters use them |
| D6 contradicted LLP 1062 D4 and D9 (BLOCKER) | D6 follows 1062: trait changes transition; keyframe references refused |
| CSS system colours had no native mapping (MAJOR) | D2: a row for every admitted keyword; the rest refused; the `Canvas` deviation declared |
| LLP 1077 D13's `SYSTEM_COLORS` already shipped copied pairs (MAJOR) | D2: the role table replaces it; the six names become aliases |
| A boxed reference breaks `ColorValue`'s `Copy` semantics (MAJOR) | D1: interned `Ref(u16)`; the parsers to change are listed |
| The Apple host's `dark: Bool` sites were unnamed; `backgroundColor` was assumed (MAJOR) | D5: each site listed; resolution is explicit everywhere |
| Theme semantics contradicted for dynamic values and the web JS target; no cycle rule (MAJOR) | D4: the theme ships in the plan on every target; cycles refused |
| Data could choose an Objective-C selector (MAJOR) | D3: literal only; Objective-C spellings; introspection checks; `_` refused |
| The agent could not pin contrast or elevation (MAJOR) | D7 |
| Platform facts (AppKit fills, `controlBackgroundColor`, `systemColorsDidChangeNotification`, Android MDC attributes, WinUI lookup) | D2, D5, Appendix A corrected |
| CSS: §6.3 citation, `AccentColor` support, case, `prefers-contrast` | Related fixed; D2's web sheet; deprecated keywords refused |
| Role names collide with Contract variables | D2: roles are strings |
| `accent` contradicted itself | D2: no `accent` role; `AccentColor` uses, `accent-color` sets |
| The colour row list was wrong (`outline-color` has no row; filter and stop colours missing) | D1 |
| No Implementer; first app unnamed | header; §5.6 |
| Scope: cut Android, Windows and dynamic `platform-color` from r1 | §7 |

## 9. As built (stage 1, 2026-10-03; stage 2 is D8 and D9)

Built on the fork's `ide/main` and used by a real app (a vehicle app's
whole palette) to validate the API.

**Built:**
- **D2 roles.** `kernel/tables/schema.json`'s `colors` table: CSS's twelve
  system colours D2 maps, and 31 Exact roles (the labels, `placeholder`,
  `separator`, `opaque-separator`, `link`, the backgrounds and grouped
  backgrounds in three levels, four fills, thirteen system hues). `build.rs`
  generates `COLOR_ROLES`. `SYSTEM_COLORS` is deleted; WebKit's
  `-apple-system-*` names are aliases.
- **D1, as a role id and an interned table.** `ColorValue::Role(u8)` indexes
  the schema's table; `ColorValue::Platform(u16)` indexes a process table of
  `platform-color()`s, capped at 1024, which the wire carries as the
  function's canonical text. `ColorValue` stays `Copy + Eq`. Every parser
  meets them through `parse_light_dark`, so every colour row and embedded
  colour takes them. **Deviation from D1:** no plan-carried table yet. A
  role's id is the schema's, which the schema digest already pins; a
  `platform-color()` is re-interned from its text on decode.
- **D3 `platform-color()`.** Names are checked as D3 says
  (`[a-z][A-Za-z0-9]*Color`, `named:<Asset>`); the fallback is required and
  is never a reference.
- **D5 on Apple.** The Apple wire carries a reference as
  `{"sys": <class property>, "c": <fallback>}`. `SystemColor.swift` looks the
  name up (`class_getClassMethod`, zero arguments, the result a colour) and
  resolves it against style, Increased Contrast and, on iOS, the view's
  user-interface level, cached per trait set. Nodes re-apply on a change of
  style, contrast or level. Inline runs resolve per appearance as they are
  read.
- **D7 contrast.** `prefer contrast more` sets the scene's contrast trait on
  iOS and a high-contrast appearance on macOS.
- **The web** writes a CSS system colour as is and an Exact role as
  `var(--exact-<role>, light-dark(<fallback>))`, which reads back as the role.

**Stage 2, initial values (2026-10-03):** `color` starts as `CanvasText`
and `tint-color` as `AccentColor` (`ColorValue::Role`; the schema's
`default` may name a role), so an unstyled node is the platform's label
colour and an untinted symbol the platform's tint, resolved for appearance
and Increased Contrast, never a snapshotted black. The Apple presenter's
fallback for an absent `color` is `UIColor.label` / `NSColor.textColor`; an
untinted symbol inherits UIKit's `tintColor` (a window or app tint
included) and uses `controlAccentColor` on macOS. SVG
paint keeps SVG's initials (`fill` black), as the web does.

**Not built yet:**
- **D4 `theme`.** The app needed none: every colour it named is a role.
- **The web's role sheet** (Safari's `-apple-system-*`, `prefers-contrast`).
  Without it the web shows each role's fallback pair, which is correct but
  not dynamic.
- ~~**Host-reported resolutions (D1).**~~ Built 2026-10-03, see below.
- **The lint** `lower-keyframe-color-ref`. (`lower-platform-color-literal`
  is built, with the runner's admission of plan literals only, D3.)

**What the app found:**
- **A role at an alpha is missing.** The app's 15% washes behind banners and
  icons (`system-orange` at 15%) stay hand-written `light-dark()` pairs, so
  they do not follow Increased Contrast. CSS answers this with relative
  colour (`rgb(from <colour> r g b / 15%)`) or `color-mix()`. The
  recommendation is relative colour over a reference, resolved by the host
  after the reference is.
- **Roles covered everything else**, including fills (`tertiary-fill`,
  `quaternary-fill`) the r2 table lacked. They are added.

**Host-reported resolutions (D1, D6), built 2026-10-03:**
- The presenter resolves every reference (each role, each interned
  `platform-color()` with a name for its platform) in both appearances under
  the current contrast and reports them (`exact_color_references`,
  `exact_colors`): on every appearance report and every preference change,
  so Increased Contrast and whatever else the platform adjusts flow through.
  A name the platform lacks is not reported, and keeps its fallback.
- `ColorValue::resolve` answers a reference with the report, else the
  fallback pair (`style/roles.rs`, one table per process). So what the
  kernel resolves itself follows the platform: paint motion's endpoints,
  gradient stops, SVG scene paint and filter colours.
- A changed report re-targets paint motion (transitioning, D6), rebuilds SVG
  scenes and re-sends the styles with a reference in a gradient or a box
  filter's `drop-shadow`, in every session that has not yet presented that
  report (`host/apple/src/colors.rs`). An SVG scene's pairs are now re-picked when
  its view's appearance changes (`SvgHost.appearance`), which they were not.
- Rows that cross as a name (`{"sys", "c"}`: colours, borders, shadows, the
  symbol palette) were already resolved per view by `SystemColor.swift`.
- **The tint is app-wide in the kernel.** `AccentColor` and `Highlight`
  (`@tint` in the schema) resolve in the kernel to one tint per process, as
  the presenter reports it; a report without them keeps the last. What the
  presenter draws itself uses each view's own tint.
- A session's first report corrects what boot resolved without motion; only
  later reports transition. A box filter's `drop-shadow` crosses resolved
  for both appearances (`p`, and `pd` when dark differs), and each view
  draws its own.
- **Still the fallback:** keyframe colours (a reference is refused in
  `keyframes`, D6 as written); text rasters in content regions resolve when
  built, not on a report; an elevated level is not reported (the kernel
  resolves at the base level); SVG scene filter colours resolve in the
  light appearance, as before (box filters don't). Linux and headless runs report nothing.

## 10. Colour fidelity: what still snapshots or clips

Read on `ide/main` at `0e64493b`. Each item was verified in the code unless
it says *inferred*.

### 10.1 Stage 1 resolves roles to sRGB snapshots and clips them

`SystemColor.channels` resolves a name with `resolvedColor(with: traits)`
and then reads `getRed(_:green:blue:alpha:)`. The result is clamped to 0–1
and scaled to 0–255 (`host/apple/Sources/ExactKit/SystemColor.swift:112`, `:120` on macOS;
macOS `:100` after `usingColorSpace(.sRGB)`). Three consequences:
- **Gamut is clipped.** On iOS, `getRed` reports extended-range sRGB. A
  component of a P3 colour outside sRGB is below 0 or above 1, and the clamp
  discards it. Any system or asset colour defined in Display P3 is drawn
  less saturated than UIKit draws it. *Inferred* for the specific system
  colours: Apple doesn't publish which are P3, so measure them (§6.2).
- **It is a snapshot per trait set.** The channels are cached by
  `(name, dark, contrast, elevated)` and re-applied on a trait change. That
  follows the four traits Exact knows about, but not one it doesn't, such as
  vibrancy under a material or a future trait. So the OS doesn't decide; Exact
  re-asks for the traits it chose.
- **8-bit quantization.** 255 steps per channel. That's fine for display, but
  a 15% wash is computed from rounded values.

**Proposal:** on Apple, carry a reference to the view as the platform's own
dynamic colour object (`UIColor`/`NSColor`) and set it where UIKit accepts
one: `UILabel.textColor`, `tintColor`, `UIView.backgroundColor`. Where Exact
draws into a `CGColor` layer, resolve with the view's live
`traitCollection` at draw time, in extended sRGB floats, without clamping.
The cache keys on the trait collection itself, not on Exact's chosen
subset.

### 10.2 Colours are 8-bit sRGB only

- **The colour row** is `rgba8` (`Color(u32)`).
- **The parser** takes hex, `rgb()`/`rgba()` and `transparent`. It refuses
  `hsl()` and named colours such as `red` (`kernel/src/style/tests.rs:150-151`),
  and it has no `color()`, `oklch()`, `oklab()`, `lab()`, `lch()` or `hwb()`.
- **So an author can't write a Display P3 colour.** `color(display-p3 1 0 0)`
  is a parse error, where every browser draws it.

**Proposal:** a wide colour value:
- `ColorValue::Wide(u16)`, an interned `{ space, [f32; 4] }` with the spaces
  `srgb`, `display-p3` and `oklch`. Interned because it keeps `Copy + Eq` and
  the common case stays `Fixed(u32)`.
- The parser learns CSS Color 4's functions and the 148 named colours.
- **Native:** Apple draws `Wide` in its own space
  (`CGColor(colorSpace: displayP3, …)`). Linux converts to sRGB, gamut-mapped
  per CSS Color 4 §13.
- **Web:** emits the function as written.
- **The kernel's own resolution** (motion, filters) works in floats.

### 10.3 Images are normalized to sRGB

A decoded image is cached as "normalized sRGB RGBA8"
(`host/apple/Sources/ExactKit/RasterImage.swift:150`). Its thumbnail path
draws into `CGColorSpace.sRGB` (`:204`, `:225`). A P3 photo from the camera
loses its wide colours on every Exact surface, where a `UIImageView` keeps
them.

**Proposal:** keep the decoded image's colour space. Draw into a context of
that space (or extended-range sRGB with 16-bit floats when it is wider than
sRGB) and tag the IOSurface with it, as `TextRaster.swift:151` already tags
sRGB. Normalize only images with no profile.

### 10.4 Two fills aren't colour-managed

`RegionRaster.swift:264` (the region's background) and `:273` (the
selection) fill with `CGColorSpaceCreateDeviceRGB()`. Device RGB isn't
managed, so on a P3 panel those fills come out more saturated than the same
value drawn anywhere else in Exact, or in UIKit. **Proposal:** use the sRGB
colour space, or the reference itself once 10.1 lands. Small and local; it
can land first.

### 10.5 Hardcoded fallbacks in the Apple host

Before stage 2: `NodeText.swift` (`?? [0, 0, 0, 255]`), `InlineText.swift`
(the same, twice), `Affordances.swift` and the symbol views (`.black`), and
the text fields' `textColor` and placeholder ink. **Fixed by D8:** each now
asks the platform (`SystemColor.canvasText`, `UIColor.label` /
`NSColor.textColor`), and an untinted symbol inherits the platform's tint. Any remaining `.black`/`.label` literal in a
paint path should resolve the row's initial value instead. A sweep for such
literals is part of D8's tests.

### 10.6 Order of the §10 work

1. **10.4:** device RGB to managed sRGB. Local, no API change.
2. **10.5:** with D8.
3. **10.1:** dynamic objects and unclamped floats on Apple. This changes the
   Apple wire for references only.
4. **10.2:** the wide colour value and CSS Color 4 parsing. A schema change.
5. **10.3:** image colour spaces. Independent of the rest; needs memory
   measurements, since 16-bit floats double an image's bytes.

## 11. Verified and not

On `feat/platform-colours` (based on `origin/main` `0b871f54`), 2026-10-03:

- **The five checks:** `cargo build --all-targets`, `cargo test --lib --bins
  --tests` (2178 passed, 0 failed), `cargo clippy --all-targets -D warnings`,
  `cargo fmt --check`, `bun scripts/caps.mjs` and `bun scripts/boot.mjs` are
  clean.
- **The Apple host's tests** (`exact-apple`, outside the default members):
  the same 11 failures as `origin/main` itself (content regions, development
  links, artifact paths); none new. Three tests that read the old black
  initials were updated to read `CanvasText` (§9).
- **iOS simulator (iOS 26):** an unstyled `text`, an untinted SF Symbol, a
  `tint-color="CanvasText"` symbol, a `system-orange` symbol and a
  `secondary-label` text in light, dark, and each with Increased Contrast:
  text black → white; the untinted symbol is the system tint (lighter under
  dark Increased Contrast, darker under light); the role symbols follow the
  platform. D9 was driven on the fork: a gradient, an SVG circle and a box
  sampled iOS's own `systemOrange`/`systemBlue` in all four states (e.g.
  250,140,45 → 194,83,6 under Increased Contrast).
- **macOS:** the host and a fixture app build; not driven.
- **Verified in code:** every file:line cited in §10, on this branch.
- **Not verified:** which system colours are outside sRGB (§6); memory for
  wide images (10.3); Firefox for D11's media queries; D9 on macOS; Linux
  (no platform colours; the fallback pair).

## 12. Under LLP 1081 (the `-exact-` naming rule)

LLP 1081 (Draft r3, not built) spells everything Exact invents with an
`-exact-` prefix and keeps CSS's names and a browser's names as that
browser exposes them to web content. This LLP landed first, so its names
are written as built. LLP 1081's stage 1 sweep renames them with its own
rows; nothing here is renamed twice.

| Built here | Under LLP 1081 | Why |
|---|---|---|
| CSS system colours (`Canvas`, `CanvasText`, `AccentColor`, …) | unchanged | CSS names (1081 D1) |
| Exact roles (`label`, `secondary-label`, `fill`, `system-orange`, …) | `-exact-label`, `-exact-secondary-label`, `-exact-fill`, `-exact-system-orange`, … | invented keywords |
| WebKit aliases that WebKit has (`-apple-system-label`, `-separator`, `-background`, `-blue`, …) | unchanged | WebKit exposes them to web content (1081 D1) |
| `-apple-system-link`, `-secondary-fill`, `-mint`, `-cyan` | never accepted | Not in WebKit's `CSSValueKeywords.in` at `bb06bdc9`. The fork's table had them; they were dropped on landing, so those four roles have no alias. |
| `-apple-system-fill` (an alias of `fill`, accepted as built: LLP 1077 D13 shipped it) | deleted by 1081's sweep; authors write `-exact-fill` | WebKit has no such keyword (1081 §2). 1081's `-exact-system-fill` and this LLP's `fill` are the same colour, so one name stays. The Apple host keeps the string only as its vibrancy key for `.fill`. |
| `platform-color()` | `-exact-platform-color()` | an invented function |
| `tint-color` (D8's initial `AccentColor`) | `-exact-tint-color`, raster images only after 1081 stage 2 | 1081 §2 and Q4 |
| `theme` (D4, proposed) | unchanged | a Contract construct, outside the rule (1081 D5) |

**The symbol question.** §6's stage-2 question 1 (an untinted symbol is
the app's tint, as in UIKit) is overtaken by LLP 1081's stage 2, which
makes a symbol take its computed `color`. With D8, `color` starts as
`CanvasText`, so after 1081 stage 2 an unstyled symbol is the label colour
again, now resolved by the platform. An app that wants the accent writes
`color="AccentColor"`. Until 1081 stage 2 lands, D8 holds as built.

## Appendix A — Android and Windows (non-normative)

The design admits these hosts without change: a column per platform in the
role table, a keyword in `platform-color`, and a host lookup. What the
review found, recorded for when those hosts exist (LLP 1076):

**Android:**
- **Material 3's container roles** (`?attr/colorSurfaceContainerLow` and
  the rest) are Material Components attributes. They need an MDC
  `Theme.Material3` host theme. Without MDC, use the framework attributes
  (`?android:attr/textColorPrimary`, `textColorSecondary`, `textColorHint`)
  or API 34's `@android:color/system_surface_container_*` resources.
- **Lookup:** `?attr` names need `Resources.getIdentifier` to find the
  attribute id before `Theme.resolveAttribute`, which may return a
  `ColorStateList` reference to resolve in turn.
- **Shrinking:** `getIdentifier` lookups of `@color/…` are stripped by
  `shrinkResources` unless the names are kept.
- **The system hues** have no Material counterpart. A role needs a declared
  fixed tone.

**Windows (WinUI):**
- **The brush names** (`TextFillColorPrimaryBrush`, …) are real.
- **`SystemFillColor*`** names statuses (Critical, Caution, Success), not
  hues, so the system hues need stand-ins.
- **Lookup:** `Application.Current.Resources[name]` resolves against the
  app's theme, not an element's `ActualTheme`, and throws on a missing name.
  Use the element's resources and `TryGetValue`.
- **The TextBox placeholder** uses the secondary text fill, not the tertiary.
