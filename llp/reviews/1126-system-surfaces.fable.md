# Review: LLP 1126 r1 (system surfaces)

**Family:** Claude Fable 5.1 (Anthropic; the same family as the author, an independent session that did not write the draft) · **Provider/runtime:** Claude Code sub-agent, model fable, effort xhigh · **Date:** 2026-10-17 · **Redacted:** no · **Method:** blind (no reviewer saw another's review), read-only checkout of origin/main 8bf68cf88 plus the r1 draft; the author's brief asked for high-level, directional and foundational findings only (prompt: the same text to each reviewer)
**Verdict:** READY WITH CHANGES

---

# Review of LLP 1126 — System surfaces (Draft r1)

Reviewed against `llp/1126-system-surfaces.rfc.md`, `rules/RULES.md`, `rules/DEFERRED.md` (§Surfaces, §Authoring models, §Components, §Process), `llp/1115-write-the-web-ship-the-platform.principle.md`, LLP 1031 (D1, D4, D6, D12), LLP 1035.006 (§11.10, group 12), LLP 1018 (D6, D7), LLP 1030 (D3a, §"Widgets, extensions, watch apps, helpers"), LLP 1047.001 / 1047.001.000, LLP 1101 (D1, D2, D9: the terminal entry), LLP 1113, LLP 1124, and `host/apple/{Package.swift, include/exact.h, build.mjs, src/store.rs}`. Read-only.

## Verdict

**READY WITH CHANGES** (direction only). The foundation is right: classify by execution model, one SwiftUI core, snapshot driver first, templates separate, time and taps left to the platform. It is the same move LLP 1115 made for presentation and LLP 1101 made for the terminal, applied to a place the app's content must go. Three things are wrong or undecided at the foundation and should be settled before stage 0 is spent: (1) D8 silently amends LLP 1030's delivery row for extensions and leaves the compatibility-cohort question as a probe when it is a decision; (2) stage 3 puts code on the watch (complications need a watchOS app and the Tier 3 toolchain), so it engages the DEFERRED watchOS line a stage earlier than §7 Q3 says; (3) D2's "entry" has a precedent in the repo (LLP 1101 D1's terminal entry, with the schema's per-row admission column) and the RFC should adopt that shape rather than a second one. The rest is probes, which are the right ones with two additions.

## Strengths

- **The classification is the right abstraction.** Snapshot / live / template / hosted / island is by what runs where, which is what decides memory, time, input and update paths. It also makes D13's cross-platform claim plausible, and the Wear OS case (complications are *data* there, a template; on Apple they are snapshots) is the proof that "class per platform may differ" is needed, not a hedge.
- **D3 is the load-bearing decision and it is correct.** A render that is a function of props + store snapshot + system environment is the only shape that fits WidgetKit's archive, an extension's budget, and LLP 1112/1113's "settled" capture. Putting work in a platform-budgeted refresh step is what a hand-built widget does.
- **D4 chooses the principled side of the layout fork and names both rejected alternatives honestly.** Kernel layout, SwiftUI placement keeps "layout stays CSS" (LLP 1115 §2) and avoids a second layout engine. The two hard cases (system-sized regions, live text width) are named and routed to Q1/Q2.
- **D6 and D7 are exactly "write the web, ship the platform."** Timer/relative text and `AppIntent`/`widgetURL` are what the platform has; an author who ticks a clock in a widget is told at bake time with an edit.
- **D9 (ContentState as a Contract shape, emitted to Swift, TS and JSON Schema) is a real win** that no port framework gives; it is the right place to put the 4 KB and the shape-digest problems.
- **Scope discipline.** The RFC asks for the DEFERRED ruling it needs (§7 Q2, Q3), keeps templates and Android as separate RFCs, and does not propose a SwiftUI presenter for iOS/macOS (D1, §6).

## Foundational concerns

### 1. D8 amends LLP 1030's delivery model for extensions and leaves a decision as a probe — HIGH

LLP 1030 §"What is not an artifact" says: *"Widgets, extensions, watch apps, helpers. Separate signed targets with binary in every column; rows in D1 when an app has one."* Today an extension is binary-only: it never takes a live bundle. D8 reverses that ("an extension runs whatever bundle the app last made current") without saying it amends 1030, and without the rule that makes it safe: LLP 1030 D3a makes a stream `(app, channel, compatibility id)` where the compatibility id is the digest of *what the binary links* (LLP 1047 D8, 1047.001). If the widget extension's binary links a smaller set than the app's (Q4's "extensions take a smaller one"), it is a second cohort, needs a second stream and a second bundle, and D8's "never fetches one" cannot hold.

**Consequence.** Either the widget drifts from the app after an update (the thing D2 exists to prevent), or the extension decodes a plan whose use-set its binary cannot serve (a refusal by name at the worst moment: in a provider under a memory limit).

**What resolves it.** Decide, do not probe: one compatibility cohort per app, so the app and every extension link the same set, in one framework (D8's first bullet already wants one copy). Pruning per surface is then a *plan* matter (data: the surface's entry reaches less, LLP 1047 D2's use-set per entry), never a *binary* matter. Q4 shrinks to "what the shared framework costs the app's launch" (LLP 1047.001.000's measurements are the method). Add to D8: the extension reads the store's current generation atomically (today the transactional swap, LLP 1023 D3 / 1031 D1, is in-process; across processes it needs a record the extension reads whole), and the RFC amends LLP 1030's extension row at acceptance (§7 of this RFC should carry a "what this amends" list, as LLP 1031 §7 does).

### 2. Stage 3 puts code on the watch; the watchOS ruling is engaged there, not at stage 4 — HIGH (sequencing)

WidgetKit complications and Smart Stack widgets on watchOS are provided by a widget extension *inside a watchOS app bundle*; an iOS app cannot ship them (I am confident of this). So stage 3's "watch complications and the Smart Stack, by the same driver" requires a watchOS app target (a stub is still an app), the Rust runtime built for `arm64_32`/`arm64` watchOS (D10's Tier 3, `-Zbuild-std`), and `ExactKit`'s non-UI slice compiling for watchOS: all of D10's first two cost lines arrive at stage 3. D10's "the watch's complications and Smart Stack are snapshot surfaces and arrive with D3–D8" and §7 Q3's "decide at stage 4" contradict each other and `rules/DEFERRED.md` §Surfaces ("watchOS, which has no UIKit, stays out").

By contrast, iPhone **Live Activities** reach the watch Smart Stack (watchOS 11) and the Mac menu bar with no watch or Mac code, and iPhone widgets reach the Mac, CarPlay and HomeView the same way. The RFC conflates "reaches the wrist" (true at stage 2 with no watch code) with "runs on the wrist" (needs the ruling).

**What resolves it.** Move watch complications to stage 4 (or a stage 3b conditional on Q3), keep stage 3 as Mac and visionOS widget extensions, and say in §1 and D10 which watch reach costs no watch code. Also note that watchOS does have UIKit's value types (`UIFont`, `UIColor`, `UIImage`), not `UIView`; the DEFERRED line's "no UIKit" was about the presenter, which is still true.

### 3. D2's "entry" should take the terminal entry's shape; D14's vocabulary check already exists — MEDIUM

The repo has one precedent for "a second root on a separate medium": LLP 1101 D1 (`app.json` `host.terminal.entry` names a root `.contract`, compiled under the `terminal` profile into its own plan with a header every other host refuses; shared `use`d components, data module, storage and routes), D2 (one vocabulary, one admission profile: `kernel/tables/schema.json` carries a per-row `"terminal"` admission, e.g. line 2399), and `rules/DEFERRED.md` §Authoring models ("A terminal entry is a separate application root on a separate medium, not a platform override"). LLP 1031 D1 deferred exactly this multi-plan shape ("N entry `.contract` files … listed in one envelope under a `plans` map") with the trigger *a named consumer whose second surface cannot be a route of the first*; a widget is that consumer.

The RFC instead proposes a component named in `app.json` inside the one plan ("One bake makes one plan"), with a new vocabulary check in the bake (D14) and pruning described as link-by-use "applied to entries". That is a second convention for the same idea, and "one plan" is in tension with "each surface's process gets only what it reaches".

**Consequence.** Two spellings of "entry" in `app.json`, two admission mechanisms (the schema's profile column and a bake check), and the pruned-plan question answered twice.

**What resolves it.** State that D2 pulls LLP 1031 D1's trigger and takes LLP 1101's shape: a surface is a root file (or a component the bake treats as a root) compiled under a `snapshot` profile into its own plan in the envelope's `plans` map; the snapshot driver's vocabulary is the schema's admission column for that profile (one table, `kernel/tables/schema.json`, as CLAUDE.md requires); D6's clock refusal and D7's handler refusal are that profile's rules. Then "the bake gives each surface's process only what that surface reaches" falls out of compiling the root, with no new mechanism.

### 4. Two processes write one store; LLP 1031 D4's snapshot isolation makes "one state" false in time — MEDIUM

D8 says the app and its extensions "read one state". LLP 1031 D4 (as built) gives a session the store at its boot and a peer's writes "only at its next boot or `data_ready`"; there is no cross-session notification, and `rules/DEFERRED.md` refuses a reactive shared store. D7's intent commits in the extension; a backgrounded app returning to the foreground shows the old value. A hand-built app observes the change; that is a tell at the whole-app level, which is §1's own argument.

**What resolves it.** A direction, not a store redesign: a generation counter in the group container that each writer bumps; the app re-asks its store-reading resources at foreground when the generation moved (the `data_ready` re-ask that exists), and the app reloads a surface's timelines after a commit that touches that surface's inputs (`WidgetCenter.reloadTimelines`). This keeps "the answer stays a pull" (LLP 1016.002 D1) and admits no shared heap. Name it in D8 so the first consumer does not discover it.

### 5. The archive is rendered per environment variant and per timeline entry; the kernel's layout is per variant too, and D4 is an Apple decision — MEDIUM

D3 lists text size among the environment the system gives, and D4 lays out "at the size the system proposes". In WidgetKit the extension renders an entry's view for each variant the system names (`TimelineProviderContext.environmentVariants`: colour schemes, rendering modes; whether Dynamic Type is a variant or a reload I am not certain). So the snapshot driver runs the kernel once per (entry × variant), inside Q3's budget; a 20-entry departure timeline is not 1 layout. The RFC does not count this. Q2's text-parity risk is also sharper than stated: SwiftUI `Text` wrapping inside a kernel-sized frame is where a one-line difference becomes a clipped line. A fallback that keeps D4 intact is to emit the kernel's *lines* (`ParagraphGeometry`/`TextBreaks` already have them) as single-line `Text`s, so SwiftUI draws and never breaks.

Separately, D13 implies D4 ports. Android's snapshot surfaces (Glance → RemoteViews; Wear tiles → ProtoLayout) have no absolute-placement primitive and the launcher re-lays out at its own sizes; there the choice is the translation D4 rejects or a bitmap. D4 should say it decides Apple, and that Android's RFC decides between translation, picture and a restricted vocabulary; D13's "an entry is portable" then means the *class* and the *data* are portable, and the vocabulary is the intersection or per platform.

**What resolves it.** Add the variant count to Q3; add a Dynamic Type change and a 20-entry timeline to Q1/Q3; name the per-line text fallback in Q2; scope D4 to Apple.

### 6. D6 adds vocabulary; under `rules/DEFERRED.md` §Components it must be HTML's — MEDIUM

"Exact admits that vocabulary as elements whose value is a date or an interval" reads as new tags. DEFERRED §Components (Charlie, 2026-09-27): "A built-in tag is an HTML element with HTML's meaning … Nothing that isn't HTML is added by it." HTML has `time` (`datetime`) and `progress`; a named format on `time` (a relative time, a timer, a countdown) and an interval on `progress` are the same elements with a presentation the platform formats, which LLP 1115 §3 already allows ("a host may resolve an `auto` value to the platform's behaviour"). Say so; otherwise D6 trips the tag budget on first reading.

### 7. Smaller foundational notes — LOW

- D1's "building snapshot first wastes nothing" overclaims: a flat tree of primitives at fixed frames is not a retained, diffed tree with `ScrollView`, `NavigationStack`, fields and crown input. The snapshot driver is justified on its own; do not lean on D10 for it.
- §2's census is the wrong one: UIKit *is* importable in a widget extension (only UIKit views are refused in the archive). The census that matters is extension-unsafe API (`UIApplication.shared`, `UIScreen.main`: 32 files in `ExactKit` use one of these or `NSApp`) and the `APPLICATION_EXTENSION_API_ONLY` build, plus what compiles for watchOS. The text engine (`Text.swift`: CoreText + Foundation only) is the good news.
- The refresh step for a TypeScript app runs Hermes in the extension; neither D8 nor Q3 names it, and `rules/RULES.md`'s boot rule ("no app JS before first pixel") needs a stated reading for an extension (the refresh may run the data module as bytecode; the render runs none). Caltrain, being Rust-only, will not surface this; Q3 should include a TypeScript app.
- The bake will generate a Swift target per extension (the `Widget`/`WidgetBundle` types, the `ActivityAttributes`, the generic intent, the Info.plist); D9 and D7 each mention a piece. Say it once in D2 so the build's new generated-code step is a known cost.
- D14 adds apparatus (a dev-build gallery, an async-lane simulator test, agent addressing of surfaces). `rules/RULES.md` §Agents needs a human's yes for each; list them in §7 for the ruling.

## Missing surfaces or misclassifications

None changes the design. For completeness against §11.10, which the RFC reopens:

- **macOS menu bar extras and Dock** (named in §11.10): a status item with an `NSMenu` is a template; a status item's popover is a window of the running app (the AppKit presenter's). Dock badge/menu: template. Add a row each or say §11.10's Mac items stay with LLP 1115.002.
- **Quick Look preview extensions** (named in §11.10): hosted (a view controller).
- **visionOS ornaments**: on a UIKit-presented window they are `UIHostingOrnament` attached in-process, which is the island/hosted shape, not a live-driver surface; only volumes and immersive spaces are live.
- **Android**: Wear OS complications (`ComplicationDataSourceService`) are templates (data, not views), and Quick Settings tiles are templates; both support D13's "class per platform may differ" and are worth the one line.
- **Snippets**: keep "to verify"; the pre-iOS 26 result-snippet form (`IntentResult` with a view) is the same archive mechanism as far as I know.

## Alternatives considered

1. **Author-written SwiftUI widgets over Exact's data seam** (what `expo-widgets` does; the app ships a Swift widget that reads the group container and the bake gives it the shapes and the store). Strongest cheap alternative: highest fidelity, no renderer, ships in weeks. It loses on the two things this repo exists for: the author (an agent) writes a second language and a second layout model, and the agent cannot `tree`/`screenshot`/`tap` it. It does not port to Android. It should still be stage 0's fidelity baseline: build Caltrain's small widget by hand once, so "would someone who knows the platform notice?" has its reference.
2. **Translate Contract layout to SwiftUI stacks.** Loses, as D4 says, on parity; but the probes should check that kernel placement survives variants and live text, because that is where SwiftUI's own layout would have given Dynamic Type and timer reflow for free. If Q1 shows a custom `Layout` survives the archive, the kernel-as-`Layout` form is the best of both and the RFC should prefer it over fixed frames.
3. **Render-only extension; the app does the refresh** (background tasks write props to the container; the extension links no executor, no transports, no Hermes). Smaller extension, simpler budget; worse freshness (BGAppRefresh is opportunistic and a force-quit app never refreshes). The right answer is probably D3 as written with the extension's *running* set measured separately for render-only and refresh (Q3), under the one-framework rule of concern 1.
4. **Picture by default.** Loses for the reasons D4 gives; on Android it may be the only honest option for some vocabulary, which is why D4 should be scoped to Apple.

## Recommended changes to the RFC (directional only)

1. Add a "what this amends" list: LLP 1030's extension row (extensions take the app's bundle; one cohort per app), LLP 1031 D1 (the multi-plan trigger is pulled), LLP 1035.006 §11.10, `rules/DEFERRED.md` §Surfaces. Turn Q4's cohort half into a decision in D8.
2. Re-stage: stage 3 is Mac and visionOS; watch complications join stage 4 or a conditional stage; state what reaches the watch and the Mac with no code there.
3. Adopt LLP 1101's entry shape and the schema's admission profile for D2 and D14; drop the separate bake vocabulary check.
4. Add cross-process store invalidation to D8 (generation counter; re-ask on foreground; `reloadTimelines` after a relevant commit).
5. Scope D4 to Apple; add the per-variant, per-entry layout count to Q3; add Dynamic Type and a long timeline to Q1/Q3; name the per-line text fallback in Q2; add a TypeScript app to Q3.
6. State D6's vocabulary as HTML's `time` and `progress` with a platform-formatted presentation.
7. Trim D10's and D13's forward text to what stage 0–2 needs (`rules/DEFERRED.md` §Process: no speculative specs); keep the cost lines, drop visionOS volumes and the Android table to a sentence each. Add the apparatus list to §7.

## Open questions for the author

1. Which is the archive's mechanism as you understand it: closures evaluated in the extension per variant and flattened (then `Layout` and `GeometryReader` should both work and Q1 is cheap), or a view description re-evaluated by SpringBoard (then fixed frames are the only option and timer text is harder)? The probe's first finding should be this, since it decides Q1, Q2 and the variant cost together.
2. Is the extension's compatibility cohort the app's (one framework, one stream), or do you want a second stream per extension? If the former, say so in D8 and delete the "smaller one" half of Q4.
3. For a TypeScript app, does the refresh step run Hermes in the extension, or does the app precompute props (alternative 3)? Caltrain will not answer this; name the TypeScript probe app.
4. Does the placeholder/redacted path (`placeholder(in:)`, which must answer without a store and quickly) render through the same driver with empty props, or does the bake produce it ahead of time?
5. Is the first consumer's widget to be judged against a hand-built one (alternative 1 as the baseline)? If yes, who builds it in stage 0.