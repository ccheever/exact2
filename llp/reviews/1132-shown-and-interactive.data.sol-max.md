# LLP 1132 — data correctness, trust and usefulness, by Sol (gpt-6.1-sol, max), of 700bc0ac10, 2026-10-18

The data is worth producing as **declared readiness measurements**. As specified, I would not use exported TTI percentiles alone as a measure of users successfully reaching usable UI.

Reviewed `700bc0ac10`; RFC references below use that revision. Findings are ranked by impact. Numerical examples are hypothetical.

1. **Trustworthiness — successful TTI percentiles conceal abandonment and failure.**

   Suppose 900 marked visits become interactive in 100 ms, while 100 are abandoned after eight seconds. Observe receives 900 successful values: p99 can remain 100 ms despite 10% never reaching readiness. A failure before the first frame leaves no measurement or cancellation at all. Router events partially preserve navigation attempts, but cannot recover every dialog, section or state-driven visit. ([RFC:133](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:133), [RFC:432](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:432))

   Dashboard readers need completion rates alongside latency; aggregators otherwise reward releases that lose their slowest attempts. **Smallest remedy: rule and consumer work.** Report successful and cancelled counts, by reason, alongside percentiles. A pre-frame cancellation carrying `shown:false` would preserve the missing denominator without a new event kind. If that remains excluded, document that cancellation rates cover shown measurements only. Never turn cancellation duration into TTI.

2. **Usefulness — a long time contains almost no explanation of its delay.**

   A five-second TTI could mean activation took five seconds, the expression stayed false, the scene remained inactive, or the web input module arrived late. Under the settled subtree rule, even a mounted control in an unselected tab can hold a root mark while the visible native controls already work. The terminal records have the same start and time in these cases. D2 knows the gates; D7 does not require exporting them. ([RFC:177](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:177), [RFC:400](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:400))

   An engineer cannot choose the right fix. **Smallest remedy: fields.** Record when all non-frame conditions held and which condition cleared last; expose current pending gates through the existing timing diagnostic. The remaining interval identifies the frame wait. This would improve actionability more than any other addition. Full gate histories can wait.

3. **Trustworthiness — journal loss preferentially removes the facts needed to interpret later values.**

   A marked root remains false while router and section churn produces 5,000 events before service attachment. Only the newest 4,096 survive. A web page hidden before attachment saves only 512. Launch anchors, the root’s marked status, earlier screen history and initial device state can disappear while later completions survive. ([README:31](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:31), [LaunchParts.swift:49](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/LaunchParts.swift:49))

   Missing history must not silently become “cold,” zero duration or an ordinary complete session. The list-row refusal reduces one source of overflow; it does not establish completeness. **Smallest remedy: field and rule.** Expose a dropped-event count at attachment and web handoff, and mark dependent calculations incomplete when their prerequisites are absent. A kill before service loading can lose all timing data; that deserves a documented limitation. Durable early recording is separate work.

4. **Semantic correctness — “interactive” does not establish that the person can interact with that UI.**

   Screen A is shown, then covered until 2,000 ms. Its mark becomes true at 300 ms. Exact records 300 ms when hidden, or perhaps 316 ms when a changed frame is still drawn beneath the cover. A consumer reading “user can now tap A” expects 2,000 ms. Later scene inactivity similarly does not block an already activated app visit. The root can also finish before the launch screen’s entrance because the app visit has no entrance. ([RFC:137](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:137), [RFC:180](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:180))

   These are accepted policy costs, not reasons to reopen no-pause. **Smallest remedy: context fields and a sentence.** Carry completion basis and hidden/covered state, and describe the value as readiness under the visit rules. Otherwise dashboard readers and cross-install aggregators cannot separate unobstructed readiness from readiness beneath a cover.

5. **Semantic correctness — “shown” mixes presentation evidence with estimates and synthetic foreground milestones.**

   A web commit’s rendering opportunity occurs at 116 ms, but its pixels reach the display at 150 ms—or that intermediate rendering never becomes visible. The recorded value is 116 ms. Apple also assumes the render server presents at the next vsync; Linux can use an actual page flip. Separately, a previously shown route still hidden beneath a cover gets a new foreground `shown` time without becoming visible. ([RFC:106](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:106), [Launch.swift:257](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:257))

   Consequently, identical visible experiences need not produce identical values. **Smallest remedy: preserve provenance.** Retain the existing presentation-method information in exported metrics and identify the synthetic foreground case. Explain Apple’s estimate alongside the web estimate. This merits a field and documentation, not a new paint-detection mechanism.

