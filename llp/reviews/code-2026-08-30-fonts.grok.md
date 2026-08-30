# Code review: fonts (LLP 1019), lane/fonts — grok

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <lane> --output-format plain`, read-only.
- **Method:** two independent code reviews at Charlie's request, 2026-08-30, one per model family, **blind to each other**. Each read LLP 1019 as the specification plus the whole staged diff (~2,800 lines, 40 files) in the lane worktree `exact2-wt-fonts` on `lane/fonts`. Neither built nor ran anything; the orchestrator's own verification was given to them up front so they would hunt for what it missed rather than repeat it.
- **Implementer:** GPT-5.6 Sol at reasoning effort xhigh (`codex exec … -s workspace-write`), orchestrated by Claude (Opus 5). The implementation ran in its own worktree so the peer sessions' shared index in the main tree could not be swept.
- **Orchestrator verification before review:** five checks green (294 tests at the time); `smoke.mjs` green on web, macOS (×3), Linux, and iOS. One regression had already been caught and fixed before review — the first implementation inserted the font hook into the positional `exact_boot`/`exact_boot_plan` C ABI and decoded the plan twice at boot, breaking macOS canvas capture (0 captures, 99.80% readback delta, 5–7 smoke failures over 3 runs, against a base that was green over 3 runs). Attribution was established by building the base commit in a separate worktree, not assumed.
- **Both verdicts on round 1: FIX FIRST.** The findings were consolidated into one fix round; §11 of LLP 1019 records the disposition and which findings the author re-verified independently.

---

## Round 1 — full review (verbatim)

# Grok 4.6 (xAI) — fonts / LLP 1019

## Findings

### HIGH — Apple takes descriptors only after `CTFontManagerRegisterFontsForURL` succeeds, so a PostScript-name collision with an installed copy drops the stack onto the system font

`host/apple/swift/Text.swift:91–96` and `:121–132`

`install` requires `register(url)` before `CTFontManagerCreateFontDescriptorsFromURL`. `register` treats only `CTFontManagerError.alreadyRegistered` (same URL, same process — the dev-reload case) as success. A bundled face whose PostScript name is already in the session — `duplicatedName`, a designer who already has “Castle Display” installed, a previous plan’s registration under a different URL — fails the guard, the stack is removed (`:100–104`), and `font(...)` at `:159–179` falls through to `systemFont(ofSize:weight:)`.

That is the D3 identity trap: the declared bytes are on disk, the resolver never holds their descriptor, measure and paint agree on the *wrong* family, and the only signal is a stderr line. Descriptor-from-URL does not need process registration (registration is what `UIFont(name:)` needed). Gating the URL descriptor on register success reintroduces a system-library lookup by another name.

**Failure:** Weird Castle’s wordmark TTF is in `assets/`, the designer also has that family installed, iOS/macOS paint San Francisco (or the installed cut) at the wordmark’s metrics.

**Fix:** Always `CTFontManagerCreateFontDescriptorsFromURL` after the sandbox check. Treat `alreadyRegistered` *and* `duplicatedName` as “the URL is still ours.” Keep `RegisterFontsForURL` as a best-effort for `UITextField` if iOS still needs it, but do not fail the catalog when it complains.

### HIGH — Web binds the Contract alias as the CSS family, so a timeout or failed `FontFace` is a local-font lookup of that string

`host/web/src/host.rs:434–466`, `host/web/src/css.rs:73–80`, `host/web/glue.js:268–305`

Linux rewrites the fontdb family to `ExactPlanStack{n}` (`host/linux/src/text.rs:327, 358`) so a query cannot hit the system library. Web does the opposite: `@font-face` / `FontFace` and `font-family` both use the Contract alias (`"Fixture Sans"`). That is what LLP 1019 D3 asked for on the happy path.

On the failure path it is a system lookup of the declared string, which D3 forbids:

- `installFonts` times out at 100 ms and sets `closed = true`; the `ready` handler then returns without `document.fonts.add` (`glue.js:289, 303–304`).
- `css_text` still emits `font-family:"Fixture Sans"` (`css.rs:73–80`).
- The browser then matches that name against installed fonts.

**Failure:** load takes 101 ms, or one face 404s and the whole family is withheld (`glue.js:290–295`). A machine with “Castle Display” / “Fixture Sans” installed — the exact1 ENG-22195 shape — renders the local cut. No FOUT, but also no last-resort; it looks like success.

**Fix:** Give each stack an opaque CSS family (`ExactPlanStack8`) in both `FontFace` and `css_text`, the way Linux already does. Keep the Contract alias out of the CSS name table. Optionally still emit `system-ui` as host last-resort (not an authored comma list).

### MEDIUM — Apple uses `descriptors.first` for every weight of a file; Linux refuses a multi-face file. 600→700 is then wrong on Apple only

`host/apple/swift/Text.swift:92–98`, `host/linux/src/text.rs:348–351`

Two declared weights that share one source (a `.ttf` collection, a two-face OTF, or a variable file declared as both 400 and 700) become two `RegisteredFace`s with the **same** descriptor. `matched` at `:135–153` will still pick the 700 *row*, then `CTFontCreateWithFontDescriptor` builds the first face in the file. Linux `ids.len() != 1` fails the whole family.

The DejaVu fixture is two single-face files, so Linux tests cannot catch this. The compiler does not inspect the file (`contract/lower/src/lib.rs:448–467` is extension + `open` only).

**Failure:** `font "Display" { 400 = "Family.ttf"; 700 = "Family.ttf" }` with both faces in one file. Web gets two `FontFace`s and CSS matches 600→700. Apple measures and paints the Regular instance at every weight.

**Fix:** Pick the descriptor whose traits match the declared weight/italic, or refuse `descriptors.count != 1` at compile/install the way Linux does. Same for a variable file used as two static weights: set the variation axis or refuse.

### MEDIUM — Web readiness barrier: a timeout permanently drops that generation, including faces that already loaded

`host/web/glue.js:265–305`

LLP 1019 D5 says hang must time out and present with fallback. The code also never installs late faces (`:289`, comment at `:267`). `Promise.all` means one slow face holds the whole set past 100 ms; then `closed` discards even the faces that had already `load()`ed. RULES.md’s 100 ms figure is a p50 budget, not a hard abort.

**Failure:** a 200 KB display face on a cold disk / first-decode on a phone takes 120 ms. Console: `font.registration.timeout`. The wordmark stays on the fallback for the life of that boot, including agent `screenshot`.

**Fix:** At timeout, add every face that has already loaded (still fail-the-family if a sibling failed). Decide whether late faces may install after first paint (FOUT) or not; do not use the p50 budget as “never show the brand face.”

### MEDIUM — Compiler accepts any app-relative `.ttf`/`.otf`; iOS and web packagers only copy `assets/`

`contract/lower/src/lib.rs:468–503`, `host/apple/build.mjs:240`, `host/web/build.mjs:29–33`, `scripts/fixtures/fonts/app.contract:1–3`

The fixture declares `400 = "DejaVuSans.ttf"` beside `app.contract`. `compile_path` is happy: the file exists under the app dir. `node host/apple/build.mjs --ios` copies only `app.dir/assets` into the bundle; the web dist copy is the same. Device iOS has no `EXACT_ASSETS` (`build.mjs:259–263` vs simulator `SIMCTL_CHILD_EXACT_ASSETS: app.dir` at `:278`). `fontURL` then joins `Bundle.main.bundlePath` + `DejaVuSans.ttf` and register fails.

**Failure:** an author copies the fixture layout, or puts faces in `fonts/` instead of `assets/`. Linux/macOS `--run` (asset root = app dir) work. iOS device and web 404 / registration.failed.

**Fix:** Refuse sources whose first component is not `assets/`, or copy every compiled face path into the bundle/dist (LLP 1011’s copy is the folder, not the referenced set).

### LOW — `use` cannot name a font; a fonts-only file is unreachable

`contract/cli/src/lib.rs:292–304` vs `:319–325`

`merge` does pull fonts from a used file, but `use X from "./fonts.contract"` requires `X` to be a component, shape, style, or `fn`. A file that only declares `font "Castle Display"` cannot be imported.

**Failure:** shared brand-face file with no dummy export.

**Fix:** Treat a font name as a valid `use` target, or say so in D1.

### LOW — Apple still shears italic on generic stacks

`host/apple/swift/Text.swift:192–198`

§5 says never smear and never shear; the italic-trait branch is supposed to be gated on a real italic face. The gate is “catalog miss” (generics), not “this family has an italic face.” `system-ui` italic on iOS is `withSymbolicTraits(.traitItalic)`, which synthesizes when the design has no italic.

Neither app uses `font-style`. Cut with D7 if the slice needs cutting.

---

### Spec reproduced, not an implementation miss

LLP 1019 D3 tells web to use the Contract alias as the `@font-face` family-name. That *is* a lookup key in the system font library the moment `document.fonts` does not contain the face. Linux’s opaque alias is the identity rule; the web sentence is the trap. The implementation followed the sentence.

The §5 diagnostic only seeing literals (`contract/lower/src/lib.rs:1111–1126`) is the limit the RFC already names. Runtime nearest-face plus `#exact-root { font-synthesis: none }` (`host/web/index.html:10`) covers derives. That combination is correct.

