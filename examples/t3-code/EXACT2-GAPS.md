# exact2 support needed for T3 Code parity

Date: 2026-10-05; re-checked 2026-10-06 on the pin; X7, X24, X27 and X44 updated 2026-10-07 after main #173, #174, #164 and #177 (adopt-main-fixes-shell); X8, X14 and #135 updated 2026-10-07 after main #186, #183 and #184 (adopt-main-fixes-r3, main `cff90b364`). The clone now builds on exact2 main `c12832e82` (the pin); the items were first checked against main `d2cb661eb`. The reference app is T3 Code `f870c419fc` (HEAD `1e2ecbd975`).
The current state of every item on the pin is the table "Current state on the pin" below; what main fixed and which workaround was removed is "Already fixed on exact2 main".
Each item lists the T3 feature it blocks, what we reviewed, the current state, why it does not work, and the exact2 support it needs.
REF = `~/Documents/work/3.open-source/t3code`. X2 = exact2 main.

## Summary

| ID | Missing in exact2 | T3 feature blocked | Kind | Workaround in the clone |
|---|---|---|---|---|
| X1 | Embedded Chromium + CDP | Browser surface (preview browser, agent browser automation) | policy + build | none (not built) |
| X2 | Developer Tools for the app UI | View › Toggle Developer Tools | policy (DEFERRED) | none |
| X3 | App-settable root font size (`rem` base) (fixed on main #185, adopted) | Interface font size (12–20 px) | framework feature | none: `setRootFontSize` from app.contract `rootFont`; Contract lengths in `rem` (`font-size-map.json`) |
| X4 | Helper executables and large resource trees in the bundle | Embedded local T3 server | build | archive in `assets/`, unpack at launch (planned) |
| X5 | Custom URL scheme delivered to the app | `t3code://` deep links, provider sign-in return | host | none |
| X6 | Module shutdown time at quit | Stop the embedded server cleanly | host | none |
| X7 | ATS keys from `app.json` (fixed on main #173, adopted) | Rendered HTML / web views that load `http://` from host names | build | none: `app.json` `host.macos.appTransportSecurity` |
| X8 | Pointer input for native views in the agent | Agent tests of terminal, browser, device views | agent API | none: main #186, adopted (every pointer row driven by the agent or with real input) |
| X9 | Root component across files; resources in child components | Large apps (`app.contract` near 1,500 lines) | contract | split views, keep state in root |
| X10 | Text rendering parity | Ellipsis, wrap points, `text-wrap: balance`, placeholder colour, weight | kernel/host | none (visible difference) |
| X11 | Shadow and blur parity | Dialog/popover shadows, glass composer | host | opaque composer, faint shadows |
| X12 | Textarea field sizing | Composer height with long chips | host | measured height |
| X13 | Hover and key events during a pan | Sidebar row-action sweep | host | partial (r12) |
| X14 | Native replies survive a let-go or refused answer | Snapshot reads with `native.later` | runner | none: fixed for a watched topic's re-ask by main #183 and adopted (`T3ReadGate.swift` removed) |
| X15 | Key equivalents under a non-Latin input source ([#110](https://github.com/ccheever/exact2/issues/110), closed by main #168 for declared chords only) | Menu items the host does not own, the terminal's web view and the module's own key readers under Korean 2-Set | host | `R10Connect.swift` re-issues chords (kept) |
| X16 | `autocorrect="off"` also turns off smart quotes, dashes, text replacement ([#111](https://github.com/ccheever/exact2/issues/111), fixed by main #160) | none now | host | none: every textarea has `autocorrect="off"` (adopted) |
| X17 | Popover side areas and `position-try` flips | Hover cards and tooltips that flip near edges | contract/host | `position-area` top/bottom; fixed placement for end-aligned and flipping layers |
| X18 | SVG path `d` animation | Morphing icons | host | cross-fade |
| X19 | Timers/clock in data sources | Debounces, cooldowns (450 ms, 10 s) | policy (LLP 1092 accepted, not built) | time passed as arguments, Contract tasks |
| X20 | Rich-text editing with atomic inline nodes; caret/selection read and range replace; paste interception; undo groups | Composer (Tiptap): @ / $ menus, chips, history recall, large paste → file | framework feature (DEFERRED "no rich value type") | native NSTextView composer (`T3Composer*.swift`) |
| X21 | Two-way WebSocket for data modules | WebSocket RPC to the T3 server, device input | framework feature (LLP 1016.000: receive-only) | Swift transport (`T3Transport.swift`, `T3Fleet.swift`) |
| X22 | Reactive layout facts (size/position, text width) and row visibility | Composer overlay reservation, menu placement, timeline minimap | framework feature | `t3-frame`, `t3-anchor`, `t3-turn` hooks |
| X23 | Scroll restore by key on a top-level list; `scroll-margin`; animated native scrollIntoView | Per-thread scroll position, minimap/citation jumps | framework feature | `R9Input.swift`, `T3TimelineTurns.swift` |
| X24 | Hover re-hit-test under a still pointer after layout (fixed on main #174, adopted) | Row under the pointer after ⌘Z / list change | host | none (the `t3-rehover` hook is gone) |
| X25 | Keyboard: keyup / modifiers-held fact, `KeyboardEvent.code` and `repeat`, capture-phase handler, compositionend on a chord | ⌘ hold hints, ⌘Q hold, Send-button modifiers, key recorder, surface launcher | framework feature | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `R9Input` |
| X26 | App menu control: standard items (Paste as Text, Speech, Help), hide host Go/Develop, page zoom, submenus, menu at the pointer | Electron application menu, context menus | host | `T3Menus.swift`, `R8KeysMenus.swift`, `T3Sidebar.swift` |
| X27 | Window chrome: title-row height and traffic-light inset; full-screen fact (frame restore fixed on main #164, adopted) | `hiddenInset` title bar, sidebar inset in full screen | host | `T3WindowChrome.swift`, `T3FullScreen.swift` |
| X28 | Notification click → app action, Dock badge, window-focus fact | Thread notifications | policy (DEFERRED refuses actions/badges) | `T3Notifications.swift` |
| X29 | `video` from `app:/` files; a PDF viewer element | Composer video preview, PDF attachments | framework feature | AVPlayerView, PDFView natively |
| X30 | TS can announce a topic / invalidate a resource; pixel readback; any-type file picker with bytes and image transcode | Wake reads, image accent colour, attachments | framework feature | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X35 | A password `input` whose value stays out of every agent output ([#134](https://github.com/ccheever/exact2/issues/134), closed by main #167: the tree, `layout` and `type` reply mask the field; the app's state and data module stay outside that) | SSH Password Required dialog | host + agent | native `t3-ssh-password` secure field (`T3SshAuth.swift`, kept); the module reads it on Continue |
| X43 | `aria-checked="mixed"` on a switch or checkbox ([#120](https://github.com/ccheever/exact2/issues/120), closed, not planned) | Scoped switches whose targets disagree (D15) | contract/host | the app draws the mixed thumb; the switch reports unchecked (kept) |
| X46 | A build step that makes app assets before the Apple bundle copies `assets/` | The terminal page (`terminal-host/build.mjs` output) | build | run `bun terminal-host/build.mjs` (app.json `commands.terminal`) before the bundle build; without it the terminal stays blank and its status names the load error |
| X47 | A focus ring on a custom pressable box ([#179](https://github.com/ccheever/exact2/issues/179), filed 2026-10-07) | Keyboard focus on the clone's custom buttons, rows and toggles | host | none: fixed by main #189 (in the branch since adopt-main-fixes-r3); the ring was seen under real Tab presses (`20261007-real-input-checks`) |


## Current state on the pin (2026-10-06, exact2 `c12832e82`)

Each open item was reproduced for its upstream issue on exact2 `4c893fef6`, which adds only a QUEUE line and a test wait (`22903cc75`) over the pin, so each reproduction holds on the pin. "fix built" means a fix exists on a local `daehyeon/fw-*` branch, not on main. Every workaround below is still in the clone.

| ID | Issue | Current state on the pin | Workaround kept |
|---|---|---|---|
| X3 | [#102](https://github.com/ccheever/exact2/issues/102) (+ #136, #137) | Fixed on main (#185 `setRootFontSize`; #176 the web JS target keeps `rem`; #159 refuses a string on a number-only row), adopted 2026-10-07 (interface-font-size): the clone sets the root font size from the setting and sizes its Contract lengths in `rem` where the reference does. Contract `calc()` takes percent ± px only, so a length that adds a layout px value to a rem one multiplies the root size in (the top bars). | none |
| X4 | [#103](https://github.com/ccheever/exact2/issues/103) | Open. `assets/` is still the only bundle tree, with mode 0644 and the path-segment rule. | archive plan (embedded server not built) |
| X5 | [#104](https://github.com/ccheever/exact2/issues/104) | Open. A scheme URL reaches only a navigation root's `navigate`. | none |
| X6 | [#105](https://github.com/ccheever/exact2/issues/105) | Open. No module quit hook; `destroy()` does not run at ⌘Q. | none |
| X7 | [#106](https://github.com/ccheever/exact2/issues/106) (+ #135) | Fixed on main (#173), adopted 2026-10-07 (adopt-main-fixes-shell): `app.json` sets `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent`, and the bundle's rendered HTML loads `http://` from a named host. #135 (an `http:` sub-resource of the app's own `assets/` page) is fixed by main #184 (a bundled page is served at `http://exact.localhost`; loopback `http:` loads, a failed load is logged); nothing to adopt: no clone page is served from `assets/`. | none |
| X8 | [#107](https://github.com/ccheever/exact2/issues/107) | Fixed by main #186 (merged 2026-10-07, adopt-main-fixes-r3): `tap … auxclick`, `clicks 1–3`, `wheel … at x y`, `modifiers` held through a contact or drag, and every agent mouse and wheel event goes through `NSApplication.sendEvent`, so module monitors see it. | partly adopted: the agent rows (terminal, device, panel, diff, tab middle click) are driven; with real input (`20261007-real-input-checks`) middle click, cursor shapes, drags out of the window and terminal double-click, right-click and drag select pass (`20261007-terminal-real-drag`); adopted |
| X9 | [#108](https://github.com/ccheever/exact2/issues/108) | Open on main; fix built. `app.contract` holds every resource. | split views, root keeps state |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Open (code-wrap breaks, placeholder colour, balance, smoothing). | none (visible difference) |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | Open. Backdrop blur sees only the parent's paint. | opaque composer |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | Open. | measured height |
| X13 | not filed | Unverified on macOS (the web behavior is designed). | partial (r12 Escape) |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | Fixed by main #183 (merged 2026-10-07, adopt-main-fixes-r3): a watched topic announced while the resource's request is in flight lets that reply land, then asks once more. A re-ask for new arguments or a `refresh` still forgets the old request (LLP 1016 D5). | none: `T3ReadGate.swift` and the reader tags are removed (adopted); `readDetail` reads per answer, which the different-arguments rule still needs (`r6-pr-actions.ts`) |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | Closed by main #168 (2026-10-07 merge): `aria-keyshortcuts` and the host's command menu items match the physical key. Not covered: menu items without the host's shortcut target (Copy, Paste, Undo, Quit, Close Window, Reload, Paste as Text), the terminal's web view, the module's own key monitors. | `R10Connect.swift` re-issues chords (kept; adopt-main-fixes-input) |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | Fixed by main #160. | none: the hook's switch-off is removed; every textarea has `autocorrect="off"` (adopt-main-fixes-input) |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Partly fixed: `position-area` top, bottom and center work (`2c6b551ba`); no `position-try` flip, no `span-left` or side areas. | end-aligned and flipping menus keep fixed placement |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | Fixed by main #188 (SVG `d` transitions on macOS). Partly adopted (adopt-main-fixes-r3): five icons morph as T3 Code's morphicons does (per-subpath turn and drift on the snappy spring, `d` between same-structure polylines). | the code-block and welcome copy buttons still cross-fade: their return runs as keyframes (X19) and keyframed `d` is refused |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Open on main; fix built. | time as arguments, Contract tasks |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | Open on main; fix built. | native NSTextView composer |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Open on main; fix built. | Swift transport |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Open on main; fix built. | `t3-frame`, `t3-anchor`, `t3-turn` hooks |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Open. | `R9Input.swift`, `T3TimelineTurns.swift` |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | Fixed on main (#174), adopted 2026-10-07: the host hit-tests a resting pointer after layout and scrolling. | none (`t3-rehover`, R10Connect's passes and the thread lists' `data-frame` removed) |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Open (keyup, `code`, `repeat`, held modifiers, capture phase; window shortcuts are heard before a focused field's key handler). Modifiers on keydown are fixed (`8a0afbeab`): Shift+F10 on draft rows and right-panel tabs is now Contract. | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `R9Input`; the tab rename field's Escape in `RightPanelTabsInput.swift` |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | Open. | `T3Menus.swift`, `R8KeysMenus.swift`, `T3Sidebar.swift` |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | Partly fixed: main #164 restores the frame autosave after the final style (adopted 2026-10-07: `R8PointerWindowFrame.swift` deleted). Still missing: a title-row height and traffic-light inset setting (`env(titlebar-area-*)` or a manifest field) and a full-screen fact in the page. #113 is closed. Re-checked on main `cff90b364` (adopt-main-fixes-r3): still missing (`bc6bc35f4` adds `fullscreenchange` for a video element, not the window). | `T3WindowChrome.swift` (empty unified toolbar), `T3FullScreen.swift` (full-screen fact through `t3.status`) |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Open (policy). | `T3Notifications.swift` |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Open. | AVPlayerView, PDFView |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Open (policy). | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Closed; main #171 fixed part 1 (a button press keeps the selection). Parts 2 and 3 are not on main: no selection end rectangle, no `clearSelection()`. Re-checked on `cff90b364`: still missing. | none to remove: Cite's `retainFocus` is the reference's `onPointerDown` `preventDefault()`; after citing the selection stays (the reference clears it) |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | Closed; main #178 fixed the macOS agent's hover on inline runs. `frame()` of an inline run and inline runs in agent `layout` are still missing (re-checked on `cff90b364`). | none (no inline-link hover card is built) |
| X35 | [#134](https://github.com/ccheever/exact2/issues/134) | Closed by main #167: a password field's value is masked in `tree`, `layout` and the `type` reply; `autocomplete` sets the AutoFill content type. The app's state slots and data module stay outside that (main's docs: "the app's own"). | native `t3-ssh-password` field (kept) |
| X43 | [#120](https://github.com/ccheever/exact2/issues/120) | Closed, not planned. | the app's mixed thumb; the switch reports unchecked |
| X47 | [#179](https://github.com/ccheever/exact2/issues/179) | Fixed by main #189, which is in this branch since the `cff90b364` merge (adopt-main-fixes-r3); no clone workaround to remove; real Tab presses draw the ring on the custom Settings nav buttons, which the base `4f523ef5c` does not (2026-10-07, `20261007-real-input-checks`). | none |

X1 ([#100](https://github.com/ccheever/exact2/issues/100)) and X2 ([#101](https://github.com/ccheever/exact2/issues/101)) are policy decisions; both features stay unbuilt.

---

## X1. Chromium and CDP (Browser surface)

**T3 feature.** The desktop Browser surface: tabs, URL bar, back/forward, Annotate (element picker), screenshot and recording, picture-in-picture window, device toolbar (17 presets), zoom 25–500 %, profiles, cookie import (Chromium/Safari/Firefox), per-page DevTools, and the agent automation host (`previewAutomation.*`: snapshot, click, type, press, scroll, evaluate, waitFor, recording). Terminal links and the "Open links in" setting also target it.

**Reviewed.**
- REF `apps/desktop/src/preview/Manager.ts` (5,245 lines): Electron `<webview>` plus `webContents.debugger` CDP. Domains used: Runtime, Network, Log, Page (`createIsolatedWorld`, `screencastFrame`), Emulation, Input (`dispatchMouseEvent`, `dispatchKeyEvent`), Accessibility (`getFullAXTree`), DOM, Target.
- REF `PickPreload.ts` (uses `react-grab`), `PlaywrightInjectedRuntime.ts` (uses `playwright-core`), `BrowserImport/*`, `preview-pip-preload.ts`.
- X2 `rules/DEFERRED.md:380-384` and LLP 1020 §5: the built-in `iframe` refuses `top` topology ("never, absent a product that is a browser shell"), navigation policy, popups, the controller ops, and permissions.
- X2 `host/apple/build.mjs:1021-1045`: module dependencies link only as static libraries or framework slices from `modules/apple/*.xcframework` ("a dynamic library is not linked into the module").
- X2 `host/apple/build.mjs:1229-1244`: the macOS bundle copies only exact's binaries and `assets/`. No `Contents/Frameworks` for third-party frameworks, and no helper apps.

**Current state.** The clone has no Browser surface. The launcher row "Browser" is always unavailable (`shell.ts:152`), and "Open links in" is disabled (`settings-source-control.contract:446`).

**Why it does not work.**
1. Chromium (CEF) needs a dynamic framework plus GPU/renderer/plugin helper apps inside the bundle. The exact2 Apple build cannot embed either.
2. CDP exists only in Chromium. WebKit has no equivalent for Network events, trusted input dispatch, the full accessibility tree, or screencast.
3. The `iframe` cannot be a browser shell (LLP 1020 §5).
4. Recording and PiP need screen capture. ScreenCaptureKit asks for Screen Recording even for the app's own windows (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:814`).

**Support needed (choose one).**
- A. **Chromium path:** let an app module ship a dynamic framework and helper app bundles (copy, sign, `@rpath`), and accept a "browser shell" product in DEFERRED. Large framework work.
- B. **WebKit path (no exact2 change required):** a `t3-browser` native view hosting WKWebView. Deviations from REF: WebKit rendering; Safari Web Inspector instead of DevTools (cannot open from code); console/network capture by injected script (no subresource status); synthetic or NSEvent input instead of CDP input; ARIA snapshot by vendored Playwright script; recording by `takeSnapshot` polling or ScreenCaptureKit with a permission prompt. Helpful exact2 addition: X8.

**Decision needed:** A, B, or keep excluded.

## X2. Developer Tools for the app UI

**T3 feature.** View › Toggle Developer Tools opens Chromium DevTools for the whole app UI (REF `apps/desktop/src/window/DesktopApplicationMenu.ts:229-251`).

**Reviewed.** X2 `rules/DEFERRED.md:615`: "no devtools UI". The agent API has `tree`, `state`, `layout`, `logs`, `perf` (DEFERRED Agent API).

**Current state.** The menu item is absent. The View menu has Reload, Force Reload and zoom (`R8KeysMenus.swift`, `T3Menus.swift:78-89`).

**Why it does not work.** The clone UI is drawn natively. There is no DOM for an inspector. DEFERRED refuses a devtools UI.

**Support needed / options.** (1) Keep it absent and document it. (2) Show the item disabled. (3) Mark the app's own WKWebViews (terminal, HTML preview) `isInspectable` in development builds so Safari can inspect them. A real equivalent needs a DEFERRED change.

## X3. App-settable root font size

**T3 feature.** Settings › Appearance › Interface font size (12–20 px, default 16) sets the root `font-size`, so every `rem` size scales (REF `apps/web/src/appearanceFonts.ts:95-125`).

**Reviewed.** The kernel resolves `rem` against a root size that the host sets (`kernel/src/style/relative.rs`, `Kernel::set_root_font_size`). The macOS host always sends 16 (`host/apple/Sources/ExactKit/PageFacts.swift:75-81`).

**Current state.** Fixed on main by #185 and adopted (2026-10-07): app.contract's `rootFont` task calls `setRootFontSize(data.look.fontSize)` at launch and on every change; the Contract sizes are `rem` where the reference's are (`font-size-map.json`). The issue record is `.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261005-x03-root-font-size.md`.

**Support needed.** A way for the app to set the root font size, as CSS `:root { font-size }` does. Example: a `setRootFontSize(px)` host command on every host. After that, the clone converts its rem-derived sizes to `rem`.

## X4. Helper executables and large resource trees in the bundle

**T3 feature.** The desktop app runs its own local T3 server (Node + `apps/server/dist` + native addons: node-pty, fff, keyring, cursor-sdk).

**Reviewed.** `assets/` is the only way to add files to the `.app` (`host/apple/build.mjs` `copyAppleStaticTrees`). Constraints:
- Path segments must match `[A-Za-z0-9._-]` (`update/src/envelope.rs:449-467`). 1,007 of 4,178 runtime files fail this rule (`@scope` folders and others).
- Files are written with mode `0o644` (`filesystem/src/directory.rs`). Executables (`node`, `spawn-helper`, `rg`) lose their exec bit.
- The bake sends every asset as base64, with a 256 MiB buffer (`scripts/filesystem.mjs`). The runtime is 134 MB before Node.
- `app.json` has no field for helper executables.

**Current state.** Not built. Plan: pack the runtime and Node into Apple Archive parts of ≤ 60 MiB in `assets/`, and unpack them into the data folder at first launch.

**Support needed.** An `app.json` field for helper executables and resource trees that keeps file modes and allows any file name, signed with the bundle. Needed only if the archive workaround fails or is too slow.

## X5. Custom URL scheme delivered to the app

**T3 feature.** `t3code://` links: Codex sign-in hand-off and the provider sign-in return. The `t3 app <dir>` command opens a workspace.

**Reviewed.** `host.macos.urlSchemes` writes `CFBundleURLTypes` (`host/apple/build.mjs:297-301`). On main, a URL that arrives after boot goes only to a navigation root's `navigate` handler; otherwise the host logs "navigate refused: no navigation root handler" (`ExactMac/main.swift:423-438`, `ExactKit/Session.swift:1330-1345`). `ExactModule` has no URL callback. Governing: LLP 1038 D8.

**Support needed.** When no route takes a scheme URL, deliver it to a data source or the app module as an event.

## X6. Module shutdown time at quit

**T3 feature.** At quit, the desktop app stops its server: SIGTERM, then SIGKILL after 2 s, and it waits up to 5 s.

**Reviewed.** On main there is no `applicationWillTerminate`; ⌘Q returns `.terminateNow` (`ExactMac/main.swift:400-413`). `destroy()` runs only from `Session.destroy` on `windowWillClose` (`ExactKit/Session.swift:1378-1398`); whether that runs at ⌘Q is unconfirmed. Governing: LLP 1069.010 Q4, D7.

**Support needed.** A module hook at quit that can delay termination for a bounded time.

## X7. ATS keys from `app.json`

**T3 feature.** Rendered HTML previews load page assets from any host, as the REF iframe does.

**Reviewed.** WKWebView blocks plain `http://` to hosts that are not IP addresses (App Transport Security; r12-render finding). `app.json` cannot set ATS keys.

**Support needed.** An `app.json` field that writes ATS keys (e.g. `NSAllowsArbitraryLoadsInWebContent`) into `Info.plist`.

**Fixed (2026-10-07).** main #173 added `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent`; the clone's `app.json` sets it, and the bundle's `Info.plist` carries `NSAppTransportSecurity` `{NSAllowsArbitraryLoadsInWebContent: true}`. Web views only: the module's `URLSession` stays under ATS. Proof: a drive of each assembled `.app` (`EXACT_MAC_BIN`; the agent's bare executable has no ATS) opening `media/page.html` in Files; the AFTER app fetched `/style.css` and `/badge.png` from `localtest.me`, the BEFORE app fetched nothing.

## X8. Pointer input for native views in the agent

**Reviewed.** `ExactNativeInput` has only `.text` and `.key` (`host/apple/modules/ExactNativeModule.swift:527-530`).

**Why it matters.** The agent cannot click, drag or scroll inside native views (terminal, browser, device screen, PDF/video). These checks need a real-input session.

**Support needed.** Pointer phases (down/move/up, wheel) in `ExactNativeInput`, as forms of `tap` (the DEFERRED agent-API rule allows a new input as a form of `tap`).

**Fixed (2026-10-07).** The premise was partly wrong (the agent already sent real mouse events to a module's view). Main #186 added what was missing: `auxclick` (button 2), `clicks 1–3`, `at x y` on every click form and on wheels, `modifiers` held from `down` to the lift, a strict `tap` parser, and delivery through `NSApplication.sendEvent`, so the app's local monitors see an agent's click or wheel as a hand's. Adopted by `20261007-adopt-main-fixes-r3`: the right-panel tab's middle click is an agent row (`tap panel-tab-<id> auxclick`); comments in `T3Sidebar.swift`, `T3Timeline.swift` and `T3PanelsNative.swift` now say which agent forms their monitors see. The other attended pointer rows (terminal selection and links, device drag, panel resize, diff drag) convert when their tasks are re-driven.

## X9. Root component across files; resources in child components

**Reviewed.** A child component cannot own a resource (`type-child-resource`, LLP 1006). Every resource and root action lives in `app.contract` (41 resources, 149 root states, 154 actions, 1,327 lines). The file cap is 1,500 lines.

**Support needed.** Resources in child components, or one root component split across files (LLP 1091 modules).

## X10. Text rendering parity

Open differences against Chrome (handoff "Remaining gaps" #2):
- truncation at word boundaries instead of per character (`text-overflow: ellipsis`);
- code-line wrap break positions;
- no `text-wrap: balance`;
- no placeholder colour;
- text looks heavier than Chrome.

**Support needed.** Chrome-matching behavior for each row, held by conformance cases.

## X11. Shadow and blur parity

- Negative-spread `box-shadow` draws faint; dialog and popover shadows are faint or missing. Lists, inset and spread are built (LLP 1077 D4, `ExactKit/BoxShadow.swift`), so the cause is the caster's blur/mask/spread, not a missing row (cause unconfirmed). Support needed: measure against Chrome and fix the caster.
- Backdrop blur sees the parent's paint and earlier siblings only, not the whole window below the node (declared, LLP 1001:670-672, `ExactKit/Backdrop.swift`). The clone draws an opaque composer instead of REF's glass.
- Placeholder colour is fixed at the text colour × 0.30 (`Mac/TextAreaMac.swift:87-96`); there is no `::placeholder` row.

## X12. Textarea field sizing

A textarea sizes to its plain value. A prompt whose chips render wider than the text can be one line taller or shorter than REF. **Support needed:** `field-sizing: content`, or a measured height hook.

## X13. Hover and key events during a pan

The host sends no hover events during a pan, and a section's local state gets no keys mid-pan. REF closes the hover card when a sweep starts and cancels the sweep on Escape. r12 added an Escape workaround; hover after a cancelled sweep is still wrong.

## X14. Parked native calls survive a refused re-read

**Reviewed.** Without the change, a re-read with equal arguments could forget a live `native.later` call and drop its reply. The clone carries an uncommitted framework edit (`js/src/parking.rs`, `js/src/lib.rs`), plus `T3ReadGate.swift`. LLP 1097 (accepted; implementation 2026-10-07..09) changes the same code.

On main, `__exact_forget` still drops a let-go answer's pending native fetches unless another answer claimed them (`js/src/prelude.js:731-735,893-915`). Awaiting another answer's promise is fixed for Hermes main placement (`f96641ddd`). LLP 1097 is accepted but not built and covers storage only.

**Support needed.** Keep a let-go answer's in-flight native request and hand its reply to the next answer for the same key, or let a module mark it as background work. Until then, merge the parking change as its own PR.

**Fixed (2026-10-07).** Main #183 (#109, LLP 1016.002 D4): when a watched topic changes while the resource has a request in flight, the runner keeps that request, lets its reply land and show, then asks the resource once more. Adopted by `20261007-adopt-main-fixes-r3`: `T3ReadGate.swift`, its wiring in `T3Module.swift`, the reader tags and `readEnd` (`r3-protocol-reader.ts`, `client.ts`) and the gate's AppKit cases are removed. Kept: the RPC trace ids (a request whose answer is replaced for new arguments or a `refresh` still loses its reply, LLP 1016 D5, and "Some requests are slow" must not count it) and `readDetail`'s per-answer reads (same rule).

---

## Already fixed on exact2 main

Checked on the pin `c12832e82` with main's own tests: `bun host/apple/build.mjs --test` (795 ExactKit tests, 0 failures; `ClipMacTests`, `CollectionMacTests.testCSSCursorKeywordsReachAppKit`, `PositionAreaTests`, `ChooserMacTests.testTheMenuPopsUpByItsPositionArea`, `AccessibilityTests.testTabindexMakesABoxFocusableAndOrdersTab`) and `cargo test -p exact-js --test it an_answer_awaiting_another_answers_fetch` (1 pass). Workarounds removed by task `20261005-main-fix-adoption`:

| Fixed on main | What the clone did | Now |
|---|---|---|
| `pointer-events: none` on boxes, inherited, and hit testing of visible overflow (`652a6c865`, `c44607c7d`) | `inert=true` on 20 tooltip, hover-card, drag-ghost, fade and probe layers; the composer controls row reached 10pt left (margin -10, padding 10) to hold the model picker's `-ms-2.5` | `pointer-events="none"` alone; the row has no hit padding and the footer reads its own width. Modal covers keep `inert` (HTML's meaning). The sidebar rail and edge tips stay window-level: a later sibling (the chat column) paints and hit-tests above a strip reaching past the sidebar |
| `cursor` keywords on macOS (`c6136f39d`) | none | pointer on the shared button styles, links and sidebar rows and buttons; `w-resize` on the sidebar rail, `col-resize` on the right panel's edge. Buttons with inline styles elsewhere, and disabled buttons, still show the arrow or the hand where the reference differs |
| `position-area` on a popover (`2c6b551ba`) | absolute offsets (`bottom="100%"`, `left=-134`) | the SnapShot Accessibility data and Usage unpriced popovers use `position-area="top"` |
| Key modifiers and `preventDefault()` (`f35b3eafc`, `8a0afbeab`); bubbling and `tabindex` (`d672f9372`) | a hidden 1x1 `aria-keyshortcuts="Shift+F10"` button while a draft row held focus; a native monitor for Shift+F10 on a right-panel tab | the draft row's and the tab's own `key` handlers read Shift+F10 |
| Awaiting another answer's fetch (`f96641ddd`) | `readDetail` reads per answer | kept: an answer replaced for new arguments still loses its replies (LLP 1016 D5); #183 covers only a watched topic's re-ask |
| ATS keys from `app.json` (#173, X7/#106) | nothing: rendered HTML could not load `http://` from a named host in the bundle | `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent` in `app.json` (adopt-main-fixes-shell) |
| Frame restored after the final style (#164, X27/#113) | `R8PointerWindowFrame.swift`: its own frame record, restored after launch | deleted; the host's autosave is the only record. Title-row and full-screen facts are still missing (X27) |
| Image `load` / `error` (#177, X44/#121) | nothing: a broken image kept its box | the reference's fallbacks on chat Markdown, expanded, Files and attachment images and the theme result icon |
| Hover follows layout under a resting pointer (#174, X24/#139) | the `t3-rehover` hook on the thread list and the legacy project list, R10Connect's re-hover passes | removed |
| `title` maps to a native tooltip (`2bfebe63e`) | no tooltip where the reference uses `title` | `title` on the pending question's toggle and Dismiss, the project group's settings button, a license's Project source link, and the device rail's text-size and more-actions triggers |

Rebase notes (clone-side edits, not exact2 asks): LLP 1091 D1 refuses names reached only through another file's `use` (e.g. `markdown.contract` uses `Icon`; `class=Control` without `use`). A `button` is now Chrome's block `<button>` with centred content (`e8bc9c846`). The clone's 9 `role="alertdialog"` overlays are columns without `popover`, so `lower-alertdialog` does not refuse them.

Fixes from main's 2026-10-07 merge (feature branch `4cdb8aa63`), task `20261007-adopt-main-fixes-input`:

| Fixed on main | What the clone did | Now |
|---|---|---|
| #111 / X16: `autocorrect="off"` keeps the typed bytes on macOS (#160) | the `t3-plain-text` hook turned AppKit's substitutions off on the Files editor, the diff comment and the script command; the composer's text view did the same when it attached; the commit message, theme JSON, scheduled prompt and scoped-setting textareas had nothing | every editable textarea has `autocorrect="off"` (`text-entry.test.ts` checks it); the Files editor's hook is `t3-file-editor` and only takes the focus its press began |
| #110 / X15: a shortcut's physical key under a non-Latin source (#168) | `R10Connect.swift` re-issues every ⌘/⌃ chord with its Latin letter | kept: #168 matches `aria-keyshortcuts` and the host's command menu items only; Copy, Paste, Undo, Quit, Close Window, Reload, Paste as Text, the terminal's web view and the module's key monitors still read the typed character |
| #132 / X33 part 1: a button press keeps the selection (#171) | Cite's button is `retainFocus=true` | unchanged: that is the reference's `onPointerDown` `preventDefault()`, not a workaround; the end rectangle and `clearSelection()` (parts 2 and 3) are not on main |
| #134 / X35: a password field's value is never in the tree, `layout` or a `type` reply (#167) | the SSH dialog's field is the module's native secure field | kept: a Contract field would carry the password through Contract state and the data module, which #167 leaves outside its guarantee; the provider and Bitbucket password inputs are now masked in the tree with no clone change |
| #133 / X34: the macOS agent hovers inline runs (#178) | nothing (no inline-link hover card) | `pr-links-previews-and-routing` can drive an inline link's hover; its card still has no frame to anchor to |

Fixes from main's second 2026-10-07 merge (`cff90b364`), task `20261007-adopt-main-fixes-r3`:

| Fixed on main | What the clone did | Now |
|---|---|---|
| #109 / X14: a watched topic's re-ask lets the reply in flight land, then asks once more (#183) | `T3ReadGate.swift` held `t3.status`, `t3.events` and `t3.fleet` while a snapshot read was in flight (quiet window, idle release, replay); the read tagged every request with a reader id and ended with `readEnd` | removed; the topics go straight to Exact. RPC trace ids and `readDetail`'s per-answer reads stay (a re-ask for new arguments still drops replies) |
| #107 / X8: agent middle and triple clicks, wheel at a point, held modifiers, events seen by local monitors (#186) | the right-panel tab's middle click (a native monitor) was an attended row | `tap panel-tab-<id> auxclick` closes the tab through the same monitor; monitor comments updated |
| #135: a bundled iframe page loads loopback `http:` sub-resources (#184) | nothing (no clone page is served from `assets/`) | nothing to adopt |
| #113 / X27, #132 / X33, #133 / X34: re-checked on `cff90b364` | — | still missing on main: a title-row height and traffic-light inset, a window full-screen fact (`bc6bc35f4`'s `fullscreenchange` is a video element's), a selection end rectangle and `clearSelection()`, `frame()` of an inline run |


## X20–X30 detail

Source: a map of every clone hook and native component to the exact2 gap behind it, checked on main `d2cb661eb`.
- **X20.** DEFERRED says "no rich value type" (`rules/DEFERRED.md:160`). LLP 1045's editor styles Markdown only. A textarea has no caret/selection events (`selectionchange` is on `text` only), and a macOS textarea fires no copy/cut/paste (`docs/contract-grammar.md:681-695`). Main added `KeyboardEvent` with `preventDefault()` (`8a0afbeab`, `f35b3eafc`), so Enter-to-send and menu keys no longer need native code.
- **X21.** "receive-only WebSocket … no frame is ever sent" (`docs/reference.md:405-408`).
- **X22.** `frame()` now reads viewport space (`e73605835`), but works only in actions and is not reactive (`docs/contract-for-humans.md:1150-1156`).
- **X23.** Main added `wheel`, ScrollEvent extents, scroll anchoring and `scrollIntoView(id, block, behavior)` (`2bfebe63e`, `09fc9b0d4`, `cef67560c`, `fbcc4ecb2`). Missing: restore by key for a top-level list (LLP 1070:261), offsets, and smooth landing on native hosts.
- **X24.** Fixed on main (#174, `MouseChainMac.swift` `followPointer`): after a batch that moves boxes, or a scroll, the next display frame hit-tests the resting pointer. Adopted: the `t3-rehover` hook is removed.
- **X25.** Shortcuts match `charactersIgnoringModifiers` (`Mac/ShortcutsMac.swift:50-64`; since #168 a non-Latin character falls back to the physical key); `aria-keyshortcuts` buttons hear chords before a focused element's `key` handler (`docs/contract-grammar.md:775-777`).
- **X26.** The host menu bar is fixed (`Mac/DevMenuMac.swift:141-181`). App chords now win over host items (`9824f0e3a`). Submenus are out (LLP 1021:614).
- **X27.** `viewport-fit=cover` has no title-row or traffic-light setting and the page has no full-screen fact. The frame autosave is restored after the final style since main #164 (`ExactMac/main.swift` `finishLaunching`); the app's frame record is removed.
- **X28.** `showNotification` exists (`4754c6d9e`), but DEFERRED refuses notification actions and badges (`rules/DEFERRED.md:367-369`); no focus fact (`runner/src/page.rs:21-36`).
- **X29.** `video` takes only http(s) or bundled assets (`Mac/NodeViewMac.swift:402-407`); `image` takes `app:/`.
- **X30.** TS `native` has only available/call/watch/later (`js/src/prelude.js:820-842`); Canvas readback is refused (LLP 1056:387); a file `input` needs a literal `accept`.

## Settings scope and the theme editor: declared differences

Task `20261005-settings-scoped-controls-and-theme-editor` (D15, D16).
- **Mixed switch, accessibility (X43, [#120](https://github.com/ccheever/exact2/issues/120)).** `ScopedSwitch` (`settings-scoped-switch.contract`) draws the reference's mixed state (thumb centred at 70 % on the unchecked track). `aria-checked` takes only a boolean, so VoiceOver hears "off" where the reference says "mixed". #120 was closed, not planned: this stays a declared difference.
- **Header drag from a button (X13).** A Contract `pan` takes a drag that starts on a nested button once it passes the slop; the reference ignores pointer-downs on the header's buttons, inputs and links. A tap on Minimize or Close still presses.
- **Window resize clamp (X22, [#127](https://github.com/ccheever/exact2/issues/127)).** No window resize event reaches a component; a window-sized, inert, clipped tracker inside the panel hears `resize=` instead, and its action shrinks the size, then clamps and stores the place as the reference's listener does.
- **Inspect app colors (X30, [#116](https://github.com/ccheever/exact2/issues/116)).** Not built (plan decision U18 pending).
- **X46** (new — record at prepare; task `20261005-terminal-surface`). The Apple build copies `assets/` as it is (`host/apple/build.mjs` `copyAppleStaticTrees`); app.json `commands` are verbs for `exact.mjs`, not build steps. Generated files are not committed (rules/RULES.md), so the terminal page and its WASM/font copies (ignored by git) exist only after `bun terminal-host/build.mjs`. A fresh checkout's bundle has no page: the terminal view stays blank and its status (`state.presentation.terminals[].error`) names the failed load. **Support needed:** a manifest-declared pre-bake command (or a bundler entry for app assets) that the Apple, web and delivery builds run.

## Media actions: declared differences

Task `20261005-media-actions` (`media-actions.*`, `media-views.ts`, `T3MediaActions.swift`).
- **Copy image is never "unavailable".** The reference disables Copy image where `navigator.clipboard.write` is missing and says "Image copying is unavailable. Use a secure browser connection or save the image."; the Mac app always has a pasteboard, so the item is enabled whenever the media has a source or an asset.
- **Tooltip style.** The media tooltip is the node's `title` (AppKit's `toolTip`), not the reference's code-style popup; it shows the same text (path, URL or name) and closes when the menu opens.
- **An image that fails to decode (X44, [#121](https://github.com/ccheever/exact2/issues/121)).** Fixed on main (#177) and adopted 2026-10-07: an `image`'s `error` now shows "Image unavailable · <alt>" (chat Markdown), "Image unavailable. The file may have been moved or deleted." (expanded image), "Unable to load workspace image." (Files) and "Unable to load image." (attachment preview). Still different: an SVG never draws on Apple (it is an `error`), where the reference's browser draws it; the Files failure text sits at the top of the panel, where the reference centres it.
- **Rendered HTML over `http://` to a named host (X7, [#106](https://github.com/ccheever/exact2/issues/106), side issue [#135](https://github.com/ccheever/exact2/issues/135)).** Fixed on main (#173) and adopted 2026-10-07: `app.json` allows arbitrary loads in web content, so the bundle loads it as the reference's frame does. #135 (an `http:` sub-resource of a page under the app's own `assets/`) is fixed by main #184; nothing to adopt, since no clone page is served from `assets/`.

## Auto balance: declared differences

Task `20261005-auto-balance` (`load-balancing.ts`, `auto-balance*.ts`, `auto-balance.contract`).
- **Host resource timing (X19, X21; [#124](https://github.com/ccheever/exact2/issues/124)).** The load runs in a command a root task sends when the fetch key changes; receipt time is the window's wall time when the load starts (at most the 5 s deadline before the reply). The 5 s deadline is T3Transport's per-request `timeout`.
- **The machine list popover (X17).** Placed with `position-area="top span-right"`; it does not flip near an edge.
- **Stale 20 s is not reproducible from a server.** The sample age uses the client's receipt time (`uses client receipt time when host clocks differ`), so no server reply can make a sample 20 s old; unit tests cover the age rule and the drive uses a busy (0.96) machine.
- **Host finding: a changed text can show its old bitmap in an agent screenshot (not filed).** The macOS host draws a paragraph of at least 16,384 device pixels into a bitmap off the main thread (`Mac/TextRasterMac.swift`), keeps the old bitmap up until the replacement lands (`NodeText.swift` `invalidateText`), parks it at its old size when the frame shrinks (`presentTextRaster` overflow branch), and `settleForPicture` waits for images only. #182's drive drew "Update available for" at the width of "Updating 1 machine". The banner title is a keyed node with a box paint, which AppKit draws on the main thread. The framework fix: `settleForPicture` waits for text raster work and redraws before the capture.

## Not exact2 asks (stay in the app module)

Keychain credentials, SSH tunnels, VideoToolbox/SceneKit device views, the terminal (WKWebView running REF's Ghostty WASM; no exact2 change needed), notifications, SnapShot capture, the offscreen Mermaid web view, agent export plumbing.