6. **Semantic correctness and trustworthiness — Observe’s population differs from “all visits to this screen.”**

   A screen’s first visit reaches TTI in 100 ms; a later return takes five seconds. Exact journals the later visit’s five-second time, but Observe emits no second navigation `tti`. A dashboard reader following the documentation’s “per navigation” description could expect the regression to appear. The once-per-screen decision matches the installed Expo Router implementation; React Navigation differs. ([RFC:440](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:440), [useObserveForRouter.ts:91](/Users/admin/Developer/observe-lab/lexy/node_modules/expo-observe/src/integrations/expo-router/useObserveForRouter.ts:91))

   Startup TTI also changes source depending on whether the root is marked: root readiness versus the first qualifying screen event. **Smallest remedy: consumer-facing limitation and provenance.** State the population beside the metric, preserve the Exact SDK distinction, and carry the source visit’s identity and kind into the row. Keep the chosen policy; do not interpret its navigation TTI distribution as repeat-navigation performance.

7. **Trustworthiness — wrong marks are observationally indistinguishable from real readiness or real slowness.**

   An app marks `true` and passes the runtime gates at 120 ms, while its authored disabled state lasts until 1,000 ms. Exact records 120 ms. Conversely, a dynamically invalid value becomes false and can produce no TTI despite usable UI. On Linux, an image-dependent mark can remain false forever because neither image event fires. ([RFC:264](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:264), [RFC:518](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:518))

   `shown.marked` distinguishes no instrumentation from instrumentation, but cannot validate the declaration. **Smallest remedy: documentation and a narrow diagnostic.** Show invalid values and pending expression state in development diagnostics; teach authors to exercise success and error paths against actual input access. Linux image-event parity remains separate work, with an explicit warning against comparing those cohorts until it lands. No general “correct mark” detector is warranted.

8. **Semantic correctness — starts measure different intervals, even when the destination experience looks identical.**

   A handler spends 500 ms before committing, followed by a 16 ms frame wait: Exact correctly reports 516 ms, whereas a commit-based interval reports 16 ms. But a response-driven redirect after a two-second request starts at its own commit, potentially reporting only 16 ms. That value does not describe the original tap-to-destination journey. ([RFC:90](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:90))

   Launch intervals also differ: process start natively, navigation origin or prerender activation on the web; Observe startup durations use their own launch-end anchor. ([README:20](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:20))

   **Smallest remedy: a documented limitation and retained anchor labels.** Explain that cause is synchronous dispatch attribution, not end-to-end user intent. Make each exported interval’s anchor visible. Following asynchronous user journeys needs separate tracing, outside this RFC.

9. **Trustworthiness — raw marks need an explicit clock contract.**

   Current journal coordinates are seconds on Apple and Linux, but milliseconds on the web. Linux’s clock includes suspend time and its process-start resolution is commonly 10 ms. Thus 100 ms of work plus a 60-second suspend can produce roughly 60.1 seconds, despite very little execution. Wall-clock timestamps also serve a different purpose from duration coordinates. ([launch_marks.rs:3](/Users/admin/Developer/observe-lab/exact2/host/linux/src/launch_marks.rs:3), [marks.js:38](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:38))

   A generic journal consumer could mis-scale values or mistake environmental elapsed time for work. **Smallest remedy: rule and documentation.** Specify units, clock origin, suspend behavior and conversion rules for the new event API; retain process-start resolution. Compute durations within their session clock, using wall time for dating. No cross-host clock synchronization mechanism is needed.

10. **Trustworthiness — unlimited late completion is useful, but its interpretation changes.**

   If 950 measurements finish in 200 ms and 50 finish after three minutes, p99 legitimately approaches three minutes. If those 50 never mark, they vanish from successful percentiles instead. The 30-second service bound does not resolve this censoring: it loads services and measurement continues. Late Apple TTI also lacks a complete frame-statistics window. ([RFC:321](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:321), [RFC:521](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:521))

   **Smallest remedy: consumer rule and documentation.** Keep late times, but declare the observation window and completion population used for percentiles; absent frame statistics mean unknown, not smooth. Service loading can itself contribute work before a late TTI—or before screen-based startup TTI when an unmarked root settles early. That qualification deserves a sentence, not a new loading mechanism.

11. **Trustworthiness — identities enable reliable joins, but do not establish exactly-once aggregate delivery.**

   A server accepts a 400 ms metric, its response is lost, and the service retries. It can submit two copies where a consumer expects one. Existing wire records carry session identity but no explicit measurement key. Meanwhile buffered paint observations and held section events can arrive after events with later occurrence times. ([service.js:232](/Users/admin/Developer/observe-lab/exact2/modules/observe/web/service.js:232), [ObserveWire.swift:96](/Users/admin/Developer/observe-lab/exact2/modules/observe/apple/service/ObserveWire.swift:96))

   **Smallest remedy: rule and forwarding an existing identity.** Make mapping idempotent by `(session, measurement, kind)`, preserve that key downstream, and distinguish occurrence time from delivery order. Web recovery must retain the originating session and timestamps. Backend deduplication is separate work; without it, document delivery as best effort with possible duplicates and omissions.

