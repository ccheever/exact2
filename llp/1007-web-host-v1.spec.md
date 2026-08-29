# LLP 1007: Web host v1 — what `host/web` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Web host, Kernel (seam), Runner (seam), Motion (CSS lowering, springs, parity), Boot, Dev loop
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Revised:** 2026-08-28 (springs lowered to frames, the browser-driven parity harness, the resident dev driver; the size pass)
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1002 D2/D3/§5 (on the web the browser executes motion; the clock is a seek; the parity harness), LLP 1003 §4 (spring lowering; the seam), LLP 1004 D5 (a reload is a restart), LLP 1005 (the runner this hosts), LLP 1006 (the compiler whose plan is baked in; §8's resident driver), LLP 1001 (the kernel tree the page mirrors), `rules/RULES.md` §Time budgets and §The five checks (`boot`), LLP 0483 / 0517 (the web on the flat plan; the wasm host interface — research)

## Summary

`host/web/` is the first host: the runner and kernel compiled to wasm,
driving the real DOM. **The DOM mirrors the kernel tree.** After every commit
the host turns the kernel's receipt into a small batch — create, props, style,
children, destroy, roots, animate — with CSS computed once, in Rust, from the
kernel's style rows, the `transition` row included. The browser is the layout
engine and the motion engine; ~150 lines of JavaScript glue apply batches,
forward events, tick the clock, and play a spring's frames, and nothing runs
per frame. The plan is baked into the wasm at build time, so a page fetches
one wasm and one module. Measured on the Caltrain app (`scripts/metrics.mjs`,
2026-08-28): **script start → first frame in the DOM, 10–11 ms** (fetch and
instantiate 348 KiB of wasm — 149 KiB gzip — boot, ~200 elements); **edit
`app.contract` → the new plan's first frame in the DOM, 18–20 ms** through the
resident dev driver, against the 100 ms budget row. The engine is held to the
browser by a recorded fixture: 105 samples, 0 disagreements. Where this
document and the code disagree, the code and its tests are the authority.

## 1. The seams (`host/web/src/host.rs`)

`Host::boot(plan_bytes, data)` decodes the plan (a validation pass), boots
the runner against a kernel, walks the live tree once, tells the spring
evaluator about it (§3), and emits the first batch. `Host::dispatch_at(view,
event, now_ms)` and `Host::advance(now_ms)` run the runner and emit a batch
for the receipts: destroyed keys → `destroy`; created keys → `create` with the
element's tag, DOM props, `cssText`, and handler kinds; touched keys → `props`
(set/clear deltas), `style` (whole `cssText`, only when it changed),
`children` (only when the ordered list changed); then, last, `animate` for
any spring the commit released (§3). `now_ms` is the page's clock
(`performance.now()` from script start), the one clock the runner's timers
and the springs share. A per-view mirror is the memo of what the page has
been told; the kernel stays the one source of truth. A runner refusal comes
back in the batch's `error` with an empty `ops` — the page, like the kernel,
is untouched.

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
stops), delay — per LLP 1002 D2. A `spring()` declaration is left out of the
CSS text and reported as skipped by name: springs are §3's.

## 3. Springs (`host/web/src/motion.rs`)

CSS cannot play `spring()`, so the host keeps the same `exact_motion::Engine`
every native host runs and uses it **once per release, as a compiler** (LLP
1002 D2). After each commit the host feeds the engine through the kernel's
seam (`Kernel::motion_sync`, LLP 1003 §4), seeks it to the commit's clock
(`Engine::advance`), and asks for the frames of any spring that just started
(`Engine::spring_frames`, landed here: the running spring's values on the
240 Hz grid, evenly spaced, from its release value to its target — the same
bits `Running::sample` returns at those times). The batch carries them as
`{"op":"animate","id","property","delay","duration","values"}` (`translate`
values as `[x,y]` pairs); the glue plays them with `Element.animate(frames,
{delay, duration, easing: "linear"})`, replacing any spring on that property.
The style row is the target and is already in the element's `cssText`, so
when the frames end the style shows through with no seam: the last frame *is*
the target.

