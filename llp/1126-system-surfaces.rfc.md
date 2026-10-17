# LLP 1126: System surfaces — an app's views outside its window

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built until Charlie has read it)
**Status:** Draft r2, 2026-10-17. r1 was reviewed blind, on direction only, by Claude Fable 5.1 (xhigh), GPT-6 Astra (max) and Grok 4.7 (xhigh); all three READY WITH CHANGES (`llp/reviews/1126-system-surfaces.{fable,astra,grok}.md`). Muse Wonder was asked for and unavailable (its quota exhausted until 2026-10-19). r2 takes their foundational findings; §R lists each and where it went. Charlie ruled questions 1 to 3 on 2026-10-17 (§0); questions 4 and 5 wait for a panel, 6 to 9 are open. Not in `llp/current/`, which holds 15 of 15.
**Systems:** a new SwiftUI renderer in the Apple host (`host/apple/Sources`), a presenter-free session composition (`ExactKit`'s `ExactSession`), the app manifest (`scripts/app.schema.json`), the bake and the schema's admission columns (`contract/cli`, `kernel/tables/schema.json`), the envelope's plans (LLP 1031 D1), the store and the update store across processes (`update/src/store.rs`, LLP 1018), the Apple build's packaging (`host/apple/build.mjs`), the agent (`scripts/agent.mjs`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-17 (r1, r2)
**Implementer:** none yet. Stage 0 (§5) is a set of probes with kill lines; a spec follows a ruling.
**Related:** LLP 1115 (write the web, ship the platform); LLP 1101 D1–D2 (the terminal entry and its admission profile: the shape D2 takes); LLP 1031 D1 (the deferred `plans` map, whose trigger D2 pulls) and D4 (session snapshots); LLP 1030 D3a (compatibility id) and its extension row; LLP 1018 (the store); LLP 1047 and 1047.001 (link by use); LLP 1035.006 §11.10; `rules/DEFERRED.md` §Surfaces, §Components, the notifications entry; LLP 1038 (a location is a URL); LLP 1024 and 1067.000 (native modules); LLP 1112 and 1113 (pictures; settled); LLP 1016.002 (push invalidates, the answer stays a pull); LLP 1069 (the Expo survey: `expo-widgets` missing); LLP 1124 (host extensions, a different sense of "extension")

## 0. Rulings

Charlie, 2026-10-17, on §7:

- **Question 1, the shape:** "rec seems ok".
- **Question 2, LLP 1035.006 §11.10:** "reopen yes". Recorded in
  `rules/DEFERRED.md` §Surfaces and in LLP 1035.006 §11.10; the take is
  waived.
- **Question 3, watchOS:** "admit watchos now. let's try to hit full apple
  platform support for launch". watchOS is admitted (`rules/DEFERRED.md`
  §Surfaces). D10 and §5 now plan the watch with the other platforms: its
  bring-up starts beside stage 0, complications come with the snapshot
  driver, and the live driver and the watch app are a stage of their own,
  not a conditional one.
- **Questions 4 and 5:** waiting for a panel (Astra, Grok, Fable).

## Summary

Apple puts an app's content in many places outside its window: Home Screen,
Lock Screen and StandBy widgets, Live Activities and the Dynamic Island,
watch complications, CarPlay and HomeView widgets, Siri and Spotlight
snippets, CarPlay apps, Control Center controls, notifications, and the
Apple Watch app. Exact can put content in none of them.

Three questions decide what each place needs, and they are separate:

- **How it runs** (its class). *Snapshot*: code runs briefly and returns a
  view value the system archives and draws with no code running (WidgetKit).
  *Live*: a running view tree (the watch app). *Template*: the app gives
  data, the system draws (CarPlay apps, controls). *Hosted*: a view
  controller the existing presenter fills (rich notifications). *Island*: a
  system view inside the running app (Swift Charts), which native modules
  already host.
- **What draws it** on that platform: SwiftUI's archive on Apple, RemoteViews
  or ProtoLayout on Android, a template.
- **Which binary runs it**: the iPhone app's extension, or a binary built for
  another OS. An iPhone widget appears in CarPlay, on HomeView and on the
  Mac with no code there; a watch complication needs a watchOS binary.

On Apple, every snapshot surface is drawn by one thing: a **SwiftUI renderer**
whose **snapshot driver** places the kernel's laid-out boxes as SwiftUI views
WidgetKit can archive. A later **live driver** shares its leaves and serves
the watch app. Templates need a mapping, not a renderer, and get their own
RFC.

The load-bearing decisions: a surface is an entry of the one app, compiled
under a `snapshot` admission profile into its own plan in the app's bundle
(D2); its render is pure and its work happens in a refresh step (D3); layout
stays CSS, placed at fixed frames with the kernel's own line breaks (D4);
time and taps belong to the platform (D6, D7); the app and its extensions are
one compatibility cohort, the app owns updates and the extensions read
committed generations (D8); what the system stores between versions (a Live
Activity's state, an archived tap) is a generic envelope that carries the
plan's digest, never per-app generated Swift (D9). The first stages ship
only what the iPhone's extension draws.

## 1. Motivation

- **Where users look.** Many apps are seen more often in a widget or a Live
  Activity than in their own window: a delivery's progress, the next train,
  a score. A hand-built app on iOS has these; an Exact app cannot, which is
  a tell at the level of the whole app (LLP 1115).
- **Reach without code elsewhere.** The iPhone's own extension already
  reaches beyond the phone. On iOS 26 iPhone widgets appear in CarPlay and
  Live Activities on the CarPlay Dashboard; the watch's Smart Stack mirrors
  the iPhone's Live Activities; the Mac shows iPhone widgets and, on macOS
  26, the iPhone's Live Activities in its menu bar. HomeView (as reported:
  announced 2026-10-13, shipping 2026-11-13) has no App Store and shows the
  paired iPhone's widgets, under the program agreement's line "Your iOS
  Application widget may be made available on CarPlay and Apple HomeView".
  None of this needs a binary for the car, the home, the Mac or the watch.
- **Ports.** LLP 1069's survey lists `expo-widgets` (widgets and Live
  Activities) as missing. An app moving from Expo that has a widget cannot
  come.
