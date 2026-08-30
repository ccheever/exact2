# Panel: LLP 1019 Fonts — declared, not loaded (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`. Reported model not independently verifiable from the CLI; requested `grok-4.6` at `xhigh`.
- **Method:** a panel at Charlie's request, 2026-08-30 — LLP 1016 §6's sense: not a refine loop, no verdict binds, no approval given or withheld. Two rounds. **Round 1 blind** (neither panelist saw the other's answer); **round 2 mutually visible** (each read the other's round-1 answer in full and was asked to write only where they differ). Workdir = a read-only panel capsule, no network: LLP 1019 r1, the panel brief, exact2's four host text paths (`host-web-css.rs`, `host-apple-Text.swift`, `host-apple-measure.rs`, `host-linux-text.rs`), the kernel text IR and its style rows, `contract-lower-tags.rs`, LLP 1005 and 1011, both app contracts (Caltrain and Weird Castle), the rules (`RULES.md`, `NOT-DOING.md`, `CLAUDE.md`), and exact1's whole font system (`fonts-and-assets.md`, `assets-fonts-{state,registry}.ts`, the Apple and Windows registries, the 2026-07-02 Apple text-stack audit, the LLP 0470 limitations excerpt).
- **Gates:** LLP 1019 r1 sha256 `159cfed094e038dc70abc700684df34cc75dc9c3c247f9041ee8afb8574a48a1`; round-1 brief sha256 `36c3e6bf507284200fe1320011a2e1d2e03b13ef7c26fd78b57faf5e087ac3b0`; round-2 brief sha256 `941d016f69d4ee6defcc9c7888e5380168ef63593bcb2934e4d55750af1f5774`; capsule (evidence files, panel answers excluded) sha256 `2e8c5c8e28bc43be56856c3a82671bd61df66c67e8ce3b81dffb349983bac5b9`. No capsule file was modified while either panelist was in flight.
- **Runs:** round 1 2026-08-30T16:04:50Z → T16:12:43Z; round 2 → T16:24:53Z.
- **Disposition:** folded into LLP 1019 r2 §11. The panel's factual catches against the live repo were re-verified by the author before folding — see §11 for which held.

---

## Round 1 — position (verbatim; the CLI's narration lines before the document marker are dropped)


---

## Round 2 — cross-response, mutually visible (verbatim)

# LLP 1019 is solving the right problem with the wrong tables

## 1. D1/D2 — fonts as a compile-time declaration

**Position.** The trade is right. A font is a Contract declaration, the compiler assigns the `u16`, and nothing resolves at runtime. That does not foreclose an OTA app, a user-chosen font, or a font that arrives with data in any app exact2 is actually building. It costs the dev loop one new declaration plus a file in `assets/`, which is the image loop.

**Reason.** exact2 has no application JavaScript and the plan is compiled before the first frame (`exact2/CLAUDE.md`, `exact2/RULES.md` §Scope). exact1’s load-bearing choice was the opposite: `await loadFont(name, source)` while the app ran (`exact1/fonts-and-assets.md`), ids handed out from 1024 when the load resolved (`exact1/assets-fonts-state.ts`), and then three cache-coherence machines so a baked `u16` could change meaning (ENG-22788, ENG-22803, Apple `invalidateExactTextMeasurementCacheForFontChanges`). Those exist only because the id was allocated too late. A new plan in ~20 ms is cheaper than any of them.

The three “foreclosures”:

- *OTA.* `exact2/NOT-DOING.md` already killed HBC, staged reload, and the update economy. A reload is a restart from a new plan. A new face is a new plan plus a file in `assets/`, the same way Caltrain’s train PNG is (`exact2/1011-image-v1.spec.md` §2–§3). You cannot hot-swap a TTF under a live id, and you should not be able to.
- *User-chosen font.* `system-ui` (id 0, already the kernel default in `exact2/kernel-text.rs`) is the OS face, including whatever the user set. A dropdown of arbitrary installed names is OQ3, not a runtime registry.
- *Font that arrives with data.* That is a document viewer. Weird Castle and Caltrain are bundled apps (`exact2/weird-castle.app.contract`, `exact2/caltrain.app.contract`). A face in a `resource` is a later product.

Dev-loop cost is the image cost: edit the Contract, save, `host/web/dev.mjs` restarts from the new plan in ~20 ms. Native recopies `assets/` the way it already does (`1011` §4, RFC §1). You cannot preview an undeclared face; you also cannot preview an undeclared image.

