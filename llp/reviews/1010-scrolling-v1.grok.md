# Review: LLP 1010 Scrolling v1 (grok)

- **Family:** grok (xAI): `grok -p`, `grok-4.6`, `--reasoning-effort xhigh`, `--disable-web-search`; document-embedded prompt, no repository access
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 ed01727ee8dbe70083ca9b62040bc6c55ca9ed28ef6d00e82ccd614c75af513a; target sha256 d5a9e9ecade3babcbbabfc325ae06848f9349871bca4a3954aa312808c5ddd78.
- **Verdict:** **NOT READY**

---

## Verbatim review

The review brief is truncated; I'll read the offloaded full prompt first so the spec, rules, and cited reviews are complete before writing the review.The prompt is longer than one read; I’ll continue from the kernel style section and collect the remaining specs and reviews.## Overall assessment

LLP 1010 is a strong as-built transcription of the kernel overflow defaults, `content_size` publication, web `data-scroll` / `overflow: auto` mapping, and the macOS `ChainingScrollView` chain-and-consume path. §3 matches `Presenter.swift` and `effective_overflow`; §5 is mostly honest about seams (dominant-axis routing, no offset to the runner, inner rubber-banding given up, Contract attributes, `List` windowing). It is not yet true at the altitude it claims for tests: the Summary and §4 describe measured values and exact limits as if `smoke.mjs` asserted them, and they contradict what that harness actually checks. A Linux host also does not get a crisp rule for **page** document extent (frame union vs kernel `content`). Fold those and it is ready.

## Findings

1. **MATERIAL —** Summary (“Every claim below is held by a smoke…”) and §4 (“The smoke asserts each of those,” including “the page moves for every delivery,” “the page reaches its end (`page 120→2348`),” and “the list scrolls to exactly its limit (`inner 0→652` for an 852 pt document in 200 pt)”) overstate the harness. `host/apple/smoke.mjs` only (a) requires a `scroll:` line, (b) for `docH > boxH` asserts hit-view exclusivity, that inner kept moving over the 200, that the page moved after, and that `page moved before the inner limit` is not `yes`, and (c) otherwise asserts the hit-view event moved the page and not the inner. It does not assert the direct→viewport or direct→inner deliveries, equality with `innerMax` (so a clamp bug can pass), `120→2348`, `0→652`, `852`/`200`, `40→471`, or that the page reached its end. Kernel pairing/`content_size` are attributed in the same section to `cargo test --workspace`, which already contradicts “every claim is held by a smoke.” **Resolve:** split observed run transcript from assertions; say the smoke checks exclusivity and “page fixed until innerMax”; either assert `inner == innerMax` / the numeric bounds or drop “exactly” and the parentheticals as CI claims. **FLAG:** the numbers themselves may be a real hand-run transcript; they are not in the assertion code.

