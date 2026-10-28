# LLP 1152: The web shootout — Exact against every plausible web framework

**Type:** RFC
**Status:** Draft r2, 2026-10-28. r1 was reviewed blind by GPT-6 Astra (xhigh), Grok 4.7 (xhigh) and Muse Wonder (xhigh), for direction and major oversights only (`llp/reviews/rfc-2026-10-28-1152-r1.{astra,grok,muse}.md`): Astra NOT READY, Grok READY WITH CHANGES, Muse NOT READY, all three finding the direction right. r2 takes what §R lists. §12 lists what Charlie decides.
**Systems:** a new `bench/web-shootout/` (the streaming proxy and probe, the Conduit server, the drives, the report); `apps/realworld` (its API origin becomes configurable, §2.1); `host/render` and `host/web-js/bench.mjs` (the render server Exact's Conduit arm is served by, and the RealWorld bench this one grows from); `bench/heavy-list` and `bench/dioxus/web` (the feed's data and the Chrome scroll harness); `bench/bakeoff` (`build.mjs` for agent-built arms); the web JS target (`host/web-js`), which is the arm under test
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-28
**Implementer:** unassigned. §10 is staged so one lane can start on the pipeline before the field is final.
**Related:** LLP 1152.000 (what `re` and Thane do that the JS target does not: the candidates for its row construction, bindings and keyed lists); LLP 1071 (the JS target; §1 "RealWorld, the headline" is the method and the numbers this continues); LLP 1047.000 §1 (that method); LLP 1053 and LLP 1050 (the list benchmarks and what they found missing); LLP 1116 (the iOS bake-off, whose builders this reuses); LLP 1087 and LLP 1102 (the authoring bench, which the secondary table continues); LLP 1027.004 (the bounded answer); `bench/dioxus/README.md` (the one web comparison so far and its last standings); `bench/heavy-list/SPEC.md` (the feed); the RealWorld project ([docs.realworld.show](https://docs.realworld.show)) and its implementation catalogue; [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark) (Krausest)

## R. Round 1, and what r2 changed

Three reviewers, blind, each asked for direction and major oversights only. All three
found the shape right (two apps plus Krausest, the field in tiers, five legs) and the
protocol not ready. Where all three raised a point, r2 takes it. Where one did and the
repository or a specification bears it out, r2 takes it too. Nothing was taken on
assertion alone.

| Change | Raised by | Where |
|---|---|---|
| Exact's Conduit arm is the render server's page, not the static shell a file server would serve | Grok | §2.1 |
| The early-tap run is its own session; the untouched load finalises LCP and CLS; one tap per fresh navigation; outcomes by observable state, never by the arm's mark; the anonymous favourite is a link, so the targets are a tag link and the sign-in submit | all three | §4.2 |
| The feed's correctness gate requires the full list's extent (a jump to a late row, a scrollbar over the whole list, position stable across a live insert); Exact enters an arm that passes, and the bounded app as it stands does not | Grok, Astra | §2.2, §4.8 |
| The first round is a calibration ladder of about ten arms; the field grows only after the ladder's noise and the agent-built-versus-hand-built delta are known | all three | §3, §10, D1 |
| The phone legs get a stage 0 feasibility spike: a touch route that exists on a physical iPhone (the bake-off driver is simulator-only) and an entry-type support matrix per engine | all three | §5, §6, D4 |
| Metric definitions corrected: LCP ends at first input; CLS over the untouched load only; long tasks are Chromium's; the quiet-window blocking time is named as ours, not TBT; INP's real definition and Chromium's 16 ms floor; rAF intervals are rAF intervals, not presented frames; memory columns never merge methods; "energy" is browser CPU | all three | §4 |
| The indexes are gone: ratios to a best arm are undefined when the best is zero, and per-arm metric sets are not comparable; the report is per-metric medians with noise, and win/tie/loss counts against Exact | all three | §9 |
| One latency per leg at the proxy edge, applied to every arm's server alike; a streaming proxy that preserves chunked responses, SSE and WebSockets; cache state declared; server CPU recorded for server-driven arms | Grok, Astra | §5, §6, D8 |
| Repetition counts, the minimum effect worth reporting and the tie rule come from stage 0's pilot; p99 over twenty taps is replaced by the maximum; thermal state recorded and gated on phones; one condition per leg in the published table | all three | §6 |
| Builders: a performance arm is signed by a person who knows the framework or labelled agent-built; the authoring table crosses both builders with each framework or is not published comparatively | all three | §8 |
| Errors fixed: the whole-feed arm is the JS target too; §11 overstated the bounded standings; React's SSR arms replay discrete events during hydration, its client arm has nothing to tap; the feed has no composer; Safari has no CPU throttling; Lighthouse runs on Chrome legs only; Core Web Vitals thresholds are observed, not "passed" | all three | §2.2, §4, §9, §11 |
| Three conclusions kept separate: these implementations' experience, the framework's cost, and the authoring cost | Astra | §9 |

Not taken: Muse's and Astra's suggestion to drop to a handful of frameworks for good. The
field stays the target (Charlie asked for Exact against everything) and the ladder is the
gate to it. Grok's alternative of running only the two existing harnesses is where stage 0
starts, not where the program ends.

## Summary

Measure Exact's web target against every web framework someone could plausibly hold up
beside it, on the same two apps, the same data, the same devices and the same probe, and
publish the table. Two apps carry the comparison: Conduit, the RealWorld Medium clone
that nearly every framework already implements, and the heavy-list feed that the iOS and
Dioxus benchmarks already measured. A third lane enters Exact in Krausest's
js-framework-benchmark, the microbenchmark every framework author already optimises for.

The metrics are the ones a person feels and the ones a team pays for: how soon content
appears, how soon a tap works and what happens to a tap that comes too early, how
interactions and typing feel once the page is live, how scrolling holds up with and
without live updates, memory at load and after a soak, CPU for the whole session, and
bytes. A correctness gate runs before any number counts. A second, separate table records
what it cost an agent to build each arm.

The first round is a calibration ladder of about ten arms, run until its noise is known
and the gap between an agent-built arm and a hand-built one is inside that noise. The
field grows from there. The first round answers two questions: where does Exact win on
the web, and what does it lose that a kernel or host change would fix. The losses become
the next LLP, as LLP 1053 and LLP 1116 did for the native benchmarks.

## 1. What the numbers must answer

1. On a phone, over a real network, does an Exact page show content and accept a tap
   sooner than a React, Svelte, Solid, Qwik or Astro page of the same app?
2. Once live, is an Exact page as responsive as the compiled frameworks (Svelte 5, Solid,
   Vue Vapor, Octane, Ripple) and not merely as responsive as React?
3. Does Exact's list scroll as well as the best DOM virtualiser, with and without a feed
   that keeps inserting at the top?
4. What does an Exact page weigh against a hello-world and against the same app in each
   framework, and where do the bytes go?
5. What does an Exact page cost in memory and CPU over a ten-minute session, on Chrome
   and on Safari?
6. What does it cost an agent to build the same app in Exact and in each framework?

Each question maps to one group of metrics in §4. A metric that answers none of them is
not collected. The answers are three separate conclusions, and the report keeps them
apart (§9): what these implementations feel like, what the framework costs, and what the
authoring cost. The experiment cannot attribute a difference in the first to either of
the others by itself.

## 2. The apps

### 2.1 Conduit (RealWorld)

The Medium clone of the RealWorld project: a feed of articles with tags and pagination, an
article page with comments, author profiles, favourites, a Markdown editor, sign-in and
settings. Exact's version is `apps/realworld`.

Why this app: the RealWorld catalogue has a maintained implementation in most of the field,
so the competitors are written by people who chose that framework, not by us. The
comparison cannot be dismissed as a strawman. Its weaknesses are the opposite side of the
same fact: implementations differ in age, quality and feature coverage, and some are
idiomatic to a version two majors back. §8 says how each is pinned and gated.

**Exact's arm is the page Exact ships.** Every public route of `apps/realworld` is
`render=cached`: the render server (`host/render`) renders it per request on loopback
from the built web app and keeps the successful page at the origin, and the shell is
served only for a client route. LLP 1071 §1 measured exactly this ("pages served per
request, `cached` routes kept at the origin") and recorded that the client-only shell is
a different product with content hundreds of milliseconds later. A file server that
falls back to `index.html`, which is what `bench/dioxus/web/bench.mjs` is, would time the
shell. So Exact's Conduit arm is the render server behind the proxy of §6, with the API
origin and its grant pointed at the bench server, and its cache state declared per
scenario (§5). The static shell is not an arm.

The existing RealWorld bench, `host/web-js/bench.mjs` (LLP 1047.000 §1's method: headless
Chrome, a fresh profile per run, a mobile profile of 150 ms round trip, 1.6 Mbps down and
4× CPU, and four measurements: content painted, a tag tapped at load, the runtime up, a
press that needs the runtime), is the method this lane grows from, and its selector
configuration per arm is how the other frameworks' Conduits plug in.

The API. RealWorld's hosted API is a shared public server, so it is not measured against.
`bench/web-shootout/conduit/` is a local server implementing the Conduit API
specification over an in-memory store seeded from one fixture (a recording of the hosted
API's public reads, with authors, articles, tags and comments, deterministic per round),
reset between runs. Every arm, Exact's included, is built against it. Exact's
`apps/realworld/app.ts` names the hosted origin in a constant and a grant; the bench build
replaces both. The API sits behind the same proxy edge as every arm's own server (§5), so
it is the same distance from the browser for all of them.

Routes measured: the public home feed (cold load, the landing case), an article page
(the LCP case), the paginated global feed (client navigation), the editor (typing), and
sign-in followed by a favourite (an authenticated mutation).

### 2.2 The feed

`bench/heavy-list/SPEC.md`: one list of 10,000 rich messages from the same `messages.json`
and the same 104 JPEGs, with quotes, paragraphs of styled runs, photo grids, link cards and
reaction chips; a tap on a chip increments its count; live mode inserts a message at the
top and bumps a reaction every 250 ms while the list keeps the reader's position, and every
visible relative time updates once a second, which is the many-independent-updates
workload a fine-grained framework exists for. The iOS benchmarks (SwiftUI, UIKit, Expo)
and the Dioxus comparison measured this screen, so a new series lines up with the old.

Why not the Bluesky clone or the bake-off's feed spec: the clone is too large to port to
twenty frameworks honestly, and the bake-off's spec is an iPhone app with navigation and
persistence, which is the authoring question (LLP 1116), not the runtime one.

**Exact's arm must meet the spec the other arms are held to.** The spec is one list of all
10,000 messages, and §4.8's gate requires the whole list's extent: a jump to a late row,
a scrollbar that spans the list, and position stable across a live insert. Two Exact feed
apps exist, both on the JS target: the whole-feed app (`bench/heavy-list/exact-heavylist`)
has the full extent and pays for whole-feed answers crossing the data seam (80 to 96 ms
per tap, and the feed shipped three times, `bench/dioxus/README.md`); the bounded app
(`bench/dioxus/exact-bounded`, LLP 1027.004) answers 200 rows around a cursor, which fixed
the tap and the bytes, and its README records the trade: the scrollbar spans the window
and an offset jump cannot reach row 9,000. Neither is the arm as it stands. Stage 2's
first task is an Exact feed arm that keeps the bounded answer and the full extent, which
is Exact work, not bench work (a collection's extent from a count the answer carries). If
it is not built by the feed round, the whole-feed app is entered, labelled with its known
cost, because it is the one that meets the gate. Any arm may window the list by hand
under the same extent rule, and says so.

### 2.3 Krausest's js-framework-benchmark

The table of 1,000 rows with create, replace, partial update, select, swap, remove, create
10,000, append and clear, timed by its own harness in Chrome, with memory and startup
columns. It is run as the project runs it, against its pinned Chrome, and Exact's keyed
entry is written to its contract. The rest of the field is already there, including the
specialists that are not products (mikado, ivi, blockdom, voby, sycamore, and the two
nearest vanilla at the time of writing, `re` and Thane, read in LLP 1152.000 for what
they do that the JS target does not). What we gain is
an external number nobody can say we chose the conditions for; what it cannot show is
anything about loading, scrolling or a phone.

The harness checks a specific DOM shape (`tbody > tr > td > a` with classes and ids).
Contract has no `table` tag, so the entry needs either one or the project's agreement to
a different shape (§12 D3). An entry under a private adapted check loses the external
number, which is the only reason the lane exists, so there is no such entry.

## 3. The field

Pinned at the round's date to the latest stable release, or the latest pre-release where
the stable one is not what the framework is now claiming (Vue's Vapor mode, Solid 2.0,
Octane and Ripple). Each arm records its version and commit in the round's results. An
unstable pin is labelled.

The field is the target. The first round runs the ladder (§3.5) and the field joins in
later rounds under §10's gates.

### 3.1 Baselines

| Arm | Role |
|---|---|
| Static HTML | the floor for load: the same markup, no script |
| Vanilla JS | the floor for runtime: hand-written DOM, no framework |
| Exact, JS target | the arm under test, the production build delivery publishes, served by the render server for Conduit |
| Exact, wasm target | reference only, Chrome only (`--wasm` is not what ships, LLP 1071 §7) |

### 3.2 Tier 1: both apps, every leg

Single-page frameworks:

| Framework | Why it is in |
|---|---|
| React (Vite, React Compiler on) | the incumbent; the number everyone compares to |
| Preact | the small React |
| Vue 3, Vapor mode | the no-virtual-DOM compile of a mainstream framework |
| Svelte 5 (runes) | the compiled framework most people name first for speed |
| Solid | highest satisfaction for five years running, the fine-grained reference |
| Angular (zoneless, signals) | the enterprise arm; zoneless is its performance claim |
| Lit | the web-components runtime |
| Octane | React's model compiled, no virtual DOM, the Inferno successor ([octanejs.dev](https://octanejs.dev)) |
| Ripple | compiler-driven, TypeScript-first, ahead of Octane on Octane's own grid ([ripplejs.com](https://www.ripplejs.com)) |
| Elm | the historical reference for fast without a compiler trick; a first-rate RealWorld |
| Leptos | the fastest Rust-to-wasm framework on Krausest's table; the wasm peer |
| Expo web (React Native Web) | a baseline, not a contender: the same app through Expo's web path, measured on every leg so the Expo team has its own numbers to work from |

Server-rendered and meta-frameworks (each measured cold over the network, where SSR,
streaming, hydration and resumability show):

| Framework | Why it is in |
|---|---|
| Next.js (App Router, server components) | what React apps actually ship as |
| React Router v7 (framework mode) | the Remix line |
| TanStack Start | rising in the write-ins; the newest React meta-framework |
| SvelteKit | Svelte's SSR |
| Nuxt | Vue's SSR |
| SolidStart | Solid's SSR |
| Angular SSR (hydration, incremental) | Angular's SSR |
| Qwik City | resumability: the direct rival on the early-tap question |
| Astro, React islands | islands with the heaviest island |
| Astro, Svelte islands | islands with a light island |
| HTMX | hypermedia: the server renders, the client patches |
| Datastar | hypermedia over server-sent events, the smallest client in the field |
| Octane, streaming SSR | its Suspense-boundary streaming claim |

### 3.3 Tier 2: Conduit only, Chrome on the Mac, a feed arm only where a port is cheap

| Framework | Why it is in |
|---|---|
| Ember | people will ask; a maintained RealWorld exists |
| Marko | streaming and resumable from a different lineage |
| Inferno | Octane's predecessor; a RealWorld exists |
| Mithril | the small virtual DOM; a RealWorld exists |
| Alpine | the no-build directive library |
| Stencil | the compiled web components, beside Lit's runtime ones |
| Imba | a language with a memoised DOM, near the top of Krausest's table for years |
| VanJS | about 1 KB, no build; the floor above vanilla |
| Arrow.js | no build, proxy reactivity, SSR, and a QuickJS-in-wasm sandbox nobody else has |
| Nue | standards-first, with published size claims against React stacks |
| Phoenix LiveView | server-driven UI over a socket, at the leg's round trip |
| Hotwire Turbo | the same architecture from the Rails side |
| Flutter web (CanvasKit or skwasm) | draws its own pixels; the architecture people assume Exact has |
| Lynx web | ByteDance's one-source web and native, like Exact from the DOM side |
| Blazor WebAssembly | .NET on the web; asked about more than used |
| Dioxus web | already measured (`bench/dioxus`); rerun on the new pipeline so the numbers are one series |

### 3.4 Not entered

Kasper.js, petite-vue, Million.js, Aurelia, Hyperapp, FAST, Enhance, Fresh: each is a
thinner version of an arm already in, or has no implementation base. The Krausest
specialists appear in that lane's table without an app: `re` (one developer's entry
merged 2026-10-24, its home the benchmark itself) and Thane (a compiler and runtime at
0.2.0) among them, each worth a Tier 2 slot only once it has docs and releases. jQuery
is not a framework in the sense asked.

### 3.5 The ladder: the first round's field

The three reviewers agreed that forty arms cannot be made credible before the method is,
and that most of a first round's budget would go to resurrecting implementations and
running devices rather than to repetitions where confidence is decided. The first round
is therefore a ladder: the baselines, React (Vite) and Next.js, Svelte 5 and SvelteKit,
Solid and SolidStart, Qwik City, Datastar as the server-driven arm, Expo web (Charlie's
baseline), and the Krausest entry. It has an incumbent, two compiled frameworks, the
resumability rival, a hypermedia arm and a wasm-free floor, which is enough to answer the
six questions once, and few enough arms to run every leg at the repetition count stage 0
sets.

The ladder grows to Tier 1 and then Tier 2 under §10's gates: the round's noise is known
and smaller than the gaps it reports, and the agent-built arm of a framework measures the
same as a hand-built one within that noise (§8).

## 4. Metrics

Every metric names how it is measured so two rounds agree. "The probe" is §6's script,
injected into every arm the same way. "CDP" is Chrome's protocol, available on the Mac
and the Android legs only. A metric has one method per engine; where an engine has no
method the cell reads "not measured", and a column never mixes two methods. Stage 0
produces the matrix of metric by engine with the method, the resolution and the limits of
each (§10), from the pinned engines, not from documentation.

### 4.1 Load (question 1)

Measured in the untouched session (§4.2): no input until LCP has been finalised by the
page becoming hidden and the shift window has closed.

| Metric | Definition | Measured by |
|---|---|---|
| TTFB | navigation start to first response byte, every arm (it carries the proxy's latency and any server work) | Navigation Timing |
| First Contentful Paint | the standard | `PerformanceObserver` `paint` |
| Largest Contentful Paint | the standard: candidates end at the first input or when the document is hidden; the probe hides the document to finalise | `PerformanceObserver` `largest-contentful-paint` |
| Cumulative Layout Shift | the standard, over the untouched load; shifts after input are excluded by the specification's `hadRecentInput`, so there is no "first scroll" CLS | `PerformanceObserver` `layout-shift` |
| Long-task time and count, FCP to quiet | the sum of long-task time over 50 ms and the count, from FCP to quiet; ours, not Lighthouse's Total Blocking Time, which ends at its own Time to Interactive | `PerformanceObserver` `longtask`, Chromium only; not measured in Firefox or Safari |
| Interactive | the arm's own mark `performance.mark('app:interactive')` when its root is live (hydrated, resumed, or the SPA mounted); Exact's is the `bootMs` the JS target writes on its root (`host/web-js/rt.js`), which the bench build re-emits as the mark | the probe reads the mark; diagnostic, never a result (§4.2 is the result) |
| Quiet | the end of the first 2 s window after LCP with no long task, Chromium only; elsewhere the end of the last network response plus 2 s | derived |

Each is recorded cold (fresh profile, empty cache) and warm (second navigation in the same
profile) at the leg's one condition (§5).

### 4.2 Early interactivity (question 1)

The metric Exact and Qwik claim and the standard vitals miss: what happens to a tap before
the page is ready. r1's procedure, a tap every 50 ms in the same session as the load,
would have ended LCP at its first tap, hidden the shifts it caused, made the tap storm the
session's INP, and on Exact's anonymous home tapped a link to `/register`, because the
signed-out heart is a link so it works before the runtime loads. r2's procedure:

- **Its own sessions.** The untouched load (§4.1), the early-input runs, and the
  steady-state session (§4.3) are separate navigations. The early-input runs are no input
  to LCP, CLS or INP.
- **One tap per fresh navigation**, at an offset from navigation start that steps by
  100 ms from first paint to 1 s past quiet, each offset in a fresh profile. The probe
  records the input's actual timestamp (the device's spacing is what it is, §5) and
  watches for a uniquely defined outcome until a declared timeout of 5 s.
- **Two targets**, because they are two claims. A control that is live in the arm's HTML
  before any script: on Conduit a tag link, whose outcome is the tag's feed shown, as
  `host/web-js/bench.mjs` measures it. And an action that needs the app's code: on Conduit
  the sign-in form's submit with the form empty, whose outcome is the API's errors shown
  (the same bench's press), and on the feed a reaction chip, whose outcome is its count.
  The favourite is kept for the signed-in mutation of §4.3, after the token is in place.
- **Outcomes by observable state.** Lost: the outcome never appeared. Deferred: it appeared
  later than 100 ms after the input. Immediate otherwise. The arm's `app:interactive`
  mark is recorded beside the outcome as a diagnostic, never as the result: Exact writes
  its mark before replaying the presses it captured, and frameworks' marks are not
  comparable to each other.

Reported: the earliest offset whose tap was not lost, and for each offset band the share
lost, deferred and immediate. The architectures differ by design: Exact's capture script
replays a press once when the runtime is up; Qwik resumes the handler; a React SSR arm's
`hydrateRoot` replays discrete events on server-rendered markup once its hydration
machinery has loaded, and drops them before that; a client-rendered arm has no element to
tap until it has rendered. The table shows which happened, and the SSR and client arms of
one framework are different rows.

### 4.3 Interaction latency (question 2)

In the steady-state session, after quiet, with the token in place.

| Metric | Definition |
|---|---|
| Visible feedback, toggle | a favourite or reaction toggle: the input's timestamp to the first frame after the DOM shows the change (a `MutationObserver` on the control, then two animation frames, so the paint has happened); median, p95 and the maximum over 20, since a p99 over 20 is one observation |
| Completed outcome, toggle | the same input to the server-confirmed state (the API's response applied), separately, because an optimistic arm and a server-confirmed arm are different answers |
| Visible feedback, navigation | open an article, then back: input to the first frame with the article's title (or the feed's first row) |
| Keystroke to visible feedback | 200 characters typed into Conduit's editor at 60 ms intervals, the same text in every arm; median and p95 per key (the feed has no composer in its spec and gets no typing test) |
| INP | the standard: events grouped by `interactionId`, the longest event per interaction, the worst interaction when there are fewer than fifty and the 98th percentile above, one worst interaction discarded per fifty |

Event Timing (`PerformanceObserver` `event`) is recorded for INP and for the diagnostic
view. It is not the latency metric: Chromium delivers no entry under 16 ms, and
`bench/dioxus/README.md` already records that floor on the same taps, so it cannot
separate arms that are all near it. The probe's own timestamps can. The `first-input`
entry is the retired First Input Delay and is not collected.

### 4.4 Scrolling (question 3)

The feed only, static and live mode, at the speeds `bench/dioxus/web` uses (3k, 6k, 12k,
24k px/s for 3 s each) and a flick-and-settle on the phones.

| Metric | Definition |
|---|---|
| rAF rate | `requestAnimationFrame` callbacks per second over the run; named as what it is, since a callback runs before the paint and is not a presented frame. On Chrome a trace of a representative run validates it against presented frames once per round |
| late callbacks per second | callbacks whose interval exceeded 1.5× the run's median interval, as the existing harness defines it, so the series lines up with 2026-10-03's; the display's refresh rate is recorded beside it |
| worst interval | the longest interval |
| blank samples | the fraction of per-callback samples down the list's centre line that hit no row (white while scrolling) |
| rows per second | progress, by the row at the top each callback, with backward steps counted (a windowed arm's `scrollTop` steps back at a shift, so pixels per second understate it) |
| main thread per callback | task, script, style and layout ms (CDP `Performance.getMetrics`; Chrome only) |
| CPU during scroll | all browser processes (Chrome by `ps` on the Mac and `top` on Android; Safari by `ps` on the Mac; the iPhone by `xctrace` if §12 D4 allows, else not measured) |
| position stability, live mode | the content under the viewport's top after each insert, in px moved |

Progress and blank samples are reported together: a smooth viewport that is not moving,
or shows stale rows, does not win.

### 4.5 Memory (question 5)

Each column is one method, named, and never merged with another:

- JS heap: CDP `Performance.getMetrics` `JSHeapUsedSize` (Chrome legs). Safari: not
  measured (`measureUserAgentSpecificMemory` needs cross-origin isolation and measures a
  different quantity, so it is not the Safari substitute).
- Process RSS: the renderer process by `ps` on the Mac (Chrome and Safari) and `top` on
  Android, named as RSS, not private footprint. iPhone: not measured unless D4.
- DOM nodes: a walk of the composed tree that enters shadow roots, so a web-components
  arm is counted like the rest.
- GPU memory for the canvas arms: Chrome's GPU memory counters by CDP, Chrome only.

Each at three points: after quiet, after the full scroll of the feed (or ten paginated
feed pages of Conduit), and after a ten-minute soak of the scripted session in a loop. The
growth from the second to the third is the leak signal. The soak runs on a named subset of
five arms per round (Exact, vanilla, and the three the round is most interested in), not
the field.

### 4.6 Browser CPU (question 5)

Total CPU seconds of every browser process over the whole scripted session, named as
browser CPU, not energy: it omits GPU, radio, display and server work. Chrome and macOS
Safari by process accounting; the iPhone by `xctrace` if D4 allows, else not measured.
Server-driven arms also record their server's CPU seconds for the same session (§5), so
work moved to the server is not work that disappeared.

### 4.7 Size (question 4)

Bytes transferred for HTML, JavaScript, CSS, fonts, data and images, and the request count,
raw and as served (brotli), counted at the proxy so every engine is counted the same way.
Framework weight is reported two ways so an arm cannot hide weight in the app: each
framework's own hello-world from its CLI template built for production, and the app's
bytes minus that hello (the marginal cost of the app). For Exact the hello is `apps/hello`
on the JS target, and the marginal bytes split into runtime, plan and data.

### 4.8 Correctness gate

Before an arm's numbers count it passes the same functional drive (§6): every measured
route and interaction produces the expected state, and a screenshot at each step matches
the reference within a tolerance the report shows. The feed's drive includes the extent
gate of §2.2: a jump to row 9,000, a scrollbar whose extent is the list's, and position
stable across a live insert. The gate also requires basic semantic equivalence: the
measured controls are reachable by keyboard and named in the accessibility tree, so a
canvas-rendered or div-only arm cannot win the table while failing as an interface.
Axe-core's violation count is reported as a column beyond that. An arm that fails the gate
appears in the table with its failure and no numbers.

### 4.9 Authoring, the secondary table (question 6)

Lines of source, production build time, dev-loop reload time, and for the agent-built
arms the builder's tokens, wall clock and the bugs the expert pass had to fix. Token counts
compare within a model and dollars across models, as LLP 1087 requires. Octane, Ripple
and Arrow.js all market themselves as frameworks for agents to write, so this table is
where they compete with Exact's authoring claim. It is published comparatively only under
§8's crossed design, and it is separate from the performance table.

## 5. Environments

| Leg | Browser | Device | The published condition |
|---|---|---|---|
| Mac Chrome | Chrome, pinned major | the Studio (a quiet, dedicated machine; §12 D7) | unthrottled, CPU 1×: the interaction and scroll lab |
| Mac Safari | Safari, the OS's | the Studio | unthrottled (Safari has no CPU throttling route; none is claimed) |
| Mac Firefox | Firefox, pinned | the Studio | unthrottled |
| iPhone Safari | Safari | iPhone 17 Pro Max, a real device | the proxy's 4G (9 Mbps, 170 ms): the headline load leg |
| Android Chrome | Chrome | Nothing 4a Pro (the mid-range phone the Android work measures on) | the same, and CPU 1× (the phone is the slowdown) |

Plus one stress cell on Mac Chrome: CPU 4× and the 4G profile, which is where a heavy
framework shows on a fast machine. Other conditions are diagnostics, not table cells. The
phone legs record thermal state before each run and a run above the device's threshold is
discarded, as `bench/heavy-list` warns; refresh rate, low-power mode and cache state are
recorded with every run.

Safari on the iPhone is the leg that matters most and the one nobody else publishes. It is
also the one with the largest instrumentation risk, so it enters at stage 0 (§10) as a
feasibility spike rather than at stage 2 as a promise.

**Topology.** One streaming reverse proxy stands between the browser and everything: each
arm's own production server (a static file server, an SSR server, Exact's render server,
a LiveView or Turbo server), and the Conduit API. The leg's latency is applied once, at
that edge, to every response alike, so every arm's server and the API are the same
distance from the browser and nothing stacks (r1's separate API round-trip knob is gone).
The proxy preserves chunked responses, server-sent events and WebSockets, flushes the first
head bytes through without waiting for the body, and sends no `no-cache` (a render server
would render again). A chunked fixture in stage 0 asserts the first byte and the first HTML
chunk arrive before the body. Server CPU is recorded for every arm that has a server doing
per-request work.

**Cache state** is a declared scenario, not an accident: cold (a fresh browser profile and
a render cache emptied) and warm (the second navigation; the render cache as the first
left it). Mutable fixtures are reset between runs.

## 6. The harness

`bench/web-shootout/` grows from two harnesses that exist: `host/web-js/bench.mjs`
(Conduit under a mobile profile, with per-arm selectors) and `bench/dioxus/web/bench.mjs`
(the feed: headed Chrome through playwright-core with a fresh profile, synthesised scroll
gestures, blank sampling). What is new:

- **The measuring proxy** of §5, which also injects the probe as the first script in
  `<head>` and records every response's bytes. The probe costs each arm the same bytes and
  the same parse time, and it is the only way to measure inside Safari on a phone, where
  there is no CDP. Stage 0 calibrates its cost by running three arms with and without it.
  The probe collects §4's observers, marks and timestamps and posts them to the proxy at
  the end of the session.
- **The Conduit server** (§2.1), behind the same proxy.
- **Real input.** Chrome legs synthesise through CDP as today. macOS Safari gets
  `safaridriver`'s actions. Android gets `adb shell input` and, for scroll, a recorded
  swipe. The physical iPhone needs a touch route that exists: `bench/bakeoff/drive.mjs`
  does not, it drives simulator apps through `simctl`. The candidates are an XCUITest
  runner on the device driving Safari by bundle id, and `safaridriver` with the device's
  Remote Automation on. Stage 0 picks one by trying both, and until one works the phone
  reports only what the probe can see.
- **Drives.** One drive per app, run identically on every arm: the untouched load, the
  early-input navigations, the steady-state session (toggles, navigation, typing, sign-in
  and favourite), the scroll runs, the soak. A drive is the correctness gate when run with
  screenshots.
- **Statistics, set in stage 0.** Interleaved rounds with rotating order, as
  `bench/heavy-list` does, so drift lands on every arm. Stage 0's pilot on the three
  baseline arms fixes, before any other arm is built: the repetition count per metric
  (independent navigations, not samples within one session), the minimum effect worth
  reporting, and the tie rule (overlapping intervals read as a tie, and the report says
  so). That count is the count for every later arm. Failures and timeouts are kept, not
  dropped.
- **Lighthouse** runs once per arm on the Chrome legs (Mac and Android) as a cross-check
  only; it audits through Chromium and cannot score Safari or Firefox. Its numbers go in
  the round's records and in §9's headline row for those legs, never in the table.
- **The report.** `report.mjs` writes the round's tables into `results/<round>/` with the
  pins, the device and browser versions, the thermal and cache state of every run, and
  every arm's build log. The Krausest lane's results are copied in from its own harness.

## 7. Fairness rules

Written down before any arm is built, because the memorable mistake in a shootout is a
rule invented after a number. Charlie's standing rule for the native benchmarks holds
here: a choice that matches what the framework's own documentation does needs no ask;
nothing is ever done to make a comparison unfair.

1. Production builds with each framework's documented production configuration, served by
   the framework's documented production server. No flags its documentation does not
   recommend, no flags removed.
2. The same CSS, the same data, the same image dimensions and formats, served by the same
   proxy. An arm may restate the CSS in its own idiom (scoped styles, utility classes)
   but not change what it paints.
3. The same observable contract, stated before porting: data volume, page size, the
   list's extent, what a mutation shows before and after the server answers, what is
   cached, and what must be rendered. A Conduit arm that pages by a different count or
   skips a rendered element fails the gate, not the comparison.
4. The 10,000-row list may use the virtualisation library the framework's documentation
   recommends, or the framework's own, under the extent gate. An arm that windows the
   list by hand says so. Where a framework has none and its community has one, the arm
   uses the community's and names it.
5. Every arm emits `app:interactive` (§4.1) when its root is live. It is diagnostic; the
   early-input runs are the result, and they catch a mark placed early.
6. Two arms where it matters: idiomatic (what the documentation's getting-started path
   produces, which is what an agent produces) and expert-tuned (what a framework's own
   advocate would ship), each labelled, with the same tuning opportunity for every
   framework in the ladder, Exact included. Conduit's community pins are idiomatic by
   construction and get an expert arm where the framework's maintainers publish one or a
   knowledgeable reviewer tunes one.
7. Community Conduit implementations are pinned at a commit and taken as they are, with
   their framework version upgraded to the round's pin only when the upgrade is
   mechanical. An implementation two majors behind is not evidence about the current
   framework: it is entered in a labelled historical table, not the headline one, and the
   headline arm for that framework is a current-major port under §8.
8. Exact's arm follows the same rules: the production JS build delivery publishes, served
   by the render server, no bench-only flag, and an expert arm only if it is labelled as
   one.
9. A gap in Exact that an arm exposes is fixed in Exact after the round and the arm is
   rerun in the next, never patched around in the arm. The gap is the finding. The round
   that found it is published as found (§12 D5).

## 8. Builders and arms

- **Conduit.** From the RealWorld catalogue, pinned, checked at pin time for building
  against current tooling. Where no current-major implementation exists (Octane, Ripple,
  Datastar, Arrow.js, VanJS, Nue, Lynx web, Imba and Leptos are the likely cases), an
  agent ports the React arm under the bake-off's `build.mjs`. The port's source arm is
  recorded. Stage 1 pilots three difficult catalogue arms first and records what their
  resurrection cost, before the rest are scheduled.
- **The feed.** Built by agents from `bench/heavy-list/SPEC.md` and a web addendum (the
  viewport, the scroll container, the `app:interactive` mark, the chip's selector, the
  extent gate), with the bake-off's builder prompt adapted per framework. The bake-off's
  builders are GPT-6.1 Sol through `codex exec` and Claude Opus 5.5 through `claude -p`.
- **Who signs an arm.** A performance arm in the headline table is signed by a person who
  knows the framework, having reviewed it against the framework's documentation and
  tuned the expert variant, or it is labelled agent-built in the table. A second model's
  pass is a review, not a signature.
- **Calibration before the field grows.** For React and Solid, the feed is built two
  ways: by the agent pipeline, and by adapting a known-good public virtualised list. The
  difference is published. The field grows past the ladder only when that difference is
  inside the round's noise; until then the table's agent-built arms carry that label and
  the delta beside them.
- **The authoring table.** Comparative authoring costs are published only for arms both
  builders built, each framework by each model under the same budget, including a fresh
  Exact build (an app that has been tuned in the repository is not a fresh authoring
  sample). Otherwise the table reports per-arm cost without ranking frameworks by it.
- **Exact's arms.** `apps/realworld` as it is, with the API origin made configurable,
  served by the render server, and the feed arm of §2.2.
- **Expo web.** Built the Expo way (`npx create-expo-app`, Expo Router, `expo export
  --platform web`), by an agent from the same specs, with the expert pass by someone who
  knows Expo. Its numbers are a baseline for Expo's own work, so the round records where
  its time and bytes go, not only the totals.
- **The Krausest entry.** Written by hand to that project's contract and offered upstream
  once it passes its checks unchanged.
- **Hello-worlds.** Each framework's CLI template, unmodified, built for production.

## 9. Reporting

The round's report has one table per leg, one row per arm and the columns of §4, each
cell a median with its noise band from the repetition count stage 0 set. The headline
load table is iPhone Safari at the 4G condition, because §1's first question is a phone
on a network; Mac Chrome unthrottled is the interaction and scroll lab and the table where
a compiled client framework is expected to lead. A short findings section says what Exact
won, what it lost, and what each loss traces to, in the discipline of the perf program:
the losing metric is traced at the frame level before anything is built to fix it.
Publication outside the team is §12 D5.

No published metric covers §4, and r1's attempt at one did not survive review: a ratio
to the round's best arm is undefined when the best is zero (CLS, lost taps, blank samples,
late callbacks are often zero), a geometric mean is a weighting whether or not it is
called one, and a mean over whichever metrics an arm happened to measure compares
different questions. There is no index and nothing is sorted by one. Instead:

1. **The familiar numbers, as a headline row.** Each arm's Lighthouse Performance score on
   the Chrome legs (it cannot audit Safari), and each arm's lab observations of LCP, INP
   and CLS on the phone legs beside the Core Web Vitals "good" thresholds (2.5 s, 200 ms,
   0.1). A lab observation is not a Core Web Vitals pass, which is the 75th percentile of
   real visits over 28 days, and the row says "observed", not "passes".
2. **Win, tie and loss counts against Exact**, per group of §4, from the per-metric
   comparisons under the tie rule. This is the summary, and it is honest about ties.
3. **Three conclusions kept apart.** What these implementations feel like (the tables),
   what the framework costs (the hello-worlds, the marginal bytes, the Krausest lane and
   the idiomatic-versus-expert deltas), and what the authoring cost (§4.9). The report
   does not attribute a difference in the first to either of the others without a
   measurement that isolates it.

## 10. Stages

| Stage | What lands | Done when |
|---|---|---|
| 0, the pipeline and the feasibility spike | the streaming proxy and probe with its calibration, the Conduit server, the drives, `report.mjs`; the three baseline arms on Mac Chrome; the physical iPhone's touch route and the metric-by-engine support matrix on the pinned Safari, Chrome and Firefox; the pilot that fixes repetition counts, the minimum effect and the tie rule | three baseline arms through three rounds with noise reported and smaller than their differences; a touch route that works on the phone or a written finding that none does; the support matrix |
| 1, Conduit on the ladder | the ladder's Conduit arms (pinned upstream where current, ports where not, three difficult pins piloted first), Mac Chrome, Mac Safari and the phones | the first Conduit tables, every arm through the gate |
| 2, the feed on the ladder | an Exact feed arm that passes the extent gate (or the whole-feed app, labelled); the ladder's feed arms, idiomatic and expert; the React and Solid calibration pair | the feed tables on five legs; the calibration delta published; the first findings section |
| 3, the field | Tier 1 and then Tier 2 under the gates (noise known, calibration delta inside it); Firefox; the Krausest entry; hello-worlds; the soak and browser CPU on the named subset; the authoring table under the crossed design | the whole report |
| 4, the gaps | an LLP in the form of LLP 1053: what Exact lost, which losses to fix, each a lane | the rerun of the arms the fixes touched, published beside the original round |

Stage 0 can start before §12 is decided; nothing in it depends on the field. An owner, a
budget and a time limit per stage are named when the lane starts, and a stage that
overruns its limit reports what it has rather than adding arms.

## 11. What is already known

The one web comparison so far (`bench/dioxus/README.md`, 2026-10-03, Chrome on a Mac,
three rounds) set expectations the first round will test. Exact's bounded feed led Dioxus
Web on first row (45 vs 129 ms), LCP (112 vs 172 ms), bytes (7.54 vs 7.85 MB) and main
thread at every speed, with no blank samples and at most one late callback per run for
either; it still used more JS heap (4.1 vs 1.8 MB) and its taps were slower (24 vs 16 ms
by Event Timing, at its 16 ms floor). The whole-feed app's page weight (the feed shipped
three times) and its 80 to 96 ms taps are what the bounded answer fixed, and the extent it
gave up (§2.2) is what stage 2 owes. The JS target's runtime is about 20 KB (LLP 1071),
which is the hello-world the size column will show. LLP 1071 §1's RealWorld numbers under
the mobile profile are the Conduit baseline the first round is compared against.

## 12. Decisions for Charlie

The panel's recommendations are noted where they differ from r1's.

- **D1, scope of the first round.** r2: the ladder of §3.5 on both apps plus the
  Krausest lane, growing to Tier 1 and Tier 2 under §10's gates. All three reviewers
  recommended a narrow first round (Astra 96, Grok 80, Muse 88). The alternative is r1's
  Tier 1 at once, which the panel judged would spend the budget on ports and devices
  before the method was proven.
- **D2, idiomatic and expert arms.** Both for the ladder's feed arms with equal tuning
  opportunity, and an expert Conduit arm where a knowledgeable reviewer tunes one (Astra
  94). The alternative is idiomatic only, which is what an agent produces and lets every
  framework's advocate dismiss the table.
- **D3, the Krausest entry's markup.** Contract has no `table` tag. Ask the project first
  whether a keyed non-table entry can be official; if not, either a table tag in Contract
  (a Contract question with a web-is-the-standard answer, decided on its own merits, not
  for a benchmark) or no entry and a line saying why. No privately adapted check (all
  three, 82 to 98).
- **D4, measuring inside Safari on the iPhone.** The touch route and the entry-type
  matrix move to stage 0 (all three). Memory, CPU and browser CPU on the phone need
  `xctrace` against Safari's content process on a developer-mode phone; tried in stage 0,
  timeboxed, and the columns read "not measured" if it does not hold.
- **D5, publication.** The panel disagreed with r1: publish the first methodologically
  valid round, losses included, after a private factual-correction window for the
  frameworks' advocates, then publish the rerun after Exact's fixes beside the original
  (Astra 96, Grok 72, Muse 72). r1's alternative, publishing after the fixes, is a
  different claim from the one rule 9 makes.
- **D6, who builds the arms.** Upstream for Conduit where current; a person who knows the
  framework signs a headline performance arm or it is labelled agent-built; the authoring
  table under the crossed design of §8 (all three, 70 to 97). Alternating two models does
  not answer "that model is bad at Svelte".
- **D7, the machine.** The Studio, dedicated, with the chip, load, power, thermal and
  display conditions written into the round as `bench/dioxus/README.md` did (all three,
  64 to 93). The alternative is the mini, which shares its time with the verify offload.
- **D8, latency.** One latency per leg at the proxy edge, applied to every arm's server
  and the API alike, with a 0 ms column beside the leg's value so framework time and
  network time separate (Grok 78, Astra 96). r1's separate 50 ms knob for server-driven
  arms stacked with the leg's profile and charged them twice.

## 13. Open questions

- Whether Vue's Vapor mode, Solid 2.0, Octane and Ripple are stable at pin time, and
  whether an unstable arm is labelled or deferred a round.
- Whether the RealWorld catalogue's implementations still build against current tooling;
  stage 1's pilot of three difficult pins answers it with a cost.
- Which iPhone touch route works, XCUITest on the device or `safaridriver` with Remote
  Automation, and at what tap spacing; stage 0 answers it.
- Which `PerformanceObserver` entry types the pinned Safari and Firefox deliver (paint,
  `largest-contentful-paint`, `layout-shift`, `event`); stage 0's matrix answers it from
  the engines, not from documentation.
- Whether an Exact feed arm with the bounded answer and the full extent is a small change
  or a Contract one; stage 2 finds out first.
- Whether Flutter web and Blazor can share the CSS at all, or are entered as "the same
  screen, their own styling", labelled.
