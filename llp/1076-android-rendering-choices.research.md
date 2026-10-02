# LLP 1076: Android rendering choices

**Type:** Research
**Status:** Draft
**Systems:** Android host; kernel layout and presentation; Linux painter; text and input; native embedding; web target
**Author:** Codex for Charlie Cheever
**Date:** 2026-10-02
**Revised:** 2026-10-02
**Related:** [1000](1000-exact2-root.explainer.md) (map); [1001](1001-kernel-v1.spec.md) (CSS semantics and layout); [1015](1015-linux-host-v1.spec.md) (existing painter); [1012](1012-agent-api-v1.spec.md) (inspection and deterministic driving); [1024](1024-native-modules.rfc.md) (native components); [1031](1031-brownfield-embedding.rfc.md) (embedding); [1071](1071-exact2-web-target.rfc.md) (web target); [1075](1075-native-platform-control.explainer.md) (native ownership and access); [0559](research/0559-flutter-lessons-for-the-refresh-program.research.md) (predecessor research, never authority)

## 1. Question and scope

What should render an Exact app on Android: native Views, Compose, Flutter,
GPUI, Exact's own painter, or something else?

The motivating question is whether owning rendering could make a better Android
experience than projecting every element to a platform widget. Flutter makes
that a credible architectural direction. It does not establish that GPUI, or
our own renderer, would match Flutter's mobile integration or outperform Android
Views or Compose on our workloads.

This is a survey and a proposed way to obtain evidence, not a selected host
architecture or implementation plan. Android remains deferred by
[`rules/DEFERRED.md`](../rules/DEFERRED.md). Writing this research does not move it
onto the doing-list. No working-set document is displaced for this survey.

External sources were checked on 2026-10-02. Local source observations refer to
the working checkout at HEAD `e953a2aafde09a345ef0779b6531a54061104911`, with
unrelated staged and unstaged changes. No Android implementation was built or
run. Evidence is distinguished as local source, upstream documentation or
maintainer report, earlier measurement, and architectural inference.

## 2. Rendering and platform integration are separate choices

An Activity or Fragment is a host/lifecycle choice, not a graphics backend.
Different renderers can occupy its content area. Likewise, using a `View` does
not require a standard widget for every Exact node: one custom View can paint
an entire Exact tree.

The relevant responsibilities are:

| Responsibility | Question |
|---|---|
| Application state and actions | Does Exact remain the authority, or does another framework acquire application state? |
| Layout | Does Exact supply final geometry, or does a second system lay out the same content? |
| Painting | Who turns shapes, text, images, clips and effects into pixels? |
| Interaction | Who owns hit testing, scrolling, gestures, focus and editing? |
| Platform integration | Who provides IME, accessibility, lifecycle, navigation, pickers and embedded native content? |

Exact's CSS defaults and behavior remain the parity target. Another toolkit's
similar-looking flex API is not proof of semantic compatibility. A presenter
must deliberately map defaults, intrinsic measurement, wrapping, clipping,
scroll coordinates and events, and decide who owns each piece of geometry.

