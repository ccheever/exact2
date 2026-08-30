# Code review: fonts (LLP 1019), lane/fonts — sol

- **Family:** OpenAI: `codex exec --json -m gpt-5.6-sol -c model_reasoning_effort=xhigh -s read-only -`, cwd = the lane worktree, read-only, no writes.
- **Method:** two independent code reviews at Charlie's request, 2026-08-30, one per model family, **blind to each other**. Each read LLP 1019 as the specification plus the whole staged diff (~2,800 lines, 40 files) in the lane worktree `exact2-wt-fonts` on `lane/fonts`. Neither built nor ran anything; the orchestrator's own verification was given to them up front so they would hunt for what it missed rather than repeat it.
- **Implementer:** GPT-5.6 Sol at reasoning effort xhigh (`codex exec … -s workspace-write`), orchestrated by Claude (Opus 5). The implementation ran in its own worktree so the peer sessions' shared index in the main tree could not be swept.
- **Orchestrator verification before review:** five checks green (294 tests at the time); `smoke.mjs` green on web, macOS (×3), Linux, and iOS. One regression had already been caught and fixed before review — the first implementation inserted the font hook into the positional `exact_boot`/`exact_boot_plan` C ABI and decoded the plan twice at boot, breaking macOS canvas capture (0 captures, 99.80% readback delta, 5–7 smoke failures over 3 runs, against a base that was green over 3 runs). Attribution was established by building the base commit in a separate worktree, not assumed.
- **Both verdicts on round 1: FIX FIRST.** The findings were consolidated into one fix round; §11 of LLP 1019 records the disposition and which findings the author re-verified independently.

---

## Round 1 — full review (verbatim)

# GPT-5 family review

## Findings

### **HIGH** — Valid app-relative font declarations are omitted from web and iOS artifacts

- Location: [contract/lower/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:448), [host/web/build.mjs](/Users/ccheever/projects/exact2-wt-fonts/host/web/build.mjs:29), [host/apple/build.mjs](/Users/ccheever/projects/exact2-wt-fonts/host/apple/build.mjs:231), [scripts/fixtures/fonts/app.contract](/Users/ccheever/projects/exact2-wt-fonts/scripts/fixtures/fonts/app.contract:1)
- Failure: The compiler accepts any file beneath the app directory, but web and iOS package only `app/assets/`. The fixture declares root-level `DejaVuSans.ttf`, so the built web page requests a nonexistent file and the iOS bundle cannot resolve it; both present fallback text after registration failure. macOS and Linux can still work because their asset root is the app directory.
- Suggested fix: Preserve every declared source at its app-relative path when packaging. Alternatively, require `assets/…` in the compiler and revise D1 and the fixture accordingly.

### **HIGH** — Duplicate attributes bypass the mandatory no-synthesis diagnostic

- Location: [contract/syntax/src/parser.rs](/Users/ccheever/projects/exact2-wt-fonts/contract/syntax/src/parser.rs:1051), [contract/lower/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:522), [contract/lower/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:776)
- Failure: The parser accepts duplicate attributes, `font_use()` analyzes the first `font-family`/`font-style`, while binding deduplication makes the last value effective. For example, `font-family="A" font-family="B" font-weight=700` passes when A has a bold face even if B has only 400, then emits B/700 and silently renders B’s regular face.
- Suggested fix: Run the diagnostic on the effective last-wins attribute tuple after expansion/deduplication, or reject duplicate direct attributes.

### **HIGH** — Linux delegates matching to a CSS Fonts 3 algorithm, not the required CSS Fonts 4 algorithm

- Location: [host/linux/src/text.rs](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:429), [fontdb 0.23.0 selector](/Users/ccheever/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fontdb-0.23.0/src/lib.rs:1207)
- Failure: With real faces at 400 and 500, a requested weight of 475 resolves to 500 on the web and Apple, but `fontdb` explicitly checks 400 first for requests from 450 through 500 and resolves to 400. The fixture’s 600 correctly resolves to 700, but D6’s full 1–1000 domain is not portable.
- Suggested fix: Rank declared Linux faces with the same explicit CSS Fonts 4 algorithm used by Apple, then shape using the selected declared face/weight.

### **HIGH** — Overlapping web reloads can apply an older plan after a newer one

- Location: [host/web/dev.js](/Users/ccheever/projects/exact2-wt-fonts/host/web/dev.js:11), [host/web/glue.js](/Users/ccheever/projects/exact2-wt-fonts/host/web/glue.js:268), [host/web/glue.js](/Users/ccheever/projects/exact2-wt-fonts/host/web/glue.js:435)
- Failure: EventSource handlers are not serialized. If plan A waits on a slow font and plan B boots and presents first, A’s generation check prevents its fonts from installing but `boot()` still unconditionally applies A’s captured batch after its load or timeout. The DOM then represents A while the wasm runner represents B.
- Suggested fix: Give the complete boot operation a generation token and discard stale batches after `await installFonts`; also serialize or cancel obsolete reloads.

