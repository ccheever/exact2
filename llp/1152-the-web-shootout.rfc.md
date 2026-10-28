# LLP 1152: The web shootout — Exact against every plausible web framework

**Type:** RFC
**Status:** Draft r1, 2026-10-28. Not yet reviewed. §12 lists what Charlie decides.
**Systems:** a new `bench/web-shootout/` (the measuring proxy and probe, the Conduit server, the drives, the report); `apps/realworld` (its API origin becomes configurable, §2.1); `bench/heavy-list` and `bench/dioxus/web` (the feed's data and the Chrome harness this one extends); `bench/bakeoff` (`build.mjs` and `drive.mjs`, reused for agent-built arms and for touches on the iPhone); the web JS target (`host/web-js`), which is the arm under test
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-28
**Implementer:** unassigned. §10 is staged so one lane can start on the pipeline before the field is final.
**Related:** LLP 1071 (the JS target, what ships on the web); LLP 1053 and LLP 1050 (the list benchmarks and what they found missing); LLP 1116 (the iOS bake-off, whose builders and grader this reuses); LLP 1087 and LLP 1102 (the authoring bench, which the secondary table continues); LLP 1027.004 (the bounded answer, which the feed arm must use); `bench/dioxus/README.md` (the one web comparison so far and its last standings); `bench/heavy-list/SPEC.md` (the feed); the RealWorld project ([docs.realworld.show](https://docs.realworld.show)) and its implementation catalogue; [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark) (Krausest)

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

The first round answers two questions: where does Exact win on the web, and what does it
lose that a kernel or host change would fix. The losses become the next LLP, as LLP 1053
and LLP 1116 did for the native benchmarks.

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
not collected.

## 2. The apps

### 2.1 Conduit (RealWorld)

The Medium clone of the RealWorld project: a feed of articles with tags and pagination, an
article page with comments, author profiles, favourites, a Markdown editor, sign-in and
settings. Exact's version is `apps/realworld` (LLP 1071's anonymous HTML for public routes,
the runtime preloaded and run after first paint, a press before it replays once).

Why this app: the RealWorld catalogue has a maintained implementation in most of the field,
so the competitors are written by people who chose that framework, not by us. The
comparison cannot be dismissed as a strawman, and the port cost is near zero for Tier 1.
Its weaknesses are the opposite side of the same fact: implementations differ in age,
quality and feature coverage, and some are idiomatic to a version two majors back. §8 says
how each is pinned and gated.

The API. RealWorld's hosted API is a shared public server, so it is not measured against.
`bench/web-shootout/conduit/` is a local server implementing the Conduit API
specification over an in-memory store seeded from one fixture (a recording of the hosted
API's public reads, with authors, articles, tags and comments, deterministic per round).
Every arm, Exact's included, is built against it. Exact's `apps/realworld/app.ts` names the
hosted origin in a constant and a grant; the bench build replaces both. The server adds a
configurable round trip (§5) so server-driven arms are not flattered by localhost.

Routes measured: the public home feed (cold load, the landing case), an article page
(the LCP case), the paginated global feed (client navigation), the editor (typing), and
sign-in followed by a favourite (an authenticated mutation).

### 2.2 The feed

`bench/heavy-list/SPEC.md`: one list of 10,000 rich messages from the same `messages.json`
and the same 104 JPEGs, with quotes, paragraphs of styled runs, photo grids, link cards and
reaction chips; a tap on a chip increments its count; live mode inserts a message at the
top and bumps a reaction every 250 ms while the list keeps the reader's position. The iOS
benchmarks (SwiftUI, UIKit, Expo) and the Dioxus comparison measured this screen, so a
new series lines up with the old.

Why not the Bluesky clone or the bake-off's feed spec: the clone is too large to port to
twenty frameworks honestly, and the bake-off's spec is an iPhone app with navigation and
persistence, which is the authoring question (LLP 1116), not the runtime one.

Exact's arm is the bounded-answer app (`bench/dioxus/exact-bounded`, LLP 1027.004). The
whole-feed arm shipped the feed three times and paid 80 to 96 ms per tap crossing the wasm
seam (`bench/dioxus/README.md`, Findings); it is not a fair representative of the JS
target and is not entered.

### 2.3 Krausest's js-framework-benchmark

The table of 1,000 rows with create, replace, partial update, select, swap, remove, create
10,000, append and clear, timed by its own harness in Chrome, with memory and startup
columns. It is run as the project runs it, against its pinned Chrome, and Exact's keyed
entry is written to its contract. The rest of the field is already there, including the
specialists that are not products (mikado, ivi, blockdom, voby, sycamore). What we gain is
an external number nobody can say we chose the conditions for; what it cannot show is
anything about loading, scrolling or a phone.

The harness checks a specific DOM shape (`tbody > tr > td > a` with classes and ids).
Whether Exact's JS target can emit it or the entry needs a check adapted to Exact's markup
is §12 D3.

## 3. The field

Pinned at the round's date to the latest stable release, or the latest pre-release where
the stable one is not what the framework is now claiming (Vue's Vapor mode, Solid 2.0,
Octane and Ripple). Each arm records its version and commit in the round's results.

### 3.1 Baselines

| Arm | Role |
|---|---|
| Static HTML | the floor for load: the same markup, no script |
| Vanilla JS | the floor for runtime: hand-written DOM, no framework |
| Exact, JS target | the arm under test, the production build delivery publishes |
| Exact, wasm target | reference only, Chrome only (`--wasm` is not what ships, LLP 1071 §7) |

### 3.2 Tier 1: both apps, every environment

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
| Phoenix LiveView | server-driven UI over a socket, measured at a simulated round trip |
| Hotwire Turbo | the same architecture from the Rails side |
| Flutter web (CanvasKit or skwasm) | draws its own pixels; the architecture people assume Exact has |
| Lynx web | ByteDance's one-source web and native, like Exact from the DOM side |
| Blazor WebAssembly | .NET on the web; asked about more than used |
| Dioxus web | already measured (`bench/dioxus`); rerun on the new pipeline so the numbers are one series |

### 3.4 Not entered

Kasper.js, petite-vue, Million.js, Aurelia, Hyperapp, FAST, Enhance, Fresh: each is a
thinner version of an arm already in, or has no implementation base. The Krausest
specialists appear in that lane's table without an app. jQuery is not a framework in the
sense asked.

## 4. Metrics

Every metric names how it is measured so two rounds agree. "The probe" is §6's script,
injected into every arm the same way. "CDP" is Chrome's protocol, available on the Mac and
the Android legs only. Where a metric has no Safari route it says so, and the Safari cell
reads "not measured" rather than a number from a different method.

### 4.1 Load (question 1)

| Metric | Definition | Measured by |
|---|---|---|
| TTFB | navigation start to first response byte | Navigation Timing; SSR and server-driven arms only (static arms are the file server's constant) |
| First Contentful Paint | the standard | `PerformanceObserver` `paint` |
| Largest Contentful Paint | the standard | `PerformanceObserver` `largest-contentful-paint`, finalised at first input or 10 s |
| Cumulative Layout Shift | the standard, over the load and the first scroll | `PerformanceObserver` `layout-shift` |
| Total Blocking Time and long tasks | sum of long-task time over 50 ms, and their count, from FCP to quiet | `PerformanceObserver` `longtask` (Chrome and Firefox; Safari does not expose it, so TBT is "not measured" there) |
| Interactive | the arm's own mark `performance.mark('app:interactive')` when its root is live (hydrated, resumed, or the SPA mounted); Exact's is the `bootMs` the JS target writes on its root (`host/web-js/rt.js`), which the bench build re-emits as the mark | the probe reads the mark; §7 requires it of every arm and the early-tap probe verifies it |
| Quiet | the end of the first 2 s window after LCP with no long task | derived |

Each is recorded cold (fresh profile, empty cache) and warm (second navigation in the same
profile), at each network and CPU condition of §5.

### 4.2 Early interactivity (question 1)

The metric Exact and Qwik claim and the standard vitals miss: what happens to a tap before
the page is ready.

| Metric | Definition |
|---|---|
| First successful tap | time from navigation start to the first tap that takes effect |
| Early taps lost | of the taps between FCP and the first success, the fraction that never took effect |
| Early taps deferred | the fraction that took effect later than 100 ms after the tap |

Procedure: from FCP, the probe taps the first article's favourite control (Conduit) or the
first row's reaction chip (the feed) every 50 ms until 3 s past quiet, through a real
input on each device (§6), and records for each tap whether and when the expected state
change appeared. A lost tap and a deferred tap are different outcomes and are reported
separately: Exact replays a press once, Qwik resumes the handler, React drops the event
until hydrated.

### 4.3 Interaction latency (question 2)

| Metric | Definition |
|---|---|
| Tap to paint, toggle | a favourite or reaction toggle: input timestamp to the next paint that shows the change; p50, p95, p99 over 20 |
| Tap to paint, navigation | open an article, then back: input to the next paint with the article's title (or the feed's first row) |
| Keystroke to paint | 200 characters typed into the editor (Conduit) or the composer (the feed) at 60 ms intervals; p50, p95 per key |
| INP | the standard: the worst interaction (p98) over the whole scripted session |

Measured by `PerformanceObserver` `event` with `durationThreshold` 16 and `first-input`,
as `bench/dioxus/web/bench.mjs` already does, with the paint confirmed by the probe's
next animation frame.

### 4.4 Scrolling (question 3)

The feed only, static and live mode, at the speeds `bench/dioxus/web` uses (3k, 6k, 12k,
24k px/s for 3 s each) and a flick-and-settle on the phones.

| Metric | Definition |
|---|---|
| fps | frames presented over the run |
| late frames per second | frames whose interval exceeded 1.5 display periods |
| worst frame | the longest interval |
| blank samples | the fraction of per-frame samples down the list's centre line that hit no row (white while scrolling) |
| rows per second | progress, by the row at the top each frame, with backward steps counted |
| main thread per frame | task, script, style and layout ms per frame (CDP `Performance.getMetrics`; not on Safari) |
| CPU during scroll | all browser processes (Chrome by `ps` on the Mac and `top` on Android; Safari by `ps` on the Mac; the iPhone by `xctrace` if §12 D4 allows, else not measured) |
| position stability, live mode | the content under the viewport's top after each insert, in px moved |

### 4.5 Memory (question 5)

JS heap (`performance.measureUserAgentSpecificMemory` where available, else CDP), renderer
private footprint (CDP `Memory` on Chrome; `ps` on macOS Safari; not measured on the
iPhone unless D4), DOM node count (`document.querySelectorAll('*').length`, uniform), and
GPU memory for the canvas arms (Chrome's `chrome://gpu` counters by CDP). Each at three
points: after quiet, after the full scroll of the feed (or ten paginated feed pages of
Conduit), and after a ten-minute soak of the scripted session in a loop. The growth from
the second to the third is the leak signal.

### 4.6 Energy (question 5)

Total CPU seconds of every browser process over the whole scripted session, as the
practical proxy for battery across hosts. Chrome and macOS Safari by process accounting;
the iPhone by `xctrace`'s energy template if D4 allows, else not measured.

### 4.7 Size (question 4)

Bytes transferred for HTML, JavaScript, CSS, fonts, data and images, and the request count,
raw and as served (brotli). Framework weight is reported two ways so an arm cannot hide
weight in the app: each framework's own hello-world from its CLI template built for
production, and the app's bytes minus that hello (the marginal cost of the app). For Exact
the hello is `apps/hello` on the JS target, and the marginal bytes split into runtime,
plan and data.

### 4.8 Correctness gate

Before an arm's numbers count it passes the same functional drive (§6): every measured
route and interaction produces the expected state, and a screenshot at each step matches
the reference within a tolerance the report shows. Axe-core's violation count is reported
as a column, not a gate. An arm that fails the gate appears in the table with its failure
and no numbers.

### 4.9 Authoring, the secondary table (question 6)

Lines of source, production build time, dev-loop reload time, and for the agent-built feed
arms the builder's tokens, wall clock and the bugs the expert pass had to fix. Octane,
Ripple and Arrow.js all market themselves as frameworks for agents to write, so this table
is where they compete with Exact's authoring claim (LLP 1087). It is separate from the
performance table and reported after it.

## 5. Environments

| Leg | Browser | Device | Conditions |
|---|---|---|---|
| Mac Chrome | Chrome, pinned major | the Studio (a quiet, dedicated machine; §12 D7) | CPU 1× and 4× slowdown; network unthrottled, 4G (9 Mbps, 170 ms RTT), slow 3G |
| Mac Safari | Safari, the OS's | the Studio | the same, with throttling at the proxy |
| Mac Firefox | Firefox, pinned | the Studio | unthrottled and 4G |
| iPhone Safari | Safari | iPhone 17 Pro Max, a real device | over Wi-Fi to the Studio, throttled at the proxy; touches through XCTest |
| Android Chrome | Chrome | Nothing 4a Pro (the mid-range phone the Android work measures on) | the same, touches through `adb`; CDP over `adb forward` |

Safari on the iPhone is the leg that matters most and the one nobody else publishes. The
Mac legs come first because they are the ones the pipeline can run unattended today.

Network and CPU conditions apply at the proxy and at the browser respectively, never by
Lighthouse's simulation. A server-driven arm (LiveView, Turbo, HTMX, Datastar) and the
Conduit API both see the proxy's round trip, so every arm's server is the same distance
away.

## 6. The harness

`bench/web-shootout/` extends `bench/dioxus/web/bench.mjs`, which already runs headed
Chrome through playwright-core with a fresh profile, synthesised scroll gestures, Event
Timing taps and blank sampling. What is new:

- **The measuring proxy.** Every arm is served through one local proxy that injects the
  probe as the first script in `<head>`, applies the leg's throttling and round trip, and
  records every response's bytes. The probe costs each arm the same bytes and the same
  parse time, and it is the only way to measure inside Safari on a phone, where there is
  no CDP. The probe collects §4's observers and marks and posts them to the proxy at the
  end of the session.
- **The Conduit server** (§2.1), with its round-trip knob, behind the same proxy.
- **Real input.** Chrome legs synthesise through CDP as today. Safari on the iPhone gets
  real touches from `bench/bakeoff/drive.mjs`, which drives any app by bundle id through
  the XCTest runner, Safari included. Android gets `adb shell input` and, for scroll, a
  recorded swipe. macOS Safari gets `safaridriver`'s actions. The early-tap procedure needs
  taps at 50 ms spacing, which XCTest cannot promise; the phone legs report it at the
  spacing the device achieved, which the probe records.
- **Drives.** One drive per app, in the bake-off driver's vocabulary, run identically on
  every arm: load, early taps, scroll, toggles, navigation, typing, sign-in and favourite,
  the soak. A drive is the correctness gate when run with screenshots.
- **Statistics.** Interleaved rounds with rotating order, as `bench/heavy-list` does, so
  drift lands on every arm. Medians with p95 in the table and run-to-run noise beside
  them: a difference inside the noise reads as a tie, and the report says so.
- **Lighthouse** runs once per arm per leg as a cross-check only. Its numbers go in the
  round's records, never in the table.
- **The report.** `report.mjs` writes the round's tables into `results/<round>/` with the
  pins, the device and browser versions, and every arm's build log. The Krausest lane's
  results are copied in from its own harness.

## 7. Fairness rules

Written down before any arm is built, because the memorable mistake in a shootout is a
rule invented after a number. Charlie's standing rule for the native benchmarks holds
here: a choice that matches what the framework's own documentation does needs no ask;
nothing is ever done to make a comparison unfair.

1. Production builds with each framework's documented production configuration. No flags
   its documentation does not recommend, no flags removed.
2. The same CSS, the same data, the same image dimensions and formats, served by the same
   proxy. An arm may restate the CSS in its own idiom (scoped styles, utility classes)
   but not change what it paints.
3. The 10,000-row list may use the virtualisation library the framework's documentation
   recommends, or the framework's own. An arm that windows the list by hand says so.
   Where a framework has none and its community has one, the arm uses the community's
   and names it.
4. Every arm emits `app:interactive` (§4.1) when its root is live. The early-tap probe
   catches a mark placed early.
5. Two arms where it matters: idiomatic (what the documentation's getting-started path
   produces, which is what an agent produces) and expert-tuned (what a framework's own
   advocate would ship), each labelled. Tier 1's feed arms get both; Conduit's community
   pins are idiomatic by construction and get an expert arm only where the framework's
   maintainers publish one.
6. Community Conduit implementations are pinned at a commit and taken as they are, with
   their framework version upgraded to the round's pin only when the upgrade is
   mechanical. An implementation two majors behind is entered at its own version, which
   the table shows.
7. Exact's arm follows the same rules: the production JS build delivery publishes, no
   bench-only flag, and an expert arm only if it is labelled as one.
8. A gap in Exact that an arm exposes is fixed in Exact after the round and the arm is
   rerun in the next, never patched around in the arm. The gap is the finding.

## 8. Builders and arms

- **Conduit, Tier 1 and 2.** From the RealWorld catalogue, pinned. Where no implementation
  exists (Octane, Ripple, Datastar, Arrow.js, VanJS, Nue, Lynx web, Imba, Leptos are the
  likely cases; the catalogue is checked at pin time), an agent ports the React arm under
  the bake-off's `build.mjs` and an expert pass by a different model family reviews it
  against the framework's documentation. The port's source arm is recorded.
- **The feed, every arm.** Built by agents from `bench/heavy-list/SPEC.md` and a web
  addendum (the viewport, the scroll container, the `app:interactive` mark, the chip's
  selector), with the bake-off's builder prompt adapted per framework, then the expert
  pass. The bake-off's builders are GPT-6.1 Sol through `codex exec` and Claude Opus 5.5
  through `claude -p`; the shootout alternates them per arm as `pairs.json` does, so no
  framework is built by one model only.
- **Expo web.** Built the Expo way (`npx create-expo-app`, Expo Router, `expo export --platform web`), by an agent from the same specs, with the expert pass by someone who knows Expo. Its numbers are a baseline for Expo's own work, so the round records where its time and bytes go, not only the totals.
- **Exact's arms.** `apps/realworld` as it is, with the API origin made configurable, and
  `bench/dioxus/exact-bounded` for the feed, both on the production JS build.
- **The Krausest entry.** Written by hand to that project's contract and offered upstream
  once it passes its checks.
- **Hello-worlds.** Each framework's CLI template, unmodified, built for production.

## 9. Reporting

The round's table has one row per arm and the columns of §4 for the primary leg (Mac
Chrome, unthrottled), with the other legs as further tables. A short findings section says
what Exact won, what it lost, and what each loss traces to, in the discipline of the perf
program: the losing metric is traced at the frame level before anything is built to fix
it. Publication outside the team is §12 D5.

No published metric covers §4. Lighthouse's Performance score is five lab load metrics
under simulated throttling (Total Blocking Time 30%, LCP 25%, CLS 25%, FCP 10%, Speed
Index 10%, unchanged since Lighthouse 10), and the Core Web Vitals (LCP, INP, CLS) are
field thresholds for load, one interaction and stability. Neither sees scrolling, memory,
energy, bytes or an early tap. The report therefore shows three things, in this order:

1. **The familiar numbers, as a headline row.** Each arm's Lighthouse Performance score
   on the phone legs, and whether it passes all three Core Web Vitals at their "good"
   thresholds (2.5 s, 200 ms, 0.1) on the same legs. Readers know these, so they anchor
   the rest. The score is not a column of the table because its throttling is a model
   and it moves several points run to run.
2. **A per-group index, the Krausest way.** Each metric is a ratio to the best arm in the
   round on that leg, and each group of §4 (load, early interactivity, interaction
   latency, scrolling, memory, energy, size) is the geometric mean of its ratios. Seven
   numbers per arm; 1.0 is the best arm. Ratios need no weights, which is what makes the
   index defensible to a framework's advocates. A metric an arm does not measure on a leg
   is left out of that arm's mean and the cell says how many it averaged.
3. **One overall index as the sort key only.** The geometric mean of the seven group
   indexes orders the table, with the seven beside it. It is not a headline, because one
   number averages away the trade-off this shootout exists to show: an arm that leads on
   load and loses on scroll.

## 10. Stages

| Stage | What lands | Done when |
|---|---|---|
| 0, the pipeline | the proxy and probe, the Conduit server, the drives, `report.mjs`; three arms (static, vanilla, Exact) on Mac Chrome | three rounds whose noise is reported and is small against the differences |
| 1, Conduit Tier 1 | the pinned community arms, the agent ports, Mac Chrome and Safari | the first full Conduit table, with every arm through the gate |
| 2, the feed and the phones | the agent-built feed arms, idiomatic and expert; the iPhone and Android legs | the feed table on five legs; the first findings section |
| 3, the rest | Tier 2, Firefox, the Krausest entry, hello-worlds, the soak and energy columns, the authoring table | the whole report |
| 4, the gaps | an LLP in the form of LLP 1053: what Exact lost, which losses to fix, each a lane | the rerun of the arms the fixes touched |

Stage 0 can start before §12 is decided; nothing in it depends on the field.

## 11. What is already known

The one web comparison so far (`bench/dioxus/README.md`, 2026-10-03, Chrome on a Mac)
set expectations the first round will test: Exact's bounded feed matched or beat Dioxus
Web on scroll (no late frames at any speed, less main-thread time) and lost on bytes, heap
and tap latency until the bounded answer landed. The page weight that comparison found
(the feed shipped three times) is the kind of gap §10 stage 4 exists for. The JS target's
runtime is about 20 KB (LLP 1071), which is the hello-world the size column will show.

## 12. Decisions for Charlie

- **D1, scope of the first round.** Tier 1 on both apps plus the Krausest lane, Tier 2 on
  Conduit only. Recommended as written; the alternative is Tier 1 only, which answers the
  six questions and defers the long tail.
- **D2, idiomatic and expert arms.** Both for Tier 1's feed arms, as §7 rule 5 says. The
  alternative is idiomatic only, which is cheaper and is what an agent produces, but lets
  every framework's advocate dismiss the table.
- **D3, the Krausest entry's markup.** The harness checks `tbody > tr > td > a`. Contract
  has no `table` tag (`contract vocab table` answers with `display="grid"` or a `column`
  of `row`s), so the JS target emits no table markup today. The choice is between a table
  tag in Contract, which is a Contract question with a web-is-the-standard answer, and
  submitting the entry with a check adapted to Exact's markup, which the project may not
  accept. The lane asks the project first and reports.
- **D4, measuring inside Safari on the iPhone.** Memory, CPU and energy need either
  `xctrace` against Safari's content process on a developer-mode phone, or WebKit's
  inspector protocol through `webinspectord`. The recommendation is to try `xctrace` in
  stage 2 and report the columns as not measured if it does not hold; the probe's in-page
  metrics do not depend on it.
- **D5, publication.** Internal for the first round; a public write-up after the second,
  when the gaps found in the first have been fixed and rerun. The alternative is to publish
  the first round as it is, losses included, which is more credible and less flattering.
- **D6, who builds the arms.** The bake-off's two builders alternating, with the expert
  pass by a third family. The alternative is one builder for all arms, which is simpler
  and leaves the table open to "that model is bad at Svelte".
- **D7, the machine.** The Studio, dedicated to the rounds while they run. The alternative
  is the mini, which is quieter in thermal terms but shares its time with the verify
  offload.
- **D8, the server-driven arms' round trip.** Measured at 0, 50 and 150 ms with the table
  showing 50 ms. The alternative is to show 0 ms, which is what their own demos show and is
  not what a user gets.

## 13. Open questions

- Whether Vue's Vapor mode, Solid 2.0, Octane and Ripple are stable at pin time, and
  whether an unstable arm is labelled or deferred a round.
- Whether the RealWorld catalogue's implementations still build against current tooling;
  each is checked at pin time and a dead one is replaced by an agent port and labelled.
- Whether the early-tap procedure at 50 ms spacing is achievable on the phones through
  XCTest and `adb`; the probe records the spacing achieved and the report shows it.
- Whether Flutter web and Blazor can share the CSS at all, or are entered as "the same
  screen, their own styling", labelled.
