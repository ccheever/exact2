# LLP 1020: The webview — Castle decks as embedded web content

**Type:** RFC
**Status:** Draft
**Systems:** Kernel (one node type, two props), Contract (one tag, two attributes, two events), Web host (the element itself), Apple host (a WKWebView arm loaded on demand; the capture handshake), Linux host (declared absent), Agent API (the guest joins the eight operations — no ninth), Weird Castle (the content model: the client exists to surface web decks)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Revised:** 2026-09-04 (Charlie clarified the application boundary; §9)
**Related:** `rules/NOT-DOING.md` §Components (`webview`; §6 records the trade this RFC owes), weird-castle `llp/0000` §Web decks only (the governing invariant this serves), LLP 1001 (the kernel: a box is a box), LLP 1009 D3 + LLP 1014 (the on-demand module shape and the capture handshake this reuses), LLP 1012 (the eight operations), LLP 1016/1018 (the Weird Castle lane precedent), LLP 1017 §8.1 (literal HTML/CSS names, no aliases). Predecessor record (research, never authority): exact1 LLP 0433 `~/projects/exact/llp/0433-embedded-webview.rfc.md` (799 lines, Implemented, 4 review rounds) and its issue trail, cited per finding in §3.

## 1. Summary

An app embeds web content through an **`iframe`** node: a leaf with a
kernel-owned box whose content is a web engine's. On the web host the node
*is* an `<iframe>` — zero new bytes, spec semantics by identity. On Apple
hosts it is a WKWebView inside an Exact-owned wrapper document holding a
real inner `<iframe>` (exact1's frame topology — the one part of LLP 0433
that survived review intact and the only topology this RFC admits), loaded
as a separate artifact at the first `iframe` commit, never on the boot
path. The Linux host renders the box empty and says so. The guest joins
the agent's eight operations through a script bridge the host owns; no OS
input synthesis ever touches the engine, which is the exact mistake that
made exact1's webview unverifiable.

v1 is two props (`src`, `sandbox`), two events (`load`, `message`), zero
imperative operations. exact1 shipped ten props, fourteen events, and a
thirteen-op controller on day one; §5 keeps the receipts for what each cut
costs and what earns it back.

## 2. Motivation

Weird Castle's governing invariant is **"web decks only"** (weird-castle
`llp/0000`, confirmed 2026-08-03): the client surfaces Castle web decks —
static web bundles, real JS, a game loop on `requestAnimationFrame`,
network to the relay and api.castle.xyz. The webview is not a feature of
that app; it is the app's content model. Every screen so far (login,
session, title) has been the frame around a hole this RFC fills.

exact1 built this — fully, and fast (LLP 0433: RFC to Implemented in two
days, web + macOS + iOS). It then spent three weeks generating startup
regressions, drift tickets, and STRAIN verdicts in every architecture doc
that followed. The record is unusually good, which makes this the rare RFC
that can be written *from evidence*: §3 maps each exact1 cost to the
structural reason it cannot recur here, or to the honest admission that it
can.

## 3. What exact1 taught, and what this design does about it

### 3.1 Bloat, startup, and app performance

**The finding.** The webview tag family joined all three of exact1's
native startup graphs (issue `20260807-startup-budget-webview-host-
component-authority`). After deferring everything deferrable, +1289 remained
as an "irreducible synchronous eager floor": `getTagConfig('webview')` had
to answer at any first mount because a public built-in tag on a dynamic
tier has no user import to ride. It took a supervisor session, a
dual-attribution worktree measurement, and a written-reason budget raise
to contain a component most apps never mount.

**Why it cannot recur.** exact2 has no dynamic tier and no runtime tag
registry. The Contract compiles ahead of time; a tag not in the plan costs
zero everywhere; a tag in the plan is a row in a table the host already
loads. The lazy seam exact1 lacked exists here by construction — and the
Apple arm takes the on-demand artifact shape the GPU module proved
(LLP 1009 D3, `rules` §Optional capability): the WKWebView arm is a
separate artifact `dlopen`ed at the first `iframe` commit, WebKit
untouched until then. The web arm is an element the browser already has.
Boot-path delta on every host: **zero**, verified the way LLP 1008 §6
verifies the GPU module's absence.

**Runtime.** The guest runs in the engine's own content process; its game
loop never enters the host's frame. The kernel does no per-frame work for
an `iframe` — it is a box (LLP 1001). The one real cost is capture-time
(§4 D6), paid only when a screenshot or a canvas above demands pixels.