Two missing decisions will cost a rewrite if left implicit:

1. **`font-family` values are literals (or other compile-time constants), not arbitrary strings.** The runner’s dynamic style path takes numbers and CSS-ish strings (`exact2/1005-plan-and-runner-v1.spec.md` §6). `font_family` is a `u16` (`exact2/kernel-schema-text-rows.json` bit 70). A derive that evaluates to `"Menlo"` has nowhere to intern. Theme switching is a `when` with two literal stacks, not a runtime name.
2. **The compiler refuses a missing font file.** LLP 1011 does not: a missing `image` source is a runtime stderr line. A missing face is every text node in the wrong metrics. exact1’s `asset()` failed at build if the file did not exist (`exact1/fonts-and-assets.md`). Do not inherit the image precedent here.

**What would change my mind.** A real exact2 app that must accept a face after boot (user font picker over the system library, a document with embedded fonts, an OTA face with no new Contract). None of the capsule apps are that. A `font-family` bound to a string slot would also force a runtime intern; I would then want that intern to be a closed table of already-declared ids, not a new registry.

## 2. D3 — the id names a stack, not a face

**Position.** Refusing a built-in table of concrete family names is right. Interning a whole CSS list as *one `fonts` table entry that also holds faces* is a category error, and as specified it will recreate scar 4 on Apple. Split the tables, or descope author stacks in v1 and always cascade to a last-resort.

**Reason.** D2 and D3 describe different things under the same name. D2: `fonts` (name, `range:faces`) and `faces` (weight, italic, path). That is a *family*. D3: the compiler interns `font-family="Castle Display, system-ui"` as one table entry. That is a *stack*. A row cannot be both. `font_family` is a single `u16` (`kernel-schema-text-rows.json`); the kernel will never see the list. The host has to expand it. If the interned row only carries Castle Display’s faces, system-ui is gone.

exact1’s alias table (`exact1/assets-fonts-state.ts` `builtinFontFamilyIds`) is scar 2: it missed `-apple-system` / `BlinkMacSystemFont`, fell through to `arial`, and bold painted thin on native (ENG-22195). Do not bring `helvetica neue`, `arial`, `menlo` back as builtins. Generics at low ids, declared families above them — yes. Do **not** alias `system-ui`, `ui-sans-serif`, and `sans-serif` to one id the way exact1 did (all three were `1`, and `ui-rounded` was `1` too). They are different CSS generics; a host may map them to the same face, the table must not.

When a host cannot honor a stack:

- *Web* — emit the CSS list. The browser cascades. This is the only host where D3 is free.
- *Linux* — `Attrs` currently hardcodes `Family::SansSerif` (`exact2/host-linux-text.rs`). A stack becomes a `families` vector. cosmic-text already falls through; this is doable.
- *Apple* — this is where D3 dies if D4 is taken literally. `Text.font(size:weight:italic:)` returns one `PlatformFont`, cached on `"size/weight/italic"` (`exact2/host-apple-Text.swift`). “Extend that function with the family” still returns one face. exact1 already knew the API for per-glyph fallback is a CoreText cascade list (`exact1/fonts-and-assets.md`, scar 4 / LLP 0470). One `UIFont` is whole-family fallback. Mixed script and emoji will be right on web and wrong on Apple again.

Minimum that does not regress emoji: every custom family is `[declared, last-resort]`, even if the author wrote a single name. Author-written CSS lists (`Castle Display, Georgia, serif`) are extra. Weird Castle does not need them (`1019-fonts.rfc.md` §6: “one or two brand faces”).

Also missing: the CSS `font-family` list grammar. Quoted names, unquoted names, whitespace. Specify a subset or you will parse it four times.

**What would change my mind.** A `stacks` table (`range` of family-or-generic ids) plus a `families` table, with `font_family` indexing a stack, and Apple resolving through `kCTFontCascadeListAttribute` rather than `UIFont(name:)`. Then D3 holds. A Weird Castle screenshot that actually needs a three-name stack would also justify the extra table now.

## 3. D4/D6 — one resolver per host; family first, then weight

**Position.** “One function *per host*, shared by measure and paint” is holdable in this repo. “One function” as a cross-host invariant is not, and a comment will not enforce it. Family-first-then-weight is the right policy and it is CSS Fonts 4 §5.2. `snap_weight` is a cosmic-text workaround. Do not promote it to the other three hosts.