The release value and velocity are the engine's — a spring that interrupts a
transition in flight (its own, or an eased one) starts from where the
property is, carrying its velocity, by the same CSS Transitions §3 rule the
engine applies natively. Because the engine is seeked only at commits, the
host reads its state exactly when a decision is made and never per frame. A
spring that reaches its target says nothing (the page's animation finished
too); a property that moves on without a spring cancels its frames so the
style (or a CSS transition) takes over. `contract/corpus/spring.contract` is
the accept fixture; `host/web/tests/springs.rs` holds the release, the
interrupt from the presentation value (bit-equal to the closed form), the
quiet clock, and the pair format.

## 4. The ABI and the glue (`host/web/src/abi.rs`, `glue.js`)

Six exports, no `unsafe`: `exact_in(len)` resizes a host-owned input buffer
and returns its address; `exact_out()` returns the output buffer's;
`exact_boot()`, `exact_boot_plan(len)` (boot from plan bytes in the input
buffer — the dev loop's restart, §6), `exact_dispatch(view, kind, len,
now_ms)`, `exact_advance(now_ms)` each return the output's length. The glue
writes a UTF-8 payload into the input buffer and reads a UTF-8 JSON batch from
the output. `exact_web::host!(DataType, PLAN)` instantiates the exports for
one app; `apps/caltrain/web` is that one line plus a `build.rs` that compiles
and bakes `app.contract` into `OUT_DIR` (never committed) for
`include_bytes!`.

`glue.js` is host code: it fetches and instantiates the wasm, applies batches
(elements by view id; `children` reorders in place so keyed rows keep their
elements and state; `animate` plays or cancels a spring), attaches
`click`/`input` listeners only where a node has a handler, and — when the plan
has timers — calls `exact_advance` on a 250 ms interval. `boot(bytes?)` tears
the page down (interval, animations, elements) and boots from the baked plan
or from bytes, and is exposed as `globalThis.exact.reload` for the dev loop.
It stamps `data-boot-ms` on the root when the first batch is in the DOM and
`data-paint-ms` on the next animation frame. `index.html` resets only what a
bare `<div>` would not have (`body` margin; `button`/`input` UA styles),
because the kernel's defaults are already CSS's.

## 5. The parity harness (`host/web/src/parity.rs`, `parity.html`, `parity.mjs`)

The browser is the oracle for `exact-motion` (LLP 1002 §5, owed since 1003).
`parity.rs` declares twenty cases — every easing keyword, `cubic-bezier()`,
the four `steps()` jump positions, `linear()` with stops, delay, negative
delay, zero duration, `translate`/`scale`/`rotate`, `all`, the §3.2 reversing
case, a non-reversing interrupt, and a spring — each as a `transition` row
turned into CSS by the host's own emitter (§2), an initial value, and a script
of target changes and sample times. `parity.html` runs them in a real browser
by seeking each transition with `Animation.currentTime` — the same operation
the engine's clock is (LLP 1002 D3) — and writes what `getComputedStyle`
reports; the spring case plays the engine's lowered frames through
`Element.animate` exactly as the glue does, sampled on the grid and between
grid points. `parity.mjs` serves the page to headless Chrome, writes
`host/web/tests/fixtures/browser-motion.txt`, and runs the check.
`host/web/tests/parity.rs` holds the engine to the fixture within `1e-3`
(computed style serializes to about six digits) with no browser in the loop,
under `cargo test --workspace`. Recorded 2026-08-28 from Chrome 151: **105
samples, 0 disagreements.** One thing the recorder learned: a transition CSS
has cancelled must not be paused by script, or it revives as a plain animation
that sorts after — and overrides — the one that replaced it.

## 6. The dev loop (`host/web/src/dev.rs`, `dev.mjs`, `dev.js`)

LLP 1004 D5 taken literally: an edit yields a new plan; the page restarts
from it. `exact_web::dev::Session` watches one `.contract` file (a stat every
10 ms; a save with identical bytes is not an edit), compiles, bakes against
the app's data source, and writes the plan atomically. The app's `dev` bin
(`apps/caltrain/web/src/bin/dev.rs`, one line) runs it as a resident process;
`node host/web/dev.mjs` runs that, serves `dist/` with `dev.js` added to the
page, pushes each ready plan over server-sent events, and prints the numbers.
`dev.js` fetches the plan and calls `exact.reload(bytes)` → `exact_boot_plan`:
a full teardown and a boot from initial state, no migration, no patch
format. A compile error is pushed to the page as an overlay with its line and
column; the last good plan stays. Measured (`scripts/metrics.mjs`, five runs):
**save → plan ready 8–13 ms** (compile 0.5–1 ms, bake 0.5–1 ms, the rest the
poll), **save → the new plan's first frame in the DOM 18–20 ms**. The
compiler's ≤20 ms slice (1004 D5) holds with no incremental compilation at
this size. The cold path — `node host/web/build.mjs`, a cargo build of the
app crate — is 6 s and is no longer the loop.

## 7. Building and measuring

`node host/web/build.mjs` — `cargo build --lib --profile web --target
wasm32-unknown-unknown` for the app's crate (the `web` profile is release with
`opt-level = "z"` and fat LTO: the runner's work is sub-millisecond, so every
byte is fetch, parse, and compile), then `wasm-opt -Oz` when binaryen is on
PATH (the build says so when it is not, and ships unoptimized), then
`host/web/dist/` (ignored by git): `app.wasm`, `index.html`, `glue.js`.
`node host/web/smoke.mjs` — serves `dist/` and renders it in headless Chrome,
asserting the app's landmarks and printing the boot stamp. `node
scripts/metrics.mjs` — every number in this document in one run (~10 s;
diagnostic, never blocking).

**Where the bytes are (2026-08-28, 348 KiB; 149 KiB gzip).** Measured from the
name section of an unstripped build: std/core/alloc ≈ 43% (string and slice
helpers, `core::fmt`, float print and parse for `px` values and JSON), kernel
16%, runner 15%, web host 8%, plan decoder 8%, motion (the `transition`
parser) 3%, hashbrown 2%, plus ≈ 60 KiB of data (the baked plan and strings).
**Taffy is 0%**: nothing on the web reaches `compute_layout`, so the linker
drops the layout algorithms; a "kernel without Taffy" feature would save
nothing here. The profile and `wasm-opt` took 436 → 348 KiB (164 → 149 KiB
gzip) with no change to script-start → DOM. The next real cut is in our own
code and in what it asks of `core::fmt`, not in dependencies.

## 8. The `boot` check (`scripts/boot.mjs`)

The fifth check counts. It reads the page, follows static imports
transitively, and fails when any module before first pixel is not the host
glue or comes from `apps/` — the rules file's own row, "App JS executed
before first pixel: none". A count, not a timer. Today: one module
(`host/web/glue.js`), one wasm reference. `dev.js` is added only by
`dev.mjs`, never to `dist/`.

## 9. Not in v1 (and where each is declared)

A text-measurement bridge for the kernel's layout on web (the browser lays
out; the kernel's Taffy layout is not run here — and is dead-code-eliminated
from the wasm, §7 — and `Kernel::with_monospace` is only a placeholder
measurer); gradients, grid, `line_clamp`, `font_family` rows; touch/keyboard
events beyond `click` and `input`; scroll position and focus restoration
across reloads; gestures on the web (`hold`/`observe` with velocity — LLP 1002
D4 — reach no page event yet); `prefers-reduced-motion` (the author's
stylesheet, LLP 1002 §4); a spring interrupted *by an easing* on the same
property (the frames are cancelled and the CSS transition starts from the
computed style at that moment — the browser's rule, unmeasured against the
engine's); state-preserving reload (a later trade against `rules/NOT-DOING.md`
§Runtime, never a silent extension of §6).

## 10. Checks that hold this

`host/web/tests/host.rs`: the first batch creates the whole tree with CSS
from the rows; later batches carry only what changed and refusals report
without ops; every row family lowers by name; the `transition` row lowers to
CSS and a spring is named; a `transition` authored in Contract reaches the
page as CSS and toggles with state. `host/web/tests/springs.rs`: §3.
`host/web/tests/parity.rs`: §5, against the recorded fixture.
`host/web/tests/dev.rs`: §6 — a save builds, an identical save is nothing, a
broken save is a named refusal and the last plan stays, the bridge boots from
bytes. `motion/tests/spring.rs`: the engine's lowering is the closed form on
the grid, bit for bit, for scalars and pairs. `scripts/boot.mjs` green; `node
host/web/smoke.mjs` and `node host/web/parity.mjs` green in headless Chrome.
All under the five checks on 2026-08-28 (158 tests across the workspace).