### **MEDIUM** — Web reintroduces the explicitly rejected `exact_out()` font-table seam

- Location: [host/web/src/batch.rs](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/batch.rs:61), [host/web/src/batch.rs](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/batch.rs:293), [host/web/src/abi.rs](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/abi.rs:220)
- Failure: Boot output now contains a top-level `fonts` table alongside kernel operations, contrary to LLP 1019 D5 and §9. An executor implementing the documented operation-only output schema will ignore the catalog and present before registration; the static plan catalog is also coupled to transient batches and their reload ordering.
- Suggested fix: Expose the web catalog through a separate plan-owned query/export analogous to Apple’s `exact_set_fonts` seam, leaving output batches operation-only.

### **MEDIUM** — A hostile validated plan can make the web load remote fonts

- Location: [plan/tables/format.json](/Users/ccheever/projects/exact2-wt-fonts/plan/tables/format.json:42), [plan/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/plan/src/lib.rs:210), [host/web/glue.js](/Users/ccheever/projects/exact2-wt-fonts/host/web/glue.js:274)
- Failure: `faces.source` is an unconstrained string in `Plan::validate`, and `new URL(source, document.baseURI)` accepts absolute schemes. A plan containing `https://example.invalid/font.ttf` passes validation and triggers a remote web request, while Apple rejects it and Linux treats it as a local path. This violates D5’s local-only rule and cross-host identity.
- Suggested fix: Validate a portable asset-path grammar in the plan: relative path only, no scheme, authority, query, fragment, root, or parent traversal. Encode path segments rather than feeding raw source strings to URL resolution.

### **MEDIUM** — Plan validation permits family and face identities that collapse differently across hosts

- Location: [plan/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/plan/src/lib.rs:218), [host/web/src/host.rs](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/host.rs:434)
- Failure: A plan with two family rows named `X`, or duplicate `(weight, italic)` faces in one family, passes validation. Native hosts retain stack-index identity, but web installs both rows under the same CSS family name; selection then crosses stack boundaries or depends on registration order.
- Suggested fix: Require unique family aliases using CSS’s comparison semantics and unique `(weight, italic)` coordinates within each family.

### **MEDIUM** — Linux’s predictable synthetic alias can collide with a system font and defeat declared-byte identity

- Location: [host/linux/src/text.rs](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:247), [host/linux/src/text.rs](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:326), [host/linux/src/text.rs](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:429)
- Failure: System fonts are loaded first, declared faces are appended under names such as `ExactPlanStack8`, and matching queries the complete database by that alias. If an installed font already owns the alias and matching coordinates, `fontdb` can select it instead of the stored declared face ID.
- Suggested fix: Match declared stacks directly against their stored face IDs. At minimum, generate an alias verified absent from the database before registration.

### **LOW** — A directory named with a `.ttf` suffix can pass the compiler’s readability check

- Location: [contract/lower/src/lib.rs](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:487)
- Failure: On Unix, opening a directory read-only can succeed. An `assets/broken.ttf/` directory therefore canonicalizes and may pass `File::open`, producing a plan every host later refuses to register.
- Suggested fix: Require `metadata().is_file()` in addition to opening the path.

### **LOW** — Apple does not preserve D4’s “measure and paint the same CTLines” invariant for colored text

- Location: [host/apple/swift/Text.swift](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:34), [host/apple/swift/Text.swift](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:223), [host/apple/swift/Text.swift](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:314), [iOS Presenter.swift](/Users/ccheever/projects/exact2-wt-fonts/host/apple/ios/Sources/ExactIOS/Presenter.swift:535)
- Failure: Color participates in the paragraph cache key. Measurement shapes a black `Spec`, while painting requests the actual-color `Spec`, so any non-black text creates and paints a second set of CTLines. This currently should select the same face, but the normative one-shaped-paragraph guarantee and its cache claim are false.
- Suggested fix: Keep color out of the shaping specification/cache and apply it at draw time, allowing the measured paragraph to be reused.

## What I verified and found correct

- The four plan tables and generated range/reference validation are present; generic stacks remain distinct low IDs, and runner values are checked against `plan.stacks`.
- The ordinary compiler refusals for undeclared families, comma lists, `.woff2`, unsupported extensions, missing paths, and parent/root escapes are present. Duplicate declared families and duplicate face coordinates are also rejected at compile time.
- Apple installs a separately supplied catalog synchronously before both `exact_boot` and `exact_boot_plan`; the original positional ABI is restored and shared by UIKit and AppKit.
- Apple resolves declared bytes through URL-derived CoreText descriptors and never calls `UIFont(name:)`. Its family-bearing caches include family identity and are cleared on plan replacement.
- Linux cache keys include family identity and a fresh text engine replaces all plan-scoped caches. With the fixture’s 400/700 pair, its current matcher selects the declared 700 face for a request of 600.
- Web CSS resolves stack IDs through the plan’s name table rather than emitting raw numeric IDs.
- For one non-overlapping boot, the web barrier handles partial failure atomically by family, presents after 100 ms on timeout, and never installs faces that finish after the barrier closes.
- `font-synthesis: none` is installed on web, and text inputs on Apple and Linux route through the same family catalog as ordinary text.
- Apple’s `exact_out()` remains operation-only; its catalog uses the separate `exact_set_fonts` callback.

