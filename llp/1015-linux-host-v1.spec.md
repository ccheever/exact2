# LLP 1015: Linux host v1 — what `host/linux` is, as built

**Type:** Spec
**Status:** Draft (unreviewed; transcribes what landed 2026-08-29)
**Systems:** Linux host (new), Kernel (one default corrected), Tooling (`scripts/agent.mjs` — a third carrier; `scripts/smoke.mjs`)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Implementer:** Claude (Fable 5), landed 2026-08-29 (this document transcribes the landing)
**Related:** LLP 1008 (the Apple host whose orchestration this repeats without its batch), LLP 1007 (the web host; the parity oracle), LLP 1001 §5–6 (layout is a host call; text measurement is injected), LLP 1002 D2/§4 (every host but the web runs `exact-motion`), LLP 1010 (scrolling — §3's chaining rule, implemented here in Rust), LLP 1011 §4 (the image policy, repeated), LLP 1012 (the agent API — this host implements its contract, not `Agent.swift`), LLP 1014 D2/D4 (children over a canvas; a painter host as the display list), LLP 1009 (the GPU module this host does not load yet), `QUEUE.md` §Open decisions ("Linux painter: wgpu or CPU raster" — the take v1 makes, §7), `rules/RULES.md` §Time budgets, `rules/NOT-DOING.md` §Surfaces. Research, never authority: exact1's 0418/0430 (one text engine measures and paints), 0323 (the measurement cache).

## Summary

`host/linux/` is the third host and the first that **paints**. The runner
and kernel run natively; after every commit the host lays the tree out
with the kernel's own layout (Taffy, measuring text with cosmic-text),
seeks the motion engine to the app's clock, and the painter draws the
kernel tree itself with tiny-skia — **the kernel is the display list**:
no batch, no mirror, no view tree of the platform's, because Linux offers
none. The pixels go to DRM/KMS dumb buffers with evdev input, or into a
buffer with no display at all — which is how the host is tested: the
agent API over stdio, a screenshot, a smoke run, on a fleet Linux box with
no GPU or on macOS in the seconds-loop. Pure Rust end to end (tiny-skia,
cosmic-text, png, drm-rs, evdev): no system library is linked, so it
builds on any Linux with no packages and on macOS as a headless binary.
Measured on `expo-build-1000` (EPYC 9454, 43 font faces), the Caltrain app
in smoke mode, warm: **process start → first frame painted 20 ms** — fonts
3.7 ms, runner + layout + 878 text measurements (498 from cache) 15.6 ms,
paint 4.9 ms at 420×860. The same binary's `layout`, `tap`, `type`,
`clock`, and `screenshot` answer `scripts/smoke.mjs linux` unchanged from
the other two hosts, and its nested-scroll numbers are the macOS ones
exactly (the node stops at 652, the page at 471). Where this document and
the code disagree, the code and its tests are the authority.

## 1. The host (`host/linux/src/host.rs`)

The Apple host's orchestration (LLP 1008 §1) with the batch deleted:
`Host::boot(plan_bytes, data, measurer, width, height)` decodes the plan,
boots the runner against `Kernel::new(measurer)`, hears the whole tree in
the engine (values, no transitions), lays the roots out under
`Offer::definite(viewport)`. `dispatch_at(view, event, now_ms)`,
`advance(now_ms)` (through `Runner::advance_timed`: each timer's commit at
its own due time, the clock landing where the runner says — LLP 1012 §2),
`resize`, `set_intrinsic`, and `tick(now_ms)` each return an error or
nothing; the painter then reads the kernel. The one thing the host keeps
beside the kernel is each node's **presentation values** — translate,
scale, rotate, opacity from `Engine::frame` — as `Presented`, which
`presented(id)` answers from the map or, for a node the engine has not
moved, from its committed rows. There is no mirror and no diff: a painter
in the same process as the kernel has nothing to keep in step. The
orchestration is ~120 lines and the same shape as `host/apple/src/host.rs`
line for line; lifting it into one crate is a mechanical move that waits
for a third native host (iOS uses the Apple one).

**The root lays out under the viewport and a root that is a block is as
tall as its content**, so the window is a viewport over a document (LLP
1010 §3): the presenter's page scroll offset and `fitDocument`'s extent
(the roots' frames, never smaller than the viewport) are the same rule as
macOS, deviation included.

## 2. The painter (`host/linux/src/paint.rs`)

One walk of the live tree in preorder, into a premultiplied RGBA
`tiny_skia::Pixmap` at the device scale. Per node, in order: **background**
over the border box as a path with per-corner radii (CSS
`background-clip: border-box`); **borders** — a uniform border with a
radius is a stroke inset by half its width, anything else is four side
rectangles (the Apple presenter's rule); an **image** by `object_fit`
(`fill`, `contain`, `cover`, `none`, `scale-down`) in the content box,
clipped to it and to the border box's rounded path (LLP 1011 §4); a
**text** node's paragraph — the one the kernel measured at this width,
§3 — in the content box; an **input**'s value, or its placeholder in
`#757575`, vertically centred, with a caret when focused; then the
**children**, clipped when the node's effective overflow (`style.rs`'s rule
from LLP 1010: a `ScrollView`/`List` scrolls on y unless its row says
otherwise, an unset axis beside a non-visible one is scrollable) is not
`visible` — a `tiny_skia::Mask` intersected down the tree — and moved by
its scroll offset when it scrolls. Motion presentation values become one
transform about the box's centre, CSS's `translate · rotate · scale`, and a
**group opacity**: when it is not 1 the subtree paints into a layer and
composites once (per node, a viewport-sized allocation — fine for a
transition, a cost for a hundred translucent cards). `display: none`
paints nothing. `Canvas`, `Toggle`, `Svg`, `NativeView`, and `Pressable`
are boxes: background, borders, children. Colors are the kernel's
`0xRRGGBBAA` straight into tiny-skia.

**The walk also records every node's painted box** — the transformed
bounding box in viewport points, the clip it was painted under, and its
scroll offset if it scrolls — in paint order. That list is what the
agent's `layout` reports and what hit-testing reads, so there is no second
geometry: a box is where its pixels went. The pointer, when the host draws
one (DRM has no compositor), is an arrow painted last.

**The one kernel change.** `text_color`'s default in `schema.json` was
`4278190335` — `0xFF0000FF`, opaque red in the kernel's packing — and no
host had read it: the web host lowers only set rows and the browser's
default is black, the Apple presenter falls back to black itself. The
first painter to read `style.text_color` directly painted every unset text
red. It is `255` now (`0x000000FF`, the web's black); `Color::BLACK` says
the same. The other rgba8 defaults are transparent and were right.

## 3. Text: one engine (`host/linux/src/text.rs`)

cosmic-text, measuring and painting from one cache — LLP 1008 §3's lesson
in Rust. A `Paragraph` is a width-specific snapshot: the shaped, wrapped
`Buffer`, its size with the same `ceil` the kernel receives, its first
baseline — cached by (spec, width), where a spec is the runs (text, size,
weight, italic, line height, letter spacing) plus alignment and
`line_clamp`. `Measurer` (the kernel's `TextMeasurer`) and the painter
share the engine through an `Rc<RefCell<_>>`: what was measured is what is
painted, by construction. Lines wrap at word boundaries only (`Wrap::Word`,
CSS `overflow-wrap: normal`); **min-content is the widest word**, found by
wrapping at width zero — the first Linux render broke every countdown
"28" into "2" / "8" because the probe used glyph wrapping, so min-content
was one glyph and the flex row shrank to it. `line-height: normal` is the
font's ascent + descent + line gap at the size (skrifa metrics from the
face the shaper picked; cached per size/weight/style); a set line height
centres the glyphs in the box, which cosmic-text does itself (its half-
leading is CSS's). `line_clamp` is `Ellipsize::End(Lines(n))`. Alignment
is per line. An empty text has no line box (the web; the Apple presenter
gives one). Glyphs are rasterized by swash once per (glyph, subpixel bin,
color) into small premultiplied pixmaps, cached; painting is one blit per
glyph at cosmic-text's pixel-snapped position, through the node's
transform when it has one (a scaled node resamples its glyphs, as a
browser does mid-transition).

Fonts are the system's (`fontdb`; `EXACT_FONTS` adds a directory), the
family always `sans-serif`; with no fonts at all the host says so on
stderr and text measures zero. The scan is the one boot cost that is the
machine's, not the app's: **3.7 ms for 43 faces** on the builder, **25 ms
for 787** on this Mac — a font cache is the known answer when it matters.
Measured on the app: 878 requests for 128 text nodes at boot (Taffy asks
several times per node), 498 from cache, 13.5 ms shaping — ~35 µs per
miss, twice CoreText's; the tail is the fixed-point layouts, not the
shaper. Chrome on Linux with the same DejaVu Sans wraps the same lines:
the app's root is 3244 pt tall here against 3246 on macOS, and the
departure rows wrap at 420 wide on both, as `QUEUE.md` §3 noted for macOS.

## 4. The presenter (`host/linux/src/presenter.rs`, `image.rs`)

What a painter holds beyond the kernel, and the operations that touch it.
**Scroll offsets** are host state per scroll container, clamped after
every layout to the content extent (LLP 1010's floor, ported); the page's
offset is the viewport over the document. **A wheel** (`wheel_at`; the
web's sign) goes to the innermost scroll container under the point that
can take its dominant axis, which takes what it can of both axes; else up
the ancestors; else the page — LLP 1010 §3's chaining rule. **A press**
(`press_at`) is a hit at a point — the last painted box containing it,
through every clip — then up to the nearest node with a `press` handler,
the path a click takes in a browser; focus follows the click: an input
takes it, anything else drops it. **Typing** (`type_text`) focuses the
input and dispatches one `change` with the whole value; a key in display
mode appends or backspaces the current value and dispatches likewise.
**`clock`** advances the runner, seeks the engine to where it landed, and
reports both. **Images** (`image.rs`): after every commit the presenter
syncs every image node's `imageSource` — a relative path resolves under
`EXACT_ASSETS` (the current directory otherwise) and must stay inside it;
`http(s)` and any other scheme do not load (the out-of-process resource is
ibex2's, `QUEUE.md` §Later) — and decodes PNG on a thread; a completion
for an older generation or a gone node is dropped; the size reaches the
kernel through `set_intrinsic` and the tree relays out (LLP 1011). In the
headless modes the presenter waits for loads in flight (bounded, 500 ms)
before the first frame, so a screenshot and an agent's first `layout` have
the pictures in them. A **reload** (`EXACT_DEV_PLAN`, display mode) boots
the new plan with state carried (`Runner::carry`) and starts scroll,
focus, and pictures over, as LLP 1007 §6 says.

## 5. The agent API (`host/linux/src/agent.rs`) and the driver

`EXACT_AGENT=1` is `Agent.swift`'s protocol on stdio, line for line: the
ready line, then JSON requests in and replies out. `tree`, `state`, `logs`,
and `settle` go to the host (`exact_runner::agent`); `layout` is the
painted boxes in ascending id with `sx`/`sy` on scroll containers, two
decimals; `tap` presses at the box's centre through `press_at` (a wheel
with `wheel: [dx, dy]`); `type` replies `typed` and `value`; `clock` is
LLP 1012 §2's fixed point — advance, seek, `settled: true` when nothing is
in flight, sixteen rounds then `settled: false`, a backwards time refused,
a timer's refusal returned with where the clock landed; `screenshot` is
the frame as a PNG (viewport points in the reply, not pixels). Pending
image loads are drained before every request. `scripts/agent.mjs` gained
the third carrier — `openStdio`, the macOS carrier with the binary and
the size in the environment (`EXACT_LINUX_BIN`, `EXACT_SIZE`) — so
`node scripts/agent.mjs linux …` and `node scripts/smoke.mjs linux` run
wherever `target/release/caltrain-linux` was built, macOS included. The
smoke's canvas reference picture (`scripts/fixtures/canvas-sky.linux.png`)
was recorded on the builder, where the fonts are DejaVu's; on a Mac the
`linux` smoke fails that one step by the font, not the painter (§7).

## 6. The display (`host/linux/src/display.rs`, `input.rs`; `app.rs`)

`exact_linux::run::<Data>(PLAN)` reads the environment once (`app.rs`
lists it) and runs one of three ways. **Headless** — `EXACT_AGENT=1`,
`EXACT_SMOKE=1`, or `EXACT_SHOT=<png>`, and always on macOS — boots under
`EXACT_SIZE` (420×860 by default) at `EXACT_SCALE` and serves, prints the
phases, or writes the frame. **The display** (Linux, otherwise) opens
`EXACT_DRM` (`/dev/dri/card0`), asks for master, takes the first connected
connector at its preferred mode and the CRTC behind its encoder, creates
two XRGB8888 dumb buffers with framebuffers, shows the first frame with
`set_crtc` and every later one by a page flip whose event it waits for —
so the loop is paced to the display's refresh and frames are painted only
when something changed. Input is evdev read directly: every device with
relative axes or a keyboard's keys, non-blocking, polled with the timers;
pointer motion (device pixels over the scale), the primary button (a
press on release inside the same box as the down), wheels (coarse and
high-resolution, folded), keys through a US keymap into the focused
input. Timers fire every 250 ms while the runner has any; motion frames
run while the engine is not quiescent; image loads and the dev plan are
polled at 100 ms. `apps/caltrain/linux` is the app: `build.rs` bakes the
plan (the Apple crate's), `main.rs` is three lines; `cargo build --release
-p caltrain-linux` is the build — **20 s cold for the whole tree on 96
cores, 7–11 s to relink after a host edit**.

**Exercised:** the whole headless path on two machines (§8). **Built,
cross-checked (`cargo check --target x86_64-unknown-linux-gnu` from
macOS, clippy clean), and not yet run:** the DRM path and evdev — the
builders' user is in neither `video` nor `input` and has no `sudo`, and
no other Linux box with a VT was in reach this session. The first run
needs a Linux machine with a seat: from a VT, `EXACT_ASSETS=apps/caltrain
target/release/caltrain-linux`.

## 7. Not in v1 (and the trades taken)

**The painter is CPU raster.** `QUEUE.md` §Open decisions asked wgpu or
CPU; v1 takes CPU, for the reasons the question named: it boots with
nothing compiled (the 100 ms budget, "the boot path compiles nothing"),
it runs and pixel-tests on a fleet box with no GPU, and its pixels are
deterministic. The cost the question also named is real: a `canvas` on
Linux is not rendered — the box paints its background and its children
over it (LLP 1014 D2's "over"), and the GPU module is not loaded. The path
to it is the module rendering into a module-owned texture and reading it
back for the painter to composite (LLP 1009 D4's fixture path, `COPY_SRC`),
with lavapipe on a server; a wgpu painter would make LLP 1014's children-
through-the-shader free and is a one-module swap (`paint.rs` and the glyph
blit) if the cold-shader-compile trade is ever taken. Charlie's to
reverse; nothing else in the host cares which.

Also not in v1, each declared: libinput and xkbcommon (pointer
acceleration, touchpad gestures, hotplug, non-US keymaps — evdev reads
raw, and nothing links); Wayland or X11 windows (DRM or headless only);
a cursor blink, selection, IME; `text_decoration`, `font_family` (always
sans-serif), RTL untested; shadows, gradients, grid (as on macOS); JPEG
and other image formats (PNG only), image URLs (ibex2); accessibility of
any kind; HiDPI beyond `EXACT_SCALE`; a font cache for the scan;
pixel fixtures against Chrome (the instrument exists — `screenshot`, the
smoke's canvas reference — the pinned font does not: a fixture that must
match across machines needs `EXACT_FONTS` pointing at one font and the
sans family set to it); the page extent past the root's frame (LLP 1010
§3's deviation, shared); lifting the orchestration out of `host/apple`
and `host/linux` into one crate.

## 8. Checks that hold this

`host/linux/tests/host.rs` (11): the tree lays out with real text and
every node has a painted box; `layout` is the agent API's shape; a press
goes through hit-testing and bubbles to the handler; typing replaces the
value and the runner hears one change; a wheel scrolls the page when the
inner container cannot; a nested scroll container takes the wheel then
chains to the page and clips what it scrolled out; a spring arrives as
presentation values frame by frame and settles at 1.5 / 0.5; the clock
fires timers at their due times; an image lays out from its decoded size
and never resolves outside the asset root; every agent request answers on
the wire; a reload carries state and starts the pictures over.
`host/linux/tests/paint.rs` (6), pixel by pixel: backgrounds land in their
boxes with their radii (`#eeeeee` inside the button, the page at its
corner; `#f7f7f7` inside the card past the 16 pt radius); text and images
leave ink in their boxes and none between; motion presents as a transform
and a group opacity (the box 1.5× wider, the ink lighter); a scroll
container clips what it scrolled out; a device scale of 2 paints 780×1688
for the same 390×844 points; a screenshot is the viewport as a PNG.
`node scripts/smoke.mjs linux`: ok on `expo-build-1000` (Ubuntu 24.04,
Rust 1.97.0) in 0.5 s and on macOS 26.6 in 7 s (the canvas reference step
aside, §5), 2026-08-29. Under the five checks the same day, on both.
