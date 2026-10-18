# Review: LLP 1126 r1 (system surfaces)

**Family:** Grok 4.7 (xAI) · **Provider/runtime:** grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents (session 01a14ae7-c73c-7fb0-ad09-ed4672e00a51) · **Date:** 2026-10-17 · **Redacted:** no · **Method:** blind (no reviewer saw another's review), read-only checkout of origin/main 8bf68cf88 plus the r1 draft; the author's brief asked for high-level, directional and foundational findings only (prompt: the same text to each reviewer)
**Verdict:** READY WITH CHANGES

---

## Verdict

**READY WITH CHANGES.** Classifying system surfaces by execution model, and serving every archive surface from one SwiftUI core whose snapshot driver places kernel frames, is the right foundation. It matches LLP 1115 (layout stays CSS; unsaid presentation is the platform's), it matches the Apple host's existing split (the kernel places, the host draws), and a pure render plus a separate refresh step is what WidgetKit and ActivityKit actually do. The document should change before implementation where it treats "same driver" as "same process," where it leaves the archive's text and placement model unspecified, and where a shared container and a server push are written as packaging rather than as decisions the binding rules already constrain.

## Strengths

The five classes are the right cut. Home Screen, Lock Screen, StandBy, complications, and Live Activities really are one WidgetKit archive. CarPlay templates, controls, and ordinary notifications really are data the system draws. Rich notifications really are a view controller. Islands really are a native module inside the running app. Building one renderer per place would be the wrong program.

Several load-bearing decisions are right and should stay:

- **D2.** A surface is a component of the one app, baked with it. A second app per surface would let the shared shapes drift. This is also the consumer LLP 1031 D1 deferred the multi-entry shape for: a widget cannot be a route of the phone window.
- **D3.** The render is a pure function of props, a store snapshot, and the system environment. Fetching, timers, and input stay in the timeline refresh, the intent, or the activity update. That is the WidgetKit budget, and it is what makes a snapshot testable.
- **D4's rejections.** Translating CSS into SwiftUI stacks would make the widget a different layout engine from the window. Rasterizing by default would fail the LLP 1115 test: vibrant and accented mode, VoiceOver, dynamic type, and live text.
- **D5.** Container background, margins, rendering mode, and dynamic colours belong to WidgetKit where the author is silent.
- **D6.** A snapshot cannot tick. `Text` timers and progress over an interval are the platform feature that makes a departure widget worth having, and naming them as elements on every host is what keeps one component.
- **D7.** A tap opens a URL or runs an intent. A single generic intent for in-widget handlers, with named Swift intents generated later for Shortcuts, matches how App Intents are discovered.
- **D10's deferral of the live driver.** The watch app's costs (Tier 3 Rust, `arm64_32`, no Metal, TN3135, no JavaScriptCore) are real, and complications must not be used to sneak that port in. Asking Charlie, rather than editing `rules/DEFERRED.md`, is the correct status for this document.
- **D14.** A gallery driven by the same snapshot driver, plus a simulator archive comparison, is the right proof. The bake refusing a view WidgetKit cannot archive is what keeps the failure out of a blank widget.

Stage 0 before any of this is built is the right order. Q1, Q2, and Q3 are the probes that can actually retire the direction.

## Foundational concerns

### 1. Snapshot surfaces that "come along" do not all run in the iPhone extension

**Severity: HIGH**

**Consequence.** Stage 3, and the claim in D10 that watch complications and the Smart Stack arrive with D3–D8, treat the snapshot driver as portable across devices. It is portable across placements of one process. An iPhone WidgetKit extension runs on the iPhone. I am confident of the split below, and less sure of the exact OS version in the CarPlay row:

| Placement | Where the extension runs |
|---|---|
| Home Screen, Lock Screen, StandBy, and iPhone widgets shown in CarPlay | iPhone extension. Stage 1 can include these if the iOS 26 claim holds. |
| HomeView, as the RFC describes it | The paired iPhone's widgets and its CarPlay audio and video apps. I cannot verify HomeView. If that description is right, it adds no process. |
| iPhone widgets on the Mac, and a Live Activity mirrored to the watch Smart Stack or the Mac menu bar | Still the iPhone's archive, displayed elsewhere. Stage 1 or 2. |
| Watch-face complications and watch Smart Stack widgets | A watchOS widget extension, on the watch. It does not exist until watchOS is a target. |
| A Mac app's own Notification Center widgets | A macOS extension. Plausible once the iOS driver exists: `Package.swift` already builds for macOS, and the Rust host already does. |
| visionOS widgets | A visionOS extension. `host/apple/Package.swift` declares macOS 14, iOS 17, and tvOS 17 only. |

`rules/DEFERRED.md` §Surfaces admits the Apple platforms UIKit covers and keeps watchOS out because it has no UIKit. LLP 1035.006 item 11.10 says these surfaces do not create a new target platform by implication. A watch complication is that new target: the snapshot driver links the kernel, so the watch extension needs the Rust watchOS targets D10 lists as a stage-4 cost. Deciding complications in stage 3 decides watchOS while question 3 still says to wait.

**What would resolve it.** Split every row in §3 into "drawn by the iPhone extension" and "a binary for that OS." Stages 1 and 2 ship only the first. Mac widgets can be stage 3. Watch complications, watch widgets, and visionOS widgets wait for an explicit platform admission, the same gate as the watch app.

### 2. Kernel-placed SwiftUI is the right render, and the current text path cannot be that render

**Severity: HIGH**

**Consequence.** The UIKit presenter already places kernel frames and draws each line with `CTLineDraw` (`host/apple/Sources/ExactKit/Text.swift`). Those lines carry resolved colours (`RegionController.swift`: "Source CTLines contain resolved colors"). Copying that path into a widget fails the reasons D4 gives for rejecting a picture. The archive is drawn in a system process that does not load the app's code, so a custom `Layout` is a poor bet; allowlisted modifiers with fixed frames are the bet that still works when only values survive. A multiline SwiftUI `Text` will rebreak and drift from the kernel. A Core Text replay into `Canvas` will not take accented or vibrant mode, will not tick, and gives VoiceOver nothing.

D6 makes this the first consumer's problem, not a later polish. Caltrain's widget is a countdown. The kernel must reserve the widest formatted value, and the archived view must be a platform timer `Text` in that frame. I am confident the timer updates after archival only when the archived view is the system's `Text`; I would want Q2 to show the width reservation rather than assert a measurement API.

The same probe is the memory gate. `ExactKit` is one target, statically linking the app archive (`Package.swift`). The files that import no UI framework are not a target a widget can link. Widget extensions are widely reported to be jetsam'd near 30 MB; I would not treat that number as a documented limit. Q3 is the right measurement and it is not yet a decision. If a headless session with the surface's real executor does not fit, the foundation changes.

The kernel can already measure a height at a width (`fit_content_height` in `kernel/src/kernel/geometry.rs`, offering `AxisOffer::MaxContent`). Fixed-family widgets do not need it. Dynamic Island and Lock Screen regions do. Q1 should ask for that height, not only whether a custom `Layout` archives.

**What would resolve it.** State the proposal the probes confirm: the provider runs the kernel once per entry; the archived value is fixed frames; each line is a SwiftUI `Text` whose string is the kernel's break and whose colour is a role, not a resolved `CGColor`; live text is one of those views with a reserved width. Name the kill line for Q3 (memory, cold time, and a tap that boots a session). If fixed frames or per-line text fail, stop and reconsider. The fallback is platform text for the strings that must tick or tint, and a picture only for leaves that are already pictures.

### 3. One container is right; a second writer, and a server push, are not yet decisions this repo allows

**Severity: MEDIUM**

**Consequence.** D8 is correct that an extension cannot see today's state. The secret store is the data-protection keychain in the app's own access group (LLP 1018 D6, D7). The plain tier is unbuilt (LLP 1018 D2). The update store opens under Application Support in the app container (`host/apple/Sources/ExactUpdates/Updates.swift`). LLP 1018's model is snapshot-in, writes-out, on one thread. An interactive intent in the extension and a commit in the app are two writers. Last write wins, and a Lock Screen refresh can run before first unlock, when `AfterFirstUnlock` items are unavailable. A render that reads the keychain goes blank there.

D9's server push collides with a refusal that is still in force. `rules/DEFERRED.md` still refuses push delivery from a server. LLP 1030.003 D4 and LLP 1114.001 D7 say exact2 sends no pushes and relays none. A Live Activity does not need that path to be worth building: D6 already ticks a date inside the archived view, and the running app can push a new `ContentState` through ActivityKit while it is alive. The 4 KB bound is real for an APNs activity update; I would not assert it for a local `Activity.update`.

D2's prune also needs one sentence tied to LLP 1047. "What the surface reaches" has to be a baked artifact in the same update the app already selected, sharing the app's compatibility id (LLP 1030 D3a). A load-time filter of the full plan decodes the full plan in the extension, which is the memory outcome Q3 must not measure by accident. It should not reopen LLP 1031's deferred `plans` map.

**What would resolve it.** The host supplies one container URL; `build.mjs` supplies an app group when the app has a surface. The app owns the updater. The extension reads a committed generation and never checks for one. Render takes timeline props and a store snapshot that was captured while the keychain was available; it does not touch the keychain itself. Two writers need a rule in this RFC, even if the mechanism is left to the spec. D9 keeps the Contract shape, the digest, and the local update. Server push stays refused until Charlie waives that line.

## Missing surfaces or misclassifications

These are the ones that would send a later RFC to the wrong class. The rest of §3 is classified correctly.

- **Watch complications** are snapshot, and the process is on the watch. Covered above. The Smart Stack row in the motivation mixes two products: a mirrored iPhone Live Activity, and a watch widget.
- **CarPlay navigation** is not a template. The table already notes the map's `CPWindow`. That window is a UIKit scene with its own lifecycle, the existing presenter in a car scene. The template RFC should own lists, grids, and now-playing, and should leave the map window alone. Android Auto has the same split: Car App Library templates, and a navigation `Surface` the app draws.
- **visionOS immersive spaces** are not the live driver in D10. Volumes and ornaments can be a retained SwiftUI tree. An immersive space is RealityKit. The table should not promise them as the watch driver's next consumer.
- **Interactive snippets** should stay "to verify." If `SnippetIntent` archives like WidgetKit, they are snapshot. If the intent's view stays live in the app process while the snippet is visible, they are a short-lived in-process SwiftUI host and forcing them through the snapshot driver is the wrong packaging. I am not sure which iOS 26 does.
- **Android, in D13, over-decides two cells.** A custom notification on Android is RemoteViews: the system process inflates a parcel, which is snapshot, not a hosted view. iOS rich notifications are hosted. I am not sure of the Android 16 Live Update API surface; the safe claim is that an ongoing notification can carry a custom RemoteViews layout and a system-drawn status promotion. Locking "a Live Activity is a template on Android" would throw away the rendered component D2 just made portable. Wear OS tiles are snapshot, as the RFC says. Wear watch-face complications are a different API and are templates (`ComplicationData`: short text, image, ranged value, drawn by the face). Quick Settings tiles are templates, the same class as Control Center controls. None of these change the SwiftUI core. They should be marked as the Android RFC's hypotheses. Absolute frames, if Q1 succeeds, are also the placement model that maps onto RemoteViews; stack translation would not.

App Clips are mislabeled as hosted extensions. An App Clip is a size-capped app with the existing presenter. Menu-bar items and Dock tiles, which item 11.10 names, are in-process AppKit. Neither changes this renderer. D12's "when an app needs it" is the right deferral for keyboards and share extensions; a keyboard's memory limit means "the existing presenter" is not free, and that is a later admission.

## Alternatives considered

**Strongest alternative: keep Rust out of the widget process.** The app, or a small refresh helper, writes a display list. A Swift-only inflater turns that list into the archived SwiftUI view.

It wins if Q3 fails. A widget that cannot boot a session cannot use this RFC, and no amount of renderer work fixes that. Precomputing every family and content-size category avoids running the kernel at archive time.

It loses if Q3 passes. The system picks the size, the rendering mode, and the text size when it archives, often while the app is not running. A display list baked for one environment is the wrong layout for another, so the inflater grows a second layout engine or the extension keeps the kernel anyway. Timeline refresh and an interactive intent need the data crate and the store in that same process. Splitting a "tiny data runtime" from a "layout runtime" is a second Exact.

**Translating the surface into SwiftUI stacks, RemoteViews, and tile protos.** This loses on LLP 1115's own terms. Those three layout engines disagree with each other and with the window. The kernel exists so a box has one answer. It would also throw away the portability D4 gives Android: a frame list can fill a RemoteViews `FrameLayout`, while a SwiftUI stack translation cannot.

**Rasterizing with the current painter.** This is the cheapest probe and the wrong default. It fails vibrant mode, accented mode, VoiceOver, dynamic type, and D6. It stays the right leaf for a canvas the app already drew.

I would not adopt a widget-specific layout vocabulary. Authors and agents write CSS, and a surface is already a separate component under D2. The restriction belongs in the bake's vocabulary check (D14), not in a second layout language.

## Recommended changes to the RFC

Directional only:

1. Divide §3 and §5 by process. Stages 1 and 2 are the iPhone extension and the placements it is displayed in. Mac widgets are the first other binary. Watch and visionOS stay behind Charlie's platform ruling, complications included.
2. Make D4 a proposal with a kill line. Fixed frames, kernel line breaks, semantic SwiftUI text, reserved width for live text. Q1 includes content margins and a Live Activity height from `fit_content_height`. Q2 includes accented and vibrant mode and a ticking value. Q3 states the memory and cold-time numbers that end "session in the extension," and it boots the pruned artifact with the executor that surface uses, including one intent tap.
3. Keep D2 as one plan and one compatibility id, with a baked prune per surface (LLP 1047's use-set). Say explicitly that this takes LLP 1031 D1's trigger and does not add a `plans` map.
4. Decide the container rule in D8: one container the host supplies, extension reads a committed generation, render does not touch the keychain, two writers have a rule.
5. Narrow D9 to the Contract shape and updates from the running app. Server push stays behind the existing refusal. Say that D6 is what makes the first activity useful without a push.
6. Mark D13's Android class assignments as hypotheses for that RFC, with custom notifications as snapshot and Wear face complications as template.

## Open questions for the author

1. For Q3, what measured result retires a session in the extension? A number, not a hope that the boot rule will be enough.
2. Is the watch row in stage 3 intended to be only the iPhone Live Activity mirrored into the Smart Stack? If complications are meant to run on the watch in that stage, question 3 has already been answered.
3. Is D9's server payload meant to waive `rules/DEFERRED.md`'s push refusal, or is a type for an app's own backend a later admission? For Caltrain's trip, is a `ContentState` updated while the app runs, plus a platform countdown, the product?
4. Does the first widget show public timetable props only, or account state from the keychain? The second has to work on the Lock Screen before first unlock.
5. HomeView is outside what I can verify from this checkout or from stable platform knowledge. Does any HomeView behaviour require a class other than "iPhone widget" and "CarPlay template app"?