Native Android UI already uses GPU acceleration. GPUI or a custom GPU backend
offers a different organization of work and more rendering control, not first
access to the GPU. Layout, shaping, scene preparation and resource management
still consume CPU time. [Android hardware acceleration](https://developer.android.com/develop/ui/views/graphics/hardware-accel).

## 3. Candidate approaches

The assessments in this table are architectural inferences, not performance
rankings. Hybrid use is possible across several rows.

| Candidate | Exact integration | Attraction | Main cost or uncertainty |
|---|---|---|---|
| Android Views | Project nodes to standard or custom Views; place them using Exact geometry where appropriate. | Established controls and platform integration. | Widget behavior, text measurement, recycling and styling must agree with Exact semantics. |
| Jetpack Compose | Build a Compose presenter for the Exact tree or selected components. | Modern Android UI primitives and customization. | Coordinate composition and state invalidation with Exact; avoid competing layout and scroll owners. |
| Exact plus Android Canvas | Paint Exact geometry through custom Views and Android graphics APIs. | Own visuals without introducing another full UI framework or bundled graphics engine. | Build semantic accessibility and input bridges for custom content; avoid excessive per-frame JNI traffic. |
| Exact Rust painter | Reuse painting and text machinery from Linux with an Android surface and host adapter. | Largest reuse of Exact's existing rendering semantics. | Mobile surface lifecycle, presentation, text integration, accessibility and GPU compatibility. |
| GPUI | Project the Exact tree into GPUI, or embed a bounded GPUI component. | Rust components and GPU rendering infrastructure. | Experimental Android ecosystem; overlap with Exact layout, state and event machinery. |
| Flutter | Embed its engine and adapt Exact output to Dart widgets or render objects. | Mobile-oriented custom rendering and framework infrastructure. | Another engine, language layer and UI system; choose how much of Flutter to adopt. |
| WebView | Run Exact's web target inside an Android shell. | Reuse the browser implementation and CSS behavior directly. | Browser startup/memory, keyboard and viewport behavior, native integration and delivery. |

### 3.1 Android Views

Views need not mean XML authoring or one Fragment per screen. A programmatic
presenter can create only the controls it needs, and Exact can own outer layout.
Standard editable controls are particularly relevant because drawing text is
much less work than implementing editing and its platform integrations.

The risk is not that Views are inherently incapable of good UI. It is the cost
of fitting their measurement, defaults and interaction behavior to Exact's
contract. Native control integration also does not solve application-level
navigation ownership automatically; LLP 1075's distinction applies here too.
[Android custom components](https://developer.android.com/develop/ui/views/layout/custom-views/custom-components).

### 3.2 Jetpack Compose

Compose separates composition, layout and drawing and can skip phases whose
inputs have not changed. It supports interoperability with Views. It is a
serious modern Android baseline, rather than another name for the traditional
widget hierarchy. [Compose phases](https://developer.android.com/develop/ui/compose/phases);
[interoperability](https://developer.android.com/develop/ui/compose/migrate/interoperability-apis).

For Exact, the design question is whether Compose receives already-laid-out
geometry or participates in measurement and layout. A translation to ordinary
Compose layouts may be convenient but risks two layout authorities and a new
set of CSS mismatches. A lower-level Compose presenter can preserve Exact
geometry, but may use less of the framework's benefit. Either option needs
measurement before claiming redundant work is material.

### 3.3 Exact plus Android Canvas

This is the missing middle ground between native widgets and shipping our own
graphics stack. Custom Views can paint shapes, images and text through
`onDraw(Canvas)`, with native controls mixed in where useful.
[Android custom drawing](https://developer.android.com/develop/ui/views/layout/custom-views/custom-drawing).

Exact could own layout, visible-node selection and presentation values while
Android owns drawing execution. A bounded command batch could cross the
Rust/Kotlin boundary instead of a JNI call per primitive. That batch is an
implementation hypothesis, not an existing Android ABI.

We would still need custom hit testing, accessible virtual nodes, and a policy
for native editing overlays. Platform text metrics must agree with the text
actually drawn; measuring with one stack and painting with another is a parity
risk. Android Canvas and direct Skia are distinct implementation choices.

### 3.4 Reuse Exact's Rust painter

**Local source:** [host/linux/Cargo.toml](../host/linux/Cargo.toml),
[paint.rs](../host/linux/src/paint.rs), [text.rs](../host/linux/src/text.rs), and
[gpu.rs](../host/linux/src/gpu.rs) provide a real foundation: a kernel-tree paint
walk, Vello/wgpu GPU rendering, tiny-skia CPU rasterization, and cosmic-text
measurement/shaping. This is reuse of an existing implementation, not a proposal
to invent a renderer from zero.

The current GPU backend renders to a texture and reads pixels back into the
CPU frame consumed by the Linux presenter/display. An Android port should
investigate rendering to a presentable surface without a per-frame CPU readback.
That change is not established by merely compiling wgpu for Android.

Separate portable painting/text logic from DRM/KMS, evdev, and Linux host
services. Android must supply surfaces, frame scheduling, input, lifecycle and
services. Native input controls and accessibility adapters remain relevant even
if every visible shape is painted by Rust. Font discovery, fallback, emoji,
font scale and complex scripts need their own device coverage.

Vello/wgpu is one backend. Direct Skia or another graphics library is a possible
backend alternative, not a separate application architecture. Changing backend
does not eliminate the Android host work.

### 3.5 GPUI

**Upstream reports:** [gpui-mobile](https://github.com/itsbalamurali/gpui-mobile)
implements Android rendering with wgpu/Vulkan or GL and cosmic-text/swash text,
and shows its demo on a Motorola phone. This establishes a community path to
running GPUI on Android; this research did not reproduce it.

[GPUI Kit's mobile guide](https://gpui-kit.com/docs/mobile/) calls mobile support
experimental and reports limited Android chat-component validation. It uses a
compatibility fork and warns about keeping GPUI, platform, renderer and Kit
versions aligned. General app compatibility remains unestablished.

[Zdroid](https://github.com/Dylanmurzello/zed-android-port) is an independent Zed
Android port with [published APK releases](https://github.com/Dylanmurzello/zed-android-port/releases).
The maintainer reports primary testing on an Adreno Samsung tablet with a
hardware keyboard; phones and other GPU families have less coverage. This is
stronger evidence than a counter demo, but not broad mobile-product validation.

Two experiments answer different questions: a GPUI component could save us
rich-content or editor work; a full Contract-to-GPUI presenter must preserve
Exact's semantics and avoid duplicate ownership. GPUI is a UI framework, not
just a replacement paint function. Compare both against the existing painter
before assuming its Rust implementation makes it the natural default.

### 3.6 Flutter

Flutter owns its widget and rendering stack, and supports embedding a
`FlutterView` into an existing Android layout. It is technically a candidate,
including for a bounded surface.
[Architecture](https://docs.flutter.dev/resources/architectural-overview);
[Android view embedding](https://docs.flutter.dev/add-to-app/android/add-flutter-view).

For Exact, distinguish adopting Flutter widgets and mobile behavior from using
its engine mainly to paint Exact geometry. The first buys more infrastructure
but introduces a larger semantic mapping; the second may retain substantial
engine/runtime cost while using fewer framework benefits. Drawing through
Flutter does not automatically expose custom Exact content to accessibility.

Startup, binary size, engine retention and Rust/Dart transfer need measurement.
Flutter's successful architectural model does not prove the cost is worthwhile
inside Exact, nor guarantee superiority to an optimized native implementation.

### 3.7 WebView and other frameworks

WebView can host web content inside an Android View. Exact's web target already
provides DOM/CSS presentation, making this an important reuse and fidelity
baseline. Offline packaging and a narrow native bridge are possible designs;
they have not been implemented by this survey.
[Android WebView](https://developer.android.com/develop/ui/views/layout/webapps/webview).

Slint, Qt and React Native are additional toolkit candidates, but each adds
another framework and adapter. Slint has an
[Android backend](https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/).
These remain secondary until a specific capability or measured cost justifies
their inclusion. A browser/PWA is also a delivery alternative, but does not
constitute a new native Android rendering backend.

## 4. Hybrid rendering is a first-class candidate

A native shell can host an Exact-painted or GPUI-painted content region, native
editing controls, and separately embedded video or web content. The useful
boundary is ownership, not whether every pixel came from the same library.

Test focus transitions, selection geometry, clipping, z-order, transforms,
keyboard avoidance and accessibility traversal across those boundaries.
Native overlays cannot simply be positioned once and assumed to track a moving
or clipped painted node. Do not let both sides shrink for one keyboard event
or maintain independent scroll positions for the same content.

An optional rich-content renderer must fit the existing loaded-artifact or
executor model, rather than add a core-crate feature matrix. A chosen primary
Android host is a separate decision from an optional component.

## 5. What the earlier macOS benchmark tells us

**Earlier measurement:** the 2026-09-30 pilot compared published GPUI 0.2.2
against Exact's native Apple messages path at 1,000 and 10,000 rows. Twelve
complete runs covered idle, tail updates, scrolling and width changes, with
three repeats per count/engine and 1,152 active workload commands.

Artifacts are outside the repository at
`/Users/ccheever/projects/gpui-exact2-benchmark/`; `results/summary.json` records
the final results and caveats, and `results/raw.json` the observations. These
machine-local artifacts are not a durable repository benchmark suite.

At 10,000 rows, median application CPU as a percentage of one core was:

| Phase | Exact | GPUI |
|---|---:|---:|
| Idle | 0.25% | 2.50% |
| Update 32 tail rows | 5.25% | 2.37% |
| Scroll | 1.37% | 1.62% |
| Resize | 8.87% | 6.49% |

This suggests profiling particular workloads. It does not rank renderers.
Exact used its real data validation and overscan; GPUI used typed records and
zero overdraw. The pilot measured process CPU and RSS, excluding GPU work and
WindowServer. Command acknowledgments had different endpoints. It measured no
physical input-to-pixel latency, sustained frame rate, energy or mobile editing.
An earlier batch with offscreen GPUI updates was discarded and is not evidence.

There is no Android performance result, and no basis here for attributing the
CPU differences specifically to GPU rendering.

## 6. Evidence that would let us choose

These are candidate experiments, not scheduled work. Use existing consumers
and tooling rather than adding a generic benchmark framework. Start with one
credible implementation and baseline; expand only when the comparison would
resolve a remaining decision.

### 6.1 Same behavior, then comparable work

Use a simple real app for startup/navigation and Messages Stress or rich
Markdown for rendering pressure. Keep the same data, viewport, fonts where
possible, content, update cadence and required interactions. Record actual
visible/mounted rows, overscan, image caches, transfers and validation work.
Equivalent screenshots alone do not establish equivalent work.

For full presenters, retain Contract/CSS semantics and Exact's authoritative
state. For a component experiment, state explicitly which interior behaviors
belong to the component. Compare an optimized implementation rather than a
deliberately naive tree of native Views.

### 6.2 Functional and platform proof

- Variable-height scrolling while visible text and images update; follow-end,
  nested scrolling, cancellation and touch feedback.
- Editing with composing IMEs, CJK, RTL, emoji, selection handles, clipboard,
  hardware keyboards and keyboard show/hide.
- TalkBack reading order, roles, actions, focus and dynamic updates; large
  font scale and display scaling.
- Rotation, surface recreation, background/foreground, process restoration,
  system back and native content embedded beside the renderer.
- Agent inspection and clock-driven motion agree with actual visible behavior.

Native editing can be used in more than one candidate. Keep that choice
constant when isolating renderer performance.

### 6.3 Measurements

Measure release builds on physical devices: at least a midrange phone and a
second GPU family, with a tablet if dense desktop-like UI is a target. Record
device/OS, build revision, renderer dependency versions, refresh rate and
thermal conditions. Alternate run order and report dispersion, not only means.

Collect cold/warm first pixel and first usable interaction; frame-time
distribution and missed deadlines during scroll/update overlap; physical
input-to-pixel latency where instrumentation permits; CPU, GPU and memory;
energy over a sustained workload; package size; and edit-to-verified loop time.
Distinguish application work from render/compositor work and cached from fresh
shader/font paths. A compile check, emulator run or mutation acknowledgment is
not a device-performance result.

Retain Exact's current startup and edit-loop budgets as evaluation targets.
Adopting a runtime that changes how the first frame is produced needs an
explicit accounting of the boot graph, not an assumption that a later smooth
scroll excuses a slower launch.

## 7. Provisional assessment and open questions

**Inference:** the most informative initial shortlist is Exact's Rust painter
with native controls, Exact geometry painted through Android Canvas, a Compose
presenter, and WebView. Each tests a materially different reuse/integration
trade. GPUI is a strong challenger if its components remove significant work;
Flutter is more compelling if we want its mobile framework broadly, rather
than only its pixels. This is a research priority, not an adopted ordering.

Questions that could change that assessment:

1. Is the intended Android product mostly ordinary forms/navigation, rich
   reading and chat, editing, or custom animated content?
2. How much Linux painting/text code can be separated from its host without
   introducing another tree or platform policy into the kernel?
3. Does direct Android surface presentation meet startup and frame budgets on
   both Adreno and Mali, including fresh caches?
4. Can a hybrid painted surface provide correct TalkBack, IME and selection
   behavior with reasonable maintenance? Which controls should stay native?
5. Does GPUI save enough component work to outweigh its additional ownership
   model and Android fork maintenance? Which exact fork/version is the trial?
6. Does Compose add useful platform behavior when Exact supplies geometry, or
   do we mostly use it as a drawing wrapper?
7. Would the existing web target meet the actual Android product requirements
   with less implementation work than a new native presenter?
8. What measured failure in the best existing candidate justifies introducing
   another engine?

**Confidence:** high in the existence of the documented approaches and the
local Linux painter; moderate in the maintenance/reuse assessments; low in any
performance ordering or claim of Android integration completeness. The next
decision should follow device evidence and product requirements, not the labels
“native,” “Rust,” or “GPU.”