**Reason.** exact1’s scar 3 was a *count* problem: `styledPlatformFont` was shared and correct; `hostTextGeometryFont` forgot `fontApplyingWeight`; `swiftUIFont` was a fifth path (`exact1/2026-07-02-d3-apple-text-stack-review.md` §2.1). exact2 deleted the extra consumers. There is no JS `measureParagraph`, no SwiftUI text path, no second Apple engine. Apple already has one function used by measure and paint (`host-apple-Text.swift`); Linux already has one `TextEngine` shared by the measurer and the painter (`host-linux-text.rs`); web is the browser. That shape can be kept **if no fifth path is added**.

What actually enforces measure/paint agreement is not “add no second one”:

1. The family id is on the run that both sides hash. Today it is not. Apple `CRun` has size/weight/italic and drops `font_family` (`host-apple-measure.rs`). Linux `Run` / `Spec::from_request` drop it too (`host-linux-text.rs`). Apple’s font cache key is `"size/weight/italic"`; Linux’s `weights` map is `(weight, italic)` with no family. Adding a family without extending those keys is a silent wrong-font cache hit.
2. One paragraph object is what was measured and what is painted (already true on Apple and Linux; keep it).
3. OQ4’s width fixture, one Contract, 400 vs 700 actually different. Without that, this will rot the way exact1’s fourth resolver did.

`snap_weight` (`host-linux-text.rs`) exists because “cosmic-text’s fallback takes the requested weight literally and ranks any face whose variable `wght` axis covers it above the family’s nearest static face” — San Francisco leaked in at 500/600 while DejaVu served 400/700. That is a shaper bug. The *policy* it is approximating is CSS matching: family first, then stretch/style/weight *inside* that family. Web already does that if you emit `font-family` and `font-weight` and do not snap. Apple `systemFont(ofSize:weight:)` already does it for the system family. Porting `snap_weight` to those hosts would *create* divergence.

Further, `snap_weight` queries only `Family::SansSerif` and caches without a family key. The moment a second family exists the cache is wrong. For a variable font with a `wght` axis, D6 says set the axis; `fontdb::Query` may instead pick a named static instance. Linux may keep `snap_weight` as a local clamp, per family, and must not snap a real `wght` axis.

Stretch is part of CSS matching and exact2 has no stretch row. Say so, and ignore it in v1.

**What would change my mind.** A host whose measure engine is not its paint engine (UILabel vs CoreText was exact1; exact2’s Apple path is already CoreText both ways). Evidence that cosmic-text without `snap_weight` now honors family-first would let Linux delete it rather than document it as policy.

## 4. D5 — register at boot, no loading state, remote fonts out of v1

**Position.** Boot registration, no app-visible loading state, no `display` policy, remote fonts out of v1: yes on Apple and Linux, **false on web as written**. “No FOUT” is a lie for `@font-face`. That does not break “the web is the standard” if you tell the truth about first frame and settle after `document.fonts.ready`. D5 is also wrong that the boot batch carries the font table.

**Reason.** Apple `CTFontManagerRegisterFontsForURL(…, .process, …)` and Linux `db.load_font_file` are synchronous for local files. Bundled bytes are on disk. No `display` knob, no splash, no `useFonts`. Remote URL sources were exact1 scar 5 (`display` accepted and ignored on native, `exact1/0470-font-registry-limitations.md`). Keep them out.

Web is not that. `host-web-css.rs` emits declarations; the browser loads `@font-face` asynchronously even from `url(assets/…)`. First paint can be fallback (FOUT) or invisible (FOIT under `font-display: auto`’s block period). Native first frame is the brand face. Image already has this shape of timing hole: web’s `<img>` loads in the browser, macOS measures `0` until `exact_intrinsic` (`1011` §1, §3–§4). Fonts are worse, because they change **every text metric**, not one replaced element.

Honest story:

- Native: register from the plan table, then first layout. First frame is the declared face or a hard host error (stderr + last-resort), not a loading state the app sees.
- Web: emitting `@font-face` is not registration. First frame is allowed to be fallback. **`document.fonts.ready` (or `FontFace.load` on each declared face) is boot work**, analogous to decoding the plan, not an app `display` policy. The parity oracle applies to the *settled* frame. First-frame screenshots across hosts will disagree unless the web host waits, and waiting must not blow the 100 ms cold-start budget (`RULES.md` Time budgets) — local files usually don’t; a hang must time out and present with fallback.
- Do not invent `font-display` in Contract. If the web host waits at boot, `display` is unused. If it does not wait, CSS `auto` is the browser’s policy and native still will not FOUT.

