# Review: LLP 1053 r1, 2026-09-26 (grok)

- **Family:** xAI — `grok` CLI, `--reasoning-effort xhigh --permission-mode plan --no-subagents`; requested the default grok-4.7 but served as `grok-4.6-build` per its usage record; Charlie accepted 4.6 as a sanity check. 2026-09-26T01:26:49Z to 2026-09-26T01:37:52Z.
- **Method:** the shared brief (sha256 `23ee50a38017d02202b35d77b93a01bacb2cb716254d507a7864bf33e283ed87`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1053 r1.
- **Redacted:** nothing.
- **Disposition:** LLP 1053 §0 (r2).

---

## Review (verbatim)

# Audit of LLP 1053 (Draft)

Verified against this checkout at `e992cefe` (matches the RFC’s “origin/main” pin). Binding rules: the web is the standard; `rules/NOT-DOING.md` is the doing-list trade. I did not build, and I did not look outside this tree.

The RFC’s split (Contract names for rows the kernel already has / host text work / runtime shapes that need measurement or design) is the right frame. Several “what exists today” host claims are wrong, two CSS stories are wrong enough to change the work, and G8/G9/G11 point at the wrong seam.

The already-fixed commits exist (`a96e8094`, `b58430d2`, `80223cee`, `0e047888`, `43708204`). The in-flight iOS text bugs are out of scope here.

---

## 1. Per gap

### G1 `aspect-ratio` — **agree to add**, not as “trivial / no risk”

**Recommendation:** add. **Judgement:** add, with a real value grammar and replaced-element parity, not a one-line `tags.rs` entry.

**“What exists today”:** mostly right, citation slightly off.

| Claim | Code |
|---|---|
| Kernel row exists | `kernel/tables/schema.json:1148-1154` — `aspect_ratio`, `f32`, layout, default `0` |
| Contract does not name it | `contract/lower/src/tags.rs:198-492` has no `"aspect-ratio"` |
| Taffy honours it | Assigned in `kernel/src/style.rs:979-983` (`> 0` and finite → `Some`). `layout.rs:731` is only the measure callback using that Taffy field to decide whether height is known, not the feed |
| Layout only | Correct. Web already emits the row as CSS (`host/web/src/css.rs:286-293`) |

**CSS the RFC understates.** CSS `aspect-ratio` is `auto \|\| <ratio>`, and `<ratio>` is `<number [0,∞]> [ / <number [0,∞]> ]`, including `16 / 9` and `16/9`. The kernel stores one `f32`. `StyleValue::f32` (`kernel/src/style.rs:346-367`) accepts only a number, so `"16/9"` and `"auto"` would fail today. `0` is the stand-in for none (`style.rs:979`); CSS `0` is invalid and must not round-trip as auto. `auto && <ratio>` (preferred ratio plus auto) is not in Taffy; leaving it out is fine if declared.

A set row already wins over an image’s intrinsic ratio (`kernel/src/style.rs:1073-1078`, LLP 1011). Block-flow Taffy still stretches an auto-width replaced leaf (LLP 1001 §1). That is not “risk: none.”

**Parity:** non-replaced `width` + `aspect-ratio: 16 / 9`; image with intrinsic size vs an authored row; min/max through the ratio; web `aspect-ratio:16/9` (not a rounded float) vs Apple/Linux frames.

---

### G2 per-side `border-*-color` — **agree to add**; hosts are further along than the RFC says

**Recommendation:** add. **Judgement:** add now. This is the one “kernel row, Contract shorthand only” item that is actually cheap.

| Claim | Code |
|---|---|
| Per-side kernel rows | `schema.json:1293-1316`, codec `current-color`, default `"currentcolor"` |
| Contract only the shorthand | `tags.rs:448-453` (`border-color` → four `StyleId`s). No `border-top-color` etc. Width and style already have longhands (`tags.rs:428-447`) |
| iOS can paint differing sides | True: `BoxLayerIOS.swift:48-61` falls back to `draw(_:)` when sides differ; `NodeViewIOS.swift:1288-1298` fills per-side rects |
| “web and Linux to verify” | **Understated.** Linux already paints per-side (`host/linux/src/paint.rs:146-152` via `border_colors`). macOS does too (`NodeViewMac.swift:1265-1288`). Web already maps `border_color_*` → `border-*-color` (`css.rs:234-244`) and emits `currentcolor` (`host/web/tests/it/host.rs:310`) |

Apple resolves `currentcolor` against computed `text_color` before the presenter (`host/apple/src/style.rs:180-193`). Kernel `border_colors` (`kernel/src/style.rs:903-911`) is `None` → current colour.

**CSS the RFC misses.** Initial value is `currentcolor` (already stored). `border-color` as 1–4 values (`red blue`) is still missing; today’s shorthand writes one value to all four sides. Adding longhands must not flatten `currentcolor` to a baked RGB at compile time.

**Parity:** four different colours; three sides `currentcolor` and one hex; `light-dark()` on one side; `color` change retints unresolved sides. Web, Apple (rounded uniform vs square per-side), Linux.

QUEUE already names this (`QUEUE.md:33`).

---

### G3 `flex-grow` longhand — **agree to add the name**; **disagree** that the easy port’s `flex=1` was just a missing longhand

**Recommendation:** add. **Judgement:** add the longhand, with a parity case that `flex-grow: 1` ≠ `flex: 1`.

| Claim | Code |
|---|---|
| Kernel `flex_grow` | `schema.json:1074-1079`, default `0` |
| `flex-basis` named; `flex-grow` only via `flex` | `tags.rs:460-462`: `"flex"`, `"flex-shrink"`, `"flex-basis"` — no `"flex-grow"` |
| Shorthand | `contract/lower/src/lib.rs:1242-1258`: `flex: <n>` → grow `n`, shrink `1`, basis `"0%"`. That **is** CSS for a single number |

**CSS the RFC gets wrong.** `flex-grow: 1` leaves `flex-shrink: 1` and **`flex-basis: auto`**. `flex: 1` is `1 1 0%`. The easy port used `flex=1` because that is the fill idiom, not because the longhand was forgotten. Completing the longhand set is still right (web-is-the-standard). It will not replace `flex=1` in list rows.

Also still missing, and not in this RFC: `flex: none` / `auto` / two- and three-value forms. `flex` plus `flex-grow` on one node are different attributes, so both bind; cascade order is unspecified in lowering (`lib.rs:1165+`). Need a rule: last attribute wins per row, and the shorthand resets grow+shrink+basis as CSS does.

Web already emits `flex-grow` (`css.rs:286-287`). Taffy already reads it (`style.rs:1014`).

**Parity:** `flex: 1` vs `flex-grow: 1` in a row with leftover space (heights/widths must differ); `flex-grow` with explicit `flex-basis`.

---

### G4 `font-variant-numeric` — **disagree with “add now / cost small / hosts partially read it”**

**Recommendation:** add `tabular-nums` and `normal`. **Judgement:** do **not** ship a Contract name until a host actually applies OpenType features. A name with no effect is a lying API.

**“What exists today” is wrong.**

| RFC | Code |
|---|---|
| Kernel row `u8` | True: `schema.json:1448-1456`, inherited, `text`+`layout`, default `0`. Carried on `TextStyle` (`kernel/src/text.rs:103-117`) |
| Contract unnamed | True |
| “web passes CSS through” | **False.** `host/web/src/css.rs:121-124` **skips** `FontVariantNumeric` (`"not lowered in v1"`). `glue.js:957-961` only maps the row for **agent inspection of the browser’s computed style**, which stays the initial value because nothing is emitted |
| “Apple maps it to monospaced-digits” | **False.** Apple `CRun` (`host/apple/src/measure.rs:27-48`) has no field; Swift `Run` (`Text.swift:29-38`) has none. `monospacedDigitSystemFont` in `ExactIOS/main.swift:135` is overlay UI, not the engine |
| “Linux sets `tnum`” | **False.** `Run::from_style` (`host/linux/src/text.rs:91-102`) drops `font_variant_numeric`. No `tnum` in the Linux text crate |

LLP 1019 §7 already said this row is unreachable from Contract and should get an attribute **or be declared unreachable on purpose** (`llp/1019-fonts.rfc.md:507-510`). The sample `1.0` in `kernel/src/arena.rs:764` is not a bit lexicon.

**CSS.** `font-variant-numeric` is a **space-separated list** of exclusive pairs (`lining-nums`/`oldstyle-nums`, `proportional-nums`/`tabular-nums`, fractions, `ordinal`, `slashed-zero`), not a single enum. A `u8` bitmask is a reasonable wire form **if bits are specified**. `tabular-nums` changes **advance widths**, so it is a measure invalidation (the schema flags are right). Cost is moderate: CoreText features, cosmic-text/swash features, and stop skipping on web.

**Parity:** `"111"` vs `"111"` tabular vs proportional at a known face; `normal` restores; inherited into a nested run. Do not add `oldstyle-nums` until a face in the corpus actually has `onum`.

---

### G5 `white-space: nowrap` (and `pre-line`) — **agree to add `nowrap` now**; **split `pre-line` out**

**Recommendation:** add both. **Judgement:** add **`nowrap` now**. Defer `pre-line` until the walker has a third mode.

| Claim | Code |
|---|---|
| Enum is `normal`, `pre-wrap` | `schema.json:819-825`. Property exists (`tags.rs:368`) |
| Every engine needs a no-wrap mode | True, and **more than an enum**: |
| Apple ordinary layout | `Text.swift:78` stores `whiteSpace`; `layout` (`Text.swift:598-606`) always `CTTypesetterSuggestLineBreak`. `whiteSpace` is only used in flow collapse (`TextFlow.swift:74`) |
| Linux | `shaping.rs:230-235` chooses `Wrap::Word` / `WordOrGlyph` from **`overflow_wrap`**, not `white_space` |
| Shared walker | `textflow/src/walker.rs:42-48, 155` — `Normal` vs `PreWrap` only. Wasm protocol (`textflow/src/web.rs:210-213`) rejects any other discriminant |
| Web ordinary text | Emitting `white-space:nowrap` is enough **for non-flowed DOM text**. Flowed text still goes through the walker (`host/web/textflow-glue.js:130`) |

**CSS the RFC understates.** Today’s shorthand maps onto two longhands:

| `white-space` | `white-space-collapse` | `text-wrap-mode` |
|---|---|---|
| `normal` | `collapse` | `wrap` |
| `nowrap` | `collapse` | `nowrap` |
| `pre-line` | `preserve-breaks` | `wrap` |
| `pre-wrap` (have) | `preserve` | `wrap` |

`nowrap` **collapses** newlines to spaces; it is not “one line of raw source.” `pre-line` is not “cheap once the enum grows”: the walker is a boolean preserve flag today.

The CSS label idiom is `nowrap` + `overflow: hidden` + `text-overflow: ellipsis`. Kernel `text_overflow` exists (`schema.json:826-831`, `text.rs:157-158`) but Apple ellipsis is **`line-clamp` only** (`Text.swift:623-625, 726-741`). Web can do the CSS idiom once `nowrap` is emitted. Native cannot, without an ellipsis path that is not clamp.

`line-clamp=1` as a workaround is worse than the RFC says: web **skips** legacy clamp on flex/grid (`css.rs:99-118`), and list rows are flex (`tags.rs:63-72`). QUEUE already names `white-space: nowrap` (`QUEUE.md:33`).

Min-content today is “longest word” (Apple `Text.swift:744-787`; Linux `text.rs:647-650` at width `0`). For `nowrap`, CSS min-content **equals** max-content **equals** the collapsed unwrapped line. That is a behaviour change in all three engines, not a flag.

**Parity (must be browser-oracle):** collapsed newlines; min- and max-content equal the unwrapped width; `nowrap` + `overflow: hidden` + `text-overflow: ellipsis` on a **flex row**; a long unbreakable word; flowed vs unflowed text.

---

### G6 `text-transform` — **disagree with “add now” as specified**

**Recommendation:** add uppercase/lowercase/capitalize/none, kernel-side, Unicode full mapping, UAX #29 capitalize. **Judgement:** **not yet.** Direction is CSS-correct; the proposed implementation fights the web host.

There is no row, enum, or host path (search is empty).

**CSS the RFC gets wrong or leaves open:**

- It **is** inherited. A new row needs `inherited` + `text` + `layout` in `schema.json`.
- **Web-is-the-standard:** the web host should emit CSS `text-transform` and let the browser transform. Kernel-mutating run strings (`RFC §G6`) would **double-transform** if CSS is also emitted, and would make DOM copy the transformed text always. Native should transform for measure/paint; web should not.
- **`capitalize`** is CSS “first typographic letter unit of each word, if lowercase, in titlecase; **other characters unaffected**.” It is not Unicode titlecase and not “UAX #29 then titlecase the word.” `McDonald` / punctuation / `ij` in Dutch stay messy; say so.
- **`ß` → `SS`** is correct for locale-insensitive full uppercase. Turkish `i`/`İ` waits.
- Copy is unspecified in CSS. Chrome copies transformed text, Safari the source. Picking one is a product note, not “follow Chrome’s rendering and the engine’s copy path” as if those were one rule.
- Transform **changes measures**. It must run before shaping on native. Search/find on source vs display is a third policy.

**Parity:** `ß`, `i`/`I`, nested spans with `none`, capitalize leaving inner capitals, copy on web vs Apple (document the difference if you will not match). Cost is moderate **plus** a copy decision. Do not bake a DataSource workaround into the kernel string.

---

### G7 expression-valued `font-family` — **agree, narrowly**

**Recommendation:** expressions over declared names and generics. **Judgement:** allow **compile-time-known names only** (literals and ternary/`match` arms of literals). Do not accept runtime strings.

| Claim | Code |
|---|---|
| Literal only | `lib.rs:1261-1265`, `fonts.rs:181-186` |
| Generics exist | `fonts.rs:13-22` (`system-ui` … `ui-monospace` … `ui-rounded`) |
| Single-member stacks | `fonts.rs:45-51, 188-193` — comma lists refused |
| Web generic stacks | `css.rs:164-190` |

The `when` workaround duplicates **nodes**, not just a row. Other styles already walk ternary arms (`values.rs:68-71`); `FontFamily` returns before that and demands `Expr::Str`.

A runtime string from a DataSource cannot be checked at compile time. “Refuse unknown names when they are literal” leaves a hole. Resolve each known arm to a stack id (`u16`) at compile time; keep the kernel row a number.

**Parity:** `on ? "ui-monospace" : "system-ui"` vs two `when` trees; undeclared name still refused; expression of a data string refused.

---

### G8 keyed partial answers — **agree “measure first”**; **disagree that a patch form is the next design**

**Recommendation:** measure a 10k replace-one-field tick; if slow, add a keyed patch answer. **Judgement:** measure to **document** cost. Do **not** add a DataSource patch protocol. That design is already rejected.

**“What exists today” mixes two seams.**

1. **Instance `each` reuse** — `runner/src/instance.rs:909-944`: skip rekey when `compare::same` says the subject list is the **same allocation**; otherwise key every item. `compare::same` (`runner/src/compare.rs:21-30`) is **pointer identity** for lists/records.
2. **Producer record sharing** — LLP 1041 / `apps/messages-stress/data/tests/it/reuse.rs:70-80`: a tick may allocate a **new list** of handles while keeping `Rc` identity on 9,968 of 10,000 records.

A new 10k `Vec` of new records every 250 ms will rekey everything, even though the runner “shares unchanged records.” Sharing is the **producer’s** job (`Rc`), not a new wire form.

LLP 1027.004 already measured this class of work and chose **windowed resources** (cursor + K rows), explicitly **no delta protocol** (`llp/1027.004-bounded-resource-answers.plan.md:23-29, 47-54`: ~0.3 ms for a 100-row page vs ~78 ms for a whole-history answer at 100k). `rules/NOT-DOING.md` (2026-09-18) parks further shared-representation / structural reconciliation behind that consumer.

A 10k **mounted** list is also an LLP 1010 virtualization problem. Patching the answer does not bound views.

The “~1 ms on the phone” gate is arbitrary and weaker than numbers already on file.

**Better direction:** (1) share record `Rc`s in the benchmark DataSource, as Messages stress does; (2) window or virtualize so N is not the tick cost; (3) optional microbench of “new list, shared records” vs “new records” for the report. No new answer grammar.

---

### G9 launch-time values — **disagree with a not-bakeable resource flag**

**Recommendation:** small design; prefer a not-bakeable resource over a `mount` action. **Judgement:** a **`mount` action is the wrong model**, and a **generic not-bakeable flag is also wrong**. Use a **runner-owned host fact** (the `exactViewport` pattern) or the **store**.

What already exists:

- Bake uses an empty store; device-dependent reads are a first-frame answer, not a compile of the developer’s machine (`runner/src/store.rs:30-31`, LLP 1018 D4).
- Runner-owned sources: `exactDelivery`, `exactViewport`, `exactSurface` (`plan/src/lib.rs:564-566`). `exactViewport` has a bake placeholder 390×844 (`runner/src/viewport.rs:6-27`) and is replaced at boot.
- First-run marker **is** `secret.keep` + empty bake store.

A not-bakeable flag is a hole in “the first frame is compiled data.” A `mount` action is an effect hook; this is **data**. Locale / env / “live mode” should be named fields on a reserved source, with a bake placeholder, like viewport. If the heavy port’s “Live: on” is a **build** flavour, it is bake-time, not mount-time. The 250 ms timer is a missing boot fact, not a missing lifecycle.

---

### G10 bake inputs outside the app directory — **agree: keep the refusal**

Refusals are real and broader than “bake”:

- `use … from` must stay under the app root (`contract/cli/src/sources.rs:122-130`)
- Font sources: relative, under `assets/`, no `..` (`contract/lower/src/fonts.rs:110-141`)
- Portable asset paths (`plan/src/lib.rs:542-552`)
- Symlink assets that escape (`bake/src/receipt.rs` around the “escape” fixture)

Copy or link the JSON into the app. Do not weaken hermeticity for a benchmark.

---

### G11 native switch — **disagree with “add as `<input type=checkbox switch>`” from this RFC**

**Recommendation:** add, lower priority. **Judgement:** **do not build from 1053.** If it is built later, expose the **existing `Toggle` node type**, do not overload `input`.

| Claim | Code |
|---|---|
| Contract has no switch | True: `tags.rs:131-136` — `input` is `NodeType::TextInput`. No `toggle` tag. No `checked` → `toggleValue` |
| Kernel has no switch | **False.** `schema.json:38-39` `Toggle`; `schema.json:88-90` `toggleValue` |
| Web | `Toggle` → `<input type="checkbox">` (`host/web/src/element.rs:118, 303-304`; LLP 1007) |
| Apple | Kind string `"toggle"` (`host/apple/src/host.rs:1208`); no `UISwitch`/`NSSwitch` in ExactKit |
| Linux | Toggle is a **box** (`llp/1015-linux-host-v1.spec.md:125-127`) |

HTML `switch` on checkbox is Safari 17.4+; other browsers show a checkbox. That is a **web** compromise. Mapping it onto `TextInput` is a type lie (measure, keyboard, `value` vs boolean). LLP 1035.006 already catalogs switches as native settings-slice work (`llp/1035.006-public-ui-coverage.plan.md:192`), not started. `rules/NOT-DOING.md:221` keeps ~15 tags; this is a new control (intrinsic size, a11y, events) on every host. Cosmetic for the easy benchmark. **Drop from 1053.**

---

## 2. Missing from the RFC / CSS the RFC gets wrong

**Highest-severity omission: CSS `direction`.** The kernel has inherited `direction` (`schema.json:1221-1228`). Contract **`renamed` maps `"direction"` to `flex-direction`** (`tags.rs:532`) and never binds `StyleId::Direction`. Authors cannot write CSS `direction: rtl`. That collides with the in-flight mixed-script / Arabic clip bug. This is a Contract bug, not a new feature.

Other kernel rows with no Contract name (same class as G1–G4), not in the table:

- `align-content` (kernel layout uses it; `kernel/tests/it/browser_cases.rs` has cases; `tags.rs` has `align-items` / `align-self` only)
- Grid: `grid-template-*`, `grid-column`/`row`, `grid-auto-flow`, `justify-items` — kernel + Taffy; web **skips** (`css.rs:125-133`); Contract unnamed (collection.rs only **forbids** the names on virtualized lists)
- `box-shadow` via `shadow_*` (`schema.json:1318-1343`); web composes `box-shadow`; Contract unnamed
- `backdrop-blur` (`schema.json:1351-1356`, web `backdrop-filter`); Contract unnamed

**Shorthands vs longhands (same class as G3):**

- `flex` only accepts `<n>` (`lib.rs:1242-1258`). Missing `none` / `auto` / two- and three-value. Interplay with G3 must be specified.
- `border-color` is one value to four sides, not CSS 1–4 values. `currentcolor` is already the initial (`schema.json:1296-1297`).

**`white-space` (G5):** the 2024 split is `white-space-collapse` + `text-wrap-mode` (+ `white-space-trim`). Adding shorthand keywords without naming that mapping will fight the next longhand. `nowrap` collapses; `pre-line` is preserve-breaks, not “nowrap with newlines.”

**`aspect-ratio` (G1):** grammar above; storage is a single `f32`; `auto && ratio` absent; replaced-element win already specified.

**`text-transform` (G6):** full case mapping vs simple; capitalize does not titlecase the rest; copy is engine-defined; web must emit CSS, not mutate strings.

**`font-variant-numeric` (G4):** list of features, not one keyword; no bit assignment; hosts do not apply it.

**`overflow: auto`:** enum is `visible | hidden | scroll` (`schema.json:744-750`). Collection code talks about `auto` (`collection.rs:136-137`) but the kernel cannot store it.

**`text-align: start | end`:** declared deviation is `left` not `start` (LLP 1001 §1). Related to the `direction` hole.

**Toggle vs `input`:** G11 ignores an existing node type.

---

## 3. Risks and costs the RFC understates, per host

### Web

- Layout/text that is **CSS** is cheap: G1, G2, G3, G5 `nowrap`, G6 if emitted as CSS, G7 as stack ids. The oracle is the browser.
- `line-clamp` on flex rows is already skipped (`css.rs:99-118`). G5 ellipsis must be the CSS idiom, not clamp.
- G4 is **not** free: it is explicitly not lowered.
- G6 kernel-side transform **double-applies** if CSS is also emitted.
- G11 `switch` is Safari-only; Chrome/Firefox stay checkboxes — acceptable only if the control is a checkbox semantically.
- Flowed text (`textflow-glue.js`, wasm white-space `0|1` only) does **not** get nowrap “for free.”

**Parity:** DOM `getComputedStyle` + box geometry via `agent.mjs` `layout`; do not grep CSS strings.

### Apple

- G2: already paints; verify rounded-uniform (one `borderColor`) vs square per-side; iOS layer path vs `draw(_:)`.
- G4: C ABI + `Run` + CoreText features; measure digits.
- G5: ordinary `layout` must stop wrapping; `minContentWidth` must change; ellipsis without `line-clamp`; TextKit/CoreText vs CSS overflow wrapping (`Text.swift:585-587` already special-cases CSS `overflow-wrap: normal`).
- G6: transform before `CTTypesetter`; copy/paste policy vs web; Arabic/line-height bugs in flight will interact.
- G11: real `UISwitch`/`NSSwitch` intrinsic size, one `valueChanged`, VoiceOver. Not “one input kind.”
- Mixed-script line-height disagreement (RFC §2) will show up in G5/G6 tests.

**Parity:** XCTest metrics + a driven iOS/macOS screenshot of nowrap ellipsis and per-side colour; same fixtures as web.

### Linux

- G2: already paints via `border_colors`.
- G4: cosmic-text/swash features; nothing today.
- G5: `Wrap::None` (or infinite width) for nowrap; intrinsic min/max; walker third mode for `pre-line`.
- G11: no native switch; a composed checkbox is a different product.
- Headless raster is the oracle stand-in; pin against web, not against GTK.

**Parity:** existing Linux text tests (`host/linux/src/text/flow_tests.rs` already covers `white-space` width/breaks) plus nowrap min=max and ellipsis.

### Cross-cutting

- Enum growth in `WhiteSpace` is a **wasm protocol** change (`textflow/src/web.rs:210-213` fails closed on unknown). Apple `u32` and C `u8` discriminants must stay aligned (`measure.rs:78-79`).
- `font_variant_numeric: u8` without a bit lexicon will fork hosts.
- File cap 1,500 lines: `tags.rs` is already large; G1–G4 names still fit.

---

## 4. G8 and G9: architecture

**G8 — proposed patch form is the wrong seam.**

The runner already:

- reuses `each` rows by key when the **list allocation** is unchanged;
- treats records as shared when the **DataSource** keeps `Rc` identity.

What the heavy port hits is almost certainly “new list, new records, every tick,” plus a fully realized 10k tree. LLP 1027.004 already chose **bounded answers** (window in the resource arguments) and refused a delta protocol. `NOT-DOING` parks generic reconciliation.

Better, in order:

1. Producer shares unchanged record `Rc`s (Messages stress pattern).
2. Window or virtualize so the runner never keys 10k rows per tick.
3. Measure the three cases (new records / shared records new list / windowed) and write the numbers into 1053. Do not invent `insert/update/remove` at the data seam.

**G9 — not-bakeable resource is the wrong hole; `mount` is worse.**

Launch-time **data** belongs with `exactViewport`: reserved source, bake placeholder, host fills before first settlement. First-run belongs in the store. Build flavour belongs in the bake. Do not add lifecycle hooks (`mount`) and do not add a flag that means “this resource is allowed to be blank at bake” for arbitrary sources. That undoes LLP 1018 D4 / 1004 D4.

---

## 5. Priority I would use, and what to drop

Higher than the RFC’s “trivial names first,” because G4 is not trivial and G5 is the list-app hole QUEUE already named.

1. **G5 `nowrap` only** (enum + three engines + ellipsis without clamp on native + web CSS). `pre-line` later.
2. **G2** per-side border colours (Contract longhands; host paint already there).
3. **G1** `aspect-ratio` with number and `a / b`; `auto` = unset.
4. **G3** `flex-grow` longhand + tests that it is not `flex: 1`.
5. **Fix `direction`** (stop renaming it to `flex-direction`; bind `StyleId::Direction`). Not in the RFC; cheaper and more binding than G4.
6. **G7** ternary/`match` of known family names only.
7. **G4** only with web emit + Apple feature + Linux `tnum`, not as a name-only commit.
8. **G6** after a one-page copy/web-vs-native note; web emits CSS.
9. **G8** measure + document sharing; no code.
10. **G9** only if a named host fact is required (`exactLocale` / env fields), not a generic flag.
11. **G10** leave.
12. **G11** drop from this RFC (1035.006).
13. **G5 `pre-line`**, full `flex` shorthand, `border-color` 1–4 values, grid names — later, not this pass.

**Drop from “add now”:** G4-as-name-only, G6-as-kernel-string-mutate, G8 patches, G9 not-bakeable, G11.

---

## 6. Verdict — build now

**Build now (small, CSS-true, hosts mostly ready):**

- **G2** per-side `border-*-color` (keep `currentcolor`).
- **G1** `aspect-ratio` with `<number>` and `<number> / <number>`; document `auto` as omitting the row.
- **G3** `flex-grow` longhand, with the `flex: 1` contrast test.
- **G5 `white-space: nowrap`** including min/max-content and the overflow+ellipsis idiom. Not `pre-line` in the same drop.

**Build next, still this RFC’s cluster:**

- **G7** compile-time-known `font-family` expressions.
- **CSS `direction`** unblocking (missing from 1053, should ride with G5 if mixed-script lists matter).

**Do not build until hosts exist / a smaller design exists:**

- **G4** (hosts do not read the row).
- **G6** (web emit vs kernel mutate; copy policy).

**Do not build from this RFC:**

- **G8** keyed patches (window/share instead).
- **G9** not-bakeable resources or `mount` (host fact or store).
- **G11** Safari `switch` on `input` (Toggle + 1035.006 later).
- **G10** stays refused.

**Measure, don’t design:** one 10k-row answer, one field changed, with and without record-`Rc` sharing, on the phone. Expect the sharing pattern to be enough; if it is not, the next design is a **windowed resource**, not a patch opcode.

The RFC is right that CSS properties the kernel already stores should be named. It is wrong about how far the text hosts have come (G4, G5), wrong that `flex-grow` substitutes for `flex: 1`, and wrong that G8/G9/G11 are small additions to this architecture.