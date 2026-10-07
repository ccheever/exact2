# Review: LLP 1081, names Exact invents are spelled `-exact-` (grok)

- **Family:** xAI, `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, served as `grok-4.7-build`, one fresh session in the main checkout (41 turns, 106 tool calls, `stopReason: end_turn`).
- **Date:** 2026-10-03.
- **Reviewed:** `llp/1081-names-exact-invents.rfc.md` Draft r1, untracked over `0b871f54e` (blob `09aa5a88`).
- **Redacted:** no. The document and the checkout contain no secrets the brief exposed.
- **Method:** requested by Charlie. Mutually blind to the other family (Astra max), which got the identical brief (sha256 `5c57251c52c1bc5d4deb07c8ec8c0f9aa452e9b130bdd981f26bb290f453ce29`). A static source audit: the reviewer ran only read-only searches and changed nothing. It could not fetch WebKit and said so. The RFC's author (Claude) is not a reviewer.
- **Transcription:** the joined `text` events of the streaming output, unedited. The first two sentences are the run's progress narration; the review starts at "**NOT READY.**"
- **Author's check of one claim (not part of the review):** the review believes WebKit has a bare `-apple-system-fill`. WebKit main at `bb06bdc992326258c402dd4594c2cffd77ca3795` (2026-10-01), `Source/WebCore/css/CSSValueKeywords.in`, has no such line: its fills are `-apple-system-tertiary-fill` (286), `-apple-system-quaternary-fill` (326), `-apple-system-opaque-fill`, `-opaque-secondary-fill`, `-opaque-secondary-fill-disabled`, `-opaque-tertiary-fill` (322–325) and the `-vibrancy-*-fill` keywords (1485–1487). Its `-apple-system-opaque-separator` (327) is also there.

---

I'll audit the RFC against the binding rules and the sources it cites, staying inside this checkout and changing nothing. First I'll read the rules and the draft itself.The rules and the draft are in. Next I’ll check the name table, the lexer, and the schema rows the RFC cites.**NOT READY.** The direction matches "the web is the standard": CSS stays bare, a deviation of a CSS name stays that name, and a host is not part of the spelling. The draft is not safe to implement. The classification disagrees with the names it keeps, the inventory misses a function the rule itself covers, one recommendation rests on a WebKit claim I believe is wrong, and stage 1 as written can change pixels.

## Strengths

The rule in the summary and D3 is the right one for this tree. `CLAUDE.md` already says a name follows CSS, and a difference is declared in LLP 1001. Prefixing `box-shadow` because the list is capped would fork the vocabulary the principle exists to keep. D4 is the same idea applied to hosts: `press-haptic` already has a web arm, `scroll-edge-effect` has web and Linux stand-ins, and LLP 1077 D1 says each name means one shape on every host. Putting Apple in the spelling would make the next host a rename.

The fourteen style properties in §2 are the non-CSS rows in `contract/lower/src/tags.rs` (923, 1014, 1051–1053, 1064–1074). Their "Row" numbers are `schema.json`'s `bit` values: `tint_color` 86 (2470), `exit_animation` 142 (2832), `layout_transition` 143 (2839), `press_scale` 144 (2845), `drag_timeline` 147 (2867), `symbol_rendering` through `smart_invert` 162–170 (2964–3016). Those match the schema. LLP 1001 does not: it still says `tint-color` bit 89 (128), `exit-animation` bit 145 (322), `press-scale` bit 147 (310), `drag-timeline` bit 150 (411), `corner-shape` bit 156 (343). The RFC followed the schema, which is the authority.

Refusing the old spelling through `renamed` (`tags.rs` 1082) matches `rules/RULES.md` ("Delete; don't deprecate") and the table that already turns `fontSize` into `font-size`. D7 matches the lexer that already exists: `-webkit-` and `-apple-` are one identifier only as an attribute name followed by `=` (`contract/syntax/src/lexer.rs` 192–210), which is what LLP 1077 stage 3 specified (237). Keywords in quoted values need no lexer change, and the fixtures quote them (`scripts/fixtures/visual.contract` 28, `vibrancy.contract` 27–32). Class merge is by attribute-name string (`class.rs` 21, 49), so one commit that rewrites both the class and the node does not double-apply a row.

`spring` aside, I did not find an invented `env()` name or media feature. `kernel/src/style/env.rs` admits `safe-area-inset-*` and `viewport-segment-*` only, both CSS. Fold media features in the runner are CSS names (`device-posture`, `horizontal-viewport-segments`, `prefers-*`).

## WebKit claims

I cannot re-read WebKit main as of 2026-10-03. Against what I know of `CSSValueKeywords.in` and `CSSProperties.json`:

- **`-apple-continuous` is not a `corner-shape` keyword.** That part of §2 is consistent. CSS `corner-shape` is `round | scoop | bevel | notch | square | squircle | superellipse()`. I cannot confirm the parenthetical that bare `continuous` is a `text-*-mode` keyword.
- **The labels and `-apple-system-separator` are WebKit's.** Consistent.
- **I believe "WebKit has no bare `-apple-system-fill`" is wrong.** The family I remember is `-apple-system-fill`, `-apple-system-secondary-fill`, `-apple-system-tertiary-fill`, `-apple-system-quaternary-fill`. The RFC lists tertiary and quaternary, omits secondary, and adds an `-apple-system-opaque-*-fill` family. I remember `-apple-system-opaque-separator`, not opaque fills. Exact's pair `0x78788033` / `0x7878805c` (`kernel/src/style/symbols.rs` 74) is UIKit's `systemFill` (about 20% and 36%). If WebKit's keyword is that color, D1 says keep `-apple-system-fill`.
- **`-apple-visual-effect` gated by `useSystemAppearance` is plausible.** `useSystemAppearance` is a real WebKit setting for system web views. I cannot confirm the property name or the values `-apple-system-blur-material-thin`, `-apple-system-glass-material`, `-apple-system-vibrancy-label` on that date. Do not treat Q3 as settled until someone quotes the file.

## Concerns

1. **High. D1's two tests disagree with the names §2 keeps, and with each other.** The CSS test (`llp/1081-names-exact-invents.rfc.md` 79) is "in a CSSWG draft of any maturity, or a browser ships it unprefixed." It does not require the same meaning. One engine shipping an unprefixed experiment would force a bare spelling even when Exact's meaning differs, which is the collision §1.2 is trying to prevent. "Any maturity" also needs a document boundary: a draft on drafts.csswg.org, not a GitHub issue. The browser test (80) requires the name in that engine's tables with Exact's meaning, and then says the web host emits it natively on that browser. The system colours fail the second sentence. `host/web/src/css.rs` 502–504 rewrites a system colour to its `light-dark()` pair because Chrome has no `-apple-system-*`. LLP 1077 D13 (164) already says Safari gets the name and everyone else gets the pair. Q3 then calls gated `-apple-visual-effect` "a browser's name" without that emit clause. The three kinds are not operational until the meaning clause is on both kinds and "emits natively" is dropped or limited to names the web host actually writes (`-webkit-text-stroke`).

2. **High. `spring()` is an invented function in a CSS value, and the inventory misses it.** D1 (75, 83) says a function Exact adds to a CSS property takes `-exact-`. `spring(stiffness, damping, mass)` is parsed as an easing inside `transition` (`motion/src/parse.rs` 159–170). LLP 1002 D2 (84) and `motion/src/spring.rs` 1–3 call it the timing function CSS does not have. It is not in the §2 table, not in D2, and not in §4. The comment in `spring.rs` 16 calls it "WebKit's `spring()` proposal"; a proposal that is not in the engine's tables and not a CSSWG draft is Exact's under D1, not a browser prefix. Leaving it bare makes D8 a test the rule can fail on day one. Renaming it is also a behavior hazard the plan never lists: the JS target strips springs with `/spring\(/` (`host/web-js/src/style.rs` 574), and `layout_transition_css` rewrites the private value as `spring(k, d, m)` for `presence-glue.js` (`host/web/src/css.rs` 651–656). Those two are a private protocol. An author-facing rename that does not update both, together, drops or mis-lowers springs on the web.

3. **High. Stage 1 says the web writes no declaration and that pixels cannot change. Both are false for paths the plan does not name.** LLP 1001's "the web writes no declaration" (381–382) is only rows 162–170. The JS target really emits nothing for those (`host/web-js/src/style.rs` 519–528). These renamed rows do write something: `--exact-exit-animation`, `--exact-drag-timeline`, `--exact-layout-transition` (`host/web/src/css.rs` 85–136), `--exact-press` (`host/web-js/src/rows.rs` 161–167), and `tint-color` as the registered custom property `--exact-tint` (`motion/src/property.rs` 174–181). The agent's presence list queries those custom properties by string (`host/web-js/agent.js` 139). They are `--exact-*`, not the author spelling. Stage 1 must say they stay. A rename applied to them empties the agent's presence readout and stops exit and layout motion, while a pixel diff of `scripts/fixtures/visual.contract` still passes.

   Two name switches are pixel changes even if the custom properties stay:

   - iOS vibrancy looks the colour up by the WebKit string. `VibrancyIOS.swift` 146–155 maps `-apple-system-fill` to `.fill` and returns nil on any other string. macOS only checks that a system colour is present (`Vibrancy.swift` 47). After the rename, iOS fill inside a material falls through to the static pair. The plan updates Swift comments in `Affordances.swift` and `CornerShape.swift` (§4 step 4), not this switch. Corner shape is safe: the host token is `"apple"` (`host/apple/src/style.rs` 275), not the keyword.
   - Dynamic system colours on the JS target run only when the value matches `/-apple-system-/` (`host/web-js/src/style.rs` 473–480). A bound `-exact-system-fill` skips the rewrite. Static colours are fine, because `css.rs` expands `ColorValue::System` to the pair before emit.

   `transition` and `keyframes` name `tint-color` through `Property::from_name` (`motion/src/property.rs` 167, 187–190; `contract/lower/src/svg.rs` 384–397). The plan never mentions that table. Unknown transition properties are errors (`motion/src/parse.rs` 79–80), so a missed update fails the build if sources move. It does not fail if a source keeps `transition="tint-color 90ms"` while the attribute becomes `-exact-tint-color`: the old spelling keeps working inside the value, which is an alias the RFC forbids. The value error for a colour (`values.rs` 134) does not name a replacement, so `-apple-system-fill` would not get the hint §3 promises. `BadCornerShape` (141) is the only value error the plan updates.

4. **Medium. The "15 properties" count and the "78 files, 243 lines" cost are wrong, and the cost is what §5 uses to say the prefix is cheap.** §2's table is fourteen properties plus two keywords. At `0b871f54e`, those spellings occur on 140 lines in 25 `*.contract` files, and on 145 lines in 28 files under the directories §4 names (`apps`, `examples`, `scripts/fixtures`, `host/web-js/conformance`, `contract/corpus`). Repo-wide, including Rust and JS, it is 487 lines in 145 files. None of those is 78 files or 243 lines. The direction ("uncommon, mostly fixtures") still holds. The number does not.

5. **Medium. D8 does not check the positions D1 claims.** The test walks the style-name table, corner-shape keywords, and system colours (126–133). It does not walk easing functions, `Property::name`, `env()` variables, or the Swift and JS copies of the system-colour strings. A new bare `spring`-like function passes. Until Q5, `tags.rs` stays a second declaration authority beside `schema.json`, which the RFC admits and correctly postpones.

6. **Low. D1 never says whether an `env()` argument or a media feature is a CSS position.** Nothing invented is there today. `QUEUE.md` already floats a `keyboard-inset` env. D8 would not force a prefix on it. One sentence in D1 or D5 closes this. Contract actions such as `haptic()` (`contract/types/src/checks.rs` 705) and palette calls such as `accent()` are expression language, folded before the CSS parser (`contract/lower/src/keyframes.rs` 5). They should be named beside props in D5 so nobody prefixes them under "every function."

7. **Low. The sed for Interview and Weird Castle is unspecified, and a global replace corrupts host spellings.** `--exact-exit-animation` contains `exit-animation`. Schema codec ids `"drag-timeline"` and `"symbol-palette"` (`schema.json` 2869, 2973; `kernel/build/codec.rs` 95, 102) are generator keys, not author names. The commit note should say the sed touches author spellings only, including `tint-color` inside `transition="…"` and `keyframes` lines and colour strings, and does not touch `--exact-*` or codec ids.

## Suggestions

Quote WebKit's keyword line for `-apple-system-fill` and for `-apple-visual-effect` in §2, at the revision you actually read. If `-apple-system-fill` is there and means `systemFill`, delete it from D2.

Add `spring()` to the inventory, or narrow D1 so a timing function declared in LLP 1002 is explicitly out of this rename. Do not leave the rule saying one thing and the table another.

In §4, replace "the web writes no declaration" with the real split: rows 162–170 emit nothing; exit, layout, drag, and press emit `--exact-*` custom properties that stay; `tint-color` animates as `--exact-tint`. Name `motion/src/property.rs`, `svg.rs` 384–397, `VibrancyIOS.swift` 146–155, and the `/-apple-system-/` guard. Point verification at `vibrancy.contract` and a presence page, not only `visual.contract`.

When §4 edits LLP 1001, correct its bit numbers from `schema.json`. Copying the new names onto the old bits leaves the declaration false.

State the author prefix and the host prefix as a pair: author `-exact-press-scale`, host `--exact-press`. D5's `--exact-accent` example is the host side only, and the two are easy to conflate.

## §7

1. **Agree. Keep `-exact-apple-` declined.** Coverage already changes (`press-haptic` on the web, LLP 1077 D14). A tier in the name is a rename every time a host appears, and it suggests the row means nothing elsewhere, which D1 of LLP 1077 forbids. LLP 1001 is the right place for "which host draws this."

2. **Disagree, until WebKit is quoted.** If `-apple-system-fill` is in `CSSValueKeywords.in` and is `systemFill`, renaming it to `-exact-system-fill` violates D1 and makes the iOS vibrancy switch miss (concern 3). Swapping in tertiary or quaternary fill would change pixels, and that half of the recommendation is right either way. If a re-read shows the bare keyword is really absent, `-exact-system-fill` is the right spelling, and the error text plus `VibrancyIOS.swift` have to learn it in the same commit.

3. **Agree that this is a separate RFC, and that this RFC should not wait.** Adopting the property is a design change (`backgroundMaterial` is a prop; a style row can ride a `style` class, which LLP 1053.000.000 noted a prop cannot). James's third tier is only a spelling for a property you have already decided is Safari's. The gate matters: regular Safari content does not get `useSystemAppearance`, so "the web host emits it natively" is the wrong test. Start with the value-by-value comparison against the `materials` table and D13. Keep `backgroundMaterial` where they differ.

4. **Agree, as stage 2.** A monochrome symbol is a glyph. CSS colours glyphs with `color`, and the web already draws it that way (LLP 1077 D10). `tint-color` is not inherited and starts black (`schema.json` 2470–2473; LLP 1001 128–129), so moving symbols onto `color` changes pixels wherever an ancestor set `color`. Keeping that out of the rename commit is required. Raster images still have no CSS property for a template tint, so `-exact-tint-color` there is the rule, not a special case.

5. **Agree, in a later commit.** `schema.json` is the declaration authority (`CLAUDE.md`). The style-name table in `tags.rs` is a second one. `"kind"` alone is not enough for D8: the CSS and browser entries need the spec or the WebKit revision next to them, or the hand-kept list beside the test remains.

6. **Agree, as a separate change, with `bitmap-width` held for you.** `scroll-start`, `scroll-restoration`, `bitmap-width`, and `initial-item-count` are props (`tags.rs` 1166, and D5). They are not this RFC. Making a kebab-case attribute mean a CSS property or an HTML attribute (`aria-label`, `viewport-fit`) is how an author tells the two vocabularies apart. `scroll-start` was a CSS proposal's name before CSS renamed it `scroll-initial-target`, which is the collision this RFC is about, on the prop side.

---



## Round 2: Draft r2 (2026-10-03)



- **Family:** xAI, `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one fresh session in the main checkout (69 turns, 212 tool calls, `stopReason: end_turn`).

- **Reviewed:** `llp/1081-names-exact-invents.rfc.md` Draft r2, untracked over `0b871f54e` (blob `eec424cd`).

- **Redacted:** no.

- **Method:** mutually blind to Astra's round 2, identical brief (sha256 of `brief-1081-r2.md` recorded in the Astra file). It had both round-1 reviews to check r2 against. Static audit; ran nothing, changed nothing.

- **Transcription:** the joined `text` events, unedited; the first four sentences are progress narration.

- **Author's check (not part of the review):** WebKit `CSSValueKeywords.in` at `bb06bdc9`, lines 283–290, are `-apple-system-label`, `-secondary-label`, `-tertiary-label`, `-tertiary-fill`, `-quaternary-label`, `-quinary-label`, `-grid`, `-separator`. The file has no line `-apple-system-fill`.


I'll audit the RFC against both r1 reviews and the code it cites, starting with the rules, the draft, and the two reviews.The draft and both r1 reviews are in. Next I’ll check the name tables, parsers, and the files stage 1 says it touches.The remaining checks are the line counts, the inventory claims, and the two parser sites the plan cites. I'll verify those against the files, then write the verdict.`tags.rs` is 1,499 lines, as the RFC says. Next I’ll count the inventory and re-read the host paths stage 1 and stage 2 would have to touch.**NOT READY** to commit as the implementation spec. The naming rule is in shape: D1–D7, the decline of `-exact-apple-`, the inventory, and refusing the old spelling are fit to keep. A literal reading of §4 and stage 2 still changes behavior the text says stays still, and stage 1b points at generator code that does not run. The fixes are local additions to the plan. They do not reopen the §7 rulings.

This is a static read of Draft r2 over `0b871f54e`. Nothing was edited. WebKit at `bb06bdc992326258c402dd4594c2cffd77ca3795` was not opened.

## r1 concerns

**Resolved**

- **Astra 1, Grok 1 (High, D1).** `llp/1081-names-exact-invents.rfc.md:79–97` classifies a spelling in a position by grammar and meaning. A CSS name needs a current CSSWG draft or Editor’s Draft, or unprefixed exposure to ordinary web content. A browser name must be in that engine’s tables at a pinned revision, exposed to ordinary web content, with Exact’s grammar and meaning. Native emission is gone, which is what the system colours needed: `host/web/src/css.rs:502` still lowers every one to a `light-dark()` pair. Gated `-apple-visual-effect` is Exact’s under D1 (`:93`) and Q3 (`:265`), not a browser name.
- **Astra 2, Grok 2, inventory half (High).** `spring()` and `underline-line-through` are in §2 (`:62–69`) and D2 (`:114–117`). `env()` and media features are stated as checked (`:73`). The private `spring(k, d, m)` protocol for `presence-glue.js` stays (`:143`, `:182`). The JS filters that drop springs are named and correctly left as substring matches (`:183`).
- **Astra 3, Grok 3, the paths they named (High).** `motion/src/property.rs`, keyframes at `svg.rs:384–397` and `:568–574`, `class.rs:115`, the camelCase hint at `tags.rs:1123`, `VibrancyIOS.swift:146–155`, `CornerShape::css` (`corner.rs:84`), and the JS `/-apple-system-/` guard (`host/web-js/src/style.rs:473–480`) are in §4. `--exact-tint` stays the emitted name and is refused from author source (`:145`, `:181`). Rows 162–170 still emit nothing (`host/web/src/css.rs:416`, `host/web-js/src/style.rs:518–528`). The `--exact-*` custom properties stay (`:142`).
- **Astra 7 (Medium, D4 facts).** The web-arm claims are withdrawn (`:131`). The decline now rests on an ambiguous tier, which matches both reviews.
- **Astra 8 (Medium, D5/D6).** Author identifiers, expression functions, and styleable props are outside (`:133–137`). Graduation keeps a still-true subset (`:148`).
- **Astra 10, Grok 4 (counts).** Fourteen properties is right. At this tree the union of those spellings and `spring(` is 35 `.contract` files and 158 matching lines. That parenthetical checks out.
- **Grok 7 (Low, migration script).** `:207–213` rewrites author spellings only and leaves `--exact-*`, codec ids, and comments alone.

**Still open, in part**

- **Astra 4 (High, runtime refusal).** The guard and `CornerShape::css` are named. “A computed old keyword is refused at run time, as other bad computed values are” (`:189`) does not say what that refusal is. In the same function a bad dynamic `clip-path` writes `null` (`host/web-js/src/style.rs:578–580`). The corner mapper only replaces (`:585–587`). One sentence pointing at that pattern closes it.
- **Astra 5 (High, stage 2).** The false “the web already uses `color`” is corrected (`:230`). The web stylesheet change, the raster exception, palette and multicolor, native buttons, computed sources, and `transition: color` are specified. The Apple paragraph does not cover the iOS monochrome assignment. New concern 2.
- **Astra 6, Grok 5 (Medium, D8).** The style-name move is real and the many-to-many postponement is right. The `Kind` enum still does not match D1’s second CSS arm, and the other “kind columns” are not required to be what the parsers read. New concern 6.
- **Grok 2, the serializers (High).** The private protocol and the JS substring filters are handled. `motion/src/animation/parse.rs` and the runner’s reorder writer are not. New concerns 1 and 5.
- **Grok 6 (Low).** D1 now includes `env()` and media features (`:84–85`). D8 still does not walk media features (`:165`). Nothing invented is there today (`kernel/src/style/env.rs` admits only `safe-area-inset-*` and `viewport-segment-*`; `runner/src/viewport.rs` reports CSS feature names). A future media feature added only on the agent side would not fail the test.
- **Astra 9 (Medium, verification).** The motion, vibrancy, conformance, and out-of-repo list is what that review asked for. Still unnamed: a class row present on only one branch (`class.rs` merges by attribute-name string), and a check that the agent’s `layout` field names stay field names (`:141` states it; verification does not). “Launched and driven” is macOS and the web (`:226`); iOS is a build plus the vibrancy and visual screenshots (`:220–222`).

## D1, applied to the inventory

Two people applying D1 to the names in §2 get the same kind. The fourteen properties are not CSS properties and are not in a browser’s ordinary-web tables, so they are Exact. `spring()` fails the browser test on both grammar and exposure. `-apple-continuous` is not a `corner-shape` keyword. `-webkit-text-stroke` and its longhands are the Compat Standard, so they stay browser names. The label and separator spellings, if exposed to ordinary Safari as the RFC says, stay browser names. The drafts §2 keeps (`wrap-flow`, `shape-outside`, `shape-margin`, `interpolate-size`, `field-sizing`, `corner-shape`, `timeline-scope`, `animation-range`, the two `env()` families) are CSS names in those positions. `box-shadow`, `accent-color`, `text-transform`, and `backdrop-filter`’s single `blur()` stay bare as declared subsets (D3). Keywords of an `-exact-` property stay bare (`impact-light`, `hierarchical`, `soft`). `symbol:` is a URL scheme (`:146`).

The same pass over the other CSS positions does not turn up a second invented name the inventory missed:

- `motion/src/property.rs:167–191` exposes `tint-color` and `--exact-tint`. `box-shadow-color` and `layout` are not author names (`ShadowColor` is excluded from `from_name`; `layout` is outside the author table).
- `kernel/src/style/backdrop.rs` admits `none` and one `blur()`.
- Schema enums through the decoration and affordance rows are CSS subsets or bare keywords of an Exact property. `underline-line-through` (`schema.json:1462–1468`) is the one misspelled CSS value, and it is now in stage 1b.
- Props such as `buttonStyle`, `scroll-start`, and `bitmap-width` are outside, including `scroll-start` even though CSS later used a different name for that idea (`:135`).
- `haptic()` and `accent()` are expression functions (`:137`).

The judgement D1 still leaves to a person is a future one: an unprefixed experiment whose meaning differs is Exact, and a declared approximation of a real CSS name stays bare. For this tree those calls are stable.

## Stage 1, stage 2, D8

`tags.rs` is 1,499 lines, so the cap claim is true. Moving the style-name arms into a new `style_names.rs` that `attr` looks up fits under 1,500 and leaves `schema.json` as the declaration authority. `build.rs` is also 1,499. Adding `TextDecorationLine` to the existing match arm at `kernel/build.rs:1303` does not add a line. A hand-written `from_css` can live beside `TouchAction` (`kernel/src/style.rs:1462–1476`, 1,480 lines) without editing generated code. `pascal()` treats a space as a word break, so the variant stays `UnderlineLineThrough`.

That is buildable. The citation that says to copy `GridAutoFlow` is not, and two host paths the plan does not name would change behavior. Stage 2’s web half can be implemented from the text: both stylesheets (`host/web/glue.js:357`, `host/web-js/symbols.js:9`) fill the mask with `currentColor`, and the raster template keeps `--exact-tint` (`host/web/src/element.rs:199–217` applies it only when the source does not start with `symbol:`). Linux really draws no glyph (`host/linux/src/image.rs:153–173` keeps an empty em square). iOS monochrome cannot be implemented from the sites stage 2 names.

## WebKit

Consistent with what I know: `-apple-continuous` is not a `corner-shape` keyword; CSS’s set is `round | scoop | bevel | notch | square | squircle | superellipse()`. WebKit’s `spring()` takes mass, stiffness, damping, and initial velocity, space-separated, behind a setting that is off for ordinary content. The label and separator names are WebKit’s. `useSystemAppearance` is a real gate for system web views, so treating `-apple-visual-effect` as unavailable to ordinary content is plausible. I have not seen the pinned file, so the property’s values on that date are the author’s report.

Not consistent with what I remember, and not closed by the citation: the claim that bare `-apple-system-fill` is absent (`:67`). I remember `-apple-system-fill` and `-apple-system-secondary-fill` in `CSSValueKeywords.in`, which is also what the Grok r1 review believed. Astra’s memory agreed with the absence. Lines 283–290 are eight lines. The RFC places five label and separator names in that range and also places `-apple-system-tertiary-fill` at line 286. Two lines in the range are unnamed. That is the block where the bare fill and the secondary fill would be. The author’s check printed at the top of `llp/reviews/1081-names-exact-invents.grok.md` is not part of that review, and I could not re-read the file. If those two lines are the fill keywords and they mean UIKit `systemFill`, D1 says keep `-apple-system-fill`.

## New concerns

1. **High. The reorder preview writes author transition text the plan never updates.** `runner/src/instance/collection/reorder.rs:272` sets `("transition", Value::str("translate spring(300,30,1)"))`. `views.rs:4–26` builds that through `StyleId::from_name` and `bridge::set_style`, and `bridge.rs:77–89` stores the string as `StyleValue::Text`. That is the author grammar `Transitions::parse` reads. After `parse.rs` refuses bare `spring(`, this preview stops springing. §4 step 2 names only `host/web-js/reorder.js:115`, and it allows that string to stay if someone decides it is host-private. The JS comment calls it the same style op. The Rust writer is not the `presence-glue.js` protocol (`:143`). The “114 code files” sentence (`:194`) is a count under “Contract inside Rust and JS tests,” and this file is runner source, not a test. A sweep of mentions would find it (I count 120 non-Markdown, non-`.contract` files for the same pattern, two of them `spring(` false positives under `vendor/rapier3d`, so 118 against the RFC’s 114). The instruction that classifies this string points the other way. **Resolve:** name `reorder.rs:272` and `reorder.js:115`, and say both strings are author grammar and become `translate -exact-spring(300,30,1)`.

2. **High. Stage 2’s Apple sites leave iOS monochrome on `tint_color`.** `NodeViewIOS.swift:423` is `symbolView?.tintColor = color("tint_color", .black)`, after the cache key is checked. Stage 2 cites `Affordances.swift:18` and `NodeSymbolMac.swift:43`. Line 18 is the comment above `symbolLookKey`. The reads inside that function are the cache key at line 22 (`style["tint_color"]`) and the hierarchical base at line 28. The monochrome configuration returns the base unchanged (`Affordances.swift:39`). macOS sets `contentTintColor` at `NodeSymbolMac.swift:43`. Updating only the cited sites moves hierarchical and macOS monochrome to computed `color` and leaves iOS monochrome black unless `tint_color` is set. **Resolve:** name `NodeViewIOS.swift:423`, and say `symbolLookKey` includes the computed text colour on both platforms. `text_color` is the inherited row (`schema.json` around the `text_color` field); `tint_color` is not.

3. **Medium. Two more symbol faces read `tint_color`, and stage 2 does not say whether they move.** `SegmentsIOS.swift:61` tints a projected `symbol:` image with `image.color("tint_color", .label)`. The native-tab branch above it (`:44–49`) uses the accent, which stage 2’s native-button exception covers. `NavigationBarIOS.swift:133` picks `tint_color` as the ink when a bar-item badge contains a symbol, and `text_color` when it contains text. Neither is a native button’s platform glyph (LLP 1069.011 D7). **Resolve:** one sentence each, in or out. If they are in, the badge’s raster cache key has to include that colour too.

4. **Medium. Stage 1b cites a `from_css` that never runs, and misstates Linux.** `kernel/build.rs:1284–1286` sends `GridAutoFlow` and `JustifyItems` through `emit_grid_seam` and `continue`s. The arm at `:1303` that calls `from_css` is not reached for them. There is no `GridAutoFlow::from_css` in the kernel. The live order-insensitive parsers are `kernel/src/style/grid.rs:1150` (`check_auto_flow`) and `:1169` (`check_justify_items`). `TextDecorationLine` is not in that seam, so adding it to the `:1303` arm would actually emit a call, and the method has to be an inherent impl on `crate::generated::TextDecorationLine`, the same shape as `TouchAction` at `kernel/src/style.rs:1462`. “Linux matches the enum” (`:204`) does not: `host/linux/src/text.rs:92–107` copies font metrics only, and `host/linux/src/text/catalog.rs:236–255` never reads a decoration. Apple already emits the spaced spelling (`host/apple/src/content_region/wire.rs:96`) and matches with `contains`, so native pixels stay put for that reason. The generic enum error (`contract/lower/src/values.rs:128–131`) lists `name()` values and will not, by itself, say that `underline-line-through` was the old token. **Resolve:** cite the seam, the `TouchAction` impl, and Linux’s omission. Say whether “expected one of …” is the hint or whether `UnknownEnumValue` gains a rename line.

5. **Medium. An animation easing detects only the old spring spelling.** `motion/src/animation/parse.rs:442` treats a part as known when it `starts_with("spring(")`, then `easing_only` (`:448–454`) reports that a spring is not an animation easing. `-exact-spring(` does not start with `spring(`, so it misses that diagnostic and falls through to a bad animation shape. Bare `spring(` still reaches `easing()` and is refused once `parse.rs` refuses it, so this is not an alias. **Resolve:** detect the function by the suffix, and keep the “not an animation” error for `-exact-spring(`.

6. **Medium. D8’s kind column does not enforce D1 on every vocabulary it claims.** `Kind::Css(spec)` (`:154`) has nowhere to put D1’s “shipped unprefixed, no CSSWG document” (`:91`). Every bare name in today’s inventory has a spec, so stage 1 does not trip on it. For the other tables, “gets the same `Kind` column” (`:165`) does not say the parser reads that column. `SYSTEM_COLORS` (`kernel/src/style/symbols.rs:68–75`) is a real table, and adding a field is a compile break at every `(_, l, d)` destructure, which is fine. The corner keyword and the easing function are match arms. `-apple-continuous` is not in `KEYWORDS` (`kernel/src/corner.rs:111`, beside the keyword table). A side list the test walks can drift from the arm. Media features are in D1 and absent from the walked list. The waiver for schema enums (`:167`) is the Chrome oracle that already missed `underline-line-through`. **Resolve:** let a CSS kind cite either a spec short name or an engine revision; say each parser consumes its table, including the continuous corner; walk media-feature names or say there is no authoring table for them yet.

7. **Medium. The system-fill absence is the load-bearing WebKit claim, and the cited range does not show it.** Q2 (`:264`) renames `-apple-system-fill` because the pinned file has no such keyword. The citation leaves two lines of `CSSValueKeywords.in:283–290` unnamed, beside a tertiary fill at line 286. **Resolve:** paste the keyword lines 283–290 into §2. If `-apple-system-fill` is among them and means `systemFill`, delete it from D2 and keep the spelling; the values and index 5 stay either way. Do not swap in tertiary or quaternary fill.

## Suggestions

The native-button allowlist does not need a string edit. `NATIVE_ROWS` (`contract/lower/src/controls.rs:449`) is `StyleId`s, looked up with `tags::attr`. The spelling `exit-animation` there is a comment (`:714`). After the name table returns the same `StyleId`, the allowlist follows. §4 step 1 can say that.

Derive the JS system-colour guard from the six `SYSTEM_COLORS` names, not from a single prefix. Five names stay `-apple-system-`. A guard rewritten as `/-exact-/` would stop rewriting the labels.

`BadCornerShape` (`values.rs:141`) is the corner message to update. The colour error at `:134` is a grammar list and does not name `-apple-system-fill`; the system-colour refusal has to gain the rename hint on its own.

After concerns 1–5 and 7 are in the text, this is the document to implement. It does not need another pass on the rule.