2. **MATERIAL —** §4 (“the kernel's root-width rule is held by the fixture's root laying out at the viewport's width (420, not 89 or 444)”) is not held by the provided tests. `taffy_style` in `kernel/src/style.rs` does implement `width: auto` → `100%` + `BorderBox` for a non-absolute root, but `host/apple/smoke.mjs` never checks a 420-wide root (fixture mode only skips Caltrain landmarks). **Resolve:** name the test that locks 420 (or the layout unit that replaced 89/444), or mark 420 as a hand observation like the trackpad.

3. **MATERIAL —** §1 (“A host sizes a scroll container's document from it; no host unions child frames by hand”) plus §3 (“sized after every batch to the roots' extent … (`Presenter.fitDocument`)”) leave the **page** extent underspecified and easy to implement wrong. `Presenter.fitDocument` starts from `viewport.contentSize` and unions **root subview frames** (`maxX`/`maxY`); it does not use `NodeRef.content`. `host/apple/src/host.rs` `content_size` is `frame.max(content)` and is emitted only for non-visible effective overflow, which a typical root is not. CSS document `scrollHeight` is the root’s scrollable overflow, not the border box of the root view. The same-day disposition treated frame union as correct once in-flow height is in the root frame; this spec never says that, and §1’s “no host unions” reads as a ban on the page path a Linux host must write. **Resolve:** one sentence that the page document is `max(viewport, union of root frames)` and a node scroller’s document is kernel `content` (at least the box); name that overflowing `overflow: visible` / abspos descendants past the root frame are outside both extents (or fold them into `content` and have `fitDocument` read it).

4. **MINOR —** §1 (“An unset `overflow_x` follows a non-visible `overflow_y` (CSS Overflow §3 …)”) and the first bullet (“`overflow_x` follows its row (`visible` by default)”) cite CSS for a rule the code does not implement. `StyleProps::to_taffy` and `effective_overflow` (`host/apple/src/style.rs`) **copy** the non-visible `y` onto unset `x` (`Scroll`→`Scroll`, `Hidden`→`Hidden`). CSS Overflow §3 computes the other axis to **`auto`** when it was `visible`. The schema has no `auto`, so copy is a stand-in; it is not CSS, and the first bullet is false for a default `ScrollView` until bullet 4. On the web, `[data-scroll="true"] { overflow: auto; }` (`host/web/index.html`) is real `auto`, so the default case matches by a different path. **Resolve:** say the schema has no `auto`, pairing copies, default ScrollView/List is both-axes scroll in the kernel and `overflow: auto` on the web; do not call copy “CSS Overflow §3.” **FLAG:** web CSS lowering of explicit `overflow_*` rows is not in the packet; if `overflow-y: hidden` is emitted without `overflow-x`, the browser will compute `x` to `auto` while macOS clips both axes (`applyStyle` `clipsToBounds` / no `scrollsX`).

5. **MINOR —** §2 (“A `ScrollView` or `List` becomes `<div data-scroll="true">`” … “an explicit `overflow_x`/`overflow_y` row lowers by name and wins”) does not say that `data-scroll` is **type-keyed**. `host/web/src/host.rs` `props_for` sets it from `node_type.scrolls_by_default()`, not `effective_overflow`. macOS §3 is explicit that the presenter never keys off kind. A `ScrollView` with `overflow_y: visible` still gets `overflow: auto` from the stylesheet; a `View` with `overflow_y: scroll` relies entirely on lowering. **Resolve:** state the type-keyed attribute and that only inline overflow rows can override it. **FLAG:** could not read the web overflow lowering.

6. **MINOR —** §3 (“lines × 40 pt otherwise (a wheel; the browser's tick)”) is `ChainingScrollView.lineHeight` only. The page scroller is a plain `NSScrollView` (`Presenter.viewport`), so `direct→viewport` and chained events the inner does not take use AppKit’s delta, not ×40. Line-wheel tick size on inner vs page vs “the” browser is then an undeclared split. **Resolve:** 40 pt is the inner consumer; the page is native AppKit; “browser tick” is a target, not a measured UA constant.

7. **MINOR —** Completeness nits a next host / `List` windowing would still trip on, none of which is a wrong algorithm by itself: dominant-axis **ties** go to Y (`abs(dy) >= abs(dx)` in `ChainingScrollView.scrollWheel`); “block axis” is hardcoded `overflow.y` in `to_taffy`; a `ChainingScrollView` is created when an axis becomes `scroll` and is never torn down; `content` ops for `hidden`-only nodes are emitted and no-op because there is no document view; §4’s “as the window would” is `hitTest` + `scrollWheel`, not `sendEvent` (`main.swift`); §5’s heading “Not in v1” mixes absent features with **built but untested** ones (keyboard on the page `NSScrollView`, horizontal paths, web `overflow: hidden`). Windowing is correctly blocked on “no offset to the runner.”

8. **MINOR —** **FLAG** (not in the packet, so not scored as false): `ScrollView`/`List` ids 3 and 4; re-derive on `AttachRoot`; `cargo test --workspace` (171, 2026-08-29) actually covering pairing and `content_size`; `scrollCommand`/`virtualized` inert in the schema; LLP 1002 D4 / 1007 §9 / 1008 / `NOT-DOING.md` wording; fixture layout 852 and Caltrain 2348 as geometry. Same-day dispositions are reflected in §3–§5 for the items that are in the packet (fallback removed, overlay/zero insets, pairing, `data-scroll` on `List`, transforms out of `content`).

## Verdict
NOT READY


---

## Disposition (orchestrator, 2026-08-29) — verdict bound to r1 (d5a9e9ecade3babcbbabfc325ae06848f9349871bca4a3954aa312808c5ddd78); r2 folds it and is unreviewed

1. MATERIAL (harness overclaimed) — FOLDED: `smoke.mjs` now asserts the node ends at its limit, the page at its limit, and the root as wide as the viewport; §4 separates what is asserted from what was observed.
2. MATERIAL (420 not held by a test) — FOLDED: asserted in the smoke (`root WxH` vs `viewport W wide`).
3. MATERIAL (page extent underspecified) — FOLDED at §3: the page is `max(viewport, union of root frames)`; descendants past a root's frame are outside it — declared.
4. MINOR (pairing is a copy, not CSS) — FOLDED and improved: the computation is now symmetric and its `auto` stand-in is named (codex 2).
5. MINOR (`data-scroll` is type-keyed) — FOLDED at §2.
6. MINOR (40 pt is the inner consumer's) — FOLDED at §3.
7. MINOR (completeness nits) — FOLDED: ties go to y (§3); built-but-untested split from not-built (§5); wrapper teardown on `visible` implemented (codex 5).
8. MINOR/FLAG (unverifiable references) — the ids, `AttachRoot`, tests, and props are as stated in the repository; not changed.