---

## What I verified and found correct

**The five traps, on the happy path**

1. **Cache keys carry family; plan identity is “discard on replace.”** Apple font key is `"\(family)/\(size)/\(weight)/\(italic)"` (`Text.swift:157`); `install` clears `fonts` and `paragraphs` (`:80–82`). Linux `weights` is `(family, weight, italic)` (`text.rs:172`), `normal` is `(family, size, weight, italic)` (`:168, 477`), paragraph key includes `r.family` (`:87–99`); `install_plan` replaces the engine (`:301–382`).
2. **Declared name → registered bytes on Apple and Linux.** No `UIFont(name:)`. Apple: `CTFontCreateWithFontDescriptor` from the URL descriptor (`Text.swift:161`). Linux: `ExactPlanStack{n}` overwrite (`text.rs:327, 358`) + `Family::Name`. Inputs use the same `Text.font(..., family:)` on both UIKit and AppKit (`ExactIOS/Presenter.swift:441`, `ExactMac/Presenter.swift:474`).
3. **`exact_out()` is still kernel ops on the C ABI.** Fonts are `exact_set_fonts` + `install_fonts` on the decoded `Plan` (`exact.h:59–78`, `abi.rs:349–375`, `measure.rs:113–140`). `exact_boot` / `exact_boot_plan` signatures match HEAD; only `ExactTextRun` gained `font_family`. Web puts `"fonts"` *beside* `ops` in its JSON (`batch.rs:297–305`) — that is the web seam, not the Apple batch.
4. **Web CSS goes through the plan name table.** `font_names[stack_id]` (`host.rs:434–449`); test asserts `font-family:\"Fixture Sans\"` and not stack `8` (`host/web/tests/host.rs:315–318`). Generics stay unquoted (`css.rs:75–76`).
5. **Web barrier: timeout, partial failure, late load, reload.** Timeout logs and presents (`glue.js:304`); one failed face withholds the family (`:290–295`); late `ready` no-ops on `closed` / generation (`:289`); reload increments `fontGeneration` and `document.fonts.delete`s the previous set (`:269–271`). DOM is empty until `await installFonts` returns (`:454–456`).