### 3.2 Iteration-speed drag on the framework and the app

**The finding.** exact1's webview ended with **four hand-coupled
authorities** — Contract descriptor, ModuleIR, renderer TagConfig, Swift
host — and three still-open drift tickets
(`20260823-webview-moduleir-descriptor-coupling`: "nothing mechanically
prevents drift"; `20260808-guide-webview-authority-path-stale`: the guide
cited a file that no longer existed, a verify check red on main). Every
subsequent architecture doc paid a webview tax: a STRAIN verdict in the
registry migration (LLP 0526), a named hole in the capability theorem
(LLP 0499 C-3), an excluded-tag row in Contract Native (LLP 0478), a
stateful-embed analyzer warning (LLP 0487 §8 I2).

**What this design does.** The surface is small enough to have one home:
a `WebView` row in `kernel/tables/schema.json` `nodeTypes`, two prop rows
beside the forty-two that exist, and one entry in the tag table
(`contract/lower/src/tags.rs`) — the same two places `canvas` lives, with
`build.rs` generating every projection. No manifest, no descriptor, no
ModuleIR, no per-capability evidence rows: **frame-only topology makes the
engine the enforcer**, and the elaborate evidence machinery exact1 built
was almost entirely apparatus for the top topology this RFC refuses (§5).
The app-side loop is untouched: a fixture deck is static files under the
app repo served by `dev.mjs`, so deck-plus-client iteration stays inside
the 20 ms Contract loop.

### 3.3 Agent inspectability and verifiability

**The finding.** exact1's most consequential webview issue
(`20260806-webview-frame-guest-click-delivery`): synthesized OS clicks —
`CGEventPostToPid`, AX press, all of it — **never reach a WKWebView
guest**. WebKit's out-of-process input pipeline drops them. It closed as
an environment constraint; a human clicked the buttons; the agent guide
carries the caveat permanently (`docs/acto-guide.md:576`). The framework
whose motivation cited "Vercel cannot verify its own webviews" (LLP 0433
§10) shipped a webview its own agent could not verify. Separately:
`sandbox=""` mounted **fully unsandboxed** on the Apple top topology
(`20260807-webview-empty-sandbox-fail-open`) — a security fail-open its
own tests missed; and guest focus/keyboard was never verified (0433 §14
Q5), which blocked the DOM-components layer downstream.

**What this design does.** exact2's agent never synthesized OS input in
the first place — `tap` is a journal entry into the runner (LLP 1012).
The guest is driven the same way: **through the engine's scripting
interface, in-process to the content process, where delivery is not a
gamble.** The eight operations stay eight (NOT-DOING §Agent API); the
guest joins them under D4 rather than growing a controller vocabulary
beside them. The sandbox fail-open class is structurally gone: there is no
approximation code to fail open — `sandbox` lands on a real `<iframe>`
attribute on every host and WebKit enforces it (§4 D2). What this design
does **not** fix it declares: guest time is outside the settled clock, and
a cross-origin guest is opaque to the *web* arm's bridge (D4; §8 Q2, Q3).

## 4. Design

