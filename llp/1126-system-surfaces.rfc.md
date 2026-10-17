# LLP 1126: System surfaces — an app's views outside its window

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built until Charlie has read it)
**Status:** Draft r1, 2026-10-17. Proposed from outside the working set: it reopens LLP 1035.006 item 11.10 (app extensions and system surfaces, deferred) and, at its last stage, the `rules/DEFERRED.md` §Surfaces line "watchOS, which has no UIKit, stays out". Both need Charlie's ruling (§7). Not in `llp/current/`, which holds 15 of 15.
**Systems:** a new SwiftUI renderer in the Apple host (`host/apple/Sources`), the app manifest (`scripts/app.schema.json`), the bake (`contract/cli`), the plan's entries (`plan/`), the runner's headless sessions (LLP 1031), the store's location (LLP 1018), delivery's update store (LLP 1030), the Apple build's packaging (`host/apple/build.mjs`), the agent (`scripts/agent.mjs`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-17
**Implementer:** none yet. Stage 0 (§5) is a set of probes; a spec follows a ruling.
**Related:** LLP 1115 (write the web, ship the platform) and 1115.000; LLP 1035.006 §11.10; `rules/DEFERRED.md` §Surfaces; LLP 1031 (sessions embedded in a host); LLP 1018 (the store); LLP 1030 (delivery); LLP 1038 (a location is a URL); LLP 1047 (pay for what you use); LLP 1024 and 1067.000 (native modules); LLP 1112 and 1113 (a state as an image; settled); LLP 1069 (the Expo SDK survey: `expo-widgets` missing); LLP 1124 (host extensions — a different sense of "extension", §7 question 4)

## Summary

Apple puts an app's content in many places outside its window: Home Screen,
Lock Screen and StandBy widgets, Live Activities and the Dynamic Island,
watch complications, CarPlay and HomeView widgets, Siri and Spotlight
snippets, CarPlay apps, Control Center controls, notifications, and the
Apple Watch app itself. Exact can put content in none of them.

These places differ less than their number suggests. Each runs in one of
five ways, and the way it runs, not the place, decides what Exact needs:

1. **Snapshot.** Code runs briefly in an extension and returns a SwiftUI
   view value; the system archives it and draws it with no code running
   (WidgetKit: every widget, every Live Activity placement, complications,
   snippets).