**Compiler refusals** — all five are real, with tests in `contract/cli/tests/fonts.rs`:

| Refusal | Id | Where |
|---|---|---|
| undeclared family | `lower-font-undeclared` | `lib.rs:540–544, 1228–1232` |
| missing/unreadable | `lower-font-unreadable` | `:488–502` (canonicalize + `open` + `starts_with`) |
| `.woff2` and non-ttf/otf | `lower-font-format` | `:450–466` |
| comma list | `lower-font-family-list` | `:413–420, 533–537` |
| §5 synthesis | `lower-font-face` | `:1092–1125` (italic face missing, or weight ≥ 600 with no face ≥ 600 of that style) |

Also: `..` / absolute / prefix (`:468–485`), duplicate family including generics (`:423–431`), duplicate `(weight, italic)` (`:436–446`), `compile` without a path (`:399–404`), `font-family` non-literal (`:526–530, 1221–1226`). I do not see an extension or comma bypass. Content-not-matching-extension is only caught at host parse, which is what D5 says for corrupt files.

**Face matching (CSS Fonts 4 §5.2, family then weight)**

- Linux: `snap_weight` queries only that family’s alias (`text.rs:429–456`); fixture test asserts stack 8 weight 600 resolves to the declared 700 id and 111.1 pt geometry, not the system face (`host/linux/tests/text.rs:80–142`).
- Apple: `matched` is the CSS 400–500 / `<400` / `>500` rank (`Text.swift:135–153`) inside the catalog, then the file’s descriptor — not `systemFont` + a weight trait. By inspection, 400+700 with request 600 picks 700. Not run.
- Web: two `FontFace`s at 400 and 700 plus `font-weight:600` is the browser’s matcher, *if* they were added.