## What I could not verify

- I did not build or run anything, as requested, and did not repeat the owner’s completed checks.
- iOS compilation and runtime behavior remain unverified: particularly CoreText registration from the bundle, the `CTFont`/`UIFont` bridge, text-field rendering, and resolved descriptor identity. I would package a fixture under `assets/`, run `node host/apple/build.mjs --ios --run`, drive it with `scripts/agent.mjs ios`, and assert that weight 600’s descriptor URL is the declared 700 file.
- I did not reproduce the web reload race or browser timing behavior. I would test with controllably delayed font responses and send two plan reloads inside the 100 ms barrier, asserting that only the newest plan’s DOM, fonts, and runner state survive.

Verdict: FIX FIRST — The lane still has silent cross-platform identity failures, including font files that cannot reach web/iOS and mismatched Linux face selection.

---

## Round 2 — delta review after the fix round (verbatim)

*Read only what changed since round 1; judge whether the findings closed and whether the fixes broke anything. The orchestrator's post-fix verification was supplied up front.*

Own findings:

1. `CLOSED` — font sources are restricted to `assets/…` at [contract/lower/src/lib.rs:497](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:497).
2. `PARTLY` — direct duplicates are refused at [parser.rs:1059](/Users/ccheever/projects/exact2-wt-fonts/contract/syntax/src/parser.rs:1059), but styles still accept duplicate attributes at [parser.rs:523](/Users/ccheever/projects/exact2-wt-fonts/contract/syntax/src/parser.rs:523). `font_use` reads the first at [lower/lib.rs:545](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:545), while bindings remain last-wins at [lower/lib.rs:799](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:799). The same bypass works through `class=`.
3. `OPEN` — Linux still delegates weight selection to `fontdb::query` at [host/linux/src/text.rs:439](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:439).
4. `OPEN` — a stale boot still applies its captured batch unconditionally after awaiting fonts at [host/web/glue.js:454](/Users/ccheever/projects/exact2-wt-fonts/host/web/glue.js:454).
5. `CLOSED` — batches end with operation fields only at [host/web/src/batch.rs:281](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/batch.rs:281); the separate query is at [host/web/src/abi.rs:123](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/abi.rs:123).
6. `CLOSED` — portable local-path validation is enforced at [plan/src/lib.rs:214](/Users/ccheever/projects/exact2-wt-fonts/plan/src/lib.rs:214).
7. `PARTLY` — opaque stack names close duplicate family-name collapse at [host/web/src/host.rs:442](/Users/ccheever/projects/exact2-wt-fonts/host/web/src/host.rs:442), but validation still permits duplicate `(weight, italic)` face coordinates at [plan/src/lib.rs:214](/Users/ccheever/projects/exact2-wt-fonts/plan/src/lib.rs:214).
8. `OPEN` — Linux still installs predictable aliases and then queries the combined database at [host/linux/src/text.rs:326](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:326) and [host/linux/src/text.rs:397](/Users/ccheever/projects/exact2-wt-fonts/host/linux/src/text.rs:397).
9. `OPEN` — the compiler still tests only `File::open`, not `is_file`, at [contract/lower/src/lib.rs:517](/Users/ccheever/projects/exact2-wt-fonts/contract/lower/src/lib.rs:517).
10. `OPEN` — color remains in `Spec` and its cache key at [Text.swift:35](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:35) and [Text.swift:232](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:232), while measurement uses black at [Text.swift:330](/Users/ccheever/projects/exact2-wt-fonts/host/apple/swift/Text.swift:330).

New defect: the web fix adopts the same predictable `ExactPlanStack<n>` collision class as Linux. On failure/timeout no `FontFace` is added at [glue.js:297](/Users/ccheever/projects/exact2-wt-fonts/host/web/glue.js:297), so an installed system family with that generated name can satisfy the CSS lookup. The timeout bookkeeping itself otherwise matches the amended D5: loaded faces are snapshotted, known-failed families excluded, and late completions remain uninstalled.

In §10, CSS Fonts 4 ranking, overlapping reloads, and direct-ID matching are blockers, not debt. “Duplicate declared family aliases” is now obsolete because stack-index aliases isolate them; duplicate face coordinates are the remaining validation defect. The Apple cache and regular-file items can reasonably remain owed.

Verdict: FIX FIRST — The diagnostic remains bypassable through duplicate style attributes, while CSS4 ranking and reload ordering still violate LLP 1019.
