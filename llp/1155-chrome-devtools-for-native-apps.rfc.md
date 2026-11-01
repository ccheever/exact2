# LLP 1155: Chrome DevTools for native Exact apps

**Type:** RFC
**Status:** Draft r2 (r1 by Codex 2026-10-30; r2 the same day by Claude (Fable 5.1) for Charlie after a blind panel: GPT-6 Astra, GPT-5.6 Sol, Grok 4.7 and Claude Fable 5.1, `llp/reviews/rfc-2026-10-30-1155-r1.{astra,sol,grok,claude}.md`; Muse's quota was exhausted. §R lists the changes.)
**Systems:** Development tooling (`scripts/`), the Tamber build channel (LLP 1121 D13, 1122 D4), the agent's read operations (LLP 1012, 1035.002), the Apple hosts' inspect mode (`host/apple/Sources/ExactKit/Inspect.swift`); later candidates: other native hosts and the Hermes executor
**Author:** Codex for Charlie Cheever (r1); Claude (Fable 5.1) for Charlie Cheever (r2)
**Date:** 2026-10-30
**Revised:** 2026-10-30 (r2)
**Implementer:** Codex GPT-6 Astra, from 2026-10-30, for the Stage 0 gate and, if it passes, Stage 1 (§6). Charlie, 2026-10-30: "when you're happy with it, have astra implement it."
**Related:** [LLP 1012](1012-agent-api-v1.spec.md), [LLP 1035.002](1035.002-native-result-inspection.rfc.md), [LLP 1035.005](1035.005-contract-authoring-ergonomics.rfc.md), [LLP 1071](1071-exact2-web-target.rfc.md), [LLP 1115](1115-write-the-web-ship-the-platform.principle.md), [LLP 1121](1121-tamber.rfc.md), [LLP 1122](1122-design-tool.rfc.md), [LLP 1143](1143-what-the-diaries-ask-of-inspection.rfc.md), [LLP 1146.000](1146.000-capturing-app-feedback.rfc.md)

## R. Review, and what r2 changed

All four reviewers agreed that read-only is the right scope, that the Bun dev
tooling is the right home for an adapter, that the stock Styles pane is the
decision that can sink the experiment, and that the strongest alternative is a
browsable tree inside LLP 1122's inspector. Astra said READY with changes; Sol,
Grok and Claude said NOT READY. Grok would drop the Chrome frontend now, on the
protocol types alone (a `CSSRule` requires a selector; `origin` has four browser
values; nothing can say "platform-owned" or "not reported"); the other three
would run a cheap gate first. r2 runs the gate, as a stub-target test that takes
hours and no host work, because it answers Grok's question with a screenshot
instead of an argument and costs less than this review did. The changes:

| Change | Raised by | Where |
|---|---|---|
| The order of work was inverted: r1 built bounded reads on the build channel before testing whether the stock Elements and Styles panels can show an Exact node honestly, which is the one thing that can sink the experiment. Stage 0 is now two steps with no host change: a stub target serving four rows to the recorded frontend (hours), then, only if the four are visually distinct, a throwaway adapter over the agent carrier against a live app. The build-channel shape is Stage 1, only if Stage 0 passes. | all four | D1, §6 |
| The Styles projection is named as the hypothesis under test, with the candidate CDP encodings of each provenance kind listed and the one example that decides it: a native control whose runner row says the inherited page colour while AppKit paints its own tint. A `user-agent` origin for a platform-owned value would misstate LLP 1115 and is not a candidate. | all four | D3, §6 |
| Who the frontend is for. Agents read JSON from `agent.mjs` and will not open a GUI; the diaries (LLP 1143) ask for observations, not a new frontend. This is for people: the author checking a layout, the reviewer hunting tells. | Claude, Astra, Sol | §1 |
| The precedent was misread. React Native DevTools is a build of a fork (`react/react-native-devtools-frontend`, per the `debugger-frontend` package's `BUILD_INFO`), served over an inspector proxy, and it shows React components in its own panel; it never projects native views into Elements. It proves a non-browser can reuse the frontend, not that the stock Elements panel accepts a native tree. The stop rule changes with it: if the stock panel needs a fork, the answer is LLP 1122's inspector, not a fork. | Claude, Sol, Astra | §1, D2 |
| Identity needs the host's session generation: the kernel's incarnation can repeat when a host replaces its runner (`runner/src/agent.rs`, "Kernel incarnations can repeat across host replacements"), and `Inspect.swift` already keys a selection by `generation`. | Astra | D4 |
| A bounded reply is not bounded work: the macOS targeted `layout(_:)` begins with the full `layout()`, and the runner's subtree read materialises the subtree. The refactor is counted in Stage 1's budget and traversal work is measured, not only bytes. The runner already has a depth-1 form, `tree` with `shallow: true` and a target (`runner/src/agent/tree.rs`), so Stage 1's channel read is that form, not a new one. | Astra, Grok | §2, D1, §6 |
| A tree changes within one incarnation, not only on reload. The runner's `epoch` drives `DOM.documentUpdated`; there is no stock refresh control to lean on. | Sol | D4 |
| The build channel registers builds; the sample host runs two sessions. Stage 1 serves exactly one session per build and refuses a build with more. | Sol | D4 |
| The stock frontend cannot send Tamber's token header. Attachment is an unguessable WebSocket path minted per request, origin-checked, expiring, revocable. | Sol | D5 |
| Inspect ownership on the channel is already exclusive: a new `inspect` replaces the owning client and tells it (`tamber-builds.mjs`). r1 said a competing inspector is "reported rather than displaced"; the channel's rule is displacement with a report, and DevTools takes that rule rather than a new one. | Grok | D4 |
| The stop rule is sharpened. A stub `Runtime` execution context that refuses every evaluation is a tolerated protocol stub; a runtime that evaluates or materialises objects is the stop. A DevTools extension is not an option: extensions load only when Chrome inspects a Chrome tab. | Grok | D2 |
| The runner's source kinds include `heading` and `style` (a heading's implied size or weight, a platform text style; `runner/src/agent/source.rs`), beside `authored`, `dynamic`, `inherited` and `initial`. D3 lists them. | Grok | D3 |
| A diagnosis needs a defect. The qualifying problem is named before Stage 1 starts, with the time the current tools take on it recorded first; candidates are listed. And new reads benefit Tamber's inspector too, so the result records whether the diagnosis improved because of new information or because of Chrome's presentation of it. | Sol, Astra, Claude, Grok | §6 |
| LLP 1115's precedence has an amendment: a person's accessibility settings come before the author where a hand-built app would apply them (Bold Text under an authored weight). The Styles pane must not explain every authored value as the winner. | Astra | D3 |

## Summary

Try Chrome DevTools as a frontend for inspecting a running native Exact app.
The experiment answers one question: can the stock Elements and Styles panels
explain an Exact view accurately, over a small adapter onto the inspection
Exact already provides?

The first consumer is a native macOS app with an open layout or styling defect.
A person connects DevTools, selects a view, sees that instance highlighted in
the app, reads its geometry and where each style value came from, and finds
its Contract declaration. The app keeps running on its ordinary clock.

Two stages in two working days. Stage 0, hours to a day and no host change:
first a stub target that serves four style rows to the recorded frontend, to
settle whether the stock Styles pane can show Exact's provenance honestly;
then, if it can, a throwaway adapter over the agent carrier against a live
app. If the pane cannot, the experiment ends there with that finding and the
money goes to LLP 1122's inspector. Stage 1: the shipping shape, on the Tamber build channel, used to
diagnose the named defect. Read-only, unmodified frontend at a recorded
version. JavaScript debugging, source editing, network capture and performance
timelines are separate later decisions.

## 1. Why this is worth trying

Exact authors write CSS, and many people who read Exact apps already know the
browser's inspector: select an element, look at its box, read the winning
values, follow them to source. Giving that workflow a native app could make a
layout or styling problem quicker to explain to a person than the agent's
JSON or Tamber's inspector, without that person learning either.

**Who it is for.** People, not agents. Agents write most Exact apps (LLP
1143's corpus) and read `tree`, `layout <node>` and `state` as JSON from
`scripts/agent.mjs`; they will not open a GUI, and the diaries ask for
observations the hosts do not yet report, which no frontend can supply. The
audience here is the author checking a screen and the reviewer hunting tells
(LLP 1115). That audience is real, and familiarity is the whole argument.

**What it adds over what exists.** The same plan runs as a real DOM under real
DevTools on the web target (LLP 1071), and the web is the layout parity oracle.
So a defect that reproduces on the web is inspectable today. What native
inspection adds is the delta the web cannot show: what AppKit or UIKit
actually applied, which values are the platform's rather than the author's,
and the native geometry. Tamber's inspector (LLP 1122 D5) shows that delta
already, in Exact's own vocabulary, with the token chain that DevTools can
never show. DevTools wins only if its familiar presentation gets a person to
the answer faster. That is what Stage 1 measures.

**Precedent, read carefully.** React Native DevTools reuses the Chrome
DevTools frontend, but as a build of Meta's fork
(`react/react-native-devtools-frontend`; the `@react-native/debugger-frontend`
package's `BUILD_INFO` names the remote), served through an inspector proxy in
`@react-native/dev-middleware` that relays CDP between the frontend and the
device's Hermes. It shows the React component tree in its own Components
panel and does not project native views into Elements; its documentation
points to Xcode and Android Studio for the platform layer
([React Native DevTools](https://reactnative.dev/docs/react-native-devtools)).
It establishes that a non-browser runtime can drive this frontend. It does not
establish that the stock Elements panel can show a native tree, which is this
experiment's question.

## 2. What Exact already has

A source audit of the checkout on 2026-10-30. No native DevTools connection
has been built or exercised.

| Existing piece | What it supplies | What it does not establish |
|---|---|---|
| [Web agent](../scripts/agent.mjs) and its [CDP transport](../scripts/agent-launch.mjs) | A CDP *client* for input, console messages, exceptions and browser reads | A CDP *server* that Chrome's frontend can attach to |
| [Web JS target](1071-exact2-web-target.rfc.md) | A real DOM inspected through the browser's own DevTools; the layout oracle | Anything about the native host's rendering |
| [Native result inspection](1035.002-native-result-inspection.rfc.md) | The runner's `tree` (whole, a subtree, or `shallow: true` for one node with its child ids), `node` (every row with its source: `authored`, `dynamic`, `inherited` + `from`, `initial`, `heading`, `style`; `site`, `instance`, `planDigest`), `ancestors`, `sites`; the host's spaces, scroll and clip chains, `native` observations; stale ids refused by `epoch`/`incarnation` | A browser DOM or CSS cascade; these reads on the build channel (only the agent carrier has them) |
| [Apple inspect mode](../host/apple/Sources/ExactKit/Inspect.swift) | Native picking by the real hit test, a one-instance selection keyed by session `generation`, plan digest, `site` and canonical `instance` path, re-found after a reload or said gone; outlines of every copy of a site; a local escape | A browsable tree over the build channel; a hover highlight of exactly one instance |
| [Tamber's build channel](../scripts/tamber-builds.mjs) and [source join](../scripts/tamber-inspect.mjs) | Registered builds with nonces, client ownership of inspect and outline, the plan-map join with the source digest check, freshness | Request/reply reads of arbitrary nodes; CDP discovery or attachment |
| The agent carrier (LLP 1012 §4: stdio pipes on the Mac, a socket on iOS) | Request/reply `tree`, `layout <node>`, `state` against a live native app today, with no host change | An app on its ordinary clock: the agent launch freezes time, substitutes storage and paints agent chrome |
| [Hermes embedding](../js/src/shim.cc) | The native JavaScript runtime | An exposed debugger session, source maps or CDP `Runtime` objects |

The runner's reads are the same in-process calls `Inspect.swift` makes from an
ordinary development build (`s.agent("{\"op\":\"node\"…}")`). The adapter is a
translation of observations that exist, not a new observation.

## 3. The first experience

1. Run a development build connected to the dev loop. Enable the DevTools
   connection explicitly for that build.
2. Open the frontend URL the dev loop prints. Its target names the app, host
   and session, so a web build cannot be mistaken for the Mac app.
3. Browse the mounted Exact tree. Hovering a node highlights that one instance
   in the app; DevTools' picker lets a click in the app select the node under
   the pointer without the app receiving the click.
4. Read the node's box, its style rows with their origins, and its Contract
   file, line and column. Repeated rows are separate instances.
5. Use the app, resize its window, edit its source. The tree invalidates when
   the app's tree changes; a reload invalidates every id.
6. Close DevTools or leave inspect mode in the app. Highlights and picking
   end; the app keeps running.

The first target is one macOS app session. The iOS simulator is the next
qualification if Stage 1 earns continuation. The web target keeps using the
browser's own DOM.

## 4. Proposed design

### D1: a translation in the dev tooling, over the reads that exist

```text
Chrome DevTools frontend (stock, recorded version)
        | CDP over a loopback WebSocket
Exact dev-loop adapter (Bun, scripts/)
        | Stage 0: the agent carrier (no host change)
        | Stage 1: the Tamber build channel + a bounded read command
Native development host
        | the runner's reads + host observations + native picking and highlight
Running app
```

The adapter lives with the Bun development tooling and starts only when asked.
It translates CDP requests onto the runner's and host's observations and joins
them to the development map through the Tamber source logic that exists. A
small cache holds protocol identities; the app remains the authority.

**Stage 0 uses the agent carrier.** The host already answers `tree`, `layout
<node>` and the `node` op over the agent carrier under an agent launch
(`agent.mjs`'s `launch`). The frozen clock and substituted storage do not
matter for a fidelity test. Nothing in the host changes, so the first finding
costs hours.

**Stage 1 uses the build channel**, which is the shape that ships: an inspector
must not change the app's clock, input or storage. The channel gains one
bounded request/reply command, a depth-1 read (the runner's `tree` with
`shallow: true` and a target: one node and its child ids) or one node's
`node` reply, with the reply cap `Inspect.swift` already enforces (60 KiB
under the channel's 64). A subtree dump does not fit the cap and is not
offered. The host work is real and counted:
the macOS targeted `layout(_:)` begins by calling the full `layout()`
(`AgentMac.swift`), and the runner's subtree read materialises the subtree
(`runner/src/agent/tree.rs`). A bounded reply over an unbounded traversal does
not meet this design; Stage 1 measures traversal work, not only bytes.

The adapter adopts none of the agent launch lifecycle in Stage 1: no time
control, no settled resources, no input substitution. Reads carry their
observation identity (D4); separate requests are not one snapshot.

### D2: prove the minimum protocol against the actual frontend

The candidate CDP surface, checked against the protocol's `browser_protocol.json`
(version 1.3, 2026-10-30):

| Frontend need | CDP surface | Exact observation or operation |
|---|---|---|
| Expand a tree | `DOM.getDocument` (`depth`), `DOM.requestChildNodes` → `DOM.setChildNodes` | The mounted tree, by bounded child reads |
| Read a box | `DOM.getBoxModel` (content, padding, border, margin quads) | The host's `space.viewport` box and the kernel's `local {w, h, padding, border}`; a box the host cannot give is a protocol error, not a guess |
| Read styles | `CSS.getMatchedStylesForNode` (`matchedCSSRules[]`, `inherited[]`, each `CSSRule` with `selectorList`, `origin`, `style.cssProperties`), `CSS.getComputedStyleForNode` (name/value pairs) | The runner's rows and their sources (D3) |
| Pick and highlight | `Overlay.setInspectMode` (`searchForNode`) → `Overlay.inspectNodeRequested {backendNodeId}`; `Overlay.highlightNode`, `Overlay.hideHighlight` | Native inspect mode's hit test; a one-instance native outline |
| Replace a stale tree | `DOM.documentUpdated` ("node ids are no longer valid") | A new runner `epoch`, a reload, a lost generation |

Chrome's frontend will also send what it sends: domain `enable`s, `Target`,
`Page`, `Runtime`, `Network`, `Log` and `Debugger` calls on attach, `DOM.resolveNode`
for `$0`, `CSS.getStyleSheetText` when a source link is clicked, `CSS.setStyleTexts`
when a value is edited. Stage 0 records the complete request and event
transcript of the recorded frontend version and sorts it into essential,
tolerated-as-error and refused. An unsupported method gets a protocol error,
never a fabricated success; a write is refused.

**The stop rule.** If usable Elements and Styles inspection needs a frontend
fork, broad browser emulation or a pretend JavaScript runtime, the experiment
stops and says so. The line: a `Runtime` execution context that is declared
and refuses every evaluation is a tolerated protocol stub, since the frontend
may wait on `Runtime.executionContextCreated` and `Page.getResourceTree`
before it asks for the document; a runtime that evaluates, or materialises
remote objects for `DOM.resolveNode`, is the stop. A DevTools extension is
not an option either way: extensions load only while Chrome inspects one of
its own tabs, so one cannot attach to a native app's socket. The next step is then a browsable tree and hover highlight
inside LLP 1122's inspector, over the same Stage 1 reads. A fork is not an
option: it is a permanent synchronisation debt against Chromium for a
development tool.

### D3: preserve Exact's meaning in the tree and the Styles pane

The displayed tree is the mounted Exact tree, named as such. UIKit and AppKit
implementation views and unmounted list rows are not invented as children. A
native control's implementation can appear where the host's `native` detail
has it, never as authored nodes.

Style inspection distinguishes four things, and the Styles pane must let a
reader tell them apart:

- **Authored**: what the author declared, with the declaration's file, line and
  column and the component call-site chain.
- **Runner-resolved**: what the runner derived, with its kind as the `node`
  reply names it: `inherited` from a named ancestor, `dynamic` from a binding,
  `initial` from the schema, `heading` (a heading's implied size or weight) or
  `style` (a platform text style), and a token's `when` that won.
- **Platform-owned**: a value the host supplied because the author left it
  unsaid (a bar button's tint, a control's font), where the host observes it.
- **Unavailable**: a value the host does not report.

Precedence is LLP 1115's: author over platform over CSS default, amended so a
person's accessibility settings come before the author where a hand-built app
would apply them (Bold Text weights the system font under an authored weight).
An authored row is therefore not always the winner, and the pane must not say
it is. A kernel initial value never stands for what an unspecialised native
control painted. The adapter runs no second CSS engine.

**Candidate encodings, decided in Stage 0.** The stock Styles pane renders what
the target sends: `selectorList.text` verbatim, each `origin` with its own
styling (`regular`, `user-agent` greyed as the browser's defaults, `inspector`,
`injected`), `inherited[]` under "Inherited from". The candidates are:

| Kind | Candidate | What it would misstate |
|---|---|---|
| Authored | A `regular` rule whose selector text is the Contract location and whose stylesheet text, served to `CSS.getStyleSheetText`, is the declaration's excerpt | Selectors and specificity that do not exist; an editable look |
| Inherited | `inherited[]` entries, one per ancestor that set the row | Nothing material: CSS inheritance is the same idea |
| Initial | A `user-agent` origin rule | Nothing: the kernel's initial value is the CSS default |
| Dynamic, token | A `regular` rule whose property text names the binding or the token and its `when` | A value that looks like a literal |
| Platform-owned | No honest CSS spelling. Candidates: an `inspector`-origin rule named for the platform ("AppKit tint"), or omission from Styles with the observed value in the Computed pane | A `user-agent` origin is **not** a candidate: it says "browser default", and LLP 1115's point is that the platform's answer is not a default |
| Unavailable | Omission; `CSSProperty.parsedOk: false` renders as an invalid property, which is a lie | The pane cannot say "not reported" |

**The deciding example.** A native control whose runner row says `color:
inherited from #1` while AppKit paints its own tint. The pane must not show
the inherited value as what the control painted, and must not pretend a
platform selector overrode it. If the recorded frontend cannot show this case
and one unavailable value so that a person who knows DevTools and not Exact
reads them correctly, Styles fails the fidelity gate and the experiment ends
(D2's stop rule). A tree-and-box inspector with no Styles pane is not worth a
second frontend.

The plan map (LLP 1035.002 D6) supplies the Contract location; it is not a
JavaScript source map. The verified file, line and column appear beside the
selection; a source link that cannot open Contract honestly is refused. A
changed source file is marked stale by Tamber's digest check.

### D4: identity, geometry and lifecycle

**Identity.** A CDP node id belongs to one connection and names one mounted
instance. The adapter binds it to the build id, the host session's
`generation`, the plan digest, the runner's `incarnation`, the view id and the
keyed `instance` path. The generation is required: the kernel's incarnation can
repeat when the host replaces its runner, and `Inspect.swift` already keys its
selection by generation. A targeted request validates that the instance is
current and refuses a stale id by name, as `layout <node>` does; no id is
reused for a new view. A lost generation invalidates the tree.

**Invalidation.** The tree changes within one incarnation. The runner's `epoch`
advances on every change; the adapter watches it on each reply and on the
channel's status, and sends `DOM.documentUpdated` when it moves. There is no
stock "refresh" control to lean on. Reload, disconnect and a stale map clear
or invalidate all affected state, including outstanding replies.

**One session.** The build channel registers builds; the sample host (LLP 1031)
runs two sessions in one build. Stage 1 serves exactly one session per build
and refuses a build with more, naming it. Session enumeration is a later
decision.

**Highlight.** `outline {sites, plan}` outlines every copy of a site. DevTools
hover needs one instance. The outline path gains a validated instance target
and reuses its native drawing; the source-wide outline keeps its meaning for
the design tool.

**Geometry.** The host reports boxes in named spaces (LLP 1035.002 D4). The
adapter converts logical points to the inspector's coordinates once, keeping
scroll and transform where the host gives them, and never multiplies by
display scale as though points were pixels. A box the host cannot represent
(a transformed or native-substituted control) is a protocol error on
`DOM.getBoxModel`, which the frontend tolerates, not a guess. Model and
presented geometry stay distinct.

**Ownership.** Picking uses the build channel's client ownership and the
host's local escape. The channel's rule is already exclusive: a new `inspect`
replaces the owning client and tells it (`tamber-builds.mjs`). The adapter is
one more client under that rule, so turning the DevTools picker on displaces
the design tool's inspect mode with the same report, and the reverse. Closing
DevTools clears its highlight and inspect mode without closing the app. Hover
never scrolls or activates a view.

### D5: confined to development

The CDP endpoint binds to loopback and opens only on request, for one build.
The stock frontend cannot send Tamber's token header, so attachment is a
capability: an unguessable WebSocket path minted for one attachment, expiring
unused, revocable, with the request's `Origin` and `Host` checked so a web page
cannot attach. Discovery (`/json/list`) is served only on that path's parent,
to loopback, with the same checks. Stage 0 verifies the recorded frontend can
attach this way.

Production builds gain nothing: no endpoint, subscription, overlay or debugger
dependency. The adapter adds no core-crate feature and no boot-path work. It
retains no observations after detach beyond what the person saves.

Writes of any kind, JavaScript evaluation, filesystem access beyond the
verified development map, and driving the app are outside this connection.
Selection and highlight are its only effects on the app.

## 5. What later connections would cost

Possible follow-ups, not part of this experiment's bar.

| Capability | What must be established first |
|---|---|
| Style editing | An edit routed through Tamber's checked source operation with its digest, scope and reload behaviour. Transient native mutations would disagree with the files. |
| JavaScript console and breakpoints | Debugger support in Exact's linked Hermes; its inspector, thread ownership, pause and resume, TypeScript-to-bytecode mapping. Execution stays on the device. |
| Network panel | Instrumentation of the native request paths with identity, timing, cancellation and a retention policy. CDP attachment observes nothing of URLSession by itself. |
| Performance panel | Which Exact measurements are faithfully timed events. Work counts and frame summaries (LLP 1079) are not a sampled stack or causal trace. |
| Other hosts | The same adapter; native picking, coordinates, observed styles, lifecycle and transport qualified per host. |

Rust and Swift breakpoints stay the platform debugger's. Contract stepping
would need its own proposal.

## 6. Stages and decision criteria

One implementer, two working days, in this order. The timebox ends in a
finding and a recommendation; a failed gate is a completed experiment.

**Before Stage 1: name the defect and the baseline.** Pick one open, reproducible
native layout or styling defect on macOS and record how long, and in how many
steps, the current path (`agent.mjs` reads and Tamber's inspector) takes to
explain it. Candidates from `issues/`: a native bar title that ignores the
heading's authored font (`20261021-native-header-ignores-authored-heading-font.md`,
the archetypal author-versus-platform case; confirm it reproduces on the Mac's
header projection, else the iOS simulator is the host for the diagnosis) and
the title-bar area beside the traffic lights
(`20261009-titlebar-env-display-mode.md`, geometry). A defect that reproduces
on the web build does not qualify: the browser's DevTools already serve it.

**Stage 0: the fidelity gate (no host change; hours to a day).** Record the
Chrome version first. Two steps:

**0a, the stub target (hours).** A Bun CDP server with no app behind it serves
one three-node tree whose selected node carries four rows, one of each kind
that has no CSS spelling or a contested one: an authored row with its Contract
location, an inherited row from a named ancestor, a platform-owned value (an
AppKit tint under a runner row that says `color: inherited from #1`), and an
unavailable value. Encode them as D3's candidates table says and screenshot
the stock Styles and Computed panes. The gate is decided here: a person who
knows Chrome DevTools and not Exact must read which value the author wrote,
which the runner derived, which the platform owns, and that one value was not
reported. If the four are not visually distinct without a fabricated selector
or cascade, stop: write the finding here and the recommendation for LLP 1122.
Nothing else in this document is built.

**0b, the live app (the rest of the day).** The same server over the agent
carrier against a live native app under an ordinary agent launch. Deliverables:

1. The complete request and event transcript of the stock frontend on attach,
   tree expansion, selection, hover, the picker and a source-link click,
   sorted into essential, tolerated-as-error and refused (D2's stop rule
   applies to what "essential" turns out to contain).
2. The two screenshots again, on the real control, with the encoding used.
3. The attachment capability (D5) working with the recorded frontend.

**Stage 1: the shipping shape (the rest of the two days).** The build-channel
command and its host side, one-instance highlight, picking in both directions,
generation-and-epoch identity, lifecycle cleanup, on macOS. Then diagnose the
named defect through the frontend, and record the steps and the time against
the baseline. Because Stage 1's reads would help Tamber's inspector equally,
record whether the diagnosis got easier because of information that was newly
available or because of Chrome's presentation of it.

Stage 1 passes only if the actual frontend demonstrates:

- A repeated row is selected and highlighted individually; scrolling,
  remounting, reload and reconnect never redirect a stale id to another row.
  A same-plan reboot (incarnation repeats, generation changes) is exercised.
- A selected node's box agrees with the native screen after scrolling and
  resizing on a scaled display; unsupported geometry is visible as such.
- Authored, inherited, dynamic and platform-owned values keep their origins as
  Stage 0 encoded them; the accessibility-first case is shown or recorded as
  unobservable.
- The Contract location resolves to the declaration and its call sites; an
  edit marks it stale.
- The app's clock, input and storage are untouched outside picker mode;
  disconnect, local Escape and another client's ownership are exercised.
- The named defect's explanation is faster or clearer than the baseline. If
  the current tools serve it as well, say so and recommend stopping.

Measure attachment cost, child and node read latency, host traversal work per
read, bytes per reply, and idle CPU and memory detached and attached. The
detached path adds no periodic work; boot and edit-loop budgets hold. Report
measured overhead; no new blocking gate.

Verification follows the repository's checks and each touched host's tests,
with focused coverage for identity invalidation, epoch-driven
`documentUpdated`, stale observations, method refusal and cleanup; the user
experience is driven through the real frontend. Protocol fixtures alone cannot
qualify the connection. Findings go in this RFC.

## 7. Scope admission and open decisions

[DEFERRED](../rules/DEFERRED.md) §Tooling excludes a devtools UI. Its Tamber
admission (Charlie, 2026-10-14) covers inspect mode and the build channel's
four commands and three reports, not a browsable tree or a second client.
The admission for this experiment is recorded there, under the Tamber entry,
with Charlie's words of 2026-10-30. Its text:

> Admitted (Charlie, 2026-10-30, "when you're happy with it, have astra
> implement it"; LLP 1155 r2), development builds only, for the two-day
> experiment: a Chrome DevTools inspection adapter in the Bun dev tooling over
> the agent carrier (Stage 0) and the Tamber build channel (Stage 1), one
> bounded read command on that channel, a one-instance outline, and the
> mounted tree, observed layout, style provenance and Contract locations it
> shows. Read-only, loopback, capability-attached, nothing in a release build.
> Source editing, JavaScript debugging, network capture, performance timelines,
> other hosts and production attachment stay deferred. If Stage 0's fidelity
> gate fails, the admission lapses with the finding.

No take is identified; the experiment is a waiver, as the admission says. The
ten agent operations are unchanged.

| Decision | Recommendation |
|---|---|
| First useful result | Stage 0's fidelity gate; then a diagnosis of a named defect against a recorded baseline |
| First host | macOS, an app with the named defect; iOS after Stage 1 earns continuation |
| Frontend | Stock Chrome DevTools at a recorded version; if it needs a fork, LLP 1122's inspector gets a browsable tree instead |
| Adapter | Bun dev tooling; the agent carrier for Stage 0, the build channel for Stage 1 |
| Budget and ownership | Two working days, Codex GPT-6 Astra, from 2026-10-30 |
| Deferral | The narrow admission above; nothing beyond it |
| Expansion | Decided from measured fidelity, the diagnosis, overhead and maintenance; no commitment to later panels |

## 8. As built

**2026-10-30, Codex: Stage 0a FAIL.** Stop at the stub. Neither candidate
preserves platform ownership, and omission cannot communicate an unavailable
observation. No live adapter, build-channel read, host change or Stage 1 was
built. Under §7's admission, the experiment ends with this finding.

### What was exercised

[`scripts/devtools/prototype.mjs`](../scripts/devtools/prototype.mjs) is a
throwaway Bun server, retained to reproduce the finding. Its three nodes are
a Document, `main#page` (#1) and `button#tinted-control` (#2). The four facts
are synthetic: authored `font-size: 24px` at `prototype.contract:4:7`, runner
`color: inherited from #1` (red), a platform-owned blue tint, and an
unavailable `font-family`. No AppKit value was measured in this stage.

The frontend was **Google Chrome 156.0.8078.74**, from `--version`, selected
with `CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`.
The pinned full Chromium 1248 executable was absent; the installed browser
was used through `chromium({needs: ['perf']})`. Each launch used a private
profile and the existing `Cdp` client over `--remote-debugging-pipe`.
`Page.captureScreenshot` captured the stock frontend; none of the images
adds annotations or changes its UI. No fork or hosted frontend was used.

The protocol JSON was downloaded to `/tmp/exact-1153/browser_protocol.json`
from the URL in the implementation brief, version 1.3, SHA-256
`332d242771f522def6cab034fe90e4d65d87c31c20f3d8a642f54f04466ef642`.
The DOM, CSS and Overlay definitions were read before implementing the stub.

### Reading the screenshots

**[Styles: omission candidate](../scripts/devtools/evidence/stage0-styles.png).**
A DevTools reader sees `font-size: 24px` at a linked source location and an
active red `color` under “Inherited from main#page”. They would reasonably
read red as the button's effective colour. Nothing identifies the platform's
blue tint or explains why inheritance does not determine its paint. The
Contract location itself appears as a selector, though Contract has no such
selector. This is a reading of the screenshot, not a test with a person.

**[Unavailable font](../scripts/devtools/evidence/stage0-unavailable.png).**
Computed, filtered to `font-family` with Show all enabled, says “No matching
property”. A DevTools reader learns only that the filter found nothing, not
that the host did not report the font. Omission loses the distinction between
an unavailable observation and an absent property. An invalid CSS declaration
would say something different and was not used.

Two supporting captures settle the alternatives. **[Computed](../scripts/devtools/evidence/stage0-computed.png)**
shows blue and 24px, without saying AppKit owns blue. **[Inspector-origin
rule](../scripts/devtools/evidence/stage0-inspector-rule.png)** shows `AppKit tint`
as a selector marked “via inspector” and crosses out the inherited red. A
DevTools reader would see an inspector-injected CSS override. The desired
colour is visible only by inventing that cascade. Neither candidate passes
§6. **[Source click](../scripts/devtools/evidence/stage0-source.png)** did open
the exact fixture at line 4; source navigation is feasible, but does not
repair provenance.

### Attachment and protocol findings

The direct bundled URL, `devtools://devtools/bundled/inspector.html?ws=127.0.0.1:<port>/<capability>/page`,
worked when opened through CDP navigation. An attempt to enter it through
the private Chrome process's omnibox with System Events returned without
error but left `about:blank` after a bounded observation. Physical omnibox
entry is therefore unverified; it is not counted as a successful route.

`chrome://inspect/#devices`, with the second target's port configured under
Discover network targets, requested `/json/version` and `/json/list` at
the port root. Both correctly returned 404. It did not discover the target.
**D5's capability-parent discovery is incompatible with that port-only
route as exercised.** The capability-parent paths returned 200 to direct
HTTP probes; bad Host and Origin returned 403. The WebSocket accepted
`Origin: devtools://devtools`, used a random 192-bit path, refused a second
attachment, and revoked discovery after detach. An unused path expires after
ten minutes. This is prototype attachment evidence, not production hardening.

The **[complete transcript](../scripts/devtools/evidence/stage0-transcript.md)**
contains both final screenshot sessions: **29 essential requests, 151
tolerated-as-error requests, 2 refused requests and 6 events**. It includes
attachment, child expansion, selection, hover and picker requests, and the
source-link click. “Essential” names the implemented gate surface, not a
proved minimum. Overlay and geometry calls returned errors because the stub
has no app. No native picking or highlighting is claimed. The two refused
frontend calls tried to inject Chrome's performance-metrics script.

The frontend displayed Styles without `DOM.resolveNode` or target execution;
both remained unsupported. A declared execution context and resource-tree
reply sufficed for the bootstrap used here. Separate real-WebSocket probes
passed fifteen assertions covering Host/Origin, discovery, mutation and
evaluation refusal, unknown methods, unchanged reads, child identity and
revocation. All unsupported methods returned `-32601`.

### Corrections, diagnosis and limits

The brief and D1 refer to `launch`/`send` in `agent-launch.mjs`. This checkout
exports neither there. `agent.mjs` exports `open`, calls `openStdio` from
`agent-stdio.mjs` for macOS, and exposes the carrier through `op`; `Cdp.send`
in `agent-launch.mjs` is the browser-side client. A live stage would reuse
those existing functions, not reimplement the stdio wire.

The candidate header issue documents UIKit, not a confirmed Mac reproduction.
The titlebar issue records that its env-value slice landed on 2026-10-18.
Neither was run here. **Native reproduction and the agent/Tamber diagnosis
baseline were not reached or measured:** §6 requires them before Stage 1,
and its Stage 0a stop rule prevented advancing to a live app. There is no
diagnosis-time comparison and no claim that Chrome made a diagnosis faster.
The prospective Caltrain consumer's diary records that no app was launched.

Bootstrap exposed one CDP footgun: re-emitting known children replaces the
frontend's node objects, leaving its selected node obsolete and Styles empty.
The stub now emits each child's introduction once. Source-linked Styles also
need property ranges inside the style range; otherwise the frontend hides
those declarations. These were corrected before the final captures. The
bootstrap attempts are excluded from the final transcript's counts.

No native process was launched. Chrome PIDs 85438, 34367, 90217 and 30343 were
recorded at launch, each SIGKILLed and waited for; their temporary profiles
were removed. No other process was killed. No other checkout, target
directory, remote machine or sub-agent was used.

### Checks

This is a Bun-only prototype. `bun scripts/boot.mjs` passed: two modules
before first pixel, with no boot imports changed. The first
`bun scripts/caps.mjs` run failed because the starting branch already had
16 links in `llp/current/`, over 15. Finishing the experiment removes only
1155's working-set link; the RFC and its Draft status remain. The rerun passed
all ten inspected categories with 15 working-set links. The source is
120 lines, below 1,500. `git diff --cached --check` passed after removing a
trailing blank line from the generated transcript.

Cargo build, tests, clippy, fmt and workspace check, native app builds and the
iOS shared-Swift build were not run: no Rust, Swift, host or app code changed.
The Bun-only rule in the implementation brief requires caps and boot here.

### Recommendation

Give LLP 1122's inspector a browsable mounted tree and one-instance hover
highlight if that work is admitted separately. Keep authored, runner-derived,
platform-owned and unavailable observations explicit, alongside the existing
source join. The bounded reads and traversal work in Stage 1 remain a useful
proposal for that tool; they were not implemented under this failed gate.
Do not fork DevTools or retain a tree-only second frontend. The RFC remains
Draft; this finding does not accept or reject it on Charlie's behalf.