**Plan tables.** `faces` / `families` / `stacks` / `stack_members` in `plan/tables/format.json`; `PlanBuilder::new` plants eight distinct generic stacks at 0–7 (`builder.rs:32–46`); `validate_semantics` checks weight 1–1000, non-empty families, one member, family-xor-generic, those eight kinds in order, `u16` cap (`plan/src/lib.rs:210–261`). Runner refuses `font_family >= stacks.len()` (`runner/src/bridge.rs:55–67`). Dangling `FamiliesId` is `BadReference` (`plan/tests/format.rs:95–104`). Hostile plans can still carry a path the compiler would have refused; native hosts re-sandbox, web does not (see HIGH).

**ABI fix.** `Bridge.boot` / `boot_plan` decode once, call `install_fonts` in `prepare` *before* `Runner::boot` (`host.rs:139–160`, `abi.rs:162–165`). iOS and macOS share `swift/Bridge.swift` and `swift/Text.swift` via symlink; both `main.swift`s call `Exact.boot` → `exact_set_fonts(installFonts)` then `exact_boot(measureText, …)` (`Bridge.swift:40–52`). iOS Presenter got the same `family:` / `font_style` wiring as macOS (cached diff is the same two sites). CExact on both packages includes `host/apple/include/exact.h`. This is the same seam that broke canvas capture when `exact_boot` grew an argument; that signature is restored.

**§5 / LLP 1001.** Deviation is in `llp/1001-kernel-v1.spec.md:60–63`. Web emits `font-synthesis: none` on `#exact-root` (inherited; `all: unset` on `button,input` inherits it back). Apple italic-trait is off the custom-descriptor path.

## What I could not verify

- **iOS, running.** Same Swift and C ABI as the macOS smoke that passed, but no custom face has been registered under UIKit, in a simulator with `SIMCTL_CHILD_EXACT_ASSETS`, or on a device that only sees `Bundle.main.bundlePath`. I would run `node scripts/smoke.mjs ios` and an agent drive of `scripts/fixtures/fonts/app.contract` (400 / 600 / 700 widths, plus a `UITextField` with `font-family`).
- **Apple 600→700 as painted geometry.** The matcher looks like CSS Fonts 4 §5.2; nothing in this tree asserts resolved face identity on CoreText the way `host/linux/tests/text.rs` does. I would boot the fixture on macOS and compare 600 vs 700 advances (Book ~98.6, Bold ~111.1 at 13 pt).
- **Whether `CTFont as UIFont` keeps a descriptor-only face in `UITextField` on iOS.** Measure/paint go through `CTLine`; the field sets `f.font`. If the bridge is lossy without a successful `RegisterFontsForURL`, inputs would be the one place a second resolver appears (D4).
- **`duplicatedName` in a live `CTFontManager` session** and a real 101 ms `FontFace.load` on this machine. The control flow is in the code; the firing conditions are not.
- **Web matching after a successful barrier.** Tests check JSON, not `document.fonts` + computed face. I would load the fixture in `host/web/dev.mjs` and read computed `font-family` / width for `font-600`.