- **The watch.** watchOS was kept out because it has no UIKit presenter (it
  has UIKit's value types, not `UIView`). Its UI framework is SwiftUI, and
  this RFC makes the renderer that answer needs; watchOS is now admitted
  (§0, D10).

## 2. What exists

- **No SwiftUI.** No Swift file in `host/` or `apps/` imports SwiftUI. The
  Apple presenters are UIKit and AppKit over Core Animation (about 86,000
  lines in `ExactKit`).
- **No headless session.** `ExactSession`'s initializer builds the text
  engine and the presenter's machinery (`Session.swift`); LLP 1031 made a
  session an instance a host owns, not a session without a view. Extension
  code may not call `UIApplication.shared` and similar; about 32 `ExactKit`
  files use one of these, `UIScreen.main` or `NSApp`. The text engine itself
  is Core Text and Foundation only.
- **Text is drawn line by line.** The UIKit presenter draws the kernel's
  measured lines with `CTLineDraw`, sharing one set of breaks between
  measurement and painting (`TextAnswers.swift`, `Text.swift`); the kernel
  can measure a height at a width (`fit_content_height`).
- **One process owns the state.** The update store holds an exclusive lock
  for its owner's lifetime (`update/src/store.rs`). A session sees the store
  as it was at its boot and a peer's writes only at its next boot or
  `data_ready` (LLP 1031 D4). Secrets are in the app's own Keychain access
  group (LLP 1018 D6–D7).
- **Extensions are binaries.** LLP 1030 lists "widgets, extensions, watch
  apps, helpers" as separate signed targets, binary in every column.
- **A precedent for a second root.** LLP 1101 D1–D2: the terminal entry is
  a root `.contract` named in `app.json`, compiled under the `terminal`
  profile into its own plan, admitted row by row by a column of
  `kernel/tables/schema.json`. LLP 1031 D1 deferred N plans in one envelope
  until "a named consumer whose second surface cannot be a route of the
  first"; a widget is one.
- **Refusals in force.** LLP 1035.006 §11.10 defers extension surfaces.
  `rules/DEFERRED.md` §Surfaces keeps watchOS out and admits visionOS "when
  someone ports it"; its notifications entry refuses push delivery from a
  server (LLP 1030.000 D6: exact2 sends no pushes and relays none). §Components
  admits only HTML's elements as built-in tags.

## 3. The surfaces

"Binary" is the binary whose extension draws the surface. Only rows marked
*iPhone* are reachable without a binary for another OS.

| Surface | Class | Binary | Notes |
|---|---|---|---|
| Home Screen widgets | snapshot | iPhone, iPad | tinted and clear modes on iOS 26 |
| Lock Screen widgets | snapshot | iPhone, iPad | vibrant mode; may render before first unlock |
| StandBy | snapshot | iPhone | the same widgets |
| CarPlay widgets | snapshot | iPhone | ordinary iPhone widgets, iOS 26 |
| HomeView widgets | snapshot | iPhone | as reported; `disfavoredLocations` opts out |
| iPhone widgets on the Mac | snapshot | iPhone | |
| Live Activities | snapshot | iPhone | Lock Screen, Dynamic Island (compact, minimal, expanded), StandBy; mirrored to the watch's Smart Stack, the CarPlay Dashboard, the Mac's menu bar |
| Interactive snippets | snapshot or live (to verify) | iPhone | App Intents' `SnippetIntent`, iOS 26; whether the view is archived or stays live in the app is unverified |
| A Mac app's own widgets | snapshot | macOS | desktop and Notification Center |
| Complications and watch Smart Stack widgets | snapshot | watchOS | needs watchOS admitted (D10) |
| visionOS widgets | snapshot | visionOS | needs a visionOS port |
| The watch app; a watch notification's custom view | live | watchOS | SwiftUI is the only public UI framework |
| visionOS volumes | live | visionOS | immersive spaces are RealityKit: out of this RFC |
| CarPlay apps (and HomeView's audio and video apps) | template | iPhone | `CPTemplate`, with a live app lifecycle and callbacks |
| Controls | template | iPhone | Control Center, Lock Screen, Action button |
| Standard notifications; tvOS Top Shelf; a Mac menu bar item's menu; the Dock's menu and badge | template | each OS | |
| CarPlay navigation map | hosted | iPhone | a UIKit `CPWindow` scene |
| Rich notifications; iMessage, share, action, keyboard and Quick Look extensions | hosted | each OS | a view controller; each under its own memory limit |
| App Clips | (an app) | iPhone | a size-capped app with the existing presenter |
| Swift Charts, `Gauge`, StoreKit's views, Translation; visionOS ornaments on a UIKit window | island | each OS | a native module (LLP 1024, 1067.000) |

Sources: Apple's [SwiftUI views for widgets](https://developer.apple.com/documentation/widgetkit/swiftui-views),
[ActivityKit](https://developer.apple.com/documentation/activitykit),
[SnippetIntent](https://developer.apple.com/documentation/appintents/snippetintent),
[CarPlay](https://developer.apple.com/documentation/carplay),
[UNNotificationContentExtension](https://developer.apple.com/documentation/usernotificationsui/unnotificationcontentextension);
HomeView as reported by [9to5Mac](https://9to5mac.com/2026/10/14/homeview-can-display-third-party-widgets-from-your-iphone/)
and [MacRumors](https://www.macrumors.com/2026/10/14/apple-homeview-apps-carplay/).
No decision below rests on HomeView; it is reach the iPhone's widgets get.

## 4. Design

### D1 — Admit a surface by its class, its drawer and its binary

A surface is admitted by answering the three questions of the summary. Its
class decides the execution contract (D3 for snapshots); its drawer decides
the renderer, one per drawer per platform, never one per surface; its
binary decides the packaging and whether a platform ruling is needed. A new
surface that shares all three with an admitted one costs its packaging and
its presentation rules (D5).

On Apple the snapshot driver and the later live driver share the SwiftUI
leaves: text, images, shapes, colour roles, accessibility. The live driver's
retained tree, events, scrolling, navigation and controls are new work; the
snapshot driver stands on its own value, not on the live driver's.

The SwiftUI renderer is not a presenter for the running app on iOS or macOS.
UIKit and AppKit stay those.

### D2 — A surface is an entry of the one app, compiled under the `snapshot` profile

A surface takes the terminal entry's shape (LLP 1101 D1–D2):

- `app.json` names each surface: its kind (a widget kind, a Live Activity),
  its families, its root component, its props shape, and its refresh
  function. The spelling is the spec's.
- The bake compiles each surface's root under a `snapshot` profile into its
  own plan, in the same bundle as the app's plan and under the same
  compatibility id. This pulls LLP 1031 D1's deferred trigger: the envelope
  gains its `plans` map, one plan per root, one stream.
- The profile is the admission authority: a `snapshot` column on the rows of
  `kernel/tables/schema.json`, as the `terminal` column is. A row the
  snapshot driver cannot emit within WidgetKit's archive is refused at bake
  with an edit. D6's clock refusal and D7's handler refusal are this
  profile's rules.
- The surface shares what sits below the view: shapes, functions, the data
  module, storage, routes, and components through `use` where they pass the
  profile. A type change that breaks a widget fails the same bake that
  breaks the app.

A surface's plan is a baked artifact, not a filter applied to the app's
plan at load: an extension never decodes the app's whole plan.

What is compile-time on Apple stays a binary change (LLP 1030): the widget
kinds and families (`WidgetBundle`), a Live Activity's existence, the
extension's Info.plist, the intents Siri lists by name (D7). The bake
generates that Swift. The views and data the surfaces show ride the bundle.

### D3 — A snapshot's render is pure; its work happens in a refresh step

A snapshot's render is a function of its props, a read-only store snapshot,
and the environment the system gives (family, size, colour scheme, rendering
mode, text size). The `snapshot` profile makes it pure by construction: no
clock, no timer, no input, no fetch, no native module. It is complete when
its layout is done and its images are loaded; that is the whole of
"settled" here (LLP 1113's procedure, which pumps work and activates modules,
is not needed and not used).

The work happens in the platform's refresh steps, each in a session in the
extension:

- **A timeline request**: the surface's refresh function may run data
  sources and write the store, within WidgetKit's budget (Apple documents a
  typical 40 to 70 refreshes a day), and returns timeline entries, each
  props with the date it takes effect.
- **An intent** (a tap, D7): a handler commits; the surface reloads.
- **A Live Activity update**: new state (D9). The extension only renders it.
- **A placeholder** (`placeholder(in:)`, which must answer at once without
  data): the render with props the bake records from the surface's
  declared preview values.

The render runs no app JavaScript. A refresh may run an app's TypeScript data
module, as bytecode under Hermes in the extension (`rules/RULES.md`: modules
ship as bytecode); "first pixel" has no meaning in a refresh, and the
refresh's budget is Q3's.

The render may run several times per refresh: once per timeline entry and
per environment variant WidgetKit asks for. Q3 counts that.

### D4 — Layout stays CSS: fixed frames, the kernel's line breaks, platform text

The snapshot driver lays the entry out with the kernel at the size the
system proposes and emits, as the proposal stage 0 tests:

- **Fixed frames.** Each box is a SwiftUI shape or container at the kernel's
  frame, by fixed frames and offsets. A custom `Layout` is used only if Q1
  shows WidgetKit's archive keeps it.
- **The kernel's lines.** A paragraph is emitted as one single-line `Text`
  per kernel line, so SwiftUI draws and never re-breaks. Colours are roles,
  never resolved colours, so the rendering modes can tint them.
- **Live text in a reserved width.** A platform-formatted time (D6) is a
  SwiftUI `Text` with a date style, in a frame the kernel sized for its
  widest value.
- **Regions the system sizes.** For a Dynamic Island region or a Lock Screen
  Live Activity, the kernel measures the content's size at the region's
  width (`fit_content_height`) and the frame takes it.
- **Pictures only for pictures.** A canvas or an app-drawn chart is a
  picture leaf (SwiftUI's `Canvas` is on WidgetKit's list, so a Canvas 2D
  recording can replay into it). A whole surface is never a picture.

Rejected: translating CSS to SwiftUI stacks (a second layout engine,
disagreeing with the window: the bug class the kernel ends); a picture by
default (no vibrant or accented mode by role, no text size, nothing for
VoiceOver, no live text).

**Kill line.** If fixed frames with per-line text do not survive the archive,
or the text does not hold its breaks across the rendering modes and text
sizes, D4 stops and the fallback is decided then from two: narrow the
`snapshot` profile to what does survive, or the strongest alternative the
reviews named, author-written SwiftUI views over Exact's data seam
(Alternatives, §6).

D4 decides Apple. Android's snapshot drawers (RemoteViews, ProtoLayout)
re-lay out at their own sizes; its RFC decides between frames (a
RemoteViews `FrameLayout` can take them), translation and pictures.

### D5 — On a snapshot, the platform's presentation wins where the author is silent

LLP 1115 applies unchanged, accessibility included; judge against a
hand-built WidgetKit widget on the same OS. The snapshot driver supplies
what the author leaves unsaid:

- the root's background becomes the `containerBackground`, which the system
  removes where widgets have none (StandBy, the Lock Screen);
- content margins are the system's;
- under the accented and vibrant modes each node takes the mode its role
  gives (text and symbols tint; images desaturate unless the author says
  otherwise);
- colour roles (LLP 1095) resolve to the system's dynamic colours;
- a Live Activity's Dynamic Island keyline and region layout are the system's;
- accessibility labels and roles carry over from the nodes.

### D6 — Time on a snapshot belongs to the platform, through HTML's `time` and `progress`

A snapshot cannot tick, but the system can: `Text(date, style:)` and
`ProgressView(timerInterval:)` update with no code running. Exact reaches
them through HTML's own elements (`rules/DEFERRED.md` §Components admits no
other): `time` with a `datetime` and a named format (a relative time, a
timer, a countdown), and `progress` over an interval. The platform formats
them (LLP 1115 §3). The running app's presenters format the same elements on
the runner's clock, so an author writes one view for the app and the widget.

A snapshot entry that reads the clock otherwise fails the bake, with an edit
that names the element. Caltrain's departure card computes its countdown
from `nowMs`; its widget says "a countdown to `d.at`". A change that is not a
ticking date (the next train replacing the last) is a timeline entry with
its own date.

### D7 — A tap is a URL or an intent; an archived tap carries its plan's digest

- **Open the app somewhere.** A location is a URL (LLP 1038), so a link in a
  snapshot is `Link` or `widgetURL` with the route's URL.
- **Act without opening the app** (iOS 17's interactive widgets and Live
  Activities). A handler on a `Button` or `Toggle` becomes one generic
  `AppIntent` carrying the action's name, its arguments and the digest of
  the plan that rendered it. The intent runs the handler in the extension,
  commits, and reloads the surface. A handler that navigates or needs input
  is refused at bake, with an edit that makes it a link.
- **A stale tap.** The system can keep an archive across an update. A tap
  whose digest the installed plan does not know is refused without running
  anything, and the surface reloads.

Intents Siri, Shortcuts and snippets list by name must be Swift types; the
bake generates them from actions the app declares as such, as a binary
change (D2), at a later stage.

These rules are a snapshot's. A template's app (CarPlay) has a live
lifecycle and callbacks, and its RFC sets its own (D11).

### D8 — One cohort, one owner: the app updates, the extensions read

**One compatibility cohort per app.** The app and every extension link the
same set, in one framework they all load, so they share one compatibility
id, one stream and one bundle (LLP 1030 D3a). Pruning per surface is the
plan's (D2), never the binary's. This amends LLP 1030's extension row:
an extension takes the app's bundle. Q4 measures what the shared framework
costs the app's launch; it is not a choice of cohort.

**The app owns updates.** The app alone opens the update store and its lock.
It publishes generations as immutable directories in the app group's
container and keeps a generation while an extension may be reading it. An
extension reads the generation the app last made current, pinned for the
whole invocation, and never fetches, selects or repairs one.

**Two writers, one store.** An intent commits in the extension while the app
may be running. The store's writes stay transactional across processes, and
each commit bumps a generation counter in the container. The app, at
foreground and on that counter moving, re-asks the resources that read the
store (the existing `data_ready` path); after a commit that touches a
surface's inputs it reloads that surface's timelines. The answer stays a pull
(LLP 1016.002); there is no shared heap and no reactive shared store. A
widget's write reaches an open app at its next foreground, not at once (§7,
question 6).

**The render touches no secret.** A Lock Screen surface can render before
first unlock, when the Keychain's usual class is unavailable. Secrets are
read only in refresh steps; the render reads its props and the plain store.

**A session without a presenter.** The extension needs an `ExactKit`
composition that builds no presenter and calls no app-only API
(`APPLICATION_EXTENSION_API_ONLY`), with the kernel, the runner, text and the
snapshot driver. It is extracted from today's session, not added beside it.

**A budget with a kill line.** An extension runs under limits the app does
not have (a widget's is commonly reported near 30 MB and is not a documented
number). Stage 0 measures the ceiling on the oldest supported iPhone, then a
session's peak resident memory, cold refresh time and cold intent time. The
proposed kill line: the session's peak above half the measured ceiling, or a
cold refresh or a tap's commit above one second. Past it, the fallback is a
render-only extension (§6).

### D9 — What the system keeps between versions is a generic envelope

A Live Activity's state lives in the system and in pushes, across updates of
the app. If each app's state were a generated Swift `Codable`, a change to
its shape would be a binary contract: a payload the installed decoder cannot
read stops the activity. So every Exact Live Activity has one Swift state
type: an envelope with the shape's digest and the state as JSON. The plan
decodes the JSON by its Contract shape. A shape change rides the bundle; the
app keeps the shape of every activity it still has alive; a payload whose
digest the plan does not know renders the activity's last good state, marked
stale. The bake emits the shape's TypeScript type and JSON Schema for the
app's own backend.

An activity's root declares its placements (Lock Screen; the Dynamic
Island's compact, minimal and expanded regions) as named slots of one
component. The watch's Smart Stack, the CarPlay Dashboard and the Mac's menu
bar take placements the system derives from those.

Starting, updating and ending an activity are sends from the running app,
present where ActivityKit exists and absent elsewhere (Charlie, 2026-09-27,
LLP 1055.000 Q5: "allow platforms to reach their full platform rather than
sanding everything down to lowest common denominator"). With D6, an
activity updated by its app while it runs and counting down on its own is
useful by itself: the first consumer needs no push.

Updates from the app's own server, through ActivityKit push tokens, are what
most Live Activities are for (a delivery, a flight, a score), and
`rules/DEFERRED.md` refuses push delivery from a server. Exact would relay
nothing (LLP 1030.000 D6): the app registers its token with its own backend,
which sends; the 4 KB limit applies there. Admitting that is §7, question 5.

### D10 — The watch: bring-up beside stage 0, complications with the snapshot driver, then the live driver

watchOS is admitted (§0). Anything that runs on the watch needs a watchOS
binary: complications and the watch's own Smart Stack widgets come from a
widget extension inside a watchOS app. That costs, before any renderer:

- Rust's watchOS targets, Tier 3 (nightly with `-Zbuild-std`); older watches
  need `arm64_32`, whose `usize` is 32 bits;
- `ExactKit`'s presenter-free composition (D8) compiling for watchOS;
- no Metal: no wgpu, no GPU canvas, no GPU module;
- restricted low-level networking on a device ([TN3135](https://developer.apple.com/documentation/technotes/tn3135-low-level-networking-on-watchos)),
  which reaches app WebSockets, the agent's socket carrier and the dev
  connection; HTTP through URLSession is unrestricted;
- no JIT and no JavaScriptCore, so `app.ts` needs a Hermes built for
  watchOS or waits.

What reaches the wrist without any of that comes with stage 2: the iPhone's
Live Activities in the Smart Stack.

The bring-up (the toolchain, the presenter-free composition on watchOS, a
watchOS app that boots a session and draws a complication) starts beside
stage 0, since it shares nothing with stage 0's probes but their results.
Complications then come with the snapshot driver; the live driver and the
watch app follow. The live driver keeps one
observable model per node, updated from the runner's batches, places frames
with a custom `Layout` (no archive restricts it), and puts the platform's
controls, scrolling and navigation at the leaves (`Picker` on the Digital
Crown, `TextField` with the system's input sheet, `ScrollView`,
`NavigationStack` bound to the router, haptics through `WKInterfaceDevice`).
A watch app is its own small app, so its entry is the author's to write. Its
design is a later RFC's.

### D11 — Templates get their own RFC

CarPlay apps (and so HomeView's audio and video apps), controls, standard
notifications, Top Shelf, a Mac menu bar item's menu and the Dock's are drawn
by the system from data the app gives. They need a mapping, not a renderer:
an entry whose tree is restricted to a template's vocabulary maps to it.
They share D2's entry and D8's cohort. A CarPlay app has a live lifecycle and
callbacks, unlike a snapshot, so the template RFC sets its own action rules.

### D12 — Hosted and island surfaces need no new renderer

A rich notification, an iMessage app, a share, action, keyboard or Quick
Look extension, and CarPlay's navigation map are UIKit or AppKit views: the
existing presenter, in D8's presenter-free composition plus a presenter,
under that extension's own limits (a keyboard's is tight). Each is admitted
when an app needs it. An App Clip is an app. A SwiftUI-only system view in
the running app (Swift Charts, `Gauge`, StoreKit's views, Translation's
modifiers, an ornament on a visionOS UIKit window) is a native module, as
Expo Services' number transition is today.

### D13 — Android has the same questions, answered by its own RFC

The three questions are platform-neutral, and Android's answers differ in
the second: Glance widgets (over RemoteViews) and a custom notification
(RemoteViews) are snapshots drawn by a different drawer than Wear OS Tiles
(ProtoLayout); Wear OS complications, Quick Settings tiles and Android Auto's
Car App Library are templates; a Wear OS app in Compose is live. Whether
Android 16's Live Updates are a template or can carry a custom view is
unverified. These are hypotheses for Android's RFC. What D2 makes portable is
the entry, its shapes and its data; its class and vocabulary may differ by
platform.

### D14 — Development and the agent see what the extension sees

An unsupported view inside a WidgetKit archive draws nothing, with a console
line. The guards:

- **The profile refuses at bake** (D2).
- **One driver in-process and in the extension.** A development build of the
  app has a gallery of its surfaces, rendered by the same snapshot driver at
  each family's size and in each rendering mode and variant; the agent
  addresses it (`tree`, `screenshot`, `tap` as the intent, `state`) by
  surface, family and props.
- **The real extension in the async lane.** A simulator test loads the
  extension through WidgetKit and compares its archived render with the
  gallery's.

Each of the three is apparatus `rules/RULES.md` §Agents says needs a human's
yes; §7, question 8 asks for it.

## 5. Stages

0. **Probes, as a decision gate.** Throwaway, on a simulator and the oldest
   supported iPhone, against a hand-built SwiftUI widget for Caltrain as the
   fidelity baseline (the reference for "would someone who knows the
   platform notice?"). Q1 to Q5 (§8), each with its kill line (D4, D8). The
   probes exercise a real archived widget and a minimal Live Activity
   region, not the gallery. Past a kill line the stage ends with a report and
   a fallback proposal, not a workaround.
1. **The snapshot driver and iPhone widgets** for one app: D2–D8, D14.
   StandBy, CarPlay, HomeView and the Mac show them with no other code.
2. **Live Activities and interactive widgets**: D7's intents and D9, updated
   by the running app. The watch's Smart Stack and the Mac's menu bar mirror
   them.
3. **The other WidgetKit binaries**: a Mac app's own widgets, and the
   watch's complications and Smart Stack widgets, by the same driver.
   visionOS widgets come with the visionOS port (`rules/DEFERRED.md`
   §Surfaces admits it "when someone ports it"; full Apple support for
   launch, §0, asks for that port).
4. **The live driver and the watch app** (D10).

**W. watchOS bring-up**, beside stages 0 and 1: Rust's watchOS targets, the
presenter-free composition compiled for watchOS, and a watchOS app that boots
a session and shows a complication drawn by stage 1's driver as soon as it
exists.

Templates (D11) and Android (D13) are their own RFCs.

## 6. Alternatives

- **Author-written SwiftUI widgets over Exact's data seam** (what
  `expo-widgets` does: the app ships Swift views that read the group
  container; the bake gives them the shapes and the store). Highest fidelity
  and no renderer. It loses on what this repository is for: the author (often
  an agent) writes a second language and a second layout model, the agent
  cannot drive what it wrote, and nothing ports to Android. It is D4's
  fallback, and its hand-built Caltrain widget is stage 0's baseline.
- **A render-only extension.** The app, or a refresh helper, writes a display
  list; a Swift-only inflater draws it, and the extension links no kernel or
  runner. It wins if Q3 fails. It loses otherwise: the system picks size,
  rendering mode and text size at archive time, often with the app not
  running, so a list baked for one environment is wrong in another, and
  timeline refreshes and intents need the data and the store in the extension
  anyway. It is D8's fallback.
- **Translating CSS to SwiftUI stacks.** Rejected in D4.
- **A picture by default.** Rejected in D4; kept for picture leaves.

## 7. Questions for Charlie

1. **The shape.** Ruled (§0): yes.
2. **LLP 1035.006 §11.10.** Ruled (§0): reopened; the take is waived.
3. **watchOS.** Ruled (§0): admitted now; full Apple platform support for
   launch.
4. **The word.** "Surface" already names a Canvas surface (`ExactSurfaces`)
   and a platform (`rules/DEFERRED.md` §Surfaces); LLP 1124 uses "extension".
   A name for these entries is wanted before the manifest has a key.
5. **Live Activity pushes.** Admit ActivityKit push tokens, the app's own
   backend sending and exact2 relaying nothing, as a scoped exception to the
   push refusal. Proposed: decide at stage 2; stage 2 ships without it.
6. **A widget's write and an open app.** Visible at the app's next
   foreground (D8), not at once. Proposed: yes.
7. **The first consumer.** Caltrain (public timetable, no secrets, a
   countdown) is proposed for stages 0 and 1; an app that ships to users
   (Expo Services, Interview) for stage 2.
8. **Apparatus.** The gallery, the agent's surface addressing, and the
   async-lane extension test (D14). Proposed: yes, at stage 1.
9. **The working set.** If this enters `llp/current/`, which document leaves.

## 8. Open technical questions (stage 0)

- **Q1. The archive.** Is the archive a value evaluated in the extension per
  variant (then `Layout` and `GeometryReader` may both survive), or a
  description the system re-evaluates (then fixed frames only)? This first
  finding decides much of Q1 to Q3. Then: fixed frames and offsets, content
  margins, a Dynamic Island region sized by `fit_content_height`, a Dynamic
  Type change.
- **Q2. Text.** Per-line `Text` holding the kernel's breaks across fonts,
  text sizes and the accented and vibrant modes; a ticking `time` in a
  reserved width; VoiceOver reading the paragraph as one.
- **Q3. A session in an extension.** Against the measured ceiling: peak
  memory, cold refresh and cold intent time, for a pruned plan, with a
  20-entry timeline times its variants, and for a TypeScript app's refresh
  under Hermes as well as Caltrain's Rust; an app and an extension writing
  the store at once.
- **Q4. One framework.** The app's launch with the runtime in a shared
  dynamic framework against today's static archive, and link-by-use (LLP
  1047.001) for one cohort.
- **Q5. Detecting an unsupported view.** Whether the archive's refusal can
  be observed by a test, or only the profile catches it.

## 9. What this amends, at acceptance

- LLP 1030's extension row: an extension takes the app's bundle, in the
  app's cohort (D8).
- LLP 1031 D1: the `plans` map's trigger is pulled (D2).
- LLP 1035.006 §11.10: reopened (done, 2026-10-17).
- `kernel/tables/schema.json`: a `snapshot` admission column (D2).
- `rules/DEFERRED.md`: §Surfaces admits system surfaces and watchOS (done,
  2026-10-17); the notifications entry's push refusal only if question 5
  admits Live Activity pushes.
  §Components is unchanged (D6 uses HTML's elements).

## R. Round 1: what r2 took

| Finding | From | r2 |
|---|---|---|
| Class alone does not decide the renderer or the packaging; separate execution, drawer and binary | Astra, Grok, Fable | Summary, D1, §3's binary column |
| Watch complications need a watchOS binary; stage 3 decided watchOS early; "reaches the wrist" is not "runs on it" | Fable, Grok, Astra | D10, §1, §3, §5 (then stage 4; since §0, stage 3 and track W) |
| visionOS widgets need a visionOS binary; immersive spaces are RealityKit; ornaments are islands | Grok, Astra, Fable | §3, D12, §5 |
| D8 needs a cross-process owner: the update store's exclusive lock, LLP 1031 D4's snapshots, two writers | Astra, Fable, Grok | D8: the app owns updates, immutable generations, a counter and a re-ask |
| One cohort per app, decided, not probed; amends LLP 1030's extension row | Fable | D8, §9 |
| Generated per-app Swift is a binary contract; archived taps outlive plans | Astra | D9's envelope, D7's digest and stale-tap rule, D2's binary/bundle line |
| Server push is refused by `rules/DEFERRED.md` | Grok | D9, §7 question 5 |
| Take LLP 1101's entry shape and a schema admission column; pull LLP 1031 D1's trigger | Fable (Grok: one plan with a baked prune) | D2: a baked plan per root, one cohort; a load-time filter rejected |
| Text: per-line `Text` from the kernel's breaks, roles not resolved colours, reserved width for live text | Grok, Fable, Astra | D4 |
| D4 needs a kill line and a fallback; stage 0 is a decision gate | Grok, Astra | D4, D8, §5 |
| A headless session must be extracted; extension-safe API | Astra, Fable | §2, D8 |
| LLP 1113's settle procedure is not D3's contract | Astra | D3 |
| The render touches no secret (Lock Screen before first unlock) | Grok | D8 |
| D6 must use HTML's `time` and `progress` (§Components) | Fable | D6 |
| Variants and timeline entries multiply the render; TypeScript refresh under Hermes; placeholder | Fable | D3, Q3 |
| D4 decides Apple only; Android cells are hypotheses | Fable, Grok, Astra | D4, D13 |
| CarPlay templates have a live lifecycle; the navigation map is hosted; App Clips are apps; Mac menu bar, Dock, Quick Look | Astra, Grok, Fable | §3, D7, D11, D12 |
| The strongest alternatives: author-written SwiftUI; a render-only extension | Astra, Fable, Grok | §6, as D4's and D8's fallbacks; stage 0's baseline |
| D14 adds apparatus needing a human's yes | Fable | D14, §7 question 8 |
| "Snapshot first wastes nothing" overclaims | Fable | D1 |
| A widget write's visibility in an open app | Astra | D8, §7 question 6 |

Not taken: the reviewers' implementation-level notes (the exact 4 KB scope
for local updates, the HomeView details none could verify) are the spec's.