“The host reads the plan’s font table from the boot batch it already reads (`exact_out()`)” is confused. `exact_out()` is kernel ops. Font paths are not style rows and not node props. Images work because `imageSource` is a string prop on the node (`1011` §2; `contract-lower-tags.rs` positional `imageSource`). Fonts are a table of files with no node. The host that holds the `Plan` reads `plan.fonts` at boot (`1005` §1: `plan/tables/format.json` is the declaration authority). If Swift only sees the C ABI, that is a host seam to specify, not “no new entry point because ops.” `CRun` still needs `font_family` (that *is* an ABI change; D4 already admits it).

`css_text` today takes `StyleProps` only (`host-web-css.rs`). Un-skipping `font_family` and printing the `u16` would emit `font-family:7`. The web host needs the interned name list from the plan table. D4 waves this away.

**What would change my mind.** A web boot that blocks present on `document.fonts.ready` with a cap, making first frame honest on all four hosts — then “no FOUT” becomes true as a *host* property, still without an app loading state. Evidence that local `@font-face` is always ready before first paint in the actual `host/web/dev.mjs` path would also let D5 stand for that host.

## 5. OQ1 — synthesis

**Position.** Pick the nearest real face. Do not synthesize. The RFC’s motivating example is not synthesis; it is matching, and D6 already decided it.

Recommendation: nearest-face — confidence 70%

**Reason.** CSS Fonts 4 splits two steps the RFC folds together (`1019-fonts.rfc.md` §5, OQ1):

1. **Matching** (CSS Fonts 4 §5.2). Request 600 with 400 and 700 declared → the nearest heavier face is 700. Nobody smears 400. Linux `snap_weight` is this step (clumsily). D6 already says “the family wins, then the weight is matched inside it, by CSS Fonts 4 §5.2.” The `font-weight="600"` with 400 and 700 example does not belong in OQ1.
2. **Synthesis** (`font-synthesis`). Matching returned a face that is still not the requested weight or style — only Regular exists, author asked for 700 or italic — and the engine fakes it (smear / ~12° shear).

True OQ1 is (2). CSS’s initial is `font-synthesis: weight style small-caps`. `RULES.md` / `CLAUDE.md` say a semantic that could follow CSS follows CSS, and the web is the parity oracle. That is the whole case for synthesize, and it is a real case.

The case against:

- Weird Castle’s wordmark is `font-weight=800` with 9-point tracking (`exact2/weird-castle.app.contract`). A brand face shipped as one Regular, then synthesized to 800, will look worse than Regular. The honest authoring is to ship the 800 file or a variable font. §6 says that is the app.
- The four hosts do not synthesize with the same algorithm. exact1’s Apple code sheared ~12° (ENG-22086) while the doc said never synthesize (`exact1/fonts-and-assets.md`, scar 6). Following CSS *as a default* without matching algorithms is how you get maximum cross-host divergence and call it parity. Nearest-face is the same *file* on every host; rasterizers still differ, faces do not.
- Linux already nearest-faces weight via `snap_weight`. Apple already synthesizes italic on the system path (`host-apple-Text.swift`: `withSymbolicTraits(.traitItalic)` / `convert(…, toHaveTrait: .italicFontMask)`). Shipping “follow CSS” means teaching Linux to fake and teaching Apple to fake weight on custom families. Shipping nearest-face means `font-synthesis: none` on the web host’s stylesheet — a declared deviation in LLP 1001, the way Taffy’s missing `position: static` is — and deleting the synthetic italic branch for families that have no italic face.
- exact2 apps *declare* their faces. A missing bold is an authoring hole, not a web-legacy condition. CSS synthesizes by default because the web is full of `font-weight: bold` on fonts that never had a bold file. That is not this corpus.

70% and not higher because the CSS-initial rule is load-bearing and nearest-face needs a 1001 deviation plus a web stylesheet that is not the CSS initial. Charlie may decide the rule is not negotiable for rendering fallbacks. Small-caps synthesis is out either way: there is no small-caps row.