Verdict: FIX FIRST — close the two D3 holes (Apple register-gated descriptors, web Contract-alias lookup on timeout/failure) before a brand face ships; the happy path and the compiler refusals are otherwise in the shape LLP 1019 asked for.

---

## Round 2 — delta review after the fix round (verbatim)

*Read only what changed since round 1; judge whether the findings closed and whether the fixes broke anything. The orchestrator's post-fix verification was supplied up front.*

## My findings

| Finding | Status | Settled at |
|---|---|---|
| HIGH — Apple gated URL descriptors on `register()` | **CLOSED** | `Text.swift:91–104, 127–138` — descriptor from URL first; `register` is best-effort; `alreadyRegistered` and `duplicatedName` both count as ours |
| HIGH — Web used the Contract alias as the CSS family | **CLOSED** | `host.rs:450, 471`; `css.rs:73–80` via `font_names`; test at `host/web/tests/host.rs:322–328` — `ExactPlanStack8`, and `"Fixture Sans"` is asserted absent from both catalog and batch |
| MEDIUM — timeout discarded already-loaded faces | **CLOSED** | `glue.js:265–305` |
| MEDIUM — compiler accepted faces outside `assets/` | **CLOSED** | `contract/lower/src/lib.rs:497–509`; fixture now under `assets/`; iOS copies `assets/` into the bundle (`build.mjs:240`) |
| MEDIUM — `descriptors.first` for every weight of one file | **OPEN** | `Text.swift:93` unchanged; in §10 owes |
| LOW — `use` cannot name a font | **OPEN** | `contract/cli/src/lib.rs:296–307` unchanged; in §10 owes |
| LOW — generic italic still shears | **OPEN** | `Text.swift:200–206` unchanged; in §10 owes |

The barrier is the one I re-read against the event loop. The snapshot after `Promise.race` does what you asked: a `load()` that has already settled has set `row.state` (that continuation is a microtask, so it has run before the timer macrotask’s continuation). Failed siblings withhold the family; pending siblings do not. Late completions only mutate the local `rows` and never call `document.fonts.add`. There is no timeout test, so this is inspection, not a run — but I do not see a logic hole in the new code.

## New from the fixes

Nothing that should block. Two leftovers, neither introduced as behavior:

- `Text.swift:330–332` now claims measuring as black shares the paragraph cache with paint. Paint still keys `Spec` with the real color (`Presenter.swift:547–550` / macOS `:575–578`), so they still shape twice. The owe-list item is the real state; the comment is wrong.
- Opaque names incidentally close Sol’s “two families named X collapse on web” identity failure (`ExactPlanStack{n}` is per stack index). Plan validation still allows the duplicate; that is now hostile-plan hygiene, not a web lookup bug.

The `exact_fonts()` split is sequenced correctly: `readOut` copies the batch before the catalog query overwrites the output buffer (`glue.js:454–456`).

## §10 “owes” vs blocking

Correctly owed for this slice (one wordmark face, DejaVu 400/700, no `font-style`, no font-only `use` file): Linux CSS Fonts 3 ranking, Linux alias collision, Apple multi-descriptor files, color in the paragraph key, generic italic shear, `.ttf`-named directories, `use` of a font-only file.

Closest to misfiled: **overlapping web reloads** (`glue.js:435–457` still applies the captured batch after `await installFonts` with no generation token; `dev.js:11` still does not serialize). That is still a live EventSource race on the ~20 ms loop — Sol’s HIGH, not mine. First boot and `smoke.mjs` never hit it. I would not block the wordmark on it; I would not call it “owed” as if it were a polish item either. It is a real hole in the advertised restart, just not in this slice’s ship path.

Verdict: SHIP — the two D3 holes and the `assets/` packaging miss are closed, and the remaining list is correctly not on the wordmark path.