**D1 — the tag is `iframe`; the node type is `WebView`.** LLP 1017 §8.1:
every row in the tag table is a CSS property or an HTML attribute by its
HTML name, and HTML's name for embedded browsing content is `iframe` —
`webview` is the React Native word, exactly the class of name this repo
refuses (`rules` §The web is the standard). Naming it `iframe` also names
the contract: the parity oracle for every host is a bare `<iframe>` in the
dev-loop browser, definitionally. A bare `iframe` is 300×150 — the web's
replaced-element default, the same rule `canvas` got — sized only by its
style rows, content never influencing layout, nothing about the guest
visible to the kernel (exact1's 0433 §8 test — "what must the kernel know
that is webview-specific? Nothing" — still holds and still decides).
`NodeType::WebView` joins the schema beside `Canvas`; the existing
`NativeView` (id 9) stays what it is — this is not module dispatch,
because there are no modules, there is one tag.

Attributes: `src` (a URL; setting it navigates, guest navigation does not
rewrite it — iframe semantics) and `sandbox` (the HTML token list,
spec-true: absent means unsandboxed, present restricts, tokens re-grant).
Events: `load` (fires for error documents too; when the *wrapper's* own
navigation fails provisionally, the Apple arm re-serves with an error
document as the inner frame's content, because WKWebView otherwise
renders nothing — exact1's lesson, restated for frame topology: an inner
guest failure is the engine's own error page and `load`, as on the web)
and `message` (D2). Both join
the existing handler set in the tag table; per the events decision
(QUEUE §2), an action can record their payload, not branch on it.

**D2 — frame topology only; messages are strings.** exact1's round-1
review proved a top-level native webview cannot emulate iframe semantics
(`window.top` is `[LegacyUnforgeable]`; opaque-origin sandboxing cannot be
approximated without *reducing* restriction) and answered with two
topologies. Every security finding it later shipped lived in `top`. v1
takes `frame` and refuses `top` (§5): on the web the node is the element;
on Apple the engine loads an Exact-owned wrapper document via
`loadSimulatedRequest` (macOS 12+/iOS 15+) at a synthetic origin, holding
a real inner `<iframe src=… sandbox=…>` — browsing-context semantics are
WebKit's, not emulated. A reload re-serves the wrapper (exact1's
srcdoc-reload lesson: `WKWebView.reload()` does not re-run document-start
scripts on a loaded string — issue `20260806-…-srcdoc-reload-messages`).

Two lifecycle facts an iframe does not have, handled here rather than
discovered later. **The content process can die**: iOS reclaims web
content processes under memory pressure, which no web iframe does
independently of its page. v1 handles it silently — the host observes
`webContentProcessDidTerminate`, re-serves the wrapper, and reloads
`src`; there is no event (exact1's `guestprocessgone` sits in §5 with
its trigger). **The top is the wrapper, not the app**: a guest granted
`allow-top-navigation` that navigates `_top` on the web arm navigates
the whole app page away — the spec-true and unforgiving reading — while
on the Apple arm the same escape wrecks only the wrapper, which the
host detects and re-serves as above. The web arm is the oracle; the
native arm is the safer one; an app that grants the token owns the web
consequence.

One materialization the arm owns (as built, 2026-08-30): a
**bundle-relative `src`** (a schemeless path — the fixture case; a hosted
https deck never enters this) cannot be fetched from the synthetic wrapper
origin, so the arm reads the file from the app bundle under a
symlink-resolved containment check and inlines it as the inner frame's
`srcdoc`, `sandbox` verbatim, no `src` attribute on the inner element.
This is not the Contract-facing `srcdoc` §5 cuts — no author writes it —
and it carries a declared limit: a multi-file local bundle's subresources
do not resolve; multi-file local decks wait for scheme-handler serving
(the shipped Castle mechanism, §8 Q1).

Guest→app: the guest calls `window.parent.postMessage(data, …)` exactly
as it would on castle.xyz; the wrapper (native) or the host page (web)
delivers it as the `message` event. **The payload domain is a string** —
a deck sends JSON if it wants structure. exact1 built a machine-checked
conformance corpus to hold the structured-clone line across a native
bridge; one declared narrowing replaces the whole apparatus, and widening
it later is additive.

**D2r — the reply (scoped in by evidence, §8 Q1).** The shipped Castle
deck contract is request/response: a deck posts a `castleSdk` command
and expects the host's answer back in its window. So app→guest exists
in exactly one form: **a string posted into the guest frame** —
`iframe.contentWindow.postMessage` on the web arm, the wrapper relaying
via `evaluateJavaScript` on Apple, the reply naming the guest origin
whenever it is not opaque (§8 Q2's imported lesson). Who composes the
reply is M3's design (LLP 1020.000): the plausible shape is the data
seam — a guest `message` lands as an action, the command runs as a
mutation (LLP 1016) with the client's auth, and the settled result is
posted back — keeping the token out of the guest and the bridge out of
the Contract's way. General app→guest messaging beyond replies stays
cut (§5).

**D3 — loading is the GPU-module shape.** The Apple webview arm is a
separate artifact beside `libexact_gpu.dylib`, `dlopen`ed at the first
`iframe` commit; a plan with no `iframe` loads nothing; the boot path is
untouched either way (§3.1). The Linux host loads nothing ever (D5). No
cargo feature on any core crate (`rules` §Optional capability).

**D4 — the guest joins the eight operations.** No ninth op, no controller
(NOT-DOING: "a new question is answered from `tree`, `state`, or
`layout`"). The host owns a script bridge into the guest — on Apple,
`evaluateJavaScript(_:in:in:)` into the guest frame in a private content
world (reaches cross-origin frames; invisible to deck code); on the web,
the agent's page-side bridge into the iframe, which reaches exactly as far
as the browser's origin rules allow:

- **`tree`** shows the `iframe` node with `{url, loading}` and, where the
  bridge reaches, a guest subtree — a compact DOM outline, marked as
  guest content.
- **`tap` / `type`** addressed into the guest dispatch DOM events from
  inside the content process — the in-process dispatch that works where
  exact1's out-of-process synthesis silently didn't. Caveat, declared:
  script-dispatched events are `isTrusted: false` and do not run default
  actions; a deck engine listening on pointer/keyboard events sees them,
  and one that demands trusted input is driven on the web arm in the real
  browser (§8 Q3).
- **`screenshot`** composes guest pixels on Apple via
  `WKWebView.takeSnapshot` — WKWebView's layers are remote and never
  render through `cacheDisplay`/`CARenderer`, so the capture handshake
  built for images under a canvas (LLP 1014 D4 b, commit bcf5692) is the
  shape: the node hands the turn's capture a texture it produced itself.
  One split, found live (2026-08-30): the **simulator's** `takeSnapshot`
  can return the guest's background without its text, while
  `drawHierarchy` renders WKWebView in-process there — so on a simulator
  the screenshot's one source is the live `drawHierarchy` render
  (webview visible, no compose), and on macOS and a device it is
  hide-and-compose `takeSnapshot`, never both (two sources ghost, and an
  opaque blank live render would cover a good snapshot). The device row
  is asserted by the same smoke when one runs there. The web arm's page
  capture already includes iframes.
- **`clock`** does not govern the guest. A foreign runtime's
  `requestAnimationFrame` is not on the seekable clock, and pretending
  otherwise rebuilds the settle-flake NOT-DOING §Agent API exists to kill.
  Declared: `clock settle` settles Exact; guest content is a live region.
  The smoke's deck is a **fixture deck** — static files in the app repo,
  content settled at load — so every cross-host assertion is
  deterministic anyway (§7).

**D5 — Linux: absent, declared.** No system engine exists and CEF is not
worth its weight for a headless CI host (exact1 specified the Chromium
seam and never scheduled it; this RFC doesn't pretend better). The box
renders the app's background, `tree` shows the node with
`{unavailable: true}`, and the smoke's deck steps are web/macOS/iOS only.
Caltrain defines the v1 bar and contains no `iframe`, so the
four-surface bar is untouched.

**D6 — no canvas around the webview (Charlie, 2026-08-30, closing what
was Q5).** Weird Castle's app sits inside `canvas surface=night(onTitle)`
(LLP 1014); the deck screen mounts its `iframe` **outside** that wrapper
— a webview never lives under a live surface. The reason is mechanical:
a WKWebView's layers are remote and cannot be captured by the
Shadow/`CARenderer` mirror (the same class as the iOS image-decode gap,
bcf5692), so a guest under a canvas would cross only via `takeSnapshot`
— asynchronous, milliseconds each — and a playing deck under a live
shader is recapture-per-frame, the case LLP 1014 D4 already does not
recommend. What remains supported: the **agent's `screenshot`** always
composes guest pixels via `takeSnapshot` on Apple (a turn's capture, not
a frame's), and a webview under a canvas at turn-capture granularity is
tolerated for a screenshot but is not a layout the design serves. The
weird-castle structural change (the deck screen outside `night`) is
M3's, in that repo.

## 5. What v1 refuses, and what earns each back

Everything exact1 shipped that v1 cuts, with the trigger that reopens it —
so each return is a measurement or a named consumer, not drift:

- **`top` topology** — never, absent a product that is a browser shell.
  Weird Castle is not one; every exact1 security finding lived here.
- **`srcdoc`** — the first consumer that needs app-authored HTML. Decks
  arrive by URL.
- **`allow` / permissions** (camera, mic, geolocation) — the first deck
  that needs a permissioned capability; arrives with the broker design
  exact1 deferred and never finished (its `permissionBrokerRequestCount`
  stayed 0 — WebKit short-circuits iframe policy before the delegate).
  Until then the default is the spec's: default-deny for guest content.
- **App→guest messaging** — the earn-back trigger ("the first deck the
  URL cannot configure") was met the day the survey read the shipped SDK:
  every `castleSdk` deck expects replies in its window. The narrow form
  — replies to guest messages — is scoped in as D2r; **general**
  app→guest messaging (host-initiated pushes) stays cut until a deck
  needs one the reply channel cannot carry.
- **Navigation policy, verdicts, popups, the 13-op controller** — the
  first product need to intercept guest navigation. Until then: the guest
  navigates itself as any iframe may; popups are denied by `sandbox`
  omission of `allow-popups` (engine-enforced), and weird-castle authors
  its sandbox. Note exact1's finding before importing its default: "open
  popups in the system browser" silently breaks OAuth flows (0433 review
  M5) — when this returns, it returns as policy data, not a default.
- **A guest-process-death event** (exact1's `guestprocessgone`) — the
  first deck that must distinguish a crash-reload from a fresh load, or
  preserve state across one. Until then D2's silent re-serve-and-reload
  is the whole story, and a deck that keeps state keeps it server-side
  or in its own storage, as it must on the open web anyway.
- **Guest focus/keyboard as a verified row** — the first editable guest.
  exact1 left it "likely fine, unverified" (0433 §14 Q5); decks are
  games, `type` into a guest field stays best-effort until a consumer
  pins it.
- **`border-radius` masking, embedded-in-scroll gesture negotiation** —
  same posture as exact1's, now written down first: rectangular
  placement, the engine's scroll view owns guest touches.

## 6. The NOT-DOING trade this RFC owes (Charlie's, before Acceptance)

`rules/NOT-DOING.md` §Components lists `webview` by name; the rule is one
line naming what it unblocks and something off the doing-list in the same
PR.

- **The line:** `webview` (as §4's `iframe`) unblocks Weird Castle's
  entire content model — the client exists to surface Castle web decks
  (weird-castle `llp/0000`, governing invariant, confirmed 2026-08-03) —
  the way LLP 1016/1018 unblocked its login and session.
- **The take (decided — Charlie, 2026-08-30):** QUEUE §5, **view
  transitions (LLP 1013, Draft)**, moves behind this work — the deck
  lane displaces the transitions lane in the working set, and 1013 keeps
  its place on the list rather than the calendar. The `webview` line
  comes off `rules/NOT-DOING.md` §Components in the PR that lands M1,
  citing this section, per the rule's "same PR" wording.
- **Kept locked, explicitly:** `video`, `lottie`, `rive`, `fileinput`,
  `pager`, camera — nothing else moves off §Components with this.

## 7. Delivery

- **M1 — web.** Schema rows, tag row, the element in the web host,
  `load`/`message`, the fixture deck under the app repo, smoke steps: mount
  by `tap`, assert `tree` shows the guest outline, `message` recorded into
  state, screenshot fixture. The parity oracle exists the day M1 lands.
- **M2 — Apple.** The dlopened arm: wrapper + inner iframe over
  `loadSimulatedRequest`, message relay, content-world agent bridge,
  `takeSnapshot` capture handshake, the error-document `load`. Same smoke
  steps, macOS and iOS. Boot metrics before/after in `metrics.mjs` prove
  §3.1's zero.
- **M3 — the deck screen.** Weird Castle mounts a real Castle deck
  outside the `night` surface (D6) and implements the `castleSdk` host
  over `message` + D2r (Q1) — commands through the data seam with the
  client's auth, the token never entering the guest — driven by
  `exact.mjs agent` on three hosts. This is the acceptance test; an RFC
  for a content model is proven by content.

Verification is the standing recipe: the five checks, `smoke.mjs` per
host, and the M1 fixture deck asserted against the browser.

## 8. Open questions

- **Q1 — resolved by the shipped product** (surveyed 2026-08-30; Charlie:
  "look and see how the Castle app does it"). Castle **never hands a web
  deck the auth token**, anywhere. An `experimentalWeb` deck runs
  unprivileged — in the app, an RN WebView at a per-deck synthetic origin
  whose bundle bytes are served in-process by a URL-scheme handler
  (`castle-client/mobile/js/common/deckOrigin.js`; `castle-deck://` on
  iOS — our D2 wrapper's own mechanism, shipped); on castle.xyz, an
  iframe with `sandbox="allow-scripts allow-pointer-lock"` (srcdoc) or
  `+ allow-same-origin` (hosted) — and reaches Castle only through the
  **`castleSdk` command bridge**: the deck posts
  `{castleSdk: 1, requestId, command, params}`, the host executes it
  with *its* auth (`X-Auth-Token` attached a layer below, by the host's
  GraphQL client), stamps trusted context the deck cannot forge
  (`deckId`, `sessionId`, `userId`, `username`), and posts the result
  back. A deck learns at most `{userId, username}` via
  `user.getCurrent`. The deck-side SDK
  (`castle-experimental-web/sdk/src/transport.ts`) picks its transport
  by environment and defaults to `window.parent.postMessage` when
  framed — which is exactly what our frame topology presents, so a
  hosted deck speaks to weird-castle's bridge with no injection at all.
  Weird Castle adopts this model: the token never enters the guest; the
  client implements the `castleSdk` host over `message` plus the D2r
  reply channel, commands executed through the data seam (LLP 1016's
  mutations, `ibex2::host`).
- **Q2 — dissolved by the same evidence.** The shipped SDK validates
  replies by *source identity*, not origin, and posts with `'*'`
  (`castle-www/components/DeckPlayer.tsx:549`) — decks already run at
  synthetic origins in the shipped app, so nothing in the deck contract
  keys on a real embedder origin. One lesson imported rather than the
  bug: the survey found the hosted branch broadcasting authed reply data
  to `'*'` where a real origin exists to name — weird-castle's bridge
  names the guest origin in replies whenever it is not opaque.
- **Q3 — a deck that filters untrusted input.** If a real deck engine
  ignores `isTrusted: false` events, the native agent bridge cannot drive
  it; the web arm in a real browser can. Accept the split, or record it
  per deck?
- **Q4 — guest time in agent mode.** A clock shim injected into the guest
  (wrap `performance.now`/rAF, the render-rig trick) would put deck motion
  on the agent's clock. Powerful, invasive, and exactly the kind of
  apparatus this repo removes unless a consumer demands it. Not in v1;
  recorded so the demand is a measurement.
- **Q5 — resolved into D6** (Charlie, 2026-08-30): no canvas wrapper
  around the webview — the deck screen mounts outside the `night`
  surface; only the agent's turn-capture `takeSnapshot` path remains.
- **Q6 — session recording, ring-buffer style** (Charlie's ask,
  2026-08-30: record played deck sessions; always able to grab the last
  30–60 s). Doable; three lanes, not yet chosen:
  1. **Screen-level** — captures what the player saw, `night` shader
     included: ScreenCaptureKit (macOS, one TCC grant) or ReplayKit
     in-app capture (iOS, one consent alert) feeding hardware H.264
     through `AVAssetWriter`'s fragmented-MP4 segments, a ring of the
     last N segments (~60 s ≈ 15–30 MB in memory), stitched on grab.
     ReplayKit's own `startClipBuffering`/`exportClip` is exactly this
     shape but its buffer is capped short — verify the cap before
     leaning on it. No web-arm story for cross-origin decks
     (`getDisplayMedia` prompts per use).
  2. **Guest-side** — the deck records its own canvas:
     `canvas.captureStream()` + `MediaRecorder` (WebKit has it since
     iOS 14.5) with time-sliced chunks in a ring, the last-N blob handed
     out via the existing `message` channel. One implementation on every
     host including the web arm, no permission prompts, captures exactly
     the game — but needs deck cooperation (a player-side shim or SDK),
     misses the shader composite, and audio only if the deck routes it.
  3. ~~The D6 dividend~~ — withdrawn with Q5's resolution: D6 rules out
     per-frame guest capture (no canvas around the webview), so there is
     no capture stream to ride; the choice is between lanes 1 and 2.

## 9. Application boundary correction (Charlie, 2026-09-04)

**Decided:** Weird Castle is an example application. Castle-specific logic
does not belong in Exact's shared runtime. Its use as the motivating
consumer in this document never makes the Castle protocol a host semantic.

The implementation crossed that boundary: `host/web/glue.js` and
`host/apple/webarm/WebArm.swift` answer `castleSdk` identity requests, and
the Apple arm changes local Castle HTML to fill the frame. Move that
protocol, identity policy, and presentation adaptation into Weird Castle's
own source/assets/integration. Exact supplies the generic iframe and D2r
reply mechanism, with its sandbox, source/origin, and lifecycle checks.
An app-specific branch renamed as a generic helper is not the correction.

The removal is tracked in
[`issues/20260904-castle-policy-in-shared-hosts.md`](../issues/20260904-castle-policy-in-shared-hosts.md).
It includes preserving the external app's behavior and generic iframe
verification. Historical research and isolated example/test fixtures remain
evidence, not runtime policy. This records Charlie's boundary ruling; the
rest of this Draft's unbuilt proposals do not become accepted by implication.
