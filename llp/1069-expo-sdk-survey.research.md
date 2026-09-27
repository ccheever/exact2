# LLP 1069: The Expo SDK against exact2 — what to add, ranked

**Type:** Research
**Status:** Draft
**Systems:** All; chiefly the data seam and grants (device capabilities as requests), the reserved sources (device facts), Contract tags (form controls, pickers), host commands (share, haptics), the Apple and web hosts, delivery (store release), and `rules/DEFERRED.md` (which items each addition would need moved)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Revised:** 2026-09-27, the same day: renumbered from 1057 at a merge, then from 1068, which main's view-recycling RFC took first; brought to main after the day's landings (§0); `NOT-DOING.md` is now `DEFERRED.md`; the sub-LLPs 1069.000–.010 written (§7)
**Related:** LLP 1016.000 (answers that keep coming: SSE and WebSocket, designed); LLP 1018 (durable client state and secrets); LLP 1024 (native modules: views); LLP 1027, 1027.000 (TypeScript sources; explicit time and randomness); LLP 1030, 1030.003 (delivery; the install page and the store lanes left behind it); LLP 1031 (brownfield embedding); LLP 1033 (the Markdown reader's documents); LLP 1034 (scheme-aware colour); LLP 1039 (viewport facts, the pattern for every fact below); LLP 1042 (video); LLP 1045 D10 (the admitted image/video picker); LLP 1048 (web rendering); LLP 1049 and LLP 1054 (the Crew and Bluesky port reports: what real ports asked for)

## Summary

Charlie, 2026-09-27: *"Can we do a survey of all the functionality in the
Expo SDK and compare it to what is offered in exact2? What are the things we
should look at adding to exact2 now? Is there anything worth adding to
exact2 that isn't offered in Expo?"*

The survey covers the Expo SDK 57 reference (stable, 2026-06-30) and the
SDK 58 beta (2026-09-15): about 75 first-party packages, Expo Router, the
Expo Modules API and the EAS services. It compares them against what an
exact2 app can reach today on each host (§2).

The finding in one line: **exact2 is at or past Expo on everything drawn on
the screen, and has almost nothing that touches the device.** Layout, text,
SVG, motion, lists, routing, SQLite, files, secrets, fetch, signed
over-the-air delivery and server rendering are all in place. Beyond those,
the only device reach is four host commands (`copyText`, `openURL`,
`setScheme`, focus). Camera, location, notifications, sharing, haptics,
authentication sessions, sockets, form controls such as switches and
sliders, and system chrome are absent. Some are refused on purpose.

The day this was written, main closed several of the gaps it found (§0).
Reduced motion, locale, time zone, string tables, CSS gradients and native
modules in Swift all landed. The ranking below is revised to match.

**The ranked list (§5).** Seven items are recommended now. Each has a web
name, a consumer that already exists, and a cost measured in days per host:

1. **The remaining device facts:** contrast, font scale, visibility and
   online status, beside the reduced motion, locale and time zone that
   landed (§0).
2. **Form controls by their HTML names:** `input type=checkbox|range|date|time`
   and `select`.
3. **The admitted image/video picker (LLP 1045 D10):** finish it.
4. **`share`,** after the Web Share API.
5. **SSE and WebSocket answers:** build LLP 1016.000.
6. **Ids and hashing in sources:** `crypto.subtle.digest` and ids made from
   the launch seed that landed on `exactTime`, under LLP 1027.000.
7. **A system-browser auth session** for OAuth/OIDC with PKCE.

Nine items follow as "next": each needs a DEFERRED.md line or a design of
its own. Local and push notifications lead them. The capability-module seam
this list first ranked at #8 largely landed as LLP 1024 and LLP 1067. That
already puts the long tail (contacts, calendar, BLE, StoreKit) in app space.

**Where exact2 can go past Expo (§4).** Expo has no counterpart for these,
and exact2's shape makes them cheap:

- Every device capability arrives with an agent substitute, so an agent can
  drive a permission prompt, a location or a notification through the agent
  API's operations and the clock.
- Permissions are derived from grants, so the Info.plist strings and a
  readable "what this app can do" page come from one declaration.
- The Mac is a first-class document app.

**Outside the ranking:** Android is Expo's largest advantage, and DEFERRED.md
defers it until the loop is proven. §6 names the trigger rather than ranking
it.

## 0. What landed on main the same day

Between the first draft and this revision, 147 commits landed. These moved
rows of §2 and items of §5:

| landed | LLP | what it closes |
|---|---|---|
| reduced motion and reduced transparency as `exactViewport` fields; `press-scale`; the agent's `prefer` (a ninth operation, pending a ruling) | 1061 | the reduced-motion half of #1, and the DEFERRED.md sentence no host carried out |
| locale and time zone on `exactTime` (`Runner::set_place`) | 1060, 1027.000.000 | the locale half of #1 |
| a launch `seed` on `exactTime`, the explicit entropy for ids | 1027.000 (commit `22018204`) | half of #6 |
| string tables, `t(...)`, the locale as a slot | 1060 | #17 (localized strings) |
| CSS `linear-gradient()` / `radial-gradient()` as `background-image` | 1066 | `expo-linear-gradient`; gradients are no longer refused |
| `box-shadow`, `text-transform` | 1064 | |
| paint motion, stagger, exit animations, layout transitions | 1062, 1063 | more of Reanimated's ground |
| native view modules: hyphenated tags, one module artifact per app | 1024 §9 | the view half of the Expo Modules API |
| Swift data modules, `native.later` for long calls, and the web's page module | 1067 | most of #8: the function half of the Expo Modules API |
| `native.watch` device topics | 1016.002 | the change-notification half of #8 |
| `UITabBar` projection for a symbol-plus-label tablist | 1059 | native tabs |

## 1. How the two compare, and the four shapes a capability takes here

Expo is a React Native platform for iOS, Android and web. A capability there
is a JavaScript module with an imperative API, callable from anywhere,
including render. exact2 targets web, macOS, iOS and Linux. No JavaScript
runs above the data seam, so every capability has to take one of four shapes
that already exist:

| shape | what it is | examples today |
|---|---|---|
| **tag / prop / event** | Contract vocabulary, laid out by the kernel, presented by each host | `video`, `iframe`, `refresh`/`refreshing`, `contextmenu` |
| **command** | fire-and-forget, from an event handler (`Outcome.commands`) | `copyText`, `openURL`, `setScheme`, `focus`, `deliveryCheck` |
| **reserved source (a fact)** | read with `resource`; the host tells the runner; the agent can set it | `exactViewport`, `exactTime`, `exactDelivery`, `exactSurface` |
| **grant-gated binding** | callable by TypeScript/Rust logic below the seam, admitted by one line in the manifest | `net.fetch`, `sqlite.open`, `fs.read/write`, `secret.keep`, `storage.kv` |

A new capability should take the first shape that fits, in the order: fact,
command, tag, binding. It should use the web platform's name for it
(`CLAUDE.md`, "The web is the standard"). For almost everything below, a web
API exists and supplies both the name and the vocabulary. For example,
Geolocation, Web Share, Screen Wake Lock, Notifications, Push, WebAuthn,
`matchMedia`, `navigator.onLine`, `document.visibilityState` and
`<input type=range>` all do.

## 2. The comparison

Legend for the exact2 column (web / macOS / iOS / Linux):

- **P** — present.
- **p** — partial.
- **–** — absent.
- **✕** — deferred in `rules/DEFERRED.md` (renamed from NOT-DOING.md on 2026-09-27: "what isn't a current priority, not what will never be done").

Evidence is by grep over `host/`, `runner/`, `contract/` and `vendor/ibex2`,
2026-09-27.

### 2.1 UI and components

| Expo | what | exact2 | notes |
|---|---|---|---|
| React Native core views, `@expo/ui` Universal | view, text, button, input, scroll, list | P/P/P/P | 33 node types, 147 props, 134 style rows; CSS defaults |
| `@expo/ui` Switch, Slider, Picker, DateTimePicker, Checkbox, SegmentedControl; `expo-checkbox` | form controls | –/–/–/– (segments P) | the kernel has a `Toggle` node with no Contract tag; tabs present as segmented controls |
| `@expo/ui` BottomSheet, Menu, context menus | sheets, menus | P/P/P/p | detents (iOS), popover/`commandfor` menus, `contextmenu`; LLP 1021 |
| `@expo/ui` PagerView | pager | ✕ | DEFERRED.md: "No … `pager`" |
| FlashList (bundled) | virtualized list | P/P/P/p | LLP 1010, 1050.000 |
| `expo-image` | cached image, SVG, HDR, symbols | P/p/P/p | LLP 1011. Linux decodes PNG only; macOS remote images load without a cache; no `srcset`, `onerror` or animated images on macOS |
| `expo-symbols` | SF / Material symbols | P/P/P/– | 48 roles |
| `expo-blur`, `expo-glass-effect` | blur, Liquid Glass | p/P/P/– | `backgroundMaterial`; no glass value |
| `expo-linear-gradient`, `expo-mesh-gradient` | gradients | linear/radial P/P/P/P; mesh – | LLP 1066: CSS `background-image`, Chrome the oracle |
| `expo-gl`, react-native-skia | GL, 2D drawing | GPU module P; Canvas 2D awaiting a ruling | LLP 1009, 1056 |
| react-native-svg | SVG | P/P/P/P | LLP 1055; more complete natively than react-native-svg |
| Reanimated, gesture-handler | animation, gestures | P | CSS `transition`, `@keyframes`, springs, follow-and-release; a gesture arena is ✕ |
| `expo-maps` | maps | – | |
| `expo-widgets` (widgets, Live Activities) | home-screen surfaces | – | |
| `expo-live-photo` | Live Photos | – | |
| Lottie, Rive | vector animation | ✕ | |
| `expo-font` | runtime fonts | P (bundled, at build) | LLP 1019; runtime loading is not a goal |
| `'use dom'` DOM components, react-native-webview | web content in native | P/P/P/– (`iframe`) | LLP 1020 |

### 2.2 Media

| Expo | exact2 | notes |
|---|---|---|
| `expo-video` | P/P/P/– | LLP 1042; AVKit PiP and fullscreen; Linux has no decoder |
| `expo-audio` (playback, recording, streaming) | – | only inside the optional `game/audio` |
| `expo-camera` (capture, barcodes, document scan) | ✕ | "No camera anything." |
| `expo-image-picker` | –, admitted | LLP 1045 D10: an image/video picker, no camera; unbuilt (`upload`, PHPicker: 0 hits) |
| `expo-image-manipulator` | – | |
| `expo-media-library` | – | |
| `expo-speech` | – | |

### 2.3 Storage and data

| Expo | exact2 | notes |
|---|---|---|
| `expo-sqlite` | P/P/P/P | web runs sqlite3 wasm in a worker; native uses `ibex2-sqlite`. No SQLCipher, no vector search, no changesets |
| `expo-sqlite/kv-store`, AsyncStorage | p | `storage.kv` grant; the plain tier is "next" in LLP 1018 D2 |
| `expo-file-system` | P/P/P/P | `app:/data`, `app:/cache`, `app:/tmp`; IndexedDB on the web. No upload/download tasks with progress, no watching |
| `expo-secure-store` | P/P/P/P | `secret.keep`: Keychain on Apple, a 0600 file on Linux, `localStorage` on the web (not secure) |
| `expo-document-picker` | –/p/–/– | macOS `NSOpenPanel` and file associations; no save panel; iOS and web adapters owed (LLP 1033) |
| `expo-blob`, `expo-asset` | p | bundled assets; no `Blob` |

### 2.4 Networking

| Expo | exact2 | notes |
|---|---|---|
| `expo/fetch` (streaming) | P/P/P/P, streaming p | origin-scoped `net.fetch` grants, redirect credential stripping |
| WebSocket, SSE | – | the `net.websocket` grant parses but has no binding; designed in LLP 1016.000 (Draft) |
| `expo-network`, NetInfo | – | no reachability or `onLine` |

### 2.5 Device hardware and sensors

| Expo | exact2 |
|---|---|
| `expo-location` | – |
| `expo-sensors` (accelerometer, gyroscope, motion, barometer, magnetometer, pedometer, light) | – |
| `expo-haptics` | – for apps; iOS uses UIKit feedback internally in segments and swipes |
| `expo-battery`, `expo-brightness`, `expo-cellular`, `expo-device` | – |
| `expo-keep-awake` | – |
| `expo-screen-orientation` | – (orientation is `width > height` from `exactViewport`; no lock) |
| `expo-screen-capture` | – |

### 2.6 Identity and security

| Expo | exact2 |
|---|---|
| `expo-auth-session`, `expo-web-browser` auth | – (the pattern is a JWT in `secret.keep`, as in RealWorld) |
| `expo-apple-authentication` | – |
| `expo-local-authentication` (biometrics) | – |
| `expo-crypto` | p: a launch `seed` on `exactTime` supplies entropy for ids; ibex2 vendors `getRandomValues` and `randomUUID`, but the prelude refuses ambient randomness (LLP 1027.000) |
| `@expo/app-integrity`, `expo-age-range`, `expo-tracking-transparency` | – |

### 2.7 System integration and OS UI

| Expo | exact2 | notes |
|---|---|---|
| `expo-clipboard` | write P/P/P/–, read – | `copyText` |
| `expo-linking` | P | custom schemes, universal links, a `navigate` event (LLP 1038 D8) |
| `expo-web-browser` (in-app browser) | – (`openURL` hands off to the system) | |
| `expo-sharing` (share sheet, receiving shares) | – | |
| `expo-print`, `expo-mail-composer`, `expo-sms` | – (`openURL` takes `mailto:` on Apple and `tel:` on iOS; `sms:` is refused by the allowlist) | |
| `expo-contacts`, `expo-calendar` | – | |
| `expo-localization` | P | locale and time zone on `exactTime`; string tables and `t(...)` (LLP 1060); `lang`, `direction`, `Intl` in TypeScript |
| `expo-application`, `expo-constants` | p | the manifest is known at build; not readable as a fact |
| `expo-status-bar`, `expo-system-ui`, `expo-navigation-bar` | – / p | scheme P (LLP 1034); no status-bar style |
| `expo-splash-screen` | p | iOS `UILaunchScreen: {}` only |
| `expo-app-intents`, `expo-ai` (alpha) | – | |
| `expo-store-review`, IAP (third-party) | – | |

### 2.8 Background work and notifications

| Expo | exact2 | notes |
|---|---|---|
| `expo-notifications` (local, scheduled, push) | – | DEFERRED.md's "push delivery" refusal is about *updates*, not app notifications (LLP 1016.000 says the same) |
| `expo-task-manager`, `expo-background-task` | – | `host.ios.backgroundModes` passes into Info.plist with nothing behind it |
| App lifecycle (AppState) | – for apps | hosts observe it internally |

### 2.9 Routing, web and server

| Expo Router / server | exact2 | notes |
|---|---|---|
| File routes, typed routes, stacks, tabs, modals, deep links | P | LLP 1038: declared routes, a stack per tab, six verbs; web history |
| Native tabs, formSheet, zoom transition, link previews | p | segments, detents; view transitions are LLP 1013 (deferred) |
| Static rendering, SSR, streaming, loaders, API routes, middleware | P / p | LLP 1048: build-time and per-request pages from a native Rust renderer, adoption in place of hydration; streaming later; API routes are not a goal (no Node on the render path) |
| React Server Components | n/a | server-driven UI is ✕ |

### 2.10 Tooling, build and services

| Expo / EAS | exact2 | notes |
|---|---|---|
| Expo Go | ✕ for now | "the generic Go launcher … remain[s] behind proving this consumer" (LLP 1030.003) |
| Dev client, dev menu | P | `dev.mjs` restarts in about 20 ms and carries state; Apple dev menus |
| Expo Modules API (Swift/Kotlin, views and functions) | P (Apple, web) | Rust logic modules, native or Wasm, hot-updatable (LLP 1029.000); native view modules (LLP 1024 §9); Swift data modules with `native.call`/`native.later` and the web page module (LLP 1067); `native.watch` topics (LLP 1016.002) |
| Config plugins, prebuild/CNG | P, differently | `apps/<name>/app.json` generates the Apple project; no plugin system |
| `expo-brownfield` | P (Apple) | LLP 1031, `ExactSession` / `ExactView` |
| EAS Update | P | signed static bundles, anti-rollback, next-launch or app-activated. Cohorts, targeting and a console are ✕ |
| EAS Build / Submit / Launch | p | `exact release` signs, notarises and staples on macOS; `build.mjs --archive` signs an iOS IPA; no upload to TestFlight or the App Store |
| EAS Hosting | p | the render server; `serve.mjs`; the static origin |
| EAS Workflows, Maestro | P, differently | the agent API's operations, `smoke.mjs`, `.test.contract` |
| EAS Observe, Insights | – | nothing reports errors or metrics from an installed app |
| Expo Atlas | p | `boot.mjs` counts; LLP 1047's byte budget |
| MCP server, skills, agent guides | P | the agent API was designed for agents |
| TV (tvOS, Android TV) | – | |

## 3. What the gap looks like from the apps that exist

Each capability is read against the consumers already in or next to the
repo. A consumer is the bar in `rules/DEFERRED.md`.

| consumer | what it would have used |
|---|---|
| Bluesky port (LLP 1054) | form controls (settings), share (a `share` symbol role exists with nothing to press), OAuth (AT Protocol's real login; app passwords were the workaround), locale and time zone (relative dates; X1 added only the epoch), notifications (a tab of them, polled), image picker (compose) |
| Crew / Exact Live (LLP 1049, 1041) | SSE or WebSocket (crew chat, job progress; LLP 1016.000 was written for it), haptics |
| Messages | WebSocket, notifications, share, image picker |
| Interview (LLP 1045, 1048) | the picker (captioned figures, admitted), share, OAuth |
| Fieldnotes, the Markdown reader (LLP 1033) | documents on iOS and web (open/save), the save panel, multiple windows |
| Weatherlight | geolocation (it asks for a city by name today) |
| every app | reduced motion and locale (landed, §0), contrast and font scale, keep-awake for video |

## 4. What exact2 could offer that Expo doesn't

Some of these exist already, and the list below shouldn't be read as a
backlog. It's the reason the ranked items in §5 are designed the way they
are.

**Already ahead.** Expo has no counterpart for any of these:

- macOS and Linux as first-class hosts from one source, including a
  headless Linux host that runs the same smoke.
- No JavaScript before the first pixel.
- A seekable clock and the agent API on every host.
- Rust logic that is hot-updatable, native or Wasm.
- Server rendering from a native renderer, with no Node, and adoption
  instead of hydration.
- Complete SVG rendered natively.
- Text around shapes.
- One Markdown editor crate for every host.
- A GPU module loaded after first paint.
- Pay-for-what-you-use linking (LLP 1047).

**Worth adding, and designed into §5.** None of these exists in Expo:

- **B1. A substitute for every device capability, driven by the agent.** Each
  fact (§5 #1), picker result, permission prompt, notification and location
  is a host input that `state` shows and the existing operations can answer
  or set. It is the same mechanism `exactTime` uses (LLP 1069.007).
  Testing permission flows, notifications and location is hard in Expo, and
  Maestro can't answer a system dialog on every platform. Here it comes free
  if each capability is built as a fact or a request from the start. This
  should be a rule for every item in §5, not an item of its own.
- **B2. Permissions derived from grants.** A capability is admitted by a
  grant line in the manifest. The bake can therefore derive the
  Info.plist usage strings, entitlements and web Permissions-Policy from the
  same lines. It can also print one "what this app can reach" table, beside
  the delivery classifier's. Expo spreads this across config plugins and
  hand-written `infoPlist` entries. This doesn't need a new file:
  `host.ios.permissions` already exists and would become derived.
- **B3. Device inputs that are replayable.** LLP 1027.000 makes time and
  randomness explicit inputs. Location, sensors and network status can
  follow the same rule, so a recorded session reproduces exactly. (Session
  record/replay as an agent operation stays ✕; this only keeps it possible.)
- **B4. The Mac as a document platform.** This means file associations
  (partly present), open and save panels, multiple windows, Quick Look and
  Spotlight through App Intents. Expo doesn't ship macOS.
  Fieldnotes and the Markdown reader already need the first three
  (§5 #13).

## 5. The ranked list

The ranking weighs four things, in order:

1. A consumer in §3 that needs it now.
2. How common it is in shipped apps.
3. Whether it has a web name and a shape from §1, so four hosts share one
   meaning.
4. What it costs against DEFERRED.md. An item marked **trade** needs a line
   moved off that list, with something taken off the doing-list.

### Now — cheap, named by the web, consumer exists

1. **The remaining device facts (LLP 1069.000).** Reduced motion and
   transparency (LLP 1061), and locale and time zone (LLP 1060), landed the
   same day. What is left:
   - `prefers-contrast`.
   - Font scale: Dynamic Type, or the browser's root size.
   - Visibility: `document.visibilityState`, or the app being active.
   - `navigator.onLine`.
   - The resolved colour scheme. LLP 1034 D3 rules the opposite ("never a
     fact in the app"), so this one needs Charlie's ruling.

   Each is one host observation, a field on a reserved source and an
   agent-settable value. It's the cheapest item with the widest reach.
   No trade.
2. **Form controls by their HTML names.** These are `input type=checkbox`
   (rendered as a switch on Apple under `role=switch`, the ARIA name),
   `type=range`, `type=date|time`, and `select`/`option`. They map to
   `UISwitch`/`NSSwitch`, `UISlider`, `UIDatePicker`, `UIMenu`/`NSPopUpButton`
   and the DOM's own elements. The kernel's `Toggle` node is the start.
   Every settings screen needs them, and ports build them by hand today.
   No trade: these are HTML's tags, not new components.
3. **Finish the admitted picker (LLP 1045 D10).** Name it
   `<input type=file accept="image/*,video/*">`, the web's spelling. It is
   PHPicker on iOS, `NSOpenPanel` filtered on macOS and the input itself on
   the web. It is already admitted. Any `accept` wider than media stays ✕
   ("no generic file input").
4. **`share` command (Web Share API).** It takes `{title, text, url}` and
   opens `UIActivityViewController` on iOS, `NSSharingServicePicker` on
   macOS and `navigator.share` on the web. On Linux it is unavailable and
   reported as a failed command. The Bluesky and Interview ports want it.
   No trade.
5. **SSE, then WebSocket (build LLP 1016.000).** The design is written and
   the grant parses. Crew, Messages and Bluesky want it. SSE first; that is
   1016.000's open question 2, and Crew needs only SSE. No trade: 1016.000 and LLP 1027 name it as the waiting
   trigger.
6. **Ids and hashing in sources (LLP 1069.005).** The launch seed on
   `exactTime` landed. What is left is a way to turn it into ids, and
   `crypto.subtle.digest`, which is pure. Ambient `getRandomValues` and
   `randomUUID` conflict with LLP 1027.000, so 1069.005 decides how they
   arrive. Optimistic ids and PKCE (#7) need them. No trade.
7. **Auth session for OAuth/OIDC with PKCE.** One request opens the system
   browser session: `ASWebAuthenticationSession` on Apple, a redirect or
   popup on the web. It answers with the callback URL, and the token goes
   into `secret.keep`. Bluesky's real login needs it, and so does any
   third-party sign-in. Sign in with Apple and passkeys (WebAuthn) come
   later in the same shape. No trade (a request, not a service).

### Next — each needs a design or a trade

8. **Capability modules: largely landed.** LLP 1067's Swift data modules,
   `native.later` and the web page module, with LLP 1016.002's
   `native.watch`, are the Expo Modules API for functions. LLP 1024 §9 is
   the same for views. What is left from this item:
   - A module declares the grants it uses (LLP 1069.008).
   - A module supplies an agent substitute (LLP 1069.007).
   - Linux has no module seam.
   - Whether exact2 ships first-party modules (maps, contacts, StoreKit) or
     leaves them to apps.
9. **Notifications: local first, then push.** Use the Notifications API
   names for local and scheduled notifications (`UNUserNotificationCenter`),
   a permission request as a request, and a tap arriving as a `navigate`
   event. Push (APNs, Web Push) needs a sender. The refusal of "push
   delivery" is about updates, but a line should still say so.
   **Trade.**
10. **System chrome from the manifest and a few commands.**
    - Status bar style and `theme-color`.
    - Orientation lock (`screen.orientation.lock`).
    - Keep awake (Screen Wake Lock, also automatic while a `video` plays).
    - A launch screen from the manifest's `background_color` and icon.

    Cheap, but nothing asks yet. No trade.
11. **iOS store release: TestFlight and the App Store.** `host/apple/build.mjs
    --device --archive <out.ipa>` now signs an IPA with a distribution
    profile that a build service such as EAS supplies. Nothing uploads it to
    App Store Connect, and `exact release` is still macOS only. What is left
    is the upload and the metadata. **Trade:** the "EAS/AppDrop adapter"
    line in DEFERRED.md's install-page entry.
12. **Haptics.** A `haptic` command with iOS's vocabulary: selection,
    impact light/medium/heavy, success/warning/error. It maps to
    `navigator.vibrate` where the web has it, and does nothing on macOS and
    Linux. This is a **declared deviation**: the web's vibrate is too coarse
    to be the name. No trade.
13. **Documents everywhere.**
    - Open and save on iOS (`UIDocumentPickerViewController`) and the web
      (`showOpenFilePicker`, or a download).
    - `NSSavePanel` on macOS.
    - Multiple windows on macOS.

    Fieldnotes and the Markdown reader want it (LLP 1033's owed adapters).
    It widens "no generic file input", so **trade**.
14. **Geolocation (`navigator.geolocation`).** A grant, a permission
    request, and an agent-set position. Weatherlight would use it. No trade.
15. **Audio (`<audio>`).** It is the video arm without a picture: AVPlayer,
    background playback mode and Now Playing. Recording stays out. Nothing
    asks yet. **Trade** (a new tag).
16. **Errors from installed apps.** Hosts keep crash and error reports on
    disk and send them to an endpoint the app names under its `net.fetch`
    grant. This is Observe without a service. **Trade:** it borders on
    "server-side anything".
17. ~~**Localized strings.**~~ Landed as LLP 1060.

### Not now

Each of these has no consumer, is refused, or is already covered:

- **Camera and barcode/document scanning (✕).** Keep refused. The picker
  covers the media case.
- **Maps.** An `iframe` covers the web; native maps are a view module (LLP 1024).
- **Sensors:** accelerometer, gyroscope, pedometer, barometer, light.
- **Contacts and calendar.** They belong to #8.
- **Background tasks and background location.**
- **Widgets, Live Activities and App Intents.** They are interesting for B4,
  but Apple-only and heavy.
- **IAP and store review.** They belong to #8.
- **Print, mail and SMS composers.** `openURL` with `mailto:` is enough, and
  `sms:` is one allowlist entry.
- **Screen capture blocking, app integrity, age range, tracking
  transparency, brightness, battery, cellular, speech, Live Photo.**
- **Lottie and Rive (✕).**
- **Pager (✕).**
- **Expo Go-style launcher (✕, LLP 1030.003).**
- **TV.**
- **An in-app browser.** `openURL` plus #7 covers the cases that matter.

## 6. The platform question: Android

Android is not ranked, because it is not an API. Still, it is the largest
single difference from Expo. Most Expo apps ship to Android, and every
package in §2 has an Android half. DEFERRED.md defers Android "after the loop
is proven", with Windows beside it.

The shapes in §1 keep the cost of a later Android host down. A capability
built as a fact, a command or a grant-gated request adds one arm per host.
One built as a host-specific API would multiply. That is one more reason to
build #1–#7 in those shapes and not as platform escape hatches.

## 7. The sub-LLPs

Charlie asked on 2026-09-27 for an LLP for each item recommended now and for
each item in §4:

| LLP | from | subject |
|---|---|---|
| 1069.000 | §5 #1 | the remaining device facts |
| 1069.001 | §5 #2 | form controls by their HTML names |
| 1069.002 | §5 #3 | the image/video picker, `input type=file` |
| 1069.003 | §5 #4 | `share`, after the Web Share API |
| 1069.004 | §5 #5 | building LLP 1016.000: SSE, then WebSocket (a plan) |
| 1069.005 | §5 #6 | Web Crypto in sources: ids and hashing |
| 1069.006 | §5 #7 | the OAuth/OIDC sign-in session |
| 1069.007 | §4 B1 | device capabilities in the agent's hands |
| 1069.008 | §4 B2 | permissions derived from grants |
| 1069.009 | §4 B3 | replayable device inputs |
| 1069.010 | §4 B4 | the Mac as a document platform |

They are RFCs (1069.004 is a plan), not specs. None has an implementer or a
date yet.

Charlie ruled on every open question the same day. Each sub-LLP records its
own rulings in a §Rulings section. Three changed rules and specs:

- `prefer` is the ninth agent operation and sets every device fact.
  `rules/DEFERRED.md` §Agent API and LLP 1012 are amended.
- The tag count guards against invented components, not HTML's elements, so
  `select` and `option` are admitted.
- The file picker is widened to an app's `file_handlers` types, and
  Bluesky's OAuth sign-in is named as one of its asks.

Native hosts don't enforce the web's user-activation rule for pickers and
share; developers are trusted. No `DEFERRED.md` trade was taken.

## Confidence

**The Expo inventory is high confidence.** It comes from the
`docs/pages/versions/v57.0.0/sdk/*.mdx` headers, the expo/expo `packages/`
directory, and the SDK 54–58 changelog posts, all read 2026-09-27. Two items
are named in the SDK 58 beta post but absent from `packages/`: the device
hub and `@expo/agent-cli`.

**The exact2 absences are by grep, and grep can miss things.** The patterns
searched include `UNUserNotificationCenter`, `CLLocation`, `LAContext`,
`ASWebAuthenticationSession`, `UIActivityViewController`, `navigator.share`,
`NWPathMonitor`, `prefersStatusBarHidden`, `reduced-motion` and
`ReduceMotion`. The capability could exist under a name those patterns
don't match, but each of the §5 "now" items was checked again by hand.

**The ranking is judgement.** The consumer column (§3) is the evidence. The
commonness and cost estimates are not measured. No item here is specified.
Per `rules/RULES.md`, each gets a spec only when it has an implementer and a
date.