**What would change my mind.** Charlie ruling that CSS initials apply to rendering fallbacks with no 1001 exception. A four-host fixture of the actual Weird Castle face at weight 800 with only a 400 file shipped, showing synthesis closer across hosts than nearest-face (I do not expect this). An app that ships one Regular and has a legitimate need for faux-bold. If every v1 face is variable with a `wght` axis, weight synthesis never fires and I would still want `font-synthesis: none` for italic so Apple does not shear.

## 6. OQ3 — must a family be declared?

**Position.** Compile error for any name that is not a generic and not declared. Linux’s system library is not a reason to accept `"Menlo"` as an undeclared string. Generics (`system-ui`, `sans-serif`, `serif`, `monospace`, and the `ui-*` set) are the free names; `EXACT_FONT` / `EXACT_FONTS` are the Linux escape hatches that already exist.

**Reason.** Silent undeclared names are scar 2. `font-family="Menlo"` works on a Mac, falls through on a builder, and you find it by looking at the screen — the failure mode D2 exists to kill. Linux already answers “I want a face from the machine” with `sans_family` and `EXACT_FONT` / `EXACT_FONTS` (`host-linux-text.rs`): pin `sans-serif`, or load a directory. That is a host knob, not a Contract hole.

A later, still-declared form is defensible: `font "Menlo"` with **no files**, meaning “this name must exist on the host at boot; fail if it does not.” Still an id, still no runtime registry, still a compile-time name. Not v1. Do not take the shortcut of skipping the declaration.

Licensing (“cannot bundle, must use the installed face”) is the only honest argument for named system families. SF Pro is `system-ui`. Weird Castle wants a brand face it can put in `assets/`. Caltrain uses the system throughout.

**What would change my mind.** A real app that cannot bundle the face and cannot use a generic (a licensed house sans that is installed on the target OS and not shippable). Then add the file-less declaration, still mandatory.

## 7. What else is wrong, and whether to build this now

**Position.** Build a thin v1 now. The RFC as written will still cost a rewrite unless it decides how a *declared name* binds to *bytes*, and unless it stops growing surface that Weird Castle does not ask for.

**Reason.** `NOT-DOING.md` says v1 is done when one real app runs, and recommends Caltrain. Caltrain does not need a brand face. Weird Castle does: the wordmark is system font standing in for a display face (`weird-castle.app.contract`). `CLAUDE.md` already treats `~/projects/weird-castle` as a first-class consumer. The kernel row exists and is inert; web skips it as `"not lowered in v1"` (`host-web-css.rs`); Contract cannot say `font-family` or `font-style` (`contract-lower-tags.rs`). That is enough to build. It is not enough to build D3’s interned CSS parser, `font-synthesis` as a property, or `font-variant-numeric`.

Omissions that cost more than OQ1/OQ3:

1. **The matching key is the Contract name bound to the registered bytes, never the file’s name table.** exact1’s Apple registry reads PostScript / family name from the descriptor and then does `PlatformFont(name:)` (`exact1/PlatformTypes-font-registry.swift`). Web `@font-face` uses the CSS family name regardless of the name table. `font "Castle Display" = "assets/CastleDisplay-Variable.ttf"` will work on web and miss on Apple/Linux the first time the name table says `CastleDisplay` or `Castle Display VF`. That is ENG-22195’s shape. Apple should keep the descriptor from the registered URL; Linux should pin the loaded face ids. Do not look up the declared string in the system library.
2. **Cache keys and run structs omit family** (item 3). Landmines, not later work.
3. **WOFF2.** exact1 claimed TTF/OTF/WOFF2 (`fonts-and-assets.md`). Apple’s `CTFontManager` and Linux `fontdb` do not load WOFF2 the way a browser does. v1 formats are TTF and OTF; a `.woff2` in `assets/` is a compile error. Otherwise web works and native is silent.
4. **`font-variant-numeric`.** Bit 75 exists and is unreachable. RFC §7 offers “attribute with this work or declared unreachable.” Leave it unreachable. Weird Castle does not need it. Agents add none (`RULES.md` §Agents).
5. **OQ2.** Put the table in the plan (`format.json`). The runner ignores it; the host that already holds the `Plan` reads it. Do not invent a sidecar that can skew from the ids in style rows. Do not pretend `exact_out()` is that table.
6. **Registration failure.** Corrupt file, already-registered, path that escapes the asset root (images refuse `..` — `1011` §4). Specify fail-closed: that family is absent from the cascade, stderr, first frame still happens with the rest of the stack / last-resort. Do not hang boot.
7. **This RFC has no implementer and no date**, which is legal for an RFC and illegal for a spec (`RULES.md` §Scope, OQ5). Do not let it occupy a `llp/current/` slot until someone takes it. Fifteen-document working set: a 16th archives one.

