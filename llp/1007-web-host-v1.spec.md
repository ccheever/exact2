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
instantiate 404 KiB of wasm — 172 KiB gzip — boot, ~200 elements); **edit
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
`children` (only when the ordered list changed), and `animate` for
any spring the commit released (§3). The receipt's `touched` excludes created
keys (the kernel's contract), so each list is walked once: a batch costs time
linear in the nodes it names (a 10k-row restyle, 188 → 25 ms natively,
2026-09-23). When one advance collects several receipts,
all surviving elements are created before their final child lists are attached:
an early receipt reads the final kernel tree and can name a later receipt's child.
Springs retain their receipt's due time. `now_ms` is the page's clock
(`performance.now()` from script start), the one clock the runner's timers
and the springs share. A per-view mirror is the memo of what the page has
been told; the kernel stays the one source of truth. A runner refusal comes
back in the batch's `error` with an empty `ops` — the page, like the kernel,
is untouched.

Tags: node type → element (`View`→`div`, `Text`→`div`, or `span` for an
inline run, `ScrollView`→`div[data-scroll]`, `TextInput`→`input`,
`Pressable`→`button`, `Image`→`img`, `Toggle`→`input[type=checkbox]`), refined
by `semanticTag` (`main`, `header`, `nav`, `section`, `footer`, `article`,
`aside`, `dialog`). A text block with `aria-level` 1–6 and no other role is
`h1`–`h6` (deeper: a `div` with `role="heading"`); a `Pressable` is a
`<button type="button">` (an `a` with an `href`). Contract's `button` is a
flex column (LLP 1006 §3; Charlie, 2026-09-23: "One native button, flex
column"): a block `<button>` centers its content in an anonymous box whatever
`all: unset`, `display: flow-root` or `align-content` say (Chrome 153 puts a
20 px child at 40 in a 100 px button), and a flex one lays out as the kernel
does (`kernel/tests/it/browser_cases.rs`). A button holds only phrasing
content, so there a container — a box, a paragraph, a heading, a landmark —
is a `<span>` with the same style, a block unless a row says otherwise; a
button inside a button stays a document refusal. `index.html` resets the
UA heading margins, size and weight, so its box is a div's — Caltrain's 277
and the Markdown reader's 120 boxes are unchanged, and Chrome's
accessibility tree now lists 1 and 6 headings where it listed none
(2026-09-23). The document's `<html lang>` is `app.json`'s W3C `lang`, `en`
when absent (every app here is English; WCAG 3.1.1 wants it determinable),
written into `index.html` and `manifest.json` by the build — every boot starts
from that page. Props → DOM names: `text`→`textContent`, `testId`→`data-testid`,
`accessibilityLabel`→`aria-label`, `accessibilityRole`→`role`,
`placeholder`, `value`, `disabled`, `inert`, `lang`, `imageSource`→`src`; any other
prop rides as `data-<name>` so nothing is lost. `scrollTop` and `scrollLeft`
are explicit DOM-property bindings, applied after the complete batch has mounted
its children and styles. Changes on one axis leave the other alone; clearing a
binding cancels its pending assignment, and unchanged bindings do not override
manual scrolling. A pending offset equal to the DOM’s current value is not
assigned again, so a mirrored scroll event does not restart CSS snapping. A declared `scroll` handler listens to the element's
[non-bubbling DOM event](https://drafts.csswg.org/cssom-view/#scrolling-events)
and reports its actual `scrollLeft` and `scrollTop`
through dispatch kind 13. Initial offset writes and later programmatic changes
use that same browser event; unchanged positions produce no synthetic event.

**Authored inertness** (LLP 1035.001 D3, 2026-09-11): boolean true sets the
real `inert` attribute; false/clear removes it. Navigation combines it with
inactive-route suppression. The browser owns focus, hit-testing and accessibility
exclusion without removing layout. Host ancestor checks stop at an open modal
dialog, after checking that dialog’s own attribute. Rebuilt browser drives pass
explicit modal focus, control inspection, single confirmation dispatch, basic and
active-route input exclusion/restoration, and four Messages selection/deletion/
forwarding cases (`/tmp/messages-inert-ownership/verification.json`).

**Modal confirmation** (LLP 1021 D2, 2026-09-11): `commandfor`/`command`
reach the real DOM under their HTML names. A `dialog` stays hidden until opened;
the browser's `show-modal`/`close` commands own its focus, top layer, input
exclusion and close requests. Authored global keyboard shortcuts do not activate
controls outside the focused modal dialog. Messages has no confirmation-open
slot; outside cancellation preserves selection, first Escape closes confirmation,
and second Escape cancels selection (`/tmp/messages-modal-confirmation/`).

**Symbols** (LLP 1035.004, 2026-09-10) remain `img` leaves. The Rust host
supplies the schema-generated `data-symbol-path` and decorative `alt=""`.
The glue intercepts `symbol:` sources without a network request, supplies a
transparent SVG sized from computed `font-size`, and paints the generated path
as a CSS mask. Font weight changes its stroke; `tint-color` supplies its colour
through `--exact-tint`, including `light-dark()`. Font inheritance changes are
read after each batch. The mask uses the content box and `object-fit`; a fixed
box wins over intrinsic size. An unknown dynamic role has no intrinsic size or
paint and emits a journal refusal. These are generic role drawings, not Apple
artwork. Messages and Fieldnotes exercise the same path.

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
`Skipped { row, reason }`: gradients and grid rows in v1. `line_clamp`
uses the browser's legacy box only for
non-scrolling blocks. On flex, grid, `display:none`, or either scrolling axis,
it is skipped with a reason instead of replacing the authored layout,
visibility, or scrolling. Apply the clamp to a text block inside the container.
`tint_color` now lowers to `--exact-tint`
for symbol images; raster-image tint remains unsupported. An `env()` length (LLP 1001
§2) lowers to its CSS text — `env(safe-area-inset-top)`,
`calc(env(safe-area-inset-bottom) + 12px)` — and the browser resolves it
(2026-08-30). A generic `font_family` Chrome does not know carries its CSS
generic (`ui-monospace,monospace`, `ui-serif,serif`,
`ui-sans-serif,system-ui,sans-serif`, `ui-rounded,system-ui,sans-serif`):
bare, each rendered as Times — the Markdown reader's code, now Menlo
(2026-09-23).

**`transition`** lowers to CSS `transition` — property, duration, easing
(`linear`, keywords, `cubic-bezier()`, `steps(n, jump-*)`, `linear()` with
stops), delay — per LLP 1002 D2. A `spring()` declaration is left out of the
CSS text and reported as skipped by name: springs are §3's.

CSS Exclusions (LLP 1043.000) use an on-demand `textflow.wasm` beside
`textflow-glue.js`. The core host emits eligible exclusion/paragraph IDs;
the glue supplies their computed CSS shapes, boxes, and measured advances.
The leaf artifact owns the shared Rust walker and bounded preparations.
Each host generation gets its own instance; disposal resets its sources and
shape catalog. Ordinary text downloads neither artifact. No core Cargo
feature or app declaration selects this capability.

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

**Router projection (LLP 1038 D5–D7/D11, 2026-09-14).** A commit that
changes the router slot emits one `{"op":"router","top":<id>,"url":"…","removed":[…]}`
beside commands; the runner coalesces changes until the batch drains them.
`navigation.js` keeps that op for `state.navigation.url`. Its session mirror
(LLP 1038 D7, slice 2 lane a) holds `written[]` stamps `{exact:index,id,url}`,
`gone` removed ids and `cursor`; the first op replaces index 0.
Every History API write uses `location.origin + url`, keeping even a
notfound location beginning `//` on this origin as a path. The document
loads `glue.js`; its import loads `navigation.js`, for two boot modules.

| commit | code / history |
|---|---|
| Same top id | `commit`: replace the current stamp only if its URL changed |
| Nearest earlier written top, every intervening id removed | `commit`: `go(-k)`; consume its echo and move the cursor |
| Otherwise, including selecting a retained tab | `commit`: truncate Forward, append and `pushState` |

| popstate | code / dispatch |
|---|---|
| Expected echo | `connect`: consume, then drain the queue |
| One entry Back, matching the route directly beneath the selection | `popped` → `pressBack`: the selected route's enabled Back control, once, shared with Escape |
| Forward, multi-step Back, tab undo or an unwritten entry | `popped` → glue `navigate(location)` |

A traversal's synchronous router op is captured without another history write.
Acceptance selects the expected key (completed pop) or URL (`navigate`), and
stamps the target with the committed id and URL (including a Back action
replacing the revealed entry's URL in the same commit). A commit elsewhere
restores the entry first, then mirrors the changed router through the ordinary
commit table. Refusal restores with `history.go`, consumes
the echo and journals once. Queued popstates and commits wait for that echo;
restoration uses the browser's current position when a later event is queued.
A newer traversal can supersede a pending `go`; the listener reissues its echo
destination from the new position. The mirror's lower bound includes accepted
pre-boot entries at negative indices, so their pushes/pops keep working after
the root's `navigate` handler accepts them.
The browser Navigation API's entry index locates unstamped same-document entries;
where unavailable, Exact stamps supply the index and an unindexed refusal can
only replace the current stamp/URL. Chrome is the parity oracle; stale echo
indices without the Navigation API are not covered. No stack is saved across
a page reload. An in-document reboot keeps the mirror when its first router
op names `written[cursor].id`; a rebuilt router resets the mirror to index 0.
The agent's `tap` history form waits for the traversal and restoration; an
out-of-range browser no-op finishes after its bounded wait.

The glue callback calls `globalThis.exact.navigate(location)` synchronously:
dispatch kind 14 delivers the location to the root's handler and applies its
batch before the mirror checks acceptance. Forward, multi-step Back and tab
undo therefore run the app's chosen verb. A handler with no router change
journals a refusal and restores; a changed router landing elsewhere restores
then mirrors its commit. The browser fixture's `followLink` applies `go` and
verifies all three accepted traversals, a refusal, and a Forward handler
pushing `/other` followed by an in-app Back, and Back after a carried reboot
with no navigate handler. `bun scripts/smoke.mjs web` explicitly runs this
Chrome sweep through `host/web/tests/navigation.mjs`; its Cargo entry is
`#[ignore]`, never a silent pass without Chrome or a built dist.
A disabled completed-pop
control journals `history: Back refused; restoring the entry`, with no press.
Both boot exports receive UTF-8 `location.pathname + location.search` through
the input buffer (after plan bytes for `exact_boot_plan`); module reboot keeps
the current host URL, as it keeps the viewport. A fresh page opens the address
bar's declared chain. `<base href="/">` anchors scripts, assets and module
fetches at the origin root. Agent mode is read from `location`, not the base.
The navigation module also projects direct keyed route children: only the
selected route is interactive; only it and its immediate modal underlay are
visible. An unmatched key leaves the previous projection alone and journals
once per key (1035.001 D6).

**URLs that navigate (2026-09-23).** One allowlist, `navigableURL` in
`navigation.js` — http, https, mailto and tel, read by the browser's own URL
parser — gates every `href` `applyProps` writes (an authored `link`, an inline
run bound to data), a Markdown link's `href` and an iframe's `src`, and the
`openURL` command. A refused `href` is not written; a refused iframe shows
`about:blank`; both log a warning. The router sweep (`tests/navigation.mjs`)
holds a literal `javascript:` link, a `java\tscript:` run and iframe bound to
data to it.

LLP 1039 adds `exact_resize(width, height, now_ms)` to the original six
buffer/boot/event exports; later font, agent, store, delivery and module exports
also remain, so six is no longer the ABI’s total. The glue passes `innerWidth` and
`innerHeight` at boot and every window `resize`, without debounce; the new export
returns the re-answer’s batch through the same host-owned buffers, with no `unsafe`.

**Requests (LLP 1016 D2, built 2026-08-30).** The browser is the executor.
A batch carries `{"op":"grants","lines":[…]}` once at boot — the data
crate's `net.fetch <origin>` lines — and `{"op":"request","ticket":N,
"target":…,"method":…,"url":…,"headers":[[k,v]…],"body":"<base64>","cache":
"default"|"reload"}` for every request the runner handed out with the commit
(`Runner::take_requests`, after the `command` ops). The glue refuses a URL
outside the grants itself (the same `Refused` as the native hosts), else
`fetch(url, {method, headers, body, cache})` — the browser's own HTTP-cache
semantics, `reload` only for a `refresh` — and brings the outcome back on
the main thread through `exact_fulfill(ticket, kind, status, hlen, blen,
now_ms)`: `kind` 0 a response of any status, 1 a rejected fetch (a dead
network, and a CORS refusal too — the browser gives no status), 2 refused by
grant, 3 unsupported, 4 aborted; the input buffer holds `hlen` bytes of
`name: value` header lines then `blen` bytes of body (or the message). The
batch it returns is the reply's commit, applied like any other — or empty
for a ticket the runner no longer holds. The fetches in flight are a set the
agent's `clock settle` (§4 of LLP 1012) awaits before measuring its fixed
point, so `exact.agent` returns a promise for `clock` and the driver awaits
it. Forbidden request headers (`Cookie`, `Host`, `Origin`, …) are dropped by
`fetch` silently where ibex2 sends them: a source must not rely on them.

Core exports, no `unsafe`: `exact_in(len)` resizes a host-owned input buffer
and returns its address; `exact_out()` returns the output buffer's;
`exact_boot(width, height, launch_len)`, `exact_boot_plan(len, width, height, launch_len)` (boot from plan bytes in the input
buffer — the dev loop's restart, §6), `exact_dispatch(view, kind, len,
now_ms)`, `exact_advance(now_ms)` each return the output's length. The glue
writes a UTF-8 payload into the input buffer and reads a UTF-8 JSON batch from
the output. `exact_web::host!(DataType, PLAN)` instantiates the exports for
one app; `apps/caltrain/web` is that one line plus a `build.rs` that compiles
and bakes `app.contract` into `OUT_DIR` (never committed) for
`include_bytes!`.

The fixed-height list path also exports `exact_list(view, top, height,
origin, focus, interaction)`. Glue supplies measured scrollport geometry
and bounded descendant pins; the runner updates row lifetimes without
resource settlement or application actions (LLP 1010 §6.2). Resize and
scroll remain browser-owned. This ABI addition requires a rebuilt wasm
and matching glue, covered by the normal build artifact receipt.

`glue.js` is host code: it fetches and instantiates the wasm, applies batches
(elements by view id; `children` reorders in place so keyed rows keep their
elements and state; `animate` plays or cancels a spring), attaches
`click`/`input`/`pointerenter`+`pointerleave`/`focus`/`blur`/`keydown`
listeners only where a node has that handler (`hover` is the pointer pair,
`key` sends `e.key`'s name, `submit` is `keydown` Enter on an input with
the default prevented — the web's implicit submission, no form; a node with a `focus`/`blur`/`key` handler that is
not an input or button gets `tabindex="0"`, since only a focusable element
receives those), and — when the plan
has timers — calls `exact_advance` on a 250 ms interval. `boot(bytes?)` tears
the page down (interval, animations, elements) and boots from the baked plan
or from bytes, and is exposed as `globalThis.exact.reload` for the dev loop.
It stamps `data-boot-ms` on the root when the first batch is in the DOM and
`data-paint-ms` on the next animation frame. `index.html` resets only what a
bare `<div>` would not have (`body` margin; `button`/`input` UA styles),
because the kernel's defaults are already CSS's.

The textarea keeps its user-agent long-word wrapping (`overflow-wrap: revert`)
through that reset. Previously `all: unset` made an unbroken draft scroll
horizontally even with `white-space: pre-wrap` and `field-sizing: content`.
Six draft comparisons against a plain textarea in an unstyled document hold
the restored behavior (`/tmp/messages-wrapping/web-ua-comparison.json` and
`web-corrected-inspection.json`). The compiler now also declares `break-word`
on its textarea tag. `overflow-wrap` is a schema row, projected as CSS and
included in computed-style observations. Messages explicitly requests
`break-word` on bubble text: an unbroken message now wraps instead of overflowing
its border box (`/tmp/messages-overflow-wrap/`).

An explicit `retainFocus` ancestor prevents pointer-down focus changes on
non-editable content, including buttons and passive sheet headers. Inputs,
textareas, selects, and contenteditable targets keep their normal focus behavior.
This matches the native unhandled-touch retention policy; an authored focus
command can still transfer focus.

**The page's environment (2026-08-30).** The viewport meta follows the first
root's `viewportFit` and `interactiveWidget` props (`syncViewportFit`, on
every `roots` op and on a change of either): `cover` appends
`viewport-fit=cover`, so the page lays out under a phone's status bar and
home indicator and the CSS's `env(safe-area-inset-*)` carry the insets;
`resizes-content` appends `interactive-widget=resizes-content`, so Chrome
shrinks the layout viewport to the keyboard (Safari knows only the default)
— Safari re-reads the meta when its content changes. The keyboard is the
browser's: by default the layout viewport stays, the visual viewport
shrinks, the focused field is scrolled into it. The
agent's `layout` reports both as `env` (LLP 1012 §1): the insets read off a
hidden element padded by `env()`, `keyboard-inset-height` as `innerHeight`
less the visual viewport's height (zero on a desktop).

The `contextTarget` presentation policy (LLP 1001 §1, 1008 §9) finds the nearest
absolute ancestor of the declared preview. It magnifies the existing content
with a CSS transform (15%, at most 26 added points of width), leaving its text
layout intact. Following siblings move by half the added height before panel
alignment, preserving their source-relative position after the panel moves up.
`contextMagnify=false` disables host enlargement and its extra-height
offsets while preserving source placement, clamping and authored transforms.
Otherwise the default magnification applies.
Top-aligned immediate side siblings move horizontally to preserve their gap to
the enlarged preview edge; zero-height side slots keep the balloon's original
percentage-width basis and the row's height.
Clamping includes the farther extent of the enlarged preview or following
controls. The vertical clamp intersects the viewport with the panel's containing
block, so an authored region can reserve space for a participant popover.
Source-edge alignment and viewport clamping use painted rectangles;
fractional CSS `top` values stay fractional across updates. This separate panel
projection leaves the authored individual transform rows alone and resets before
recomputation. The controls remain live DOM content, including focus retention,
reaction-strip scrolling and outside dismissal.

The preview captures its source rectangle in root coordinates before its entry
batch can change focus. Height changes retain that rectangle; width changes
recapture it after layout. The source's vertical scroll contents receive the
same presentation displacement as the clamped preview. If the scroll region
itself moved (a centered reply thread after keyboard dismissal), its clip retains
its entry position too. These transforms reset before each projection and on
dismissal; they do not write authored scroll offsets. Projection follows batch
focus/scroll settlement and ResizeObserver settlement, so a later end-follow
adjustment cannot leave the source behind. The cache ends when the preview or
source disconnects or the target changes.

The `copyText(text)` command accepts exactly one string and starts
`navigator.clipboard.writeText` synchronously during batch dispatch, retaining
the browser's user-activation context. Its promise joins the existing in-flight
set so driver settlement can wait for the write. Invalid arguments, an absent
secure-context clipboard API, and rejected writes are logged as errors; there
is no fallback that selects text or moves focus. It never reads the clipboard.

`selectText("html-id")` uses the same post-batch target and eligibility checks
as `focus`, then focuses the input/textarea and calls its native `select()`.
It selects the complete value, including UTF-16 surrogate pairs, and reads no
clipboard. Read-only editors remain selectable; their own pointer, double-click
and context-menu behavior takes precedence over a containing reply/Tapback
handler. A non-editor target is refused in the journal.

An input with `emojiPicker=true` carries that explicit DOM policy attribute.
Its input handler waits for composition to finish, clears the field, and sends
`change` only for a single emoji grapheme, using `Intl.Segmenter` and the gate
in LLP 1001 §1. It does not open an OS emoji panel. The browser receives no
search text from a native keyboard's separate search field.

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
from it. A refused save reports every independent refusal (at most 20, as
`contract build` does) in the overlay and the server's output, whichever
producer compiled it — the resident compiler, `js/bake` or the Rust
producer's `exact-logic-bake` (2026-09-23). A page the dev server serves
names its current generation (`<meta name="exact-dev-generation">`), and its
first boot is that generation — `dev.js` fetches and verifies it when
`glue.js` asks (`devFirst`) — never the plan baked into `app.wasm`, which a
fresh load of any URL, a deep link included, used to show until the dev
client caught up (2026-09-23). A module client prepares that generation's
module after paint; a Rust module still arrives as an update. Without a
generation, or with none in 5 s, the page boots the baked plan. `exact_web::dev::Session` watches one `.contract` file (a stat every
10 ms; a save with identical bytes is not an edit), compiles, bakes against
the app's data source, and writes the plan atomically. The app's Cargo-default dev bin
(`apps/caltrain/web/src/bin/caltrain-dev.rs`, one line) runs it as a resident process;
`bun host/web/dev.mjs` runs that, serves `dist/` with `dev.js` added to the
page, pushes each ready plan over server-sent events, and prints the numbers.
Each in-repo adapter has an app-specific executable name so parallel workspace
builds never overwrite another app’s `dev` output. Caltrain selects its adapter
with `default-run` because it also has a metrics binary; single-binary packages,
including external apps, use Cargo’s ordinary default selection.
`dev.js` fetches the plan and calls `exact.reload(bytes)` → `exact_boot_plan`:
a full teardown of the page and a boot of the new plan **carrying the old
runner's state** (`Runner::carry` / `Runner::boot_carrying`, `Host::boot_with`):
each slot by name where its carried value conforms to the slot's — possibly
new — type, else its initializer; each settled resource by name where its
value still fits the declared shape, reused only where its source name,
arguments and data-module identity still match (a carried `stationId` gets its
own board, and the plan's baked boot values are never taken over carried state);
the clock, so timers continue. Carried store dependencies are source-qualified too;
kept answers for removed or redirected resources (or changed data logic) are
forgotten during reload, including their persisted entries, so a pending answer
cannot restore a previous source's seed on a later reload. Cold-launch kept-answer
matching remains the name/arguments/shape rule of LLP 1027 D4.
Carried state is never why a boot fails: what no longer fits starts fresh.
The tree, ids, derives, and the DOM are rebuilt — five screens deep stays
five screens deep, but scroll, focus, and a spring in flight do not survive
(identity matching between the old and new trees is the later trade, and
`rules/NOT-DOING.md` §Runtime records this one). No patch format, no
generations. A compile error is pushed to the page as an overlay with its line and
column; the last good plan stays. Measured (`scripts/metrics.mjs`, five runs):
**save → plan ready 8–13 ms** (compile 0.5–1 ms, bake 0.5–1 ms, the rest the
poll), **save → the new plan's first frame in the DOM 18–20 ms**. The
compiler's ≤20 ms slice (1004 D5) holds with no incremental compilation at
this size. The cold path — `bun host/web/build.mjs`, a cargo build of the
app crate — is 6 s and is no longer the loop.

**The Rust side (2026-08-30).** `dev.mjs` also watches the crates the wasm
is built from — `kernel`, `plan`, `motion`, `runner`, `host/web`, `gpu`,
the vendored Taffy, and the app's `data`, `web`, and `gpu` crates (Node's
own recursive `fs.watch`; `.rs`, `.toml`, `.json`, `.wgsl`, `.js`,
`.html`; `target/` and `dist/` skipped). An edit there, debounced 200 ms,
runs the same warm build (`host/web/build.mjs`), restarts the resident
compiler — its plans must match the new format — and pushes `rebuilt`:
the page **reloads** rather than restarts in place, since a new wasm is a
new program and no state carries across it. A build that fails shows its
errors in the page's overlay, as a contract that fails does, and the page
keeps the last good wasm. No bundler, on purpose: there is nothing to
bundle (no app JS, `rules/NOT-DOING.md`), and the day a JavaScript bundle
exists it is one more built artifact this watch reloads — a bundler then
is a build step, not the loop. The native apps take the plan push already
(LLP 1008 §5, §9); a Rust edit there is a new binary, `build.mjs --run`.

## 7. Building and measuring

**An app outside this repo** (2026-08-30; weird-castle, `~/projects/weird-castle`,
consuming exact2 by path from `../exact2`): `scripts/app.mjs` `resolveApp` is
where every script learns what an app is — `apps/<name>` here, or the directory
`EXACT_APP_DIR` names, with its own cargo workspace and its own `target/`.
`exact new <path>` generates that workspace (LLP 1036.001 D2): the crates.io
patches, the toolchain and the lock come from this checkout, and `resolveApp`
checks the patches and toolchain on every run. exact2's profiles are injected,
not copied (D1). `build.mjs`, `dev.mjs`, the Apple
`build.mjs`, and `scripts/agent.mjs --app` resolve through it; `dist/` and the
Swift products stay this repo's one slot per host, last build wins. The app's
`exact.mjs` sets `EXACT_APP_DIR` and calls these scripts unchanged. Diagnostics resolve the
same app: `metrics --app` (including `--rebuild` and `--long`) and
`smoke deploy --app` capture the complete working source graph through the
existing deploy snapshot before editing anything. Their builds, web output,
Git state, signing keys, and update state are private to the invocation and
removed on success or failure. External app locks must already describe that
graph; capture does not rewrite the live lock. Metrics report source identity,
verify a changed visible text after dev DOM acceptance, and label `--scaling`
as the fixed Caltrain runner workload, independent of app selection.

`bun host/web/build.mjs` — `cargo build --lib --profile web --target
wasm32-unknown-unknown` for the app's crate (the `web` profile is release with
`opt-level = "z"` and fat LTO: the runner's work is sub-millisecond, so every
byte is fetch, parse, and compile), then `wasm-opt -Oz` with
`--one-caller-inline-max-function-size 20 --converge` when binaryen is on PATH.
This keeps small helpers inline while avoiding large expansions that make
the compressed download larger despite shrinking the raw wasm (2026-09-24:
bound 20 with convergence ships about 3 KB less Brotli per app than bound 50
alone, for about 0.1% more raw bytes and about 2.5 s more wasm-opt). The build
says when binaryen is absent and ships unoptimized. The output is
`host/web/dist/` (ignored by git): `app.wasm`, `index.html`, `glue.js`.
The production build minifies the host JavaScript with the pinned Rolldown.
For TypeScript apps it also bundles the module loader's stateless storage
wrapper, removing one request dependency. The filesystem and SQLite adapters
remain shared modules, so TypeScript and Rust requests keep the same filesystem
mutation queues. Development still serves the original modules.
`bun host/web/smoke.mjs` — serves `dist/` and renders it in headless Chrome,
asserting the app's landmarks and printing the boot stamp. `node
scripts/metrics.mjs` — every number in this document in one run (~10 s;
diagnostic, never blocking).

**Serving (2026-09-23).** `bun host/web/serve.mjs` (a build, or `--origin`)
makes Brotli (quality 11) and gzip variants of each wasm, JS, CSS, HTML,
JSON and WGSL body once per digest, off the request path — it warms the
served tree at startup; a request before that gets identity bytes — and
answers `Accept-Encoding` with `Content-Encoding` and `Vary`. Every 200
carries a digest `ETag` (one per representation) and answers
`If-None-Match` with 304. Content-addressed files are `immutable`; the
update protocol's `.exact/` heads and pointers `no-store`; everything else —
the page, install pages, a build's canonical files — `no-cache`. The dev
server and the drivers' servers send identity bytes. `index.html` preloads
`app.wasm` (`as=fetch crossorigin`: `fetch`'s cors/same-origin, the same
URL) and `navigation.js` (`modulepreload`), so neither waits for glue.js to
run. Caltrain's first load in headless Chrome at 150 ms RTT and 10 Mbps,
five cold loads each (load 100–140): FCP 1,864 → 760 ms; bytes before FCP
1,449 → 457 KB, all 1,888 → 611 KB; `app.wasm` requested at 617 → 173 ms,
by the preload, once per load.

**Where the bytes are (2026-08-28, 404 KiB; 172 KiB gzip).** Measured from the
name section of an unstripped build: std/core/alloc ≈ 59% (string and slice
helpers, `core::fmt`, float print and parse for `px` values, JSON, and spring
frames, and the `BTreeMap`/`BTreeSet` instantiations the engine brings),
kernel 10%, runner 10%, plan decoder 7%, web host 4%, hashbrown 3%, motion 3%,
libm 1%, plus ≈ 70 KiB of data (the baked plan and strings). **Taffy is
~1%** (tree bookkeeping only): nothing on the web reaches `compute_layout`,
so the linker drops the layout algorithms; a "kernel without Taffy" feature
would save nothing here. The `web` profile and `wasm-opt` took 436 → 348 KiB
(164 → 149 KiB gzip) with no change to script-start → DOM; springs (§3) then
added the engine and its std instantiations, 348 → 404 KiB. The next real
cuts are in our own code — the engine's maps could be vectors — and in what
it asks of `core::fmt`, not in dependencies. `parity` and `dev` are not in
the wasm.

## 8. The `boot` check (`scripts/boot.mjs`)

The fifth check counts. It reads the page, follows static imports
transitively, and fails when any module before first pixel is not the host
glue or comes from `apps/` — the rules file's own row, "App JS executed
before first pixel: none". A count, not a timer. Today: two host modules
(`host/web/glue.js`, `host/web/navigation.js`), one wasm reference (LLP 1038 D7). `dev.js` is added only by
`dev.mjs`, never to `dist/`.

The count enforces an import boundary, not the content of an allowed file
or its execution cost. Since 2026-09-04, `boot --json` also reports reachable
source bytes and file digests without a new blocking budget. `metrics --json`
records built artifact identities (including wasm), observed runtime fetches,
long tasks and browser timing. DOM readiness, nonempty text, first paint,
contentful paint and the first CDP action are distinct observations; absent
Paint Timing entries remain unmeasured. The host's `frameCallbackMs` stamp
names a pre-paint callback and is never treated as proof of presentation.
GPU scheduling uses two rAFs to permit a rendering opportunity; actual
resource timing determines the measured load phase. See the
[boot measurement correction](../issues/closed/20260904-boot-count-misses-in-module-growth.md).

## 9. Not in v1 (and where each is declared)

A text-measurement bridge for the kernel's layout on web (the browser lays
out; the kernel's Taffy layout is not run here — and is dead-code-eliminated
from the wasm, §7 — and `Kernel::with_monospace` is only a placeholder
measurer); gradients, grid, `line_clamp`, `font_family` rows; pointer
coordinates and moves (a drag), `keyup`, double-click, wheel offsets reaching
the runner (a hover is enter/leave, a key is `keydown`; LLP 1005 §3); scroll
position and focus restoration
across a reload (the tree is rebuilt; §6); gestures on the web (`hold`/`observe` with velocity — LLP 1002
D4 — reach no page event yet); `prefers-reduced-motion` (the author's
stylesheet, LLP 1002 §4); a spring interrupted *by an easing* on the same
property (the frames are cancelled and the CSS transition starts from the
computed style at that moment — the browser's rule, unmeasured against the
engine's); reusing DOM nodes across a reload by identity (a later trade
against `rules/NOT-DOING.md` §Runtime, never a silent extension of §6).

## 10. Checks that hold this

`host/web/tests/host.rs`: the first batch creates the whole tree with CSS
from the rows; later batches carry only what changed and refusals report
without ops; every row family lowers by name; the `transition` row lowers to
CSS and a spring is named; a `transition` authored in Contract reaches the
page as CSS and toggles with state. `host/web/tests/springs.rs`: §3.
`host/web/tests/parity.rs`: §5, against the recorded fixture.
`host/web/tests/dev.rs`: §6 — a save builds, an identical save is nothing, a
broken save is a named refusal and the last plan stays, the bridge boots from
bytes and carries state across boots, a slot carries where its type still
fits and starts fresh when it changed or was renamed.
`apps/caltrain/tests/app.rs`: a reload keeps its station and clock on an
edited, unbaked plan and re-requests nothing whose arguments did not change. `motion/tests/spring.rs`: the engine's lowering is the closed form on
the grid, bit for bit, for scalars and pairs. `scripts/boot.mjs` green; `node
host/web/smoke.mjs` and `bun host/web/parity.mjs` green in headless Chrome.
All under the five checks on 2026-08-28 (158 tests across the workspace).

## 10. The store (LLP 1018, as built 2026-08-30)

Before `exact_boot`, `glue.js` reads every `exact.secret.<name>` key of
`localStorage` and hands the pairs (NUL-separated) to `exact_store(len)`; the
runner keeps the granted names as its snapshot, so a resource that reads the
store answers on the first frame. A `{"op":"store","tier":"secret","name":…,
"value":…|null}` op, emitted after a commit like `command`, sets or removes the
key. Agent mode (`?agent=1`) reads and writes nothing: a drive starts from
nothing and leaves nothing. A dev reload carries the running store
(`Carried::store`) rather than re-reading the page's.

The production directory origin uses `serve.mjs --origin <dir>` (LLP 1030.000 D3):
one atomic inventory pointer selects immutable release files. The index binds
its base URL to that release, the native envelope names immutable payload URLs,
and Exact's local absolute image/font/deck URLs bind to the same base. Native
stream blobs and web release paths receive immutable cache headers; canonical
aliases, pointers, and removals are no-store. Local build/dev serving retains
its unversioned paths. The agent's HTTP carrier calls the same static handler.

**Modal routes** (2026-09-09, Messages): the selected keyed route's
`navigationPresentation="modal"` keeps its preceding route visible and inert
behind the authored overlay. An isolated pseudo-element supplies the host's
20% black backdrop; the app does not duplicate UIKit's presentation dimming.
Escape invokes the root's named, enabled Back control within the selected route
unless the modal's `closedby` is `none`; explicit Close remains available.
An already-prevented key is not reused for navigation. While the document has an
open modal dialog or auto/hint popover, the modal handler leaves Escape to the browser, including
when `closedby="none"` protects the underlying sheet. Manual popovers do not
consume close requests. Messages' discard confirmation, retained form focus and
draft, subsequent sheet refusal, and dismissible Compose cases are verified in
`/tmp/messages-popover-escape/` (2026-09-11); authored `key` handlers remain separate.
The iOS projection uses UIKit's sheet and its local keyboard viewport (LLP 1008).
The route-policy props reach the DOM, including updates to the close policy.
`navigationPresentation="fullscreen"` and `navigationSource` preserve the selected
route and source-id intent in the authored viewport (2026-09-11, LLP 1035.001 D4).
Escape also reaches a fullscreen owner when inerting the previous editor leaves
focus on the page body; a focused host input outside Exact keeps its keys. The
rebuilt Messages flow, outside-input refusal and nested `closedby="none"` pass in
`/tmp/messages-fullscreen/r5/`. This does not implement CSS View Transitions.

**`layout <node>`** (2026-09-09, LLP 1035.002 D1): `layout` with an `id`
adds `node` — the runner's rows and sources (the wasm's `node` message)
merged with what the page knows (`glue.js` `nodeDetail`): the box in the
viewport, the client box, `devicePixelRatio` as the capture scale, the
scroll chain (the page first, then `data-scroll` ancestors), the ancestors
whose computed `overflow` or `clip-path` clip, `hidden` from
`checkVisibility`, authored `inert` from ancestor attributes up to and including
an open modal dialog (implicit document-wide modal inertness is not yet reported),
in-viewport and clipped-away from the rects — and `browser`, the browser's computed value
of every inherited row (`color`, the font rows, `line-height`,
`letter-spacing`, `text-align`, `direction`, `white-space`), the oracle
beside the kernel's answer. No window or screen space is reported: the
page has none.

**`state`** (2026-09-10, LLP 1035.002 D2): the glue appends `focus` from
`document.activeElement` (the view's id, the editor when it is an input or
textarea, the element's tag as the responder), `keyboard` from
`visualViewport`'s height against `innerHeight` with the root's
`interactiveWidget` as the policy, and `navigation` from the
`[navigationBack]` container's routes (the stack is the prefix through the
route it names; `modal` and `closedby` from the selected route's
attributes); the browser has no interactive pop, so the transition is
always `idle`. The glue tags its `layout` and `clock` replies with the
runner's `epoch`/`incarnation`/`clock` (D3, `tagged`); the driver tags the
input and capture it delivers through CDP.

### CSS line height (LLP 1035.000.000, 2026-09-11)

The typed line-height row emits a bare ratio, a `px` length, or `normal`.
Ratios remain ratios in CSS and agent row inspection; the browser computes
inheritance against each element's font. Zero emits `0` or `0px`, never
`normal`. Literal HTML/CSS counterparts cover inherited 1.5, fixed 24px,
zero and a paragraph with smaller inline children.
