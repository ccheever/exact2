# LLP 1081: Names Exact invents are spelled `-exact-`

**Type:** RFC
**Status:** Draft r3, 2026-10-03. Nothing here is built. Two review rounds by Astra and Grok (r1 and r2 both NOT READY; round 2 found the rule settled and the gaps in the plan). §8 lists what each revision changed. Charlie asked for the recommendations in §7 to be taken (2026-10-03, "do your rec").
**Systems:** Contract (a new `contract/lower/src/style_names.rs`: the style-name table with each name's kind, moved out of `tags.rs`; `tags.rs` `renamed`; `contract/syntax/src/lexer.rs`: the vendor-prefix rule; keyframe and class checks), Motion (`motion/src/property.rs` names, `motion/src/parse.rs` easing functions), Kernel (`corner.rs` parse and serialize; `style/symbols.rs` system colours; the `TextDecorationLine` enum in `schema.json`), Web host and JS target (keyword mapping, the dynamic system-colour guard, symbol masks in stage 2), Apple host (`VibrancyIOS.swift`'s colour switch; symbol colour in stage 2), every Contract source in the repo (35 `.contract` files, 158 lines, plus Contract inside Rust and JS tests, at `0b871f54e`), LLP 1001 §"Additional deviations as built"
**Author:** Claude (Opus 5.5) for Charlie Cheever, from James's proposal (2026-10-02)
**Date:** 2026-10-03
**Related:** `CLAUDE.md` ("the web is the standard"; deviations are declared in LLP 1001); `rules/RULES.md` ("Delete; don't deprecate"); LLP 1001 (the declarations this renames); LLP 1002 D2 (`spring()`); LLP 1061 (`press-scale`); LLP 1063 (`exit-animation`, `layout-transition`); LLP 1057.003 (`drag-timeline`); LLP 1035.004 and LLP 1011 §3 (`tint-color`); LLP 1077 D1 (`-apple-continuous`, and the lexer rule of its stage 3), D10–D18 (the iOS affordance rows), D13 (the `-apple-system-*` colours); LLP 1053.000 (`backgroundMaterial`); LLP 1069.011 D12 (styleable props); the native-button limitations report of 2026-10-02 (James's `-exact-apple-button-style`); LLP 1053.000.000 (James's `-exact-apple-glass-container`). Web: CSS Values 4 §2.3 (vendor-prefixed names are reserved to their vendor), the Compat Standard (`-webkit-` names every browser ships), CSS Text Decoration 3 (`text-decoration-line`), WebKit at `bb06bdc992326258c402dd4594c2cffd77ca3795` (2026-10-01): `Source/WebCore/css/CSSValueKeywords.in`, `CSSProperties.json`, `parser/CSSPropertyParserConsumer+Easing.cpp`. Reviews: `llp/reviews/1081-names-exact-invents.{astra,grok}.md`.

## Summary

James proposed a naming rule for CSS properties:

> CSS properties specific to Exact should be prefixed with -exact. Properties
> specific to Exact for Apple platforms should use -exact-apple. Properties
> that are 100% the same as Safari's should start with -apple.

This RFC adopts most of it. Every name in a CSS position is one of three kinds:

- **A CSS name** is written bare.
- **A browser's name** is written exactly as that browser spells it.
- **Every name Exact invents starts with `-exact-`.**

D1 defines each kind by grammar and meaning, not spelling alone. It declines the `-exact-apple-` tier (D4): one namespace is enough, and LLP 1001 already says which hosts draw what.

The rule renames 14 properties, 2 keywords and 1 function, and corrects one keyword to CSS's own spelling (§2). Old spellings are refused with a message naming the new one. Stage 1 is the rename and changes no pixel except one web bug it fixes. Stage 2, from Q4, makes a symbol take its colour from CSS's `color`.

## 1. Motivation: what the rule changes

Charlie's question was what the rule would change in practice. It changes four things.

1. **An author can see what isn't web.** "The web is the standard" only works if the author knows which names are CSS. `content-transition: numeric`, `hover-effect: lift` and `transition: transform spring(300, 30, 1)` all read like CSS, and a model writing Contract will assume they behave like CSS. Today the only way to find out is to look in LLP 1001. With the prefix, `-exact-content-transition` and `-exact-spring()` say so where they are written.
2. **A browser's name can't collide with Exact's.** `spring()` already does. WebKit has its own `spring()` (`CSSPropertyParserConsumer+Easing.cpp:245`: `spring(<mass> <stiffness> <damping> <initial-velocity>)`, space-separated, behind the `springTimingFunctionEnabled` setting). Exact's is `spring(stiffness, damping, mass)`, comma-separated, with no velocity. Same name, different grammar and argument order. `hover-effect`, `scroll-edge-effect` and `content-transition` are plausible future CSS names, too. CSS Values 4 reserves a vendor prefix to its vendor, and that is what this prefix buys.
3. **A browser's prefix comes to mean that browser.** Today `-apple-` means either "Safari ships this" or "this is about Apple". Under the rule it means only the first, which makes it checkable. Checking finds two names that fail (§2).
4. **The next RFC doesn't have to argue the spelling.** LLP 1061, 1063, 1057.003 and 1077 §5 each spelled a new non-CSS row its own way. With this rule there is nothing to decide.

What the rule does not change:

- No row, field, bit, wire format or host-private name changes (D5 lists them).
- A CSS name Exact implements as a subset or an approximation keeps its name (D3). `box-shadow` stays `box-shadow`.
- Contract's attributes and props, author-defined identifiers and Contract's expression functions are outside the rule (D5).
- The rule doesn't decide whether a property should exist. The native-button report was right to reject `-exact-apple-button-style`, and the reason was what it read and where it meant something, not its name.

## 2. What exists today

**Properties.** Contract's style-name table (`tags.rs:527`, `attr`) admits these names that are not CSS. LLP 1001 declares each one. The row is the bit in `schema.json`; LLP 1001 cites older numbers for several of them, and those are stale.

| Name | Row | Defined by |
|---|---|---|
| `tint-color` | 86 | LLP 1035.004, LLP 1011 §3 |
| `exit-animation` | 142 | LLP 1063 |
| `layout-transition` | 143 | LLP 1063 |
| `press-scale` | 144 | LLP 1061 D1 |
| `drag-timeline` | 147 | LLP 1057.003 D1 |
| `symbol-rendering`, `symbol-palette`, `symbol-value`, `symbol-effect` | 162–165 | LLP 1077 D10–D12 |
| `press-haptic` | 166 | LLP 1077 D14 |
| `content-transition` | 167 | LLP 1077 D15 |
| `scroll-edge-effect` | 168 | LLP 1077 D16 |
| `hover-effect` | 169 | LLP 1077 D17 |
| `smart-invert` | 170 | LLP 1077 D18 |

**A function.** `spring()` in a `transition` (and `-exact-layout-transition`) easing, parsed at `motion/src/parse.rs:159`. LLP 1002 D2 calls it the timing function CSS lacks. WebKit's `spring()` differs in grammar and order (§1.2) and is off for ordinary web content, so Exact's is Exact's.

**Keywords that claim to be WebKit's and are not.** Neither appears in WebKit's tables at the pinned revision:

- **`corner-shape: -apple-continuous`** (LLP 1077 D1). WebKit's `corner-shape` takes CSS's keywords only. LLP 1077 chose `-apple-` "following WebKit's own `-apple-system`", which made a name that claims Safari support Safari doesn't have.
- **`-apple-system-fill`** (LLP 1077 D13). WebKit's fills are `-apple-system-tertiary-fill` (`CSSValueKeywords.in:286`), `-apple-system-quaternary-fill` (326), `-apple-system-opaque-fill`, `-opaque-secondary-fill`, `-opaque-secondary-fill-disabled`, `-opaque-tertiary-fill` (322–325) and `-apple-system-vibrancy-*-fill` (1485–1487). There is no bare `-apple-system-fill`. Both of Grok's rounds remembered one, so here is the block (`CSSValueKeywords.in:283–290`, each line ending `enable-if=WTF_PLATFORM_COCOA` except `-quinary-label`'s `WTF_PLATFORM_MAC`): `-apple-system-label`, `-apple-system-secondary-label`, `-apple-system-tertiary-label`, `-apple-system-tertiary-fill`, `-apple-system-quaternary-label`, `-apple-system-quinary-label`, `-apple-system-grid`, `-apple-system-separator`. No line anywhere in the file is `-apple-system-fill`.

**A keyword misspelling CSS.** `text-decoration-line: underline-line-through` (`schema.json:1467`). CSS writes the pair as two keywords, `underline line-through`, in either order. The web host emits the enum string as it is (`host/web/src/css.rs:514`), so browsers drop the declaration today and a doubly decorated run is undecorated on the web. Native hosts test the string with `contains("line-through")` and an enum match, so they draw it.

**Names that pass, and keep their spelling.**

- CSS names, including drafts no browser ships: `wrap-flow`, `shape-outside` and `shape-margin` (CSS Exclusions 1, CSS Shapes 1); `interpolate-size`, `field-sizing`, `corner-shape`, `timeline-scope`, `animation-range`; the `env()` names `safe-area-inset-*` and `viewport-segment-*` (`kernel/src/style/env.rs:398`); the media features `device-posture`, `horizontal-viewport-segments`, `vertical-viewport-segments` and `prefers-*` (`runner/src/viewport.rs`).
- `-webkit-text-stroke` and its two longhands (the Compat Standard).
- `-apple-system-label`, `-secondary-label`, `-tertiary-label`, `-quaternary-label` and `-separator` (`CSSValueKeywords.in:283–290`, every Cocoa platform, exposed to web content).

## 3. Design

**D1. Three kinds of name, by grammar and meaning.** The rule covers a name in a CSS position:

- a style property, which is also an attribute name a `style` class can carry;
- a keyword Exact accepts in a CSS property's value;
- a function Exact accepts in a CSS value (easings, colours, shapes, images);
- an `env()` variable name;
- a media feature name.

Each such name is exactly one kind. The test is about the exact spelling **in that position, with that meaning**: a word that appears somewhere in CSS does not license Exact to use it as something else.

| Kind | Spelling | Test |
|---|---|---|
| CSS | bare | Defined in that position with that meaning by a current CSS Working Group document (a published draft or an Editor's Draft on drafts.csswg.org, not an issue, an explainer or a superseded definition), or shipped unprefixed to ordinary web content by a stable browser engine |
| A browser's | exactly the browser's (`-webkit-…`, `-apple-…`) | In that engine's own property, keyword or parser tables at a pinned revision, exposed to ordinary web content (not behind a setting that is off by default), with the grammar and meaning Exact gives it |
| Exact's | `-exact-` + a name | Everything else, including a browser's spelling with a different grammar or meaning (`spring()`), and one only privileged embedders get (`-apple-visual-effect`) |

- **Same meaning allows a declared subset.** An Exact host may implement a CSS or browser name partially or approximately, declared in LLP 1001 (D3). It may not implement a different thing under the name.
- **The name says nothing about how the web host renders it.** A browser's name may reach the browser as itself or be lowered: the web host writes every system colour as its `light-dark()` pair on every browser (`css.rs:502`), Safari included. That is a rendering choice LLP 1001 declares, not a test of the name. (r1 required native emission and the system colours failed it.)
- **An `-exact-` property's own enumerants are written bare** (`-exact-press-haptic: impact-light`, `-exact-content-transition: numeric-countdown`). That exemption covers only keywords the property defines for itself. **A shared grammar keeps its canonical vocabulary wherever it appears**: a colour, an easing or a transition list inside an `-exact-` property is the same grammar as anywhere else. So `-exact-layout-transition` takes `-exact-spring()`, not `spring()`, and `-exact-symbol-palette` takes `-exact-system-fill`.

**D2. The renames.** Stage 1, no aliases:

| Today | Becomes |
|---|---|
| `tint-color` | `-exact-tint-color` (raster images only after stage 2) |
| `exit-animation` | `-exact-exit-animation` |
| `layout-transition` | `-exact-layout-transition` |
| `press-scale` | `-exact-press-scale` |
| `drag-timeline` | `-exact-drag-timeline` |
| `symbol-rendering`, `symbol-palette`, `symbol-value`, `symbol-effect` | `-exact-symbol-rendering`, `-exact-symbol-palette`, `-exact-symbol-value`, `-exact-symbol-effect` |
| `press-haptic` | `-exact-press-haptic` |
| `content-transition` | `-exact-content-transition` |
| `scroll-edge-effect` | `-exact-scroll-edge-effect` |
| `hover-effect` | `-exact-hover-effect` |
| `smart-invert` | `-exact-smart-invert` |
| `spring(k, d, m)` | `-exact-spring(k, d, m)` |
| `corner-shape: -apple-continuous` | `corner-shape: -exact-continuous` |
| `-apple-system-fill` | `-exact-system-fill` |
| `text-decoration-line: underline-line-through` | `text-decoration-line: underline line-through` (CSS's spelling; either order) |

- **Old spellings are refused, not aliased, in every context an author can write them.** That means an attribute, a `style` class, a keyframe, a `transition` or `-exact-layout-transition` list, and a colour or `corner-shape` value. An alias would keep two spellings forever ("Delete; don't deprecate"). The camelCase hints that point at an old name (`dragTimeline` → `drag-timeline`, `tags.rs:1123`) point at the new one.
- **Where the hint comes from.** A literal is checked in Contract, where a hint can be given:
  - An old attribute name is looked up in `renamed` (`tags.rs`).
  - An old name or keyword inside a value is caught by a pre-pass in Contract's value lowering, before the text reaches the kernel's parser. The parsers' own errors are generic: the generator collapses a transition error into `BadTransition` (`kernel/build.rs:1309`), and `BadColor` lists grammar. The pre-pass's table holds every old token: the 14 names inside `transition` and keyframe lists, `spring(`, `-apple-continuous`, `-apple-system-fill` and `underline-line-through`. It matches case-insensitively, at a token boundary, inside composite values such as `light-dark()` and transition lists. Its error names the replacement.
  - A computed value is checked where computed values are checked today, and is refused as a bad computed value is, with no hint. On native hosts the kernel refuses the row. On the JS target the mapper writes `null`, as a bad dynamic `clip-path` does (`host/web-js/src/style.rs:578–580`).
- **Why `-exact-continuous` rather than `-exact-apple-continuous`.** D4 declines `apple` as a tier, and a name shouldn't carry one by accident. The curve is defined on every host (the kernel's `corner::outline`); UIKit is only where it came from. LLP 1001's declaration keeps that provenance.
- **`underline line-through` is a correction, not a prefix.** The pair is CSS, misspelled. The fix changes web pixels: browsers start drawing a decoration they dropped. It lands as its own commit inside stage 1, so the rest of stage 1 can be checked pixel for pixel.

**D3. A deviation doesn't rename a CSS name.** The prefix marks a name Exact invented, not every place Exact differs from a browser; LLP 1001 records those. `box-shadow` with at most eight shadows, `accent-color` on a native button and `text-transform` without language tailoring keep their CSS names. If deviation forced a rename, every subset would fork CSS's vocabulary, which is the bug class "the web is the standard" exists to prevent.

**D4. No `-exact-apple-` tier.** James's second tier would mark a name as Apple's. It is declined because the distinction is ambiguous and buys nothing the rest of the rule doesn't:

- **"For Apple" has no stable meaning.** It could mean "drawn only on Apple", which changes as hosts gain arms. Every row in §2 has at least one host that draws nothing for it today, and the web, Linux and Android may draw any of them later. Or it could mean "an Apple concept", which is a judgement each RFC would argue again. Is `-exact-continuous` Apple's? It is drawn on every host. Is `-exact-tint-color`? It started as a UIKit template image and now tints rasters on Linux and the web.
- **Exact's contract is one meaning on every host**, refused or approximated where a host can't draw it (LLP 1077 D1: "each name means one shape on every host"). A tier that names a platform suggests the name means nothing elsewhere.
- **The fact already has a home.** LLP 1001 says which host draws what, and `-apple-` already marks what Safari ships.

r1 also claimed that `press-haptic` and `scroll-edge-effect` already had web arms. They don't: the web writes no declaration for rows 162–170 (`css.rs:416`, `host/web-js/src/style.rs:518`). The `navigator.vibrate` call belongs to the `haptic()` action, and LLP 1077 lists the web's symbol-effect stand-ins as owed.

**D5. Outside the rule.**

- **Contract attributes and props**, camelCase or kebab-case: `buttonStyle`, `backgroundMaterial`, `glassGroup`, `symbolEffectValue`, `scroll-start`, `scroll-restoration`, `bitmap-width`, `initial-item-count`. These are element attributes, Contract's own vocabulary (Q6). That a `style` class can carry a name is no test: `buttonStyle` is a styleable prop (LLP 1069.011 D12).
- **Author-defined identifiers** in a CSS value: keyframe names, timeline `<dashed-ident>`s, font-family names, grid line names.
- **Contract's expression functions**, folded before any CSS parser sees the value: palette calls such as `accent()` (`contract/syntax/src/parser/keyframes.rs:66`) and actions such as `haptic()`.
- **Host-private names**, which stay exactly as they are:
  - the schema's fields and codec ids (`press_scale`, `"drag-timeline"`);
  - the bits and the wire;
  - the agent's `layout` output, which reports fields (`runner/src/agent.rs:574`);
  - the web host's custom properties `--exact-exit-animation`, `--exact-drag-timeline`, `--exact-layout-transition` (`css.rs:85–136`), `--exact-press`, `--exact-scale` (`host/web-js/src/rows.rs:161–203`) and `--exact-tint` (`motion/src/property.rs:174–181`);
  - the `spring(k, d, m)` text that `layout_transition_css` writes for `presence-glue.js` (`css.rs:651–656`), a private protocol between two host files.

  The author spelling is `-exact-…` (one hyphen); the host's is `--exact-…` (a custom property). **Author source may not name `--exact-*`.** Two parsers read motion names, and they read different text:
  - A **transition** row is stored as the author's text and parsed by the kernel (`Transitions::parse`, generated at `kernel/build.rs:1309`). It reads author names only, through a new `Property::from_author_name`, which refuses `--exact-tint`.
  - A **keyframes** rule is stored serialized: `rule.css()` writes each declaration with `css_name()` (`contract/lower/src/svg.rs:427`). The runner and the JS emitter parse that text again (`runner/src/bridge.rs:114`, `host/web-js/src/emit.rs:711`, `rows.rs:371`), so `Keyframes::parse` keeps accepting `--exact-tint` through `Property::from_name`. Author keyframe declarations are checked in Contract against author names before serialization.

  So `--exact-tint` is refused at every point an author can write it, and still decoded where the plan stores it.
- **URL schemes** (`symbol:sf/…`) are not CSS names.

**D6. Graduation.** When a CSS Working Group document, or a browser's ordinary web content, defines something an `-exact-` name already does, with the same grammar and meaning (D1), the name takes CSS's spelling and grammar, and the old name goes into `renamed`. LLP 1001 retires its "not CSS" declaration but keeps any subset or approximation that is still true. When CSS defines the same spelling with a different meaning, nothing collides; the `-exact-` name stays until someone decides whether to adopt CSS's version.

**D7. The lexer.** Contract's lexer reads a name that starts `-webkit-` or `-apple-` as one identifier only in attribute-name position (followed by `=`), so `-webkit-x` in an expression stays a negation (LLP 1077 stage 3, `lexer.rs:192–210`). `-exact-` joins that list under the same restriction. Keywords and functions are inside string values and need no lexer change. The lexer's tests cover an attribute, a `style` block line, a keyframe line, whitespace before `=`, `==` in an expression and a plain negation.

**D8. Each vocabulary's table carries each name's kind, the parser reads that table, and one test walks them all.**

- **Style names.** They move out of `attr`'s match (`tags.rs`, at 1,499 lines of its 1,500 cap) into `contract/lower/src/style_names.rs`. That file is one table, `STYLE_NAMES: &[(name, provenance, Target)]`, which `attr` looks up, so the test and the lowering read the same data.
  - `Target` keeps every lowering the match has today: a slice of rows for the ordinary case, and a variant for each special one, such as `flex`, which sets grow to the authored value, shrink to 1 and basis to 0% (`AttrTarget::Flex`, `contract/lower/src/lib.rs:1223`).
  - The table is Contract's spelling over the generated `StyleId`s. `schema.json` stays the one declaration authority for rows, bits and enums.
- **The other vocabularies** get a provenance column in their own crate's table, read by that crate's parser:
  - the system colours (`SYSTEM_COLORS`, `kernel/src/style/symbols.rs`, already the table the parser reads);
  - the `corner-shape` keywords, with `-exact-continuous` moved into the keyword table the parser consumes (`kernel/src/corner.rs`), instead of a separate arm;
  - the easing functions (`motion/src/parse.rs`), as a table the function-name match consults;
  - the author motion names (`motion/src/property.rs`, `from_author_name`), with the internal `layout` and `box-shadow-color` marked internal and excluded;
  - the `env()` names (`kernel/src/style/env.rs`).
- **Provenance** is a short string with a fixed form, so the crates below Contract need no shared type:
  - `css <spec>`: a CSS Working Group document;
  - `shipped <engine> <version>`: D1's second CSS arm, for a name an engine ships unprefixed without a CSSWG document;
  - `browser <engine> <revision>`: a browser's name, checked at that revision;
  - `exact <LLP>`: Exact's, defined by that LLP;
  - `internal`: not an author name.
- **The test** lives in Contract, which depends on Motion and Kernel. It fails unless:
  - every `exact` name starts with `-exact-` (an Exact function, `-exact-spring(`);
  - every `browser` name starts with that engine's prefix;
  - every `css` and `shipped` name has no leading hyphen;
  - every provenance string parses;
  - no name in `renamed` or the value pre-pass's old-token table is accepted by any table.

**What the test does not cover, stated so it isn't assumed:**
- **Media features have no authoring table.** They are `exactViewport` fields an app branches on (LLP 1039), not CSS a Contract value names. A future media-query syntax gets a table and joins the test.
- **Functions parsed by hand-written grammars** (`rgb()`, `light-dark()`, `calc()`, the gradient and shape functions) are CSS's, and review keeps them so.
- **A misspelled CSS keyword inside a CSS property's enum**, like `underline-line-through`. The schema's enums have no per-value provenance. Review and the Chrome conformance oracle are what catch those, and §8 records that this one slipped past both.

## 4. Implementation

### Stage 1: the rename (pixel-identical)

1. **Contract.**
   - The `style_names.rs` table (D8), with `attr` reading it.
   - The 14 old names in `renamed`. The camelCase hints point at the new names.
   - The value pre-pass and its old-token table (D2, "Where the hint comes from").
   - Author keyframe declarations checked against author motion names before `rule.css()` serializes them (D5).
   - `class.rs:115` and `svg.rs:384–397, 568–574` match on the new names.
   - The native-button allowlist, `NATIVE_ROWS` (`controls.rs:449`), holds `StyleId`s, so it follows the table with no string edit; only its comment at `:714` quotes a name.
   - The lexer (D7).
   - Every error text that quotes a renamed name.
2. **Motion.**
   - `property.rs`: `TintColor`'s author name becomes `-exact-tint-color`. There are two lookups: `from_author_name`, author names only, used by `Transitions::parse` and Contract's keyframe check, and `from_name`, which also takes the internal `--exact-tint`, used by `Keyframes::parse` on stored plan text (D5).
   - `parse.rs`: easing functions come from a table (D8). `-exact-spring(...)` parses there; plain `spring(...)` is not in it. The serializers that write an author-facing transition write `-exact-spring`.
   - `animation/parse.rs:442` recognises both `spring(` and `-exact-spring(` as a spring, so "a spring is not an animation easing" (`:448–454`) still fires for the new spelling.
   - The private `spring(` text for `presence-glue.js` (`host/web/src/css.rs:651–656`) stays (D5).
3. **Runner.** The reorder preview writes `("transition", "translate -exact-spring(300,30,1)")` (`runner/src/instance/collection/reorder.rs:272`). It goes through the ordinary style bridge into `Transitions::parse` (`views.rs:4–26`, `bridge.rs:77–89`), so it is author grammar.
4. **Kernel.**
   - `corner.rs`: `-exact-continuous` joins the keyword table the parser reads (D8), and `CornerShape::css` (`corner.rs:84`) writes it. `BadCornerShape`'s message (`contract/lower/src/values.rs:141`) lists it.
   - `SYSTEM_COLORS[5]` becomes `-exact-system-fill`, keeping its index 5 and its values.
5. **Web host and JS target.**
   - `css.rs`'s `corner-shape` keyword arm, and the JS runtime's dynamic `corner-shape` mapper (`host/web-js/src/style.rs:585`). A computed old keyword writes `null`, as a bad dynamic `clip-path` does (`:578–580`).
   - The dynamic system-colour guard (`host/web-js/src/style.rs:473`, today `/-apple-system-/`) becomes a match on the six names in `SYSTEM_COLORS`. A prefix match would miss either the five `-apple-system-` names or the fill.
   - **Computed transitions on the JS target.** Today the mapper only drops spring entries (`host/web-js/src/style.rs:572–574`; for a pressed node, `rows.rs:201–203`, which also rewrites `scale`), and passes the rest to the browser unchecked. It becomes a translation: `-exact-tint-color` becomes `--exact-tint`, the CSS name the browser animates, and an entry naming an old spelling or any `--exact-*` is dropped. The spring filters match `spring(` as a substring, which also matches `-exact-spring(`; a test pins that.
   - `host/web-js/reorder.js:115` observes `"translate -exact-spring(300,30,1)"`. `hooks.observe` hands it to the motion parser (`host/web-js/motion.js:48`, `:155`), so it is author grammar too.
6. **Apple host.** `VibrancyIOS.swift:146–155` maps `-exact-system-fill` to `.fill`. Without that change, a fill inside an iOS material falls back to its static pair, a pixel change. Comments in `Vibrancy.swift`, `Affordances.swift` and `CornerShape.swift` take the new names. `host/apple/src/style.rs`'s tests are updated.
7. **Sources.**
   - Every `.contract` file in the repo (35 files, 158 lines).
   - Contract inside Rust and JS tests, such as `contract/cli/tests/it/*`, `host/linux/src/presenter/*_tests.rs` and `host/web/src/parity.rs`.
   - Every runtime producer of author grammar found by the same sweep: the two reorder strings above, and any others it finds. The sweep covers every non-Markdown file except `vendor/`, and each hit is classified as author grammar (renamed) or host-private (kept and commented). §4 lists what was known at r3; the sweep, not this list, is the inventory.
8. **Docs.**
   - LLP 1001's declarations take the new names, a pointer here, and the bits from `schema.json`.
   - Every LLP that defines a renamed name gets a one-line note at the definition ("spelled `-exact-…` since LLP 1081"), whether or not it is in `llp/current/`: 1002, 1011, 1035.004, 1057.003, 1061, 1063 and 1077. Its text stays as the record.
   - The Contract reference, `rules/DEFERRED.md` and `QUEUE.md` are updated where they quote a name.
9. **The check** (D8).

**Stage 1b: `underline line-through`.**
- `TextDecorationLine`'s value in `schema.json` becomes `underline line-through`. The generator's `pascal()` treats a space as a word break, so the variant stays `UnderlineLineThrough`.
- It is parsed through an inherent `TextDecorationLine::from_css` that accepts either order. That function lives beside `TouchAction`'s (`kernel/src/style.rs:1462`), and `TextDecorationLine` joins the arm at `kernel/build.rs:1303` that calls `from_css`. The arm does not run for `GridAutoFlow` and `JustifyItems`, which go through `emit_grid_seam` (`build.rs:1284–1286`); r2 cited them as the model, wrongly.
- The web emits valid CSS.
- Apple already emits the spaced string (`host/apple/src/content_region/wire.rs:96`) and matches with `contains`, so its pixels stay. Linux draws no decoration at all today (`host/linux/src/text.rs:92–107`, `text/catalog.rs:236–255`). That omission is outside this RFC, and r2 was wrong to say Linux matches the enum.
- The old token is refused with the hint, through the value pre-pass (D2).

**Out-of-repo apps.** Interview, Weird Castle and the Bluesky copy (`rules/DEFERRED.md:198`) stop compiling until they are migrated. The commit message carries the migration as a script that rewrites author spellings only:
- attribute names before `=`, and `style` class lines;
- names inside `transition`, `-exact-layout-transition` and keyframe values;
- `spring(` in easings;
- the two keywords and `underline-line-through`.

The script does not touch `--exact-*`, schema codec ids or comments. Their impact is not checked from this repo, and the commit says so.

**Verification.**
- The five checks.
- **Spelling tests.** For each renamed spelling, a Contract test: the new spelling is accepted, and the old one is refused with its replacement in the error text, in each context (attribute, `style` class, keyframe, `transition` list, value). Mixed case and composite values (`light-dark()`, a transition list) are included.
- **Lexer and reserved names.** D7's lexer cases, and `--exact-tint` refused in a `transition` and in a keyframe.
- **Classes.** A class override of a renamed name, and a conditional class that sets one on one branch and clears it on the other. Class merge is by attribute-name string (`class.rs:21`).
- **Round trips.** A keyframes rule that animates `-exact-tint-color`, compiled, serialized into the plan and consumed by the runner and by the JS emitter, sampled mid-animation.
- **Computed values on the JS target.** Transition strings built from runtime data with the new, old and reserved spellings, on an ordinary node and on a pressed node. A dynamic system colour, and a dynamic old `corner-shape` keyword.
- **Reorder.** A reorder preview springs on the runner-backed hosts and on the JS target.
- `host/web-js/conform.mjs --strict`, including the `press`, `rowsmore` and `symbols` pages.
- `bun scripts/smoke.mjs web` and `macos`.
- macOS and iOS builds (Swift changes).
- **Driven before and after, with screenshots that must match pixel for pixel:**
  - `scripts/fixtures/visual.contract` (corners) and `vibrancy.contract` (a fill inside a material) on iOS and the web;
  - a press (`-exact-press-scale`), an exit and a layout move, a drag timeline and a spring transition, in `apps/interaction-gallery` and `contract/corpus/motion-feel.contract`, filmed with `screenshot … over … every …`.
- **Agent output.** `layout` still reports field names (`press_scale`), unchanged.
- Stage 1b: a doubly decorated run is drawn by Chrome, and native screenshots are unchanged.
- **Apps.** Every app in the repo compiles. The apps the rename touches are launched and driven on macOS, iOS and the web.

### Stage 2: a symbol takes `color` (Q4)

A `symbol:` image's monochrome colour, and the base colour of `-exact-symbol-rendering: hierarchical`, is its computed `color`, inherited as CSS's is, on every host. During a `color` transition it is the **presented** colour; the Apple host already propagates a presented inherited colour to descendants (`host/apple/src/paragraph.rs:383`). `-exact-tint-color` remains, for raster images only. r1 said the web already draws symbols this way. It doesn't: both web targets mask a symbol over `--exact-tint`, registered `inherits: false` with initial black (`host/web/glue.js:357`, `host/web-js/symbols.js:9`).

- **Web, both targets.** A symbol's mask is filled with `currentColor` instead of `var(--exact-tint)`. A raster template keeps `--exact-tint` (`host/web/src/element.rs:199–217` applies it only when the source is not `symbol:`). Hierarchical, palette and multicolor are drawn as a monochrome mask in `currentColor`, as they are drawn today in the tint. LLP 1001 declares that approximation.
- **iOS.** Every reader of a symbol's colour moves from `tint_color` to the node's computed `color` (`text_color`, the inherited row):
  - the ordinary symbol view's `tintColor` (`NodeViewIOS.swift:423`);
  - the hierarchical base in `symbolLookKey`'s configuration (`Affordances.swift:28`);
  - a projected tab item's symbol (`SegmentsIOS.swift:61`, default `.label`). The native-tab branch above it (`:44–49`) uses the accent and is unchanged.
  - a custom navigation-bar badge holding a symbol (`NavigationBarIOS.swift:133`), which already uses `text_color` for a text badge. Its raster cache key includes the colour.
- **macOS.** `contentTintColor` comes from the computed `color` (`NodeSymbolMac.swift:43`).
- **Colour is paint, not identity.** On both Apple platforms, the colour leaves `symbolLookKey`'s identity part (`Affordances.swift:22`) and is applied as the view's tint. A colour change, or a frame of a `color` transition, then repaints the symbol without calling `showSymbol` again. So `-exact-symbol-effect: replace` plays only when the symbol, or a configuration that changes its shape, changes (`NodeViewIOS.swift:410`, `Affordances.swift:60`).
- **Palette and multicolor.** These take their own colours (`-exact-symbol-palette`, the platform's). An empty or missing palette falls back to monochrome in `color`, as it falls back to the tint today (`Affordances.swift:32`).
- **Linux** draws no symbols today (`host/linux/src/image.rs:153–173` keeps an empty square). Nothing changes there.
- **Native buttons** colour their face's symbol by the platform's style (LLP 1069.011 D7). Unchanged.
- **Motion.** `transition: color` animates a symbol's colour. Transitions and keyframes that named `tint-color` for a symbol move to `color`. A keyframes rule shared by symbols and rasters keeps `-exact-tint-color` for the rasters and gains `color` for the symbols; the sweep classifies each by its uses.
- **A tint on a symbol.**
  - Written directly on an image with a literal `symbol:` source, `-exact-tint-color` is a Contract error that names `color`.
  - Carried there by a `style` class, it is not an error, because a class is shared across kinds of image. It is ignored for the symbol, as a computed case is.
  - With a computed source, which may switch between a symbol and a raster (`host/web-js/conformance/symbols.contract:32`), the tint applies while the source is a raster and is ignored while it is a symbol.
- **Pixels change** wherever a symbol had no tint and an ancestor sets a `color`. Before the change lands, a sweep lists those sources; their screenshots are reviewed, not required to match.
- **Verification**, on iOS, macOS and both web targets:
  - inherited and explicit `color` on a symbol, and an ancestor's `color` changed at run time;
  - intermediate frames of a `color` transition, filmed;
  - hierarchical, palette (including an empty one) and multicolor;
  - `-exact-symbol-effect: replace` not replaying on a colour change;
  - a native button's symbol, a selected projected tab, and a custom navigation badge;
  - the conformance page's symbol↔raster switch, with a shared class carrying both properties;
  - a raster that keeps its tint.

## 5. Costs

- **Length.** `-exact-press-scale=0.96` is seven characters longer than today's spelling. These names are uncommon in app source (158 lines of `.contract` across the repo), and most of that is fixtures.
- **Breakage outside the repo.** Interview, Weird Castle and the Bluesky copy stop compiling until the migration script runs.
- **Two families of system colour.** `-apple-system-label` sits beside `-exact-system-fill`. That is accurate, since one is Safari's and one is not, but it looks odd.

## 6. Considered and not taken

- **James's rule as written** (with `-exact-apple-`): D4.
- **Leave names bare and rely on LLP 1001.** This is today's state. It keeps short names, but it leaves the confusion in §1.1, `spring()`'s collision with WebKit, and the two wrong `-apple-` names.
- **Aliases for a release.** Contract already refuses a renamed name with a hint, and every consumer is either in the repo or named. An alias would be a second spelling with no end date.
- **Prefix every deviation (no D3).** That would fork CSS's names wherever a host is a subset, and every subset is declared already.
- **A shorter prefix** (`-ex-`, `-ex2-`). It saves two or three characters on names that appear on about 160 lines of `.contract`. A prefix exists to explain itself, and vendor prefixes have always been the vendor's name (`-webkit-`, `-moz-`, `-apple-`). `-ex2-` reads as "experimental" or a version, and the `2` is the repository's codename, not the product's. `-exact-` also matches the web host's own names (`--exact-accent`, `data-exact-*`). If these names ever appeared on most lines of app code, the row should become a default or a CSS name, not get a shorter prefix.

## 7. Questions for Charlie, with recommendations

Charlie took the recommendations on 2026-10-03 ("do your rec"). They are recorded here as rulings to confirm, revised where the r1 reviews changed the facts.

1. **The `-exact-apple-` tier.** **Declined (D4).** Both reviews agree.
2. **`-apple-system-fill`.** **`-exact-system-fill`**, keeping UIKit's `systemFill` values and index 5. WebKit has no bare `-apple-system-fill` at the pinned revision (§2), so the name was never WebKit's. Swapping in a WebKit fill would change pixels.
3. **`-apple-visual-effect`.** r1 asked whether to adopt it. Under r2's D1 it is not a browser's name: WebKit exposes it only under `useSystemAppearance`, which ordinary web content doesn't get. So there is nothing to adopt. James's underlying point was that a `style` class should be able to carry a material, and that doesn't need a new property. LLP 1069.011 D12 already makes `buttonStyle` a styleable prop, and `backgroundMaterial` and `glassGroup` can join that list. **Recommendation: no RFC; a `QUEUE.md` line for styleable materials.**
4. **`tint-color`.** **`color` for a symbol, `-exact-tint-color` for raster images only, as stage 2.** Both reviews agree. Stage 2 now specifies each renderer, motion, caching and the computed-source case.
5. **Names and kinds in `schema.json`.** Names and rows are many to many: `gap` sets two rows, and `translate` sets `Translate` and `TranslateZ`. So it is a mapping design, not two new fields. **Recommendation: D8's table lives in Contract with each name's kind now, and moving it into the schema is a later RFC.**
6. **Kebab-case props.** **Recommendation: camelCase for Contract's own props, as a separate change, keeping real HTML, SVG and ARIA names.** `bitmap-width`/`bitmap-height` are a "for now" spelling Charlie is watching, so they go to him first. Recorded in `QUEUE.md` until then.

## 8. Revisions

**r2 (2026-10-03), after Astra's and Grok's r1 reviews.** Both said NOT READY. Taken:

- **D1 tests grammar and meaning, not spelling** (both). It names the documents that count, requires exposure to ordinary web content for a browser's name, and drops r1's "the web emits it natively". The system colours, lowered to pairs on every browser, had failed that clause.
- **The inventory adds `spring()`** (both; WebKit's own `spring()` has a different grammar, verified at the pinned revision) **and `underline-line-through`** (Astra), and states that `env()` names and media features were checked.
- **§4 names every place a renamed spelling lives** (both): the motion property table and easing parser, keyframe validation, the JS target's system-colour guard and corner mapper, `CornerShape::css`, iOS vibrancy's colour switch, the camelCase hints, and Contract inside Rust and JS tests. D5 lists the host-private names that stay. r1's "the web writes no declaration" was true only of rows 162–170.
- **`--exact-*` refused from author source** (Astra).
- **Stage 2 corrected and specified** (Astra): the web did not already colour symbols with `color`.
- **D4 no longer claims web arms that don't exist** (Astra), and argues from ambiguity instead.
- **D5 adds author identifiers, expression functions and styleable props** (Astra). **D6 keeps still-true deviations at graduation** (Astra).
- **D8 reads the table lowering uses** (Astra). That moves the names out of `tags.rs`, which is at its line cap, and gives every vocabulary a kind column. It says what the test cannot catch.
- **Verification drives the renamed motion and colour paths, not only `visual.contract`** (Astra). It names the Bluesky copy among out-of-repo consumers (Astra) and specifies the migration script (Grok).
- **Counts corrected** (both): 14 properties, not 15; 35 `.contract` files and 158 lines, not r1's figures.
- **Pinned the WebKit revision** (Astra).

Not taken: Grok's belief that WebKit has a bare `-apple-system-fill`. The file at the pinned revision has none (§2). Its recommendation, to keep the WebKit name if it existed, is moot.

**r3 (2026-10-03), after Astra's and Grok's r2 reviews.** Both said NOT READY. Both found the rule settled, D1 operational on the inventory, and the gaps in the plan. Taken:

- **Reserved names** (Astra): the author/internal split between `Transitions::parse` (author text) and `Keyframes::parse` (serialized plan text), with `from_author_name` and a Contract-side keyframe check, plus a round-trip test.
- **Both reorder producers rename** (both): `runner/src/instance/collection/reorder.rs:272` and `host/web-js/reorder.js:115` are author grammar.
- **Computed JS transitions are translated and checked** (Astra).
- **D1 limits the bare-keyword exemption to an `-exact-` property's own enumerants**; shared grammars keep their canonical names inside it (Astra).
- **D8 rewritten** (both):
  - a `Target` that keeps special lowerings such as `flex`;
  - a provenance string, including `shipped`, read by each crate's own parser;
  - `-exact-continuous` in the corner keyword table;
  - media features and hand-parsed functions stated as outside the test.
- **Rename hints have a stated source** (both): a Contract value pre-pass for literals; computed values are refused as bad values are.
- **Stage 1b corrected** (Grok): the real `from_css` arm and the `TouchAction` model, and Linux's missing decoration stated.
- **`animation/parse.rs:442` recognises `-exact-spring(`** (Grok).
- **Stage 2 completed** (both):
  - every iOS reader (`NodeViewIOS.swift:423`, `SegmentsIOS.swift:61`, `NavigationBarIOS.swift:133`);
  - colour as paint rather than symbol identity, so `replace` doesn't replay;
  - the presented colour during a transition;
  - the empty-palette fallback, the web's monochrome approximation, direct versus class-carried tints, and shared keyframes.
- **Verification adds** classes, round trips, computed values, reorder and agent output, and drives iOS too (both).
- **§4 says the sweep, not its list, is the inventory** (Astra).
- **The WebKit block is quoted** (Grok's second challenge): lines 283–290 of `CSSValueKeywords.in` at `bb06bdc9` hold no bare `-apple-system-fill`.