Do not build remote fonts, `display`, `unloadFont`, Dynamic Type, subsetting, or a synthesis property. D7 (`font-style` in the same change) is right: shipping family without italic is a second pass over the same four hosts for a row that already exists.

**What would change my mind.** Charlie naming Caltrain as the only v1 app and deferring Weird Castle’s wordmark. Then fonts wait. A WOFF2-only brand file would force a decompressor; I would still rather convert at build than take a second format in the hosts.

## The one thing I would change

**Split what D2 and D3 collapsed, and say that identity is the declaration.** The plan gets `families` (compiler name → `faces` with paths) and `stacks` (interned CSS lists as a range of family-or-generic ids); `font_family` indexes a stack. Matching uses that declared name bound to those bytes — CSS `@font-face` family-name, Apple descriptor-from-URL, Linux loaded face ids — **never** the file’s name table and **never** `UIFont(name:)`. Without this, the first brand face whose name table does not equal the Contract string works on web and fails on Apple/Linux, and Apple’s “one `PlatformFont`” path recreates scar 4 even if every other decision here is right.

---

## Round 2 — cross-response, mutually visible (verbatim)

# Grok 4.6

## 1

Recommendation: nearest-face — confidence 75%

Moved from 70%. Sol’s own falsifier is already in the capsule, and it points the other way.

**“Web is the standard” is binding.** I am not dodging that. `CLAUDE.md:7–17` and `RULES.md:51–53` follow CSS even where a reset would usually override it, and they register unavoidable gaps in LLP 1001 (Taffy’s missing `position: static`). `font-synthesis: none` *is* a reset. The live question is whether synthesis is followable CSS or an unshared algorithm. It is the Taffy case. Web smears and shears in the browser. Apple already shears italic on the system helper (`exact2/host-apple-Text.swift:83–88`) and does not smear custom weight. Linux paints swash masks of real glyphs (`exact2/host-linux-text.rs`, `glyph` / `paint`) — no embolden, no shear. “Follow CSS” here means inventing two native faux-bolds and calling the trio a CSS initial. That is not how this repo follows CSS for layout or motion; those are held to the browser by fixtures. The 1001 door exists for algorithms the hosts do not share.

**“Preserve the requested semantic distinction” is not the rule.** The web is the parity oracle, not a semantic mood. If 700 must look heavier than 400, ship the 700 file. exact2 apps declare faces; a missing bold is an authoring hole, not a web-legacy condition. Sol’s later `font-synthesis="none"` hatch is a new property the first consumer would need on day one: the wordmark is `font-weight=800` (`exact2/weird-castle.app.contract:104`). Agents add none (`RULES.md` §Agents). Pinning `none` as the default is cheaper than a property whose only v1 use is turning the default off.

**Unspecified policy drifts — diagnosis yes, prescription no.** exact1’s doc said never (`exact1/fonts-and-assets.md:366–378`) and Apple sheared anyway (ENG-22086). That is an argument for a specified policy plus a check, not for synthesizing. Nearest-face as a 1001 deviation, `font-synthesis: none` on the web host, Apple’s italic-trait branch gated on a real italic face, and a fixture that Regular+italic still measures as Regular, is *more* specified than “synthesize, algorithms may differ.” Synthesize without a Linux algorithm is the same drift with the sign flipped: web smears, Linux nearest-faces, Apple shears italic only.

The cost asymmetry still stands and was not refuted. Sol said new Linux raster machinery versus the boot/scope budgets would move them. That machinery is not in the host. Synthesize is new work on three native paths. Nearest-face is one stylesheet line plus deleting a branch Apple already has.

## 2

B is right about permission. I was describing readiness.

`RULES.md` §Scope: `NOT-DOING.md` is binding; a spec needs an implementer and a date; an agent may not add a registry without a human saying so. `NOT-DOING.md:5–12`: v1 is one real app; the name is an open decision; the recommendation is Caltrain; everything that app does not need does not exist. `CLAUDE.md` listing `~/projects/weird-castle` as a consumer is not that naming.

