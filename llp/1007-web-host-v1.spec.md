# LLP 1007: Web host v1 — what `host/web` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Web host, Kernel (seam), Runner (seam), Motion (CSS lowering), Boot
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1002 D2 (on the web the browser executes motion), LLP 1005 (the runner this hosts), LLP 1006 (the compiler whose plan is baked in), LLP 1001 (the kernel tree the page mirrors), `rules/RULES.md` §The five checks (`boot`), LLP 0483 / 0517 (the web on the flat plan; the wasm host interface — research)

## Summary

`host/web/` is the first host: the runner and kernel compiled to wasm,
driving the real DOM. **The DOM mirrors the kernel tree.** After every commit
the host turns the kernel's receipt into a small batch — create, props, style,
children, destroy, roots — with CSS computed once, in Rust, from the kernel's
style rows, the `transition` row included. The browser is the layout engine
and the motion engine; ~120 lines of JavaScript glue apply batches, forward
events, and tick the clock, and nothing runs per frame. The plan is baked into
the wasm at build time, so a page fetches one wasm and one module. Measured
in headless Chrome on the Caltrain app: **script start → first frame in the
DOM, 9–11 ms** (fetch and instantiate 348 KiB of wasm, boot, ~200 elements).
Where this document and the code disagree, the code and its tests are the
authority.

## 1. The seams (`host/web/src/host.rs`)

`Host::boot(plan_bytes, data)` decodes the plan (a validation pass), boots
the runner against a kernel, walks the live tree once, and emits the first
batch. `Host::dispatch(view, event)` and `Host::advance(now_ms)` run the
runner and emit a batch for the receipts: destroyed keys → `destroy`; created
keys → `create` with the element's tag, DOM props, `cssText`, and handler
kinds; touched keys → `props` (set/clear deltas), `style` (whole `cssText`,
only when it changed), `children` (only when the ordered list changed). A
per-view mirror is the memo of what the page has been told; the kernel stays
the one source of truth. A runner refusal comes back in the batch's `error`
with an empty `ops` — the page, like the kernel, is untouched.

Tags: node type → element (`View`→`div`, `Text`→`div`, or `span` for an
inline run, `ScrollView`→`div[data-scroll]`, `TextInput`→`input`,
`Pressable`→`button`, `Image`→`img`, `Toggle`→`input[type=checkbox]`), refined
by `semanticTag` (`main`, `header`, `nav`, `section`, `footer`, `article`,
`aside`). Props → DOM names: `text`→`textContent`, `testId`→`data-testid`,
`accessibilityLabel`→`aria-label`, `accessibilityRole`→`role`,
`placeholder`, `value`, `disabled`, `lang`, `imageSource`→`src`; any other
prop rides as `data-<name>` so nothing is lost.

## 2. CSS, once (`host/web/src/css.rs`)

Every set row is read through the kernel's generated `StyleProps::get`
(landed here as the read-side twin of `set_dynamic`) and lowered by its name:
the CSS property is the row's name with `-` for `_` (`font_size` →
`font-size`, `align_self` → `align-self`, enums verbatim — the kernel's enum
values are already CSS spellings), with the exceptions a table names
(`text_color`→`color`, `position_type`→`position`, `border_radius_top_left`→
`border-top-left-radius`, the four `shadow_*` rows → one `box-shadow`,
`backdrop_blur`→`backdrop-filter: blur()`). Units by rule: dimensions and
lengths in `px`, percentages, `auto`; unitless where CSS is (`flex-grow`,
`opacity`, `z-index`, `font-weight`, `scale`); `rotate` in `deg`;
`translate` as two lengths. Rows the host does not lower are returned as
`Skipped { row, reason }`, never silently dropped: gradients, `font_family`,
`line_clamp`, `tint_color`, and grid rows in v1.

**`transition`** lowers to CSS `transition` — property, duration, easing
(`linear`, keywords, `cubic-bezier()`, `steps(n, jump-*)`, `linear()` with
stops), delay — per LLP 1002 D2. A `spring()` declaration is reported as
skipped: springs are the host's to lower to keyframes
(`exact_motion::spring::keyframes`), which v1 does not yet do (§6).

## 3. The ABI and the glue (`host/web/src/abi.rs`, `glue.js`)