12. **Usefulness — several measurements have no ordinary production reader.**

   A compose dialog becomes interactive in 350 ms: its journal contains that time, but Observe shows no dialog metric. The same applies to sections. Raw launch marks are useful inputs to Observe’s formulas; `settled` is only a timing diagnostic. A never-sending background stream can prevent settled even while the marked UI works. ([RFC:343](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:343), [RFC:443](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:443))

   These records are reachable through launch-module journal consumers, but the RFC provides no standard production query for them; agent/headless timing restrictions further limit reproduction. **Smallest remedy: document one concrete access path.** A section dashboard is future work. Before building one, add a compiled mark-site label: session sequence numbers distinguish instances, while repeated or absent author IDs cannot reliably group declarations across sessions.

The fields carrying the most value are measurement and owner identities, route pattern, entry/URL, router removals, cause/start, `shown.marked`, and cancellation reason. They support joining and classification. Blocker information is the most valuable missing addition.

I would trust successful milestones for a known mark definition on a specified host, and delivered cancellation reasons within their stated coverage. I would use raw launch marks to recompute explicit intervals. I would not trust unqualified cross-platform comparisons, successful-TTI percentiles as completion SLOs, or a long TTI as an explanation of what went wrong.

## The author's independent findings, written before reading Sol's (sha256 d04b65fe…)


Ranked by impact. RFC at 700bc0ac10.

1. **Correctness, high: Observe's startup `tti` fallback can count the user's own navigation.** D7: with an unmarked root, startup `tti` "comes from the first screen `interactive` in the launch session before the app goes to the background". If the launch screen is unmarked and the user taps at 5,000 to open a marked screen that is interactive at 5,300, startup `tti` is 5,300 ms. The tap itself proves the app was interactive earlier. Observe's docs ask for calls on other screens because of deep links, where the screen in place at launch differs; that case is the launch screen. Remedy (rule, small): take the fallback only from the launch screen's visit (the screen in place at launch, or a redirect's destination before the first frame). No launch-screen mark, no startup `tti`.

2. **Usefulness, high: nothing says what held an interactive time, or what a cancelled measurement was still waiting for.** A long `tti` can come from the expression (a slow resource), activation, the web's input module, an entrance, iOS's foreground activation or the frame. A `cancelled` with `left` cannot tell a user who left early from a mark that never becomes true. Remedy (field, small): `interactive` carries the condition that came to hold last (`expression`, `activation`, `input`, `entrance`, `scene`, `frame`); `cancelled` carries the conditions still unmet. The evaluator already knows both.

3. **Trust, medium-high: Observe's navigation `tti` is biased toward fast screens.** Measurements that end before interactive (the user leaves, a sheet's navigation supersedes, the background) send nothing to Observe. Slow screens are abandoned more often, so percentiles understate them, and a screen that usually shows a sheet early never reports. The journal keeps the cancellations. Remedy (sentence): the module's README states the bias; an abandonment metric waits for an Observe name.

4. **Correctness, medium: shown and interactive times of hidden or covered visits look like ordinary ones.** A screen returning beneath a fullscreen route is "shown" at a frame that does not show it; a screen beneath a sheet becomes "interactive" while inert. A consumer cannot filter these. Remedy (field, small): `shown` and `interactive` say whether the visit was hidden, or beneath a sheet or dialog, at that time.

5. **Correctness, medium: the web's screen times are estimates, one frame early, and its launch is not.** Screens are shown at "the rendering opportunity after the commit", before presentation; native hosts use presented frames; the web's launch uses paint timing. Cross-host comparisons of `cold_ttr` and `tti` skew by about a frame. Remedy (sentence where consumers read it, or a field naming the source).

6. **Usefulness, medium: the journal's events have no stated schema.** D7 names events and what they carry in prose, but not field names, units, the clock and its origin per host, or how a consumer orders events. A general metric needs a contract other consumers can rely on. Remedy (a short table in D7).

7. **Trust, medium: no deadline, so late times feed percentiles.** A mark that becomes true on an unrelated later change, or a launch interactive at 40 s after its services loaded, goes to Observe as is. The RFC leaves deadlines to consumers, and the Observe module states none. Remedy (sentence in the module's README, or a cap there).

8. **Correctness, low: the launch's start means different things.** Native hosts start at process start (pre-main included); the web at the navigation timing origin (DNS, TLS and server time included). Both are "from the user's intent", but a consumer comparing hosts should know. Remedy (sentence).

9. **Trust, low: the web keeps only the last 512 events for the next page** (`marks.js:332`), so the launch's own events drop first, `localStorage` failures are silent, and a session that never loads another page is lost: bounces, often slow ones, vanish. Remedy (keep the launch's events first; say it in D4).

10. **Trust, low: a mark set true too early cannot be told from a fast screen.** A literal `true` on a screen still showing placeholders gives `tti` = shown. Remedy (future, diagnostic): the timing log notes when a marked visit becomes interactive while its resources still load, as a hint; not a rule.

11. **Usefulness, low: sections, dialogs and settled reach only the timing log and the agent.** No consumer reads them yet. Remedy: none now; finding 6 makes them usable later.
