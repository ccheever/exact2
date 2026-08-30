# LLP 1019: Fonts — declared, not loaded

**Type:** RFC
**Status:** Accepted (Charlie, 2026-08-30 — two rulings: **OQ1 is nearest-face**, this document's recommendation, with the compile-time diagnostic, the LLP 1001 deviation, and `font-synthesis: none` on web; and **Weird Castle's wordmark is in v1 beside Caltrain**, closing `rules/NOT-DOING.md` §1's open decision and carrying the `RULES.md` §Agents approval to add the font tables. Buildable slice in §10. r2, 2026-08-30 — after a two-round panel, Grok 4.6 xhigh and GPT-5.6 Sol ultra, round 1 blind and round 2 mutually visible: `llp/reviews/1019-fonts.{grok,sol}.md`. r1's D2/D3 table design, D5's C-ABI and web-first-frame claims, and §5's framing of OQ1 were all wrong and are replaced; §11 records the fold and what the author re-verified.)
**Systems:** Kernel (the `font_family` row, the text measurer's seam), Contract (a file-scope declaration, two attributes), Plan (a font table), Web host (`@font-face` + the row it skips today), Apple host (CoreText registration, one resolver), Linux host (fontdb families, the pinned face), Build (assets into the bundle), Weird Castle (the app that needs this first)
**Author:** Claude (Opus 5) for Charlie Cheever
**Date:** 2026-08-30
**Implementer:** Claude (Opus 5) from 2026-08-30
**Related:** LLP 1001 §6 (measured leaves; the injected `TextMeasurer`), LLP 1006 §2 (Contract's file-scope declarations), LLP 1005 §1 (`plan/tables/format.json` is the one declaration authority), LLP 1011 (Image v1 — the asset precedent: a relative source in the plan, resolved by the host against the app's asset root), LLP 1007 §2 (style lowering to CSS), LLP 1008 §5 (the Apple presenter; §9 iOS over the same archive), LLP 1015 §3 (the Linux host's pinned font and its font matching), `rules/RULES.md` §Scope ("the web is the standard"), `CLAUDE.md`; exact1 (research): `docs/fonts-and-assets.md` (the design), `packages/exact-core/src/assets-fonts-{registry,state}.ts` (the runtime), `ios/ExactApp/ExactApp/Platform/PlatformTypes.swift:372–560` (the Apple registry), `packages/exact-host-windows/src/fonts.rs` (the Windows registry), `llp/0470-text-rendering-css-lab.rfc.md` §Font registry limitations, `docs/reports/2026-07-02-d3-apple-text-stack-review.md` §2.1

## Summary

Weird Castle wants a face that is not the system's. exact2 cannot give it
one: the `font_family` row exists in the kernel and **nothing writes it and
no host reads it**. exact1 could, through a good API — `asset()` for the
file, `await loadFont(name, source)` at runtime — whose one structural
choice, *fonts arrive while the app is running*, produced most of its font
bugs: three separate cache-coherence mechanisms, a hand-maintained alias
table that painted bold text thin, five font resolvers on Apple of which
the fourth forgot weight, and a `display` policy that only worked on web.

exact2 has no app JS and compiles the plan before the first frame, so it does
not have to make that choice. **A font is a declaration in the Contract,
compiled to plan tables, registered by the host before first layout.** The
family id is assigned by the compiler, never at runtime; an undeclared family
and a missing file are compile errors; there is no app-visible loading state,
so there is nothing to invalidate.

What that does *not* buy is the part that is genuinely hard, and r2 is mostly
about being honest on it. The compiler freezes identity, not resolution: hosts
still parse, register, match, and fall back. Ids are stable per plan while
native font registration is process-global, so a dev-loop restart needs a
plan-scoped catalog. `@font-face` is asynchronous, so the web needs a boot
readiness barrier and "no FOUT" was a false claim. An ordered per-glyph
cascade is not free on Apple or Linux and is declared out of v1. And the
declared name must bind to the registered *bytes*, never to the font file's
internal name table — the one mistake that would reproduce exact1's worst bug
in a new place.

Both gates named by the panel are now cleared (Charlie, 2026-08-30):
`rules/NOT-DOING.md` §1 names Caltrain as the v1 app **with Weird Castle's
wordmark in v1 beside it**, and the same ruling carries the `RULES.md`
§Agents approval for the font tables. OQ1 is ruled **nearest-face**. The
buildable slice, in dependency order, is §10 — and the one thing genuinely
blocked is not the engineering but the brand face itself, which does not yet
exist in any repo.

## 1. Where exact2 is

The row is there and inert:

- `kernel/tables/schema.json:154` — `font_family`, `u16`, `text`, `layout`,
  default 0. `kernel/src/text.rs:26` documents it as "index into the host's
  font registry (0 = system default)". No host has a font registry.
- **Contract cannot say it.** `contract/lower/src/tags.rs:188–189` accepts
  `font-size` and `font-weight`; that is the whole typography surface.
  There is no `font-family` and no `font-style` — the `FontStyle` enum row
  (bit 69) is as unreachable as the family one.
- **Web drops it.** `host/web/src/css.rs:73` lists `font_family` among the
  rows skipped with the reason `"not lowered in v1"`.
- **Apple never sees it.** `host/apple/src/measure.rs:103–105` sends the
  measurer `font_size`, `font_weight`, and an `italic` flag; the family is
  not in the run struct. `host/apple/swift/Text.swift:67` resolves every
  run through `PlatformFont.systemFont(ofSize:weight:)`, cached by
  `"size/weight/italic"` — one resolver, shared by measure and paint,
  which is the thing to preserve.
- **Linux has the only real font plumbing** and it is deliberately pinned:
  cosmic-text with `Family::SansSerif` hardcoded in `attrs`
  (`host/linux/src/text.rs:290`), the meaning of `sans-serif` settled once
  at startup by `sans_family` (`:161`), and `EXACT_FONT`/`EXACT_FONTS` as
  the escape hatches.

So this is a build, not an extension. The one asset it can lean on is
LLP 1011: an `image`'s source is a relative string in the plan that the
host resolves against the app's asset root, and `host/apple/build.mjs:240`
already copies `assets/` into the bundle.

## 2. What exact1 built

`docs/fonts-and-assets.md` (March 2026, Draft) plus
`packages/exact-core/src/assets-fonts-{registry,state}.ts`:

- **`asset('./fonts/Inter.ttf')`** — a string literal, statically analyzed,
  resolving to a hashed URL on web and a bundle path on native.
- **`await loadFont(name, source, { display })`** — source is an `AssetRef`,
  a URL, an `ArrayBuffer`, or a weight map (`{ 400: { normal, italic }, 700:
  … }`). Ref-counted handles, `unloadFont(name)` to force, a `useFonts`
  React hook for the splash gate.
- **`resolveFontFamilyId(stack)` → `u16`** at style-encode time. Builtins
  1–9 in a hand-written map (`system-ui` 1, `monospace` 2, `serif` 3,
  `helvetica neue` 4, `arial` 5, …); custom ids handed out from 1024 by
  `reserveCustomFontFamilyId()` **when the load resolved**.
- **Native registration over a JSON `ModuleSync` bridge** (module `0xfffe`,
  methods register / unregister / clear-cache) to
  `CTFontManagerRegisterFontsForURL(…, .process, …)` on Apple and an
  equivalent registry on Windows.

The API shape was good and most of it should survive. The `asset()`
constraint (literal only, so the build can find it) is exactly right, and
the weight-map source form is the right way to describe static families.

## 3. What it cost — six scars, each still in the tree

1. **The id's meaning changed after things were encoded.** Because ids were
   allocated when a load resolved, `resolveFontFamilyId` answered
   differently over time, and every consumer that had baked one had to be
   told. Three mechanisms exist only for this:
   `subscribeFontRegistryChanges` / `notifyFontRegistryChanged` so style
   encoders re-resolve (ENG-22788, `assets-fonts-state.ts:52–77`); a
   `REGISTRY_GENERATION` atomic on Windows so the Direct2D text-layout
   cache could notice its key no longer meant the same font (ENG-22803,
   `packages/exact-host-windows/src/fonts.rs:70–80`); and
   `invalidateExactTextMeasurementCacheForFontChanges()` on Apple
   (`PlatformTypes.swift:450`). None of them buys a feature.
2. **A hand-maintained alias table of concrete families.** It missed
   `-apple-system` and `BlinkMacSystemFont`, so an ordinary CSS stack fell
   through to `arial` — a static family that renders heavier weights as
   regular — and bold text came out thin on native while it was bold on
   web (ENG-22195). The apology is still a comment in
   `assets-fonts-state.ts:25–32`.
3. **Five font resolvers on Apple.** `styledPlatformFont` for measure and
   paint (correct, and shared — the thing that worked), plus
   `hostTextGeometryFont` for the JS `measureParagraph` surface, plus
   `swiftUIFont`, plus the plain-label paths. The fourth never applied
   `fontApplyingWeight`, so a weight-700 span in a custom font measured
   with the regular face — geometry disagreeing with paint
   (`docs/reports/2026-07-02-d3-apple-text-stack-review.md` §2.1).
4. **Stacks collapsed to one face; no per-glyph fallback.** Mixed script
   and inline emoji were right on web (the browser cascades) and wrong on
   native. Documented as a limitation in LLP 0470 and never fixed.
5. **`display` was accepted and ignored on native.** A knob that worked on
   one host of four.
6. **Doc/code drift on the hard parts.** `fontSynthesis` was specified with
   a value vocabulary and never implemented; the doc says weight travels as
   `f32`, the kernel carried `u16`; the doc says no synthetic italic, and
   the Apple code shipped a ~12° synthetic oblique (ENG-22086).

Scars 1, 5, and 6 are consequences of runtime loading. Scars 2, 3, and 4
are consequences of collapsing a stack to a single opaque number and then
re-deriving a face from it independently in each consumer.

## 4. The proposal

### D1 — A font is a file-scope declaration in the Contract

Beside `shape` and `resource` (LLP 1006 §2):

```
font "Departure Mono"
  400 = "assets/DepartureMono-Regular.otf"
  700 = "assets/DepartureMono-Bold.otf"
  400 italic = "assets/DepartureMono-Italic.otf"

font "Castle Display" = "assets/CastleDisplay-Variable.ttf"
```

A single path is a whole family (a variable font, or a one-weight face).
**A `faces` row of `weight: u16, italic: bool, path` cannot express that
shorthand** — Sol's catch — so either the compiler inspects the file's axes at
build time and records a range, or v1 declares itself static-only and the
variable shorthand waits. Decide it in the table (D2), not in prose.
A block names faces by weight and optional `italic`, which is exact1's
weight-map source form as syntax rather than an object literal. Paths are
portable relative paths under the app's `assets/` directory. The compiler
refuses any other first component, and web/iOS copy that directory whole;
this is stricter than `image` in v1 so an accepted face cannot work from the
app directory on macOS/Linux and disappear from the web dist or iOS bundle.
The plan repeats the local-path boundary for hostile inputs: no scheme,
authority, root, query, fragment, `..`, backslash, or URL escape.

### D2 — The compiler assigns the id; the plan carries four tables

`plan/tables/format.json` (LLP 1005 §1, the one declaration authority) gains
**four** tables, not two. A declared family and an author's fallback list are
different entities and collapsing them is a category error — r1 did collapse
them, and both panelists caught it independently:

- `faces` — source path, plus weight/style, **or** a variable axis range.
- `families` — the declared Contract alias, plus a `range:faces`.
- `stacks` — an ordered `range:stack_members`.
- `stack_members` — a `families` reference **or** a generic-family enum.

**`font_family` indexes `stacks`**, validated the way every other `idx:` is
(LLP 1005 §2's `validate`). The compiler assigns each stack its `u16` and
writes `font_family = <id>` as an ordinary style row wherever
`font-family="…"` appears. **An undeclared family is a compile error**, and
so is a **missing or unreadable font file** — LLP 1011 makes a missing
`image` source a runtime stderr line, and that precedent must not be
inherited: a missing face is not one wrong picture, it is every text node in
the wrong metrics.

Ids are constant for the life of a plan, so nothing re-resolves and nothing
is invalidated. Two limits on that claim, both from the panel:

- **"Nothing resolves at runtime" is too strong.** The compiler freezes
  *identity*. Hosts still resolve paths, parse and register faces, match
  weight and style, and do glyph fallback. Only the id → declaration binding
  is frozen.
- **Ids are stable per *plan*, and native registration is process-global.**
  A dev-loop restart (~20 ms, a new plan in the same process) may reuse id 10
  for different bytes, while `Text.fonts` and `Text.paragraphs` on Apple are
  `static` (`host/apple/swift/Text.swift:64–65`). So the host needs a
  plan-scoped catalog and must discard id-keyed caches on plan replacement.
  "No `unloadFont`" is true of the app-facing API and false of the host.

**`font-family` is literal-only.** A `derive` evaluating to `"Menlo"` has
nowhere to intern. v1 accepts string literals and closed control flow over
literals (a `when` with two literal stacks — which is how theme switching is
written); it does not accept a data string.

### D3 — the id names a stack; v1 stacks are single-member

The generics occupy the low ids and need no declaration: `system-ui` (id 0,
the kernel default today), `ui-sans-serif`, `sans-serif`, `ui-serif`,
`serif`, `ui-monospace`, `monospace`, `ui-rounded`. **Each is a distinct id.**
exact1 aliased `system-ui`, `ui-sans-serif`, `sans-serif`, and `ui-rounded`
all to `1` (`assets-fonts-state.ts:22–36`); a host may map several generics
to one face, the table must not.

exact1's table of *concrete* names (`arial`, `georgia`, `menlo`,
`helvetica neue`) does not come back. That list is scar 2.

**Ordered per-glyph cascade over an author-written list is declared out of
v1**, and this is a change from r1, which claimed a stack is "what the
platform does anyway". It is not:

- **Web** — the browser can cascade when authored lists land. In v1 each
  declared stack receives the opaque CSS family `ExactPlanStack<n>`; the
  Contract alias never becomes a browser/system font lookup key.
- **Linux** — false as r1 wrote it. `fontdb::Query.families` selects *one
  face* for `snap_weight` (`host/linux/src/text.rs:269–286`) and shaping
  receives one `Attrs.family` (`:289–302`). cosmic-text's own per-glyph
  fallback then scores every font on the machine, and the comment at
  `:209–216` records what that did: **the Caltrain button at weight 600 came
  out in URW Bookman with a space from Noto Color Emoji, 16 pt wide.** That
  is the opposite of an ordered cascade.
- **Apple** — `Text.font(size:weight:italic:)` returns one `PlatformFont`.
  Adding a family parameter still returns one face. An authored cascade needs
  `kCTFontCascadeListAttribute` on the descriptor (which is compatible with
  keeping the shared `CTLine` dataflow, and is *not* a reason the one-resolver
  shape has to break).

So v1: the tables are split as D2 says, a `stacks` row exists, and **v1 stacks
are single-member — a comma list is refused by the compiler.** Ordinary
platform last-resort fallback (so emoji and unsupported scripts still render)
is kept; an *authored* ordered cascade is a declared host limitation on Apple
and Linux, in LLP 1001 beside the Taffy deviations. Neither Caltrain nor
Weird Castle asks for one: both are Latin `system-ui` plus, at most, one
wordmark.

**Identity is the declaration, never the file's name table.** This is the
single highest-consequence item in this document. `font "Castle Display" =
"assets/CastleDisplay-Variable.ttf"` must bind that Contract string to *those
registered bytes*: an opaque plan-stack `FontFace` family on web, the
descriptor obtained directly from the sandboxed URL on Apple, the loaded face
ids on Linux. Apple process registration is best-effort for `UITextField`;
`alreadyRegistered` and `duplicatedName` never gate the URL descriptor.
**Never `UIFont(name:)` and never a lookup of the declared string in the
system font library.** exact1 did exactly that — it read the PostScript and
family names out of the descriptor and then re-looked-them-up
(`PlatformTypes.swift:430–482`) — and the first face whose internal name is
`CastleDisplay` or `Castle Display VF` works on web and silently misses on
Apple and Linux. That is ENG-22195's shape reproduced in a new place.

### D4 — One catalog per host; one shaped paragraph for measure and paint

r1 said "one resolver function, add no second one." The panel's correction is
that a function count is a code-review smell, not an invariant. What actually
holds measure and paint together is already in the tree and should be named as
the rule: **Apple measures by building `CTLine`s and paints those same lines**
(`host/apple/swift/Text.swift:103–142, 185–196`), and **Linux keeps the shaped
cosmic-text `Buffer` for both** (`host/linux/src/text.rs:105–116`). One
plan-scoped catalog, one shaped paragraph, two consumers. Preserve that
dataflow; the resolver count follows from it.

Three consequences, each a landmine today:

1. **Every cache key must gain family/stack identity — and plan identity.**
   Apple's font cache is keyed `"\(size)/\(weight)/\(italic)"`
   (`Text.swift:68`). Linux's snapped-weight cache is `(u16, bool)`
   (`text.rs:146`) and its natural-line-height cache is `(u32, u16, bool)`
   (`:142`). Every one of them is silently wrong the moment a second family
   exists. Adding the family without extending the keys is a wrong-font cache
   hit that no test would name.
2. **The run structs must carry the family.** `host/apple/src/measure.rs:30`
   sends size, weight, and italic across the C ABI; Linux's `Run` drops it
   too. This *is* an ABI change, which r1's D5 wrongly denied.
3. **Text inputs.** Contract attributes are not tag-scoped
   (`contract/lower/src/tags.rs:149–266`), and Weird Castle has `input`s.
   Either native text fields resolve through the same catalog, or
   `font-family` is refused on them. Otherwise resolver number two appears on
   day one — which is precisely how exact1 got to five.

### D5 — Registration at boot; a host-owned readiness barrier on web

Native local parsing is synchronous before the first layout pass, so there is
no app-visible loading state or `display` policy, and remote fonts stay out of
v1 (a URL source was exact1's scar 5). Apple creates its CoreText descriptor
from the sandboxed URL whether or not best-effort process registration accepts
the name; Linux loads the local bytes into its plan-scoped database.

**r1 was wrong twice about the mechanism, and both panelists caught it:**

- **The boot batch does not carry the font table.** `exact_out()` reports
  `{"ops":[…],"timers":…,"motion":…,"error":…}` with ops create / props /
  style / children / destroy / roots / frame / content / present
  (`host/apple/include/exact.h:1–16`). Font paths are not style rows and not
  node props — images work only because `imageSource` is a string prop on a
  node. The host that holds the `Plan` reads `plan.stacks`/`families`/`faces`
  at boot. Apple receives it through `exact_set_fonts`; web queries it through
  `exact_fonts`. Both are plan-owned seams, and both hosts' batches remain
  operation-only.
- **`@font-face` is asynchronous, so "no FOUT" is false on web.**
  `css_text(style: &StyleProps)` (`host/web/src/css.rs:32`) sees style rows
  only, so un-skipping the row as-is would emit `font-family:7`; the web host
  needs the interned name list from the plan table. And emitting `@font-face`
  is not registration: omitting `font-display` selects the browser's `auto`
  policy, so the first frame can be fallback metrics or invisible text and
  then reflow.

  The honest design is a **host-owned readiness barrier**: create and `load()`
  every declared `FontFace`, await the attempts, add the successes to
  `document.fonts`, and only then present. That is host boot work, analogous
  to decoding the plan — not application JS and not an app-visible `display`
  policy — and it keeps the web the parity oracle *at the first visible
  frame*. It also puts font fetch and decode inside the 100 ms cold-start
  budget (`rules/RULES.md` §Time budgets); a hang must time out and present
  with fallback rather than block. The 100 ms number is a barrier timeout, not
  a discard policy: at timeout the host installs every face already loaded,
  except a family with a sibling already failed, and ignores later completions
  to avoid a post-paint swap. If that budget cannot hold, the alternative is
  to accept FOUT explicitly and define parity as eventual. **This document
  cannot claim both an immediate first frame and synchronous web fonts.**

**Registration failure is fail-closed and loud** (`llp/foundation/0382`): a
face that will not parse, a path escaping the asset root (LLP 1011 §4 already
refuses `..`), a corrupt file — that family is absent, a diagnostic goes to
stderr, the stack falls through to the last resort, and boot still presents.
Never a silent substitution, which changes layout while pretending success.

**Formats: TTF and OTF only in v1.** exact1 claimed WOFF2
(`fonts-and-assets.md`), but `CTFontManager` and `fontdb` do not decode it the
way a browser does — so a `.woff2` would work on web and be silently absent on
native. A `.woff2` in `assets/` is a compile error; convert at build.

### D6 — Face selection follows the browser: family first, then weight

The Linux host already learned this the hard way and wrote it down
(`host/linux/src/text.rs:255–262`): cosmic-text's fallback takes the
requested weight literally and ranks *any* face whose variable `wght` axis
covers it above the family's nearest static face, so on a Mac weights 500 and
600 came out in San Francisco while 400 and 700 were the pinned DejaVu — and
the app's weight-600 button measured 104 wide against 128 on a builder with
the same bytes. `snap_weight` fixes it by asking for the family's own nearest
weight first. That rule — **the family wins, then the weight is matched inside
it** — is the one every host implements: CSS Fonts 4 §5.2, in its stages —
within a family, match style, then weight, then (later, and only later)
consider synthesis. For a variable font with a `wght` axis, any value in range
is set on the axis directly.

**`snap_weight` is an adapter, not the policy.** Both panelists said so and
they are right: it exists because cosmic-text ranks a variable face covering
the requested weight above the family's nearest static face, and it queries
only `Family::SansSerif` while caching without a family key. Porting it to
Apple (whose `systemFont(ofSize:weight:)` already matches family-first) or to
web (which already does CSS matching if you emit `font-family` and
`font-weight` and do not snap) would *create* divergence. Linux may keep it as
a local clamp, per family, and must not snap a real `wght` axis.

**Two domain questions this opens.** The kernel carries `font_weight: u16`
defaulting to 400 (`kernel/tables/schema.json:152`) while CSS Fonts 4 is
1–1000 and a `wght` axis is continuous; and there is no `font_stretch` row at
all, though stretch is part of CSS matching. Each needs a decision or an
LLP 1001 deviation, not silence.

### D7 — `font-style` lands in the same change

The `FontStyle` row exists and Contract cannot reach it. Shipping family
without italic means a second pass over the same four hosts.

**Weakened in r2.** Neither app contains `font-style`, and Sol was right that
this makes D7 — not D6 — the addition with no evidence behind it. It stands on
"one pass over four hosts beats two", which is a real but weaker argument than
the rest of this document, and it is the first thing to cut if the slice needs
cutting.

## 5. The open decision: synthesis

**r1 posed this as a binary and both panelists said the binary is false.**
CSS runs two steps in sequence, and r1's own motivating example belongs to the
first:

1. **Matching** (CSS Fonts 4 §5.2, and D6 above). Request 600 with 400 and
   700 declared → the nearest heavier real face, 700. **Nobody synthesizes
   here**, on any host or in any browser. Linux's `snap_weight` is this step.
2. **Synthesis** (`font-synthesis`). Matching has returned the closest real
   face and it is *still* not what was asked — the family ships only an
   upright Regular and the author wrote `font-weight="700"` or
   `font-style="italic"` — and the engine fakes it: smear for bold, ~12° shear
   for oblique.

Only (2) is the open question. exact1 never separated them either: its
"**No synthetic bold by default**" section
(`docs/fonts-and-assets.md:366–378`) illustrates the policy with the 400/700
→ 600 case, which is plain matching, so the doc declared a default it never
tested against the case that decides it — which is why the Apple binary
shearing ~12° (ENG-22086) never registered as a contradiction. That is the
mechanism of scar 6, not merely an instance of it.

**The case for synthesizing.** CSS's initial value is `font-synthesis: weight
style small-caps`; `CLAUDE.md` and `rules/RULES.md` §Scope bind a semantic
that could follow CSS to CSS, "even where a CSS reset would usually override
it", with the web as the parity oracle. Nearest-face can erase the requested
distinction outright — bold renders as regular, italic as upright — and the
portable guarantee ought to be semantic contrast rather than pixel-identical
fakery. An unspecified policy provably drifts; exact1 is the proof.

**The case for nearest-face.** `font-synthesis: none` *is* a CSS reset, so the
rule's "even where a reset would override it" clause is exactly what is in
dispute, not a settled answer. The three native hosts do not share an
algorithm: Apple shears italic on the system helper
(`host/apple/swift/Text.swift:83–88`) and never smears custom weight, and the
Linux painter draws swash masks of real glyphs with **no embolden and no shear
machinery at all**. So "follow CSS" here means writing two native faux-bolds
and calling the trio a CSS initial — which is not how this repo follows CSS
for layout or motion, where the browser is held to by fixtures. Nearest-face
ships the *same file* on every host; only the rasterizer differs, which is the
irreducible floor everywhere. And the cost is asymmetric: nearest-face is one
`font-synthesis: none` in the web host plus gating Apple's existing
italic-trait branch on a real italic face; synthesize is new raster work on
two native paths.

**What breaks the tie, and it is available only to exact2.** Because fonts are
declared (D1) and the compiler owns the face table (D2), the compiler *knows*
that "Castle Display" ships only a 400. A browser synthesizes by default
because it cannot know; the web is full of `font-weight: bold` on families
that never had a bold file. exact2 can refuse it at build time. A compile-time
diagnostic on `font-weight="700"` against a family with no face at or above
600 turns the runtime default into near-dead code — and when a default is
nearly dead code, take the one that costs least and diverges least.

**This document therefore recommends nearest-face, conditional on the
diagnostic shipping in the same change**, with the deviation from CSS declared
in `llp/1001-kernel-v1.spec.md` beside the Taffy `position: static` gap, and
`font-synthesis: none` emitted by the web host so the divergence is
deliberate rather than emergent. **If the diagnostic slips, synthesize** —
because nearest-face without it is the one combination that fails silently.
No `font-synthesis` property in v1 either way: its only v1 use would be
turning the chosen default off, and agents add no surface
(`rules/RULES.md` §Agents). Small-caps synthesis is out regardless — there is
no small-caps row.

The panel did not converge here and the split is narrow and informed: Grok 4.6
recommends nearest-face at 75% (moved up from 70% on seeing Sol's case), Sol
recommends synthesize at 78% (moved down from 82% on seeing Grok's). Both
numbers, and the author's, are in §11.

**Ruled (Charlie, 2026-08-30): nearest-face**, as recommended. So: match by
CSS Fonts 4 §5.2 and stop there; never smear and never shear; the compiler
refuses a literal weight or style the declared family has no face for; the web
host emits `font-synthesis: none` so the divergence is deliberate; and the
deviation from CSS's initial value is declared in
`llp/1001-kernel-v1.spec.md` beside the Taffy `position: static` gap. Apple's
existing italic-trait branch (`host/apple/swift/Text.swift:83–88`) is gated on
a real italic face. No `font-synthesis` property in v1.

The author's confidence rose from 70% to 78% before the ruling, on finding
that the diagnostic is nearly free: exact2 has **no style inheritance** (no
`inherit` in the kernel, compiler, or runner, and none in LLP 1001 or 1006),
so the compiler sees family and weight at the same node, and
`check_style_value` (`contract/lower/src/lib.rs:809`, called at `:955`) is
already the per-attribute static hook with the literal/non-literal split the
check needs. The condition the recommendation was conditional on costs a few
lines inside a function that already runs. **Its one honest limit:** style
attributes lower to bytecode (`BindingsRow { kind: Style, expr }`), so a
weight bound to a `derive` is opaque to the compiler and only the runtime
rule covers it. Every `font-weight=` in both apps is a bare integer.



## 6. What Weird Castle actually needs

`~/projects/weird-castle/app.contract` styles every `text` with `font-size`,
`font-weight`, `letter-spacing`, and `color` — system font throughout,
including the `WEIRD CASTLE` wordmark at weight 800 with 9-point tracking,
which is a display face standing in for itself. The ask is small: one or two
brand faces, plausibly a monospace, bundled.

**r1 said it needs D1–D5 and nothing beyond; that was wrong.** The app uses
weights 600, 700, and 800 (verified: one 600, six 700, one 800), so using a
brand family requires **D6** — weight matching inside the family — on day one.
It contains no `font-style` at all, so **D7 is the part with no evidence
behind it**. Caltrain is the same shape: seven 600s, nine 700s, three 800s, no
italic.

The honest authoring answer for the wordmark is to ship the real 800 face or a
variable family whose range covers it, which is what makes the §5 diagnostic
the load-bearing piece rather than the synthesis default.

## 7. Not in v1

Each of these is declared here so it is not silently assumed:

- **Remote fonts and `display` policy** — D5. A URL source is a `resource`
  if it ever matters.
- **`font-feature-settings`, `font-variation-settings`, small caps,
  `font-variant-caps`** — `font_variant_numeric` (bit 75) is the one
  OpenType row that exists, and it is also unreachable from Contract; it
  should get an attribute with this work or be declared unreachable on
  purpose.
- **Dynamic Type / `allowFontScaling`** — accessibility text scaling is its
  own decision and touches layout everywhere.
- **Subsetting, compression, resolution variants** — a build-time asset
  pipeline, deferred in exact1 for the same reason.
- **`unloadFont`** — there is no runtime registry to unload from.

## 8. Open questions

- **OQ1 (synthesis).** §5 now recommends nearest-face conditional on the
  compile-time diagnostic, with the CSS deviation declared in LLP 1001. The
  panel split 75% / 78% across families and did not converge; §11 records
  both. Charlie decides.
- **OQ2 (settled by the panel).** The tables live in the plan
  (`plan/tables/format.json`), not a sidecar that could skew from the ids in
  style rows. Both panelists agreed. What is *not* settled is the host seam:
  Swift sees only the C ABI, and `exact_out()` carries ops (D5).
- **OQ3 (declaration mandatory?).** Both panelists said yes: a compile error
  for any name that is neither a generic nor declared. `font-family="Menlo"`
  works on a Mac and falls through on a builder — the failure D2 exists to
  kill — and Linux already answers "a face from this machine" with
  `sans_family`, `EXACT_FONT`, and `EXACT_FONTS`, which are host knobs, not
  Contract holes. The one honest counter-case is a licensed house face that
  cannot be bundled but is installed on the target; the later form for that is
  a *file-less declaration* (`font "Menlo"`, meaning "must exist at boot, fail
  if not") or an explicit `local("Menlo")` source — still an id, still
  compile-time, still not an undeclared string. Not v1. Note r1's own
  Helvetica example in D3 is inconsistent with D1, which has no file-less
  syntax.
- **OQ4 (the parity test).** A font whose 400 and 700 differ measurably, one
  Contract fixture, `test` blocks (LLP 1017 §P7) across web / macOS / iOS /
  Linux. r1 proposed asserting that 400 and 700 measure differently; Sol is
  right that this is too weak — it should assert **resolved face identity and
  painted geometry**, since a wrong-font cache hit (D4) reproduces the
  difference while being wrong. The Linux host already pins a font
  (LLP 1015 §3), so the fixture may cost nothing. This is an async four-host
  sweep, never a sixth blocking check (`rules/RULES.md` §Budgets).
- **OQ5 (scope — the one the rules actually wait on).** Both panelists,
  independently and in both rounds, named the same thing as the first
  decision, ahead of every technical item here: **`rules/NOT-DOING.md` §1
  leaves the one v1 app an open decision and recommends Caltrain, and
  everything that app does not need does not exist.** Caltrain does not need a
  brand face; Weird Castle's wordmark does. `CLAUDE.md` naming
  `~/projects/weird-castle` as a consumer is not that decision, and that file
  says it does not bind. So this RFC needs one sentence from Charlie — *Caltrain
  only, or Weird Castle's wordmark is in v1 beside it* — plus, separately, the
  explicit human approval `rules/RULES.md` §Agents requires before an agent
  adds a font table at all. Until both exist, this document is a proposal and
  nothing should be built from it. Grok and Sol disagreed only on sequencing:
  Grok reads the trade as owed before implementing, Sol as owed in the same PR.

  **Ruled (Charlie, 2026-08-30):** Caltrain defines v1 **and Weird Castle's
  wordmark is in v1 beside it** — one bundled brand face in scope, nothing
  else about that app. Recorded in `rules/NOT-DOING.md` §1. Fonts were never
  on the not-doing list, so no trade is owed (Grok's reading; Sol conceded
  it). The same ruling carries the §Agents approval for the font tables.

## 9. What r1 got wrong

Kept as a record, because these are the shape of error this document is
about — confident claims about seams nobody had opened:

1. **"No new C ABI entry point"** (D5). `exact_out()` carries kernel ops.
   Verified against `host/apple/include/exact.h:1–16`.
2. **"Un-skip the row on web"** (D4). `css_text` takes `StyleProps` only
   (`host/web/src/css.rs:32`); it would emit `font-family:7`.
3. **"No FOUT"** (D5). `@font-face` is asynchronous; the web host needs a
   readiness barrier.
4. **`fonts` + `faces` is enough** (D2). A family and a stack are different
   entities; four tables.
5. **"Per-glyph fallback is what the platform does anyway"** (D3). False on
   Linux (`host/linux/src/text.rs:209–216`: URW Bookman with a Noto Color
   Emoji space) and false on Apple without a cascade list.
6. **"Extend the one resolver with the family"** (D4). Correct as far as it
   goes and silently wrong without extending the cache keys, which omit family
   on both Apple and Linux.
7. **Matching and synthesis are alternatives** (§5). They are sequential
   stages, and the example given was matching.
8. **"Weird Castle needs D1–D5 and nothing beyond"** (§6). It needs D6; it
   does not need D7.

## 10. Status and the buildable slice

**Both gates are cleared** (§5 and §8 OQ5, Charlie 2026-08-30), and this RFC
now has an implementer and a date, so it is buildable. In dependency order:

1. **Plan tables** — `faces`, `families`, `stacks`, `stack_members` in
   `plan/tables/format.json`; `font_family` validated as a `stacks` index.
   Decide the variable-axis question here (D1): axis ranges on a face row, or
   v1 is static-only.
2. **Contract** — the `font` declaration at file scope (D1), the
   `font-family` and `font-style` attributes, literal-only values, id
   assignment, and the four refusals: undeclared family, missing/unreadable
   file, `.woff2`, and the §5 weight/style diagnostic. Duplicate direct
   attributes are refused, so the diagnostic and the effective binding cannot
   inspect different tuples. Font sources must begin `assets/`. All in
   `contract/lower`, most of it in `check_style_value`.
3. **Kernel → host seam** — `font_family` onto the run structs
   (`host/apple/src/measure.rs`, Linux's `Run`), and **every cache key
   extended** with family and plan identity (`Text.swift:68`,
   `text.rs:142,146`). This is the step that is silently wrong if skipped.
4. **Hosts** — web: opaque stack names into `css_text` and `FontFace`, the
   separate `exact_fonts` catalog query, readiness barrier,
   `font-synthesis: none`. Apple: the `exact_set_fonts` catalog seam,
   descriptor-from-URL identity independent of best-effort process
   registration (never `UIFont(name:)`). Linux: `Attrs.family` from the stack, `db.
   load_font_file`, `snap_weight` per family.
5. **The fixture (OQ4)** — costs nothing:
   `scripts/fixtures/fonts/assets/DejaVuSans.ttf` and
   `DejaVuSans-Bold.ttf` are already in the repo as the Linux host's pinned
   pixel pair (LLP 1015 §3). They are a real two-face family (400 and 700), so
   the whole path — declaration, packaging, matching, per-host identity — can
   be built and tested before any brand face exists. Assert resolved face
   identity and painted geometry, not merely that 400 and 700 differ.
6. **The wordmark** — drop the brand file into
   `~/projects/weird-castle/assets/` and declare it. **This is the one thing
   that is blocked and not on the critical path:** that directory holds only
   `weird-castle-mark.png`; there is no `.ttf` or `.otf` anywhere in the repo,
   so the face itself is still an open product choice (which face, and its
   licence for bundling).

### What this owes

- Linux must replace fontdb's CSS Fonts 3 weight ranking (475 currently picks
  400, not CSS Fonts 4's 500), and match declared face ids directly so an
  installed `ExactPlanStack<n>` cannot collide with its synthetic alias.
- Web has a live race in the advertised ~20 ms dev restart: overlapping plan
  reloads are neither cancelled nor serialized.
- Apple still owes deliberate selection among multiple descriptors in one
  font file, removal of color from the shaped-paragraph cache key, and a
  no-shear generic italic policy.
- Contract still owes a regular-file check for a `.ttf`-named directory and
  the ability for `use` to name a font-only file.

D7 (`font-style`) rides along in step 2 because the row and the refusal are
the same edit; it is still the first thing to cut. Everything in §7 stays out.

## 11. The panel (2026-08-30)

Two rounds, at Charlie's request, artifacts in
`llp/reviews/1019-fonts.{grok,sol}.md`: **Grok 4.6** (xAI, reasoning effort
xhigh) and **GPT-5.6 Sol** (OpenAI, effort ultra — requested and reported
agree, verified against the codex rollout). Round 1 blind; round 2 mutually
visible, each asked to write only where they differed. Not a review: no
verdict, nothing approved or blocked.

**They converged on** every item in §9 (independently, in round 1, from
different directions), on the four-table model, on mandatory declaration
(OQ3), and — most consequentially — on OQ5 being the first decision, ahead of
all the engineering.

**They split on OQ1 and stayed split, narrowing:** Grok nearest-face 70% → 75%
(moved *up* after reading Sol, on the ground that the Linux painter has no
embolden or shear machinery at all, so "follow CSS" means inventing two native
faux-bolds); Sol synthesize 82% → 78% (moved *down* on the same evidence, but
holding that nearest-face can erase the requested distinction and that the
binding rule is the binding rule). The author lands with Grok at 70%, and only
because of the compile-time diagnostic in §5, which neither panelist weighted
as decisive.

**Each corrected the other on a fact**, and both corrections stand: Sol showed
Grok that exact1 reserved the family id *before* awaiting the load rather than
on resolution (`assets-fonts-registry.ts:119–147` — the coherence diagnosis is
unaffected, the timing claim was wrong), and that this document does carry a
date, only no implementer; Grok showed Sol that `NOT-DOING.md:141–144` governs
moving something *off* the not-doing list, which fonts were never on, so the
obligation is §1's open v1-app decision instead.

**Grok's single highest-leverage edit** was the identity rule now in D3: bind
the declared name to the registered bytes, never to the font file's internal
name table and never through `UIFont(name:)`. **Sol's** was the four-table
plan model now in D2. Both are folded.

The author re-verified every factual catch against the live repo before
folding — `exact.h`, `css.rs:32`, the Apple and Linux cache keys, the Linux
fallback comment, and both apps' weight usage — and all of them held.

## 12. As built (2026-08-30)

Built on `lane/fonts` in the worktree `exact2-wt-fonts`, deliberately not in
the main tree: two peer sessions had uncommitted work there, and
`rules/RULES.md` §Fleet's shared-index hazard is real — main's index swept
this document and the `NOT-DOING.md` ruling into itself mid-run.

**Who.** Implementer GPT-5.6 Sol at reasoning effort xhigh (`codex exec`),
orchestrated and verified by Claude (Opus 5). Reviewers: GPT-5.6 Sol xhigh and
Grok 4.6 xhigh, blind to each other, artifacts in
`llp/reviews/code-2026-08-30-fonts.{sol,grok}.md`. xhigh rather than ultra for
the implementer because a many-turn agentic build gains more from iteration
than from per-turn depth; the depth went into the reviews.

**The loop.** One build round, then three fix rounds — `rules/RULES.md`'s limit,
and all three were used.

1. **The build.** Steps 1–5 of §10. Five checks green.
2. **A regression the implementer could not see.** The first cut inserted the
   font hook into the positional `exact_boot`/`exact_boot_plan` signature and
   decoded the plan a second time at boot, breaking macOS canvas capture — 0
   captures and a 99.80% readback delta, 5–7 smoke failures over three runs.
   The font diff touches no canvas, GPU, or deck code, so the obvious
   inference was "pre-existing"; **it was wrong, and only a baseline proved
   it** — the base commit built in a separate worktree was green over three
   runs (canvas captured 13×, 0.00% delta). Fix: restore the boot signature,
   install fonts through a separate `exact_set_fonts` seam from the host's one
   validated `Plan`. That seam is better than what D5 specified.
3. **Two reviews, both FIX FIRST**, finding largely different things. Grok
   found the two identity holes that would have hit the wordmark: Apple gating
   the URL descriptor on registration success, so a designer with the brand
   font already installed silently gets the system face; and web using the
   Contract alias as the CSS family, which on timeout becomes a system lookup
   of the declared string — ENG-22195's shape, and **the spec's fault, not the
   implementation's**, so D3 is amended. Sol found the §5 diagnostic bypassable
   by duplicate attributes, and that declared faces never reach the web dist or
   the iOS bundle because both builders copy only `assets/`.
4. **A delta round split**: Grok SHIP, Sol FIX FIRST. Adjudicated by testing
   rather than by vote. Sol's `class=` bypass did not reproduce (three shapes
   compiled; the diagnostic follows the effective family every time), but the
   same hole one level over — duplicate attributes inside a `style` block —
   did, and was closed. Sol's "CSS Fonts 4 ranking and the reload race are
   blockers" was ruled against: no app declares a weight in the 450–500 band
   where fontdb and CSS Fonts 4 differ, and the reload race is a dev-loop
   EventSource race no boot or smoke reaches. Both stay owed, the reload entry
   reworded to say plainly that it is a live race in the ~20 ms restart.

**Verified, unsandboxed, by the orchestrator** — the implementer's sandbox had
no WindowServer, no Chrome, and no CoreSimulator, so every host claim it made
was untestable by it and was re-run here: five checks green (298 tests, clippy
clean; `cargo fmt` reports only the sibling `ibex` checkout another session is
mid-edit in, identically on the base commit); `smoke.mjs` green on **web,
Linux, macOS ×3, and iOS**. The compiler refusals were exercised directly on
scratch contracts outside the lane, not merely through the lane's own tests:
undeclared family, source outside `assets/`, `.woff2`, comma list, missing
file, duplicate direct attribute, duplicate attribute in a `style` block, and
the §5 diagnostic. One bug was found this way that neither reviewer reported —
a bare filename gave `path.parent() == Some("")`, so `contract build
app.contract` died on an empty asset root.

**The honest residual.** The readiness barrier's timeout path — installing
faces that already loaded — is implemented and was inspected by both reviewers
against the event loop, but has no timeout simulation. It is the one part of
this feature verified by reading rather than running. Everything in §10's owes
list is likewise unbuilt by choice, not by accident.

**Not done, and not this slice's to do:** the wordmark itself. No `.ttf` or
`.otf` exists in Weird Castle or exact2 outside the DejaVu fixture pair, so the
brand face remains an open product choice — which typeface, and whether its
licence permits bundling into a shipped binary.