Five exports, no `unsafe`: `exact_in(len)` resizes a host-owned input buffer
and returns its address; `exact_out()` returns the output buffer's; `exact_boot()`,
`exact_dispatch(view, kind, len)`, `exact_advance(now_ms)` each return the
output's length. The glue writes a UTF-8 payload into the input buffer and
reads a UTF-8 JSON batch from the output. `exact_web::host!(DataType, PLAN)`
instantiates the exports for one app; `apps/caltrain/web` is that one line
plus a `build.rs` that compiles and bakes `app.contract` into `OUT_DIR` (never
committed) for `include_bytes!`.

`glue.js` is host code: it fetches and instantiates the wasm, applies batches
(elements by view id; `children` reorders in place so keyed rows keep their
elements and state), attaches `click`/`input` listeners only where a node has
a handler, and — when the plan has timers — calls `exact_advance` on a 250 ms
interval. It stamps `data-boot-ms` on the root when the first batch is in the
DOM and `data-paint-ms` on the next animation frame. `index.html` resets only
what a bare `<div>` would not have (`body` margin; `button`/`input` UA
styles), because the kernel's defaults are already CSS's.

## 4. Building and measuring

`node host/web/build.mjs` — `cargo build --profile web --target
wasm32-unknown-unknown` for the app's crate (the `web` profile is release with
`opt-level = "z"` and fat LTO: the runner's work is sub-millisecond, so every
byte is fetch, parse, and compile), then `wasm-opt -Oz` when binaryen is on
PATH (the build says so when it is not, and ships unoptimized), then
`host/web/dist/` (ignored by git): `app.wasm`, `index.html`, `glue.js`.

**Where the bytes are (2026-08-28, 348 KiB; 149 KiB gzip).** Measured from the
name section of an unstripped build: std/core/alloc ≈ 43% (string and slice
helpers, `core::fmt`, float print and parse for `px` values and JSON), kernel
16%, runner 15%, web host 8%, plan decoder 8%, motion (the `transition`
parser) 3%, hashbrown 2%, plus ≈ 60 KiB of data (the baked plan and strings).
**Taffy is 0%**: nothing on the web reaches `compute_layout`, so the linker
drops the layout algorithms; a "kernel without Taffy" feature would save
nothing here. The profile and `wasm-opt` took 436 → 348 KiB (164 → 149 KiB
gzip) with no change to script-start → DOM. The next real cut is in our own
code and in what it asks of `core::fmt`, not in dependencies. `node host/web/smoke.mjs` —
serves `dist/` and renders it in headless Chrome (`--dump-dom`, a fresh
temporary profile, the process group killed on exit), asserting the app's
landmarks and printing the boot stamp. On 2026-08-28, three runs: 9.0–9.1 ms
script start → first frame in the DOM; 60 countdowns rendered; 25.6 KB of
DOM. Not a blocking check (it needs Chrome); run by hand.

## 5. The `boot` check (`scripts/boot.mjs`)

The fifth check now counts. It reads the page, follows static imports
transitively, and fails when any module before first pixel is not the host
glue or comes from `apps/` — the rules file's own row, "App JS executed
before first pixel: none". A count, not a timer. Today: one module
(`host/web/glue.js`), one wasm reference.

## 6. Not in v1 (and where each is declared)

Springs on the web (the host must lower them to `Element.animate` keyframes
from the current presentation value — the one place the evaluator runs on the
web, LLP 1002 D2; the CSS emitter reports them skipped); the browser-driven
parity harness holding `exact-motion` to the browser (LLP 1002 §5, still
owed); dev-loop reload (edit → rebuild → restart is `build.mjs` by hand; the
resident driver is LLP 1006 §8's); a text-measurement bridge for the kernel's
layout on web (the browser lays out; the kernel's Taffy layout is not run
here — and is dead-code-eliminated from the wasm, §4 — and
`Kernel::with_monospace` is only a placeholder measurer); gradients,
grid, `line_clamp`, `font_family` rows; touch/keyboard events beyond `click`
and `input`; scroll position and focus restoration across reloads.

## 7. Checks that hold this

`host/web/tests/host.rs`: the first batch creates the whole tree with CSS
from the rows; later batches carry only what changed and refusals report
without ops; every row family lowers by name; the `transition` row lowers to
CSS and a spring is named; a `transition` authored in Contract
(`contract/corpus/transition.contract`) reaches the page as CSS and toggles
with state. `scripts/boot.mjs` green; `node host/web/smoke.mjs` green in
headless Chrome. All under the five checks on 2026-08-28.