Sol cites the wrong paragraph. `NOT-DOING.md:141–144` is how you move something *off this list* — off the listed nos (Windows, video, GPU, …). Fonts are not on the list. Charlie does not have to delete a no to add bundled faces. He has to close the open decision at lines 9–12: name the app, or write that Weird Castle’s wordmark is in v1 beside Caltrain. That one line, plus “an agent may add a font table,” is the whole obligation.

Until that line exists, implementing is what the rules exist to stop. After it exists, a thin v1 is the right build — not LLP 1019 as written.

Conceding a scope point I had wrong: D7 (italic in the same change) is not required by either app. Neither contract contains `font-style`. Sol is right that D7, not D6, is the unevidenced add-on.

## 3

Declared out of v1.

Sol is right that I overclaimed Linux. `Attrs` takes one `Family::SansSerif` (`exact2/host-linux-text.rs:289–297`). `fontdb::Query.families` picks one face. cosmic-text’s “fall-through” is the bug they already hit: per-glyph fallback scores every installed font by weight, and Caltrain’s button came out in URW Bookman with a space from Noto Color Emoji (`exact2/host-linux-text.rs:209–216`). That is the opposite of an ordered CSS cascade. D3’s “`fontdb` families vector” / “what the platform does anyway” (`1019-fonts.rfc.md:173–176`) is false on Linux.

I do not accept “unimplementable.” Apple has `kCTFontCascadeListAttribute` (`exact1/fonts-and-assets.md:297,599`). A private Linux `FontSystem` loaded with only the stack’s files would at least stop the URW Bookman leak. Neither is free, and neither app asks: both are Latin system-ui, plus one wordmark.

v1: split the tables so a family is not a stack. Web may emit a CSS list (the one host where D3 is free). Native uses the first usable family and the platform last-resort. Ordered per-glyph fallback is a declared host limitation on Apple and Linux. Do not land `kCTFontCascadeListAttribute` in v1. Scar 4 stays documented, the way exact1 left it (`exact1/0470-font-registry-limitations.md:48`).

## 4

Wrong, with files:

- `NOT-DOING.md:141–144` applied to fonts. Fonts are not on that list. The binding text is `NOT-DOING.md:5–12` and `RULES.md` §Scope / §Agents.
- “Ordered per-glyph cascade may be unimplementable” on Linux. The capsule shows the current fallback is uncontrolled (`exact2/host-linux-text.rs:209–216`), not that a restricted `FontSystem` cannot be built. The product answer is still: declare it out.
- “Native need not be pixel-identical” is not in `RULES.md` or `CLAUDE.md`. Those files name the web as the parity oracle and 1001 as the deviation register.

Correct catches I missed:

- D1’s variable-font shorthand cannot fit D2’s `weight: u16, italic: bool, path` face row (`1019-fonts.rfc.md:136–149`).
- Helvetica “declared like any family” (`1019-fonts.rfc.md:169–172`) has no file-less syntax in D1.
- Apple font/paragraph caches are process-global (`exact2/host-apple-Text.swift:63–69`). IDs are per-plan. A same-process plan restart (`NOT-DOING.md:48–51`; the ~20 ms web restart in `CLAUDE.md:30–32`) can reuse id 10 for different bytes. Cache keys need plan/catalog identity, not only family. I treated this as a missing family field; it is also scar 1 if the process lives.
- `font-family` would be a global attribute (`exact2/contract-lower-tags.rs:149–266`), and Weird Castle has `input`s (`exact2/weird-castle.app.contract:123–124`). Native fields are resolver number two unless they share the catalog or the attribute is refused on them.
- Kernel `font_weight` is integer 100–900 billed as CSS (`exact2/kernel-text.rs:21–22`). CSS Fonts 4 is 1–1000, and a variable `wght` axis is continuous. That is already a 1001-shaped decision.
- D7 is unevidenced by the corpus.

## 5

Name the v1 app. One sentence: Caltrain only, or Weird Castle’s wordmark is in.

If Caltrain only, stop; close the RFC. If the wordmark is in, ship the real 800 (or a variable face that includes it), nearest-face, no per-glyph cascade, no italic, no `font-synthesis` property.

That is the only question that does not require reading 1019, and it is the one the rules actually wait on.