2. **Live.** A running SwiftUI view tree (the watch app, a watch
   notification's custom view; later visionOS volumes and ornaments).
3. **Template.** The app gives the system data and the system draws (CarPlay
   apps, which HomeView also runs; Control Center controls; standard
   notifications; tvOS Top Shelf).
4. **Hosted.** A UIKit view controller in an extension, which the existing
   presenter can fill (rich notifications on iOS, iMessage, share and
   keyboard extensions).
5. **Island.** A SwiftUI-only system view inside the running app (Swift
   Charts, StoreKit's views), which native modules already host.

So Exact needs one new renderer, not one per place: a **SwiftUI renderer**
that turns a subtree the kernel has laid out into SwiftUI views, with two
drivers. The **snapshot driver** renders an entry once from its inputs, under
WidgetKit's rules; it serves every WidgetKit surface on iOS, iPadOS, macOS,
watchOS and visionOS, and with them CarPlay and HomeView, which show iPhone
widgets. The **live driver** keeps a retained tree that the runner updates;
it serves the watch app. Templates need a mapping, not a renderer, and get
their own RFC. Hosted and island surfaces need nothing new.

The decisions that matter most: a surface is an entry of the one app, baked
with it (D2); a snapshot is a pure render whose work happens in a separate
refresh step (D3); layout stays CSS, computed by the kernel and placed in
SwiftUI, never translated to SwiftUI stacks or flattened to a picture (D4);
time and taps on a snapshot belong to the platform (D6, D7). The snapshot
driver comes first; the live driver reuses it.

## 1. Motivation

- **Where users look.** Many apps are seen more often in a widget or a Live
  Activity than in their own window: a delivery's progress, the next train,
  a score. A hand-built app on iOS has these; an Exact app cannot have them,
  which is a tell at the level of the whole app (LLP 1115).
- **Reach without new code.** On iOS 26 any iPhone widget can appear in
  CarPlay, and Live Activities appear on the CarPlay Dashboard. HomeView
  (announced 2026-10-13, shipping 2026-11-13) has no App Store; it shows the
  paired iPhone's widgets and runs its CarPlay audio and video apps. Apple's
  updated program agreement says "Your iOS Application widget may be made
  available on CarPlay and Apple HomeView", with `disfavoredLocations` to opt
  out. A watch's Smart Stack shows the iPhone's Live Activities. A widget
  written once reaches the phone, the car, the home and the wrist.
- **Ports.** LLP 1069's survey lists `expo-widgets` (widgets and Live
  Activities) as missing. An app moving from Expo that has a widget cannot
  come.
- **The watch.** `rules/DEFERRED.md` keeps watchOS out because it has no
  UIKit. Its UI framework is SwiftUI; with a SwiftUI renderer that reason
  goes away, and the rest of the watch's cost (§D10) can be weighed on its
  own.

## 2. What exists

- **No SwiftUI.** No Swift file in `host/` or `apps/` imports SwiftUI. The
  Apple presenters are UIKit and AppKit over Core Animation: about 86,000
  lines in `ExactKit`, 162 files importing UIKit and 153 importing AppKit.
  SwiftUI appears only in comments, as the behaviour to match.
- **What a SwiftUI renderer could reuse.** 66 of `ExactKit`'s 162 top-level
  files (about 11,700 lines) import no UI framework: the wire decode
  (`Bridge`, `BatchReader`, `Batch`, `BatchShapes`), text on Core Text
  (`TextResidency`, `TextFlow`, `InlineText`, `ParagraphGeometry`,
  `TextBreaks`, `TextAnswers`), images (`RasterImage`, `AnimatedRaster`),
  and plumbing (`Owner`, `PlanURL`, `Assets`). Clean imports do not prove
  they compile in an extension or for watchOS. All of the Rust below the
  C ABI carries over.
- **Sessions.** LLP 1031 made a session an instance an embedding host owns.
  An extension is such a host.
- **The store and delivery.** LLP 1018's store and LLP 1030's update store
  live in the app's container today, where an extension cannot read them.
- **Pictures and settled.** LLP 1112 and 1113 define when a state is
  settled enough to capture. A snapshot is a capture of the same kind.
- **Deferrals.** LLP 1035.006 §11.10 defers widgets, Live Activities and
  other extension surfaces ("not arbitrary views mounted inside Exact").
  `rules/DEFERRED.md` §Surfaces admits the Apple platforms UIKit covers and
  keeps watchOS out.

## 3. The surfaces

| Surface | Class | Where | Notes |
|---|---|---|---|
| Home Screen widgets | snapshot | iOS, iPadOS | tinted and clear modes on iOS 26 |
| Lock Screen widgets | snapshot | iOS, iPadOS | the vibrant rendering mode |
| StandBy | snapshot | iOS | the same widgets; red-tinted at night |
| CarPlay widgets | snapshot | iPhone, shown in the car | ordinary iPhone widgets (iOS 26) |
| HomeView widgets | snapshot | iPhone, shown on HomeView | the same; `disfavoredLocations` opts out |
| Mac desktop and Notification Center widgets | snapshot | macOS; iPhone widgets on the Mac | |
| visionOS widgets | snapshot | visionOS 26 | placed in the room |
| Complications, Smart Stack | snapshot | watchOS | WidgetKit's accessory families |
| Live Activities | snapshot | Lock Screen, Dynamic Island (compact, minimal, expanded), StandBy, watch Smart Stack, CarPlay Dashboard, the Mac's menu bar | one view, several placements; ActivityKit |
| Interactive snippets | snapshot (to verify) | Siri, Spotlight, Visual Intelligence | App Intents' `SnippetIntent`, iOS 26 |
| The watch app | live | watchOS | SwiftUI is the only public UI framework |
| A watch notification's custom view | live | watchOS | `WKUserNotificationHostingController` |
| visionOS volumes, ornaments, immersive spaces | live | visionOS | SwiftUI with RealityKit; windows ride the UIKit presenter |
| CarPlay apps | template | the car; HomeView runs the audio and video ones | `CPTemplate` by entitlement category; a navigation app's map is a UIKit `CPWindow` |
| Controls | template | Control Center, Lock Screen, Action button | `ControlWidgetButton` and `ControlWidgetToggle`: a symbol and a label |
| Standard notifications | template | every Apple OS | title, body, attachments, actions |
| tvOS Top Shelf | template | tvOS | |
| Rich notifications | hosted | iOS | `UNNotificationContentExtension` is a `UIViewController` |
| iMessage, share, action and keyboard extensions; App Clips | hosted | iOS | |
| Swift Charts, `Gauge`, StoreKit's views, Translation | island | every Apple OS | a native module hosts them (LLP 1024, 1067.000) |

Sources: Apple's [SwiftUI views for widgets](https://developer.apple.com/documentation/widgetkit/swiftui-views),
[ActivityKit](https://developer.apple.com/documentation/activitykit),
[SnippetIntent](https://developer.apple.com/documentation/appintents/snippetintent),
[CarPlay](https://developer.apple.com/documentation/carplay),
[UNNotificationContentExtension](https://developer.apple.com/documentation/usernotificationsui/unnotificationcontentextension);
HomeView as reported by [9to5Mac](https://9to5mac.com/2026/10/14/homeview-can-display-third-party-widgets-from-your-iphone/)
and [MacRumors](https://www.macrumors.com/2026/10/14/apple-homeview-apps-carplay/).

## 4. Design

### D1 — A renderer per class of surface, never per surface

Exact admits a surface by saying which class it is in. A class has one
renderer (or, for templates, one mapping) on each platform; a new surface in
an existing class costs its packaging and its presentation rules (D5), not
a renderer. Snapshot and live share one SwiftUI core: the code that turns a
laid-out node into SwiftUI views and modifiers. Live is the snapshot core
plus a retained tree, events and native controls, so building snapshot first
wastes nothing.

The SwiftUI renderer is not a third presenter for iOS or macOS. UIKit and
AppKit stay the presenters of the running app there.

### D2 — A surface is an entry of the one app, baked with it

A widget, a Live Activity or a complication is a component of the app's own
Contract, named in `app.json` as an entry with its class, its families and
its inputs:

```json
"surfaces": {
  "next-train": { "class": "snapshot", "kind": "widget",
                  "families": ["systemSmall", "systemMedium", "accessoryRectangular"],
                  "component": "NextTrainWidget", "refresh": "nextTrainRefresh" },
  "trip": { "class": "snapshot", "kind": "liveActivity",
            "component": "TripActivity", "state": "TripState" }
}
```

(The shape is illustrative; its spelling is the spec's.) One bake makes one
plan: the surfaces share the app's shapes, functions, store schema and data
sources, and a type change that breaks a widget fails the same bake that
breaks the app. The bake gives each surface's process only what that
surface reaches (LLP 1047's link by use applied to entries), so a widget's
extension does not carry the app's screens.

The alternative, a separate app per surface, duplicates the types the
surface and the app must agree on, and lets them drift.

### D3 — A snapshot is a pure render; its work happens in a refresh step

A snapshot entry's render is a function of its props, a read-only snapshot
of the store, and the environment the system gives (family, size, colour
scheme, rendering mode, text size). It reads no clock, starts no timer,
takes no input and fetches nothing; it is rendered when settled (LLP 1113),
and the render is what the system archives.

The work that changes what a snapshot shows happens outside the render, in
one of three refresh steps the platform defines:

- **A timeline request** (a widget's `TimelineProvider`): the entry's
  `refresh` function runs in a headless session in the extension. It may
  run data sources and write the store, within WidgetKit's budget (Apple
  documents a typical 40 to 70 refreshes a day), and returns the props of
  one or more timeline entries, each with the date it takes effect.
- **An intent** (a tap; D7): a handler runs in a headless session and
  commits; the surface then reloads.
- **A Live Activity update**: new `ContentState`, from the app or a push
  (D9). The extension only renders it.

This split is the foundation of the design. It keeps the render small
enough to run in an extension's memory and time, makes it deterministic
enough to test as an image (LLP 1112), and puts data access where the
platform budgets it.

### D4 — Layout stays CSS: the kernel lays out, SwiftUI places

The snapshot driver lays the entry out with the kernel at the size the
system proposes, and emits SwiftUI primitives (shapes, `Text`, `Image`,
gradients, `Canvas`) placed at the kernel's frames. The web stays the
layout oracle (LLP 1115: "layout stays CSS").

Two alternatives are rejected:

- **Translating CSS to SwiftUI stacks** and letting SwiftUI lay out. SwiftUI's
  stack, spacer and priority rules are not flexbox; every app would lay out
  one way in its window and another in its widget. This is the
  four-disagreeing-layers bug class the kernel exists to end.
- **Rendering a picture** (Core Graphics or the CPU painter into an `Image`)
  as the default. It is the cheapest path to pixels, and it is a tell: an
  image does not take the Lock Screen's vibrant mode or the accented mode
  by role, does not follow the text size, gives VoiceOver nothing, and
  cannot carry the platform's live text (D6). A picture is admitted for a
  leaf that is a picture anyway (a canvas, a chart drawn by the app);
  SwiftUI's `Canvas` is on WidgetKit's list, so a Canvas 2D recording can
  replay into it.

The system owns some geometry: a Dynamic Island region's size, a Lock
Screen Live Activity's height. There the kernel needs the region's proposal
and the region needs the content's size; how the two meet inside an archive
(a custom `Layout`, which Apple's list of widget views does not name, or a
measured fixed frame) is stage 0's first probe (§8, Q1).

### D5 — On a snapshot, the platform's presentation wins where the author is silent

LLP 1115 applies unchanged; judge against a hand-built WidgetKit widget on
the same OS. What the author leaves unsaid is the platform's, and the
snapshot driver supplies it:

- the entry root's background becomes the `containerBackground`, which the
  system removes where widgets have none (StandBy, the Lock Screen);
- content margins are the system's;
- under the accented and vibrant rendering modes, each node takes the mode
  its role gives (text and symbols tint; images desaturate unless the author
  says otherwise), with an author's override by name;
- colour roles (LLP 1095) resolve to the system's dynamic colours, which the
  archive keeps;
- a Live Activity's Dynamic Island keyline and its region layout are the
  system's;
- accessibility labels and roles carry over from the nodes.

### D6 — Time on a snapshot belongs to the platform

A snapshot cannot tick, but the system can: `Text(date, style: .timer)`,
`.relative` and `.offset`, and `ProgressView(timerInterval:)`, update with
no code running. Exact admits that vocabulary as elements whose value is a
date or an interval and whose format is named (a relative time, a timer, a
countdown, progress over an interval). They are elements on every host:
the running app's presenters format them on the runner's clock, so an
author writes one view for the app and the widget.

A snapshot entry that reads the clock otherwise fails the bake, with an
edit that names the element. Caltrain's departure card computes its
countdown from `nowMs`; its widget would say "a countdown to `d.at`"
instead. A change that is not a ticking date (the next train replacing the
last) is a timeline entry with its own date (D3). How a snapshot's text
reserves width for a value that changes with no code running is part of
stage 0 (§8, Q2).

### D7 — A tap on a snapshot is an intent or a URL

A snapshot has two kinds of tap, as WidgetKit does:

- **Open the app somewhere.** A location is a URL (LLP 1038), so a link in
  a snapshot entry is `Link` or `widgetURL` with the route's URL. The app
  opens at that route.
- **Do something without opening the app** (iOS 17's interactive widgets
  and Live Activities). An event handler on a `Button` or `Toggle` becomes
  an `AppIntent`. The intent boots a headless session, runs the handler
  against the store, commits, and reloads the surface. A handler that
  navigates or needs input is refused here, with an edit that makes it a
  link.

App Intents' metadata is read at build time, so intents cannot be made at
run time. One generic intent, which carries an action's name and arguments,
serves every widget tap. Intents that Siri, Shortcuts and snippets list by
name must be Swift types; the bake generates them from actions the app
declares as such (a later stage).

### D8 — An extension embeds a headless session

Each snapshot surface is packaged as a WidgetKit extension that embeds an
Exact session (LLP 1031) with no presenter: the plan, the runner, the
kernel, text, and the snapshot driver. Three things follow:

- **One copy of the runtime.** An app with a widget and a Live Activity has
  at least two processes linking Exact. The runtime is shared as one
  framework that the app and its extensions load, not a static archive
  copied into each. Whether link-by-use (LLP 1047) still applies to a
  shared framework, or extensions take a smaller one, is stage 0's (§8, Q4).
- **One container.** The store (LLP 1018) and the update store (LLP 1030)
  move to the app group's container, and Keychain items to a shared access
  group, so the app and its extensions read one state and run one plan. The
  app owns updates; an extension runs whatever bundle the app last made
  current, and never fetches one.
- **A budget.** An extension runs under memory and time limits the app does
  not have (commonly reported near 30 MB for a widget). The boot rule
  (`rules/RULES.md`: the boot path executes and compiles nothing) and the
  surface's pruned plan (D2) are what keep a session inside it. Stage 0
  measures it before anything is built (§8, Q3).

### D9 — A Live Activity's state is a Contract shape, and the bake gives it to servers

A Live Activity changes only when its `ContentState` changes, from the app
through ActivityKit or from a server through a push; Apple limits an update
to 4 KB. Its `ContentState` is a Contract shape named by the surface (D2).
The bake emits the Swift `Codable` type the extension decodes and a
TypeScript type (and JSON Schema) a server uses to build the push, so the
app, the extension and the server cannot disagree on the payload's shape.

Starting, updating and ending an activity are sends from the app, which
work where ActivityKit exists and are absent elsewhere (Charlie,
2026-09-27, LLP 1055.000 Q5: "allow platforms to reach their full platform
rather than sanding everything down to lowest common denominator"). An activity's
entry declares its placements (Lock Screen, the Dynamic Island's compact,
minimal and expanded regions) as named slots of one component; the
watch's Smart Stack, the CarPlay Dashboard and the Mac's menu bar take
placements the system derives from those.

A push can carry a `ContentState` from a newer version of the app than the
one installed. The payload carries the shape's digest; how an extension
renders a payload whose digest it does not know follows the store's
compatibility rule (LLP 1030's compatibility id), and is a spec question.

### D10 — The live driver, and the watch app, come after the snapshot driver

The live driver is the snapshot core with a retained tree: one observable
model per node, updated from the runner's batches; frames placed by a
custom `Layout` (no archive restricts it here); and the platform's
controls, scrolling and navigation at the leaves (`Button`, `Toggle`,
`Picker` on the Digital Crown, `TextField` with the system's input sheet,
`ScrollView` for crown scrolling, `NavigationStack` bound to the router,
haptics through `WKInterfaceDevice`). The agent drives it as it drives
every host.

Its first consumer is the watch app. The watch costs more than the renderer,
and this RFC names the costs without deciding them:

- Rust's watchOS targets are Tier 3 (nightly with `-Zbuild-std`); older
  watches need `arm64_32`, whose `usize` is 32 bits;
- no Metal: no wgpu, no GPU canvas, no GPU module; canvases are CPU;
- a device restricts low-level networking (Apple's
  [TN3135](https://developer.apple.com/documentation/technotes/tn3135-low-level-networking-on-watchos)),
  which reaches app WebSockets, the agent's socket carrier and the dev
  connection; HTTP through URLSession is unrestricted;
- no JIT and no JavaScriptCore, so `app.ts` needs a Hermes built for
  watchOS or waits;
- a watch app is its own small app, not a phone layout made smaller, so its
  entry is the author's to write.

Admitting watchOS reverses a line of `rules/DEFERRED.md` and is Charlie's
call (§7, question 3). Until then the live driver is not built: the
watch's complications and Smart Stack are snapshot surfaces and arrive
with D3–D8.

### D11 — Templates get their own RFC

CarPlay apps (and so HomeView's audio and video apps), controls, standard
notifications and Top Shelf draw what the system draws from data the app
gives. They need no renderer: an entry whose tree is restricted to a
template's vocabulary (a list, a grid, now playing) maps to the template.
They share D2 (an entry of the one app), D7 (actions as intents or URLs)
and D8 (a headless session where the template is filled out of process),
and their mapping is a separate RFC.

### D12 — Hosted and island surfaces need nothing new

A rich notification, an iMessage app, a share or keyboard extension is a
`UIViewController`: the existing presenter, in a session embedded in that
extension, under that extension's limits. Each is admitted when an app
needs it. A SwiftUI-only system view inside the running app (Swift Charts,
`Gauge`, StoreKit's subscription views, Translation's modifiers) is a
native module (LLP 1024, 1067.000), as Expo Services' number transition is
today.

### D13 — Android has the same classes

Android's surfaces fall into the same five: Glance widgets over
RemoteViews and Wear OS Tiles are snapshots; a Wear OS app in Compose is
live; Android Auto's Car App Library and Android 16's Live Updates are
templates; a notification's custom view is hosted. The seams are defined
in these terms, not in WidgetKit's, so that an entry is portable and its
class per platform may differ: a Live Activity is a snapshot on iOS and a
Live Update, a template, on Android.

### D14 — Development and the agent see what the extension sees

The archive is unforgiving: an unsupported view inside a widget draws
nothing, with a console line. Three things guard it:

- **The bake checks the vocabulary.** A snapshot entry that uses an element
  the snapshot driver cannot emit within WidgetKit's list fails the bake
  with an edit, as Contract refusals do.
- **One driver in-process and in the extension.** The running app's
  development build has a gallery of its surfaces, rendered by the same
  snapshot driver at each family's size and in each rendering mode; the
  agent addresses it (`tree`, `screenshot`, `tap` as the intent, `state`)
  with a surface, a family and props. The web dev host shows the entries
  laid out at the families' sizes, an approximation labelled as one.
- **The real extension in the async lane.** A simulator test loads the
  extension through WidgetKit and compares its archive's render with the
  gallery's.

## 5. Stages

0. **Probes**, throwaway, before any design is spent: §8's Q1 to Q5,
   measured on a simulator and an iPhone.
1. **The snapshot core and iOS widgets** for one app (Caltrain's next
   departures, with a countdown element, is the proposed first consumer):
   D2–D8, D14. StandBy, CarPlay and HomeView come with it, as iPhone widgets.
2. **Live Activities and interactive widgets**: D7's intents and D9
   (Caltrain's trip in progress).
3. **The other WidgetKit platforms**: watch complications and the Smart
   Stack, Mac widgets, visionOS widgets, by the same driver.
4. **The live driver and the watch app**, if Charlie admits watchOS (D10).

Templates (D11) and Android's snapshot surfaces (D13) are their own RFCs and
can run beside stages 1 to 3.

## 6. What stays out

- UIKit or AppKit views inside a snapshot (WidgetKit refuses them).
- Custom animation on a snapshot: the platform's transitions and live text
  only.
- Network or timers during a snapshot's render (D3).
- A SwiftUI presenter for the running app on iOS or macOS (D1).
- A surface-specific renderer (D1).

## 7. Questions for Charlie

1. **The shape.** Five classes, one SwiftUI core with a snapshot and a live
   driver, templates separate. Proposed: yes.
2. **LLP 1035.006 §11.10.** Reopen it for snapshot surfaces under this
   RFC. Proposed: yes; `rules/DEFERRED.md` needs a take or a waiver.
3. **watchOS.** Admit it now as stage 4's target, or decide at stage 4 with
   stages 1 to 3 measured. Proposed: decide at stage 4.
4. **The word.** "Surface" already names a Canvas surface (`ExactSurfaces`,
   `fulfill_surface`) and a platform (`rules/DEFERRED.md` §Surfaces), and
   LLP 1124 uses "extension" for something else. A name for these entries
   is wanted before the manifest has a key.
5. **The working set.** If this enters `llp/current/`, which document
   leaves.
6. **The first consumer.** Caltrain is proposed; an app that ships to users
   (Expo Services, Interview) would test more.

## 8. Open technical questions (stage 0)

- **Q1. Placement inside an archive.** Does a custom `Layout` survive
  WidgetKit's archive? If not, absolute placement by fixed frames and
  offsets, and how content sizes itself in a Dynamic Island region the
  system sizes.
- **Q2. Text.** Do Core Text's measurement (the kernel's) and SwiftUI
  `Text`'s drawing agree on line breaks for the same font and width, and
  how does live text (D6) reserve width?
- **Q3. A session in an extension.** Memory, cold time per timeline
  request, and binary size of a headless session with a pruned plan, against
  the widget extension's limits.
- **Q4. One framework.** A dynamic framework shared by the app and its
  extensions against today's static archive: launch cost for the app, size,
  and link-by-use.
- **Q5. Detecting an unsupported view.** Whether the archive's refusal can
  be observed by a test, or only the bake's vocabulary check can catch it.
