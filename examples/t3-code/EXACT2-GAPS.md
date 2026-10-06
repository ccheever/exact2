# exact2 support needed for T3 Code parity

Date: 2026-10-05. The clone builds on exact2 `c1522fdac`; we checked these items against exact2 main `d2cb661eb`. The reference app is T3 Code `f870c419fc` (HEAD `1e2ecbd975`).
Each item lists the T3 feature it blocks, what we reviewed, the current state, why it does not work, and the exact2 support it needs.
REF = `~/Documents/work/3.open-source/t3code`. X2 = exact2 main.

## Summary

| ID | Missing in exact2 | T3 feature blocked | Kind | Workaround in the clone |
|---|---|---|---|---|
| X1 | Embedded Chromium + CDP | Browser surface (preview browser, agent browser automation) | policy + build | none (not built) |
| X2 | Developer Tools for the app UI | View › Toggle Developer Tools | policy (DEFERRED) | none |
| X3 | App-settable root font size (`rem` base) | Interface font size (12–20 px) | framework feature | none |
| X4 | Helper executables and large resource trees in the bundle | Embedded local T3 server | build | archive in `assets/`, unpack at launch (planned) |
| X5 | Custom URL scheme delivered to the app | `t3code://` deep links, provider sign-in return | host | none |
| X6 | Module shutdown time at quit | Stop the embedded server cleanly | host | none |
| X7 | ATS keys from `app.json` | Rendered HTML / web views that load `http://` from host names | build | none |
| X8 | Pointer input for native views in the agent | Agent tests of terminal, browser, device views | agent API | real-input sessions only |
| X9 | Root component across files; resources in child components | Large apps (`app.contract` near 1,500 lines) | contract | split views, keep state in root |
| X10 | Text rendering parity | Ellipsis, wrap points, `text-wrap: balance`, placeholder colour, weight | kernel/host | none (visible difference) |
| X11 | Shadow and blur parity | Dialog/popover shadows, glass composer | host | opaque composer, faint shadows |
| X12 | Textarea field sizing | Composer height with long chips | host | measured height |
| X13 | Hover and key events during a pan | Sidebar row-action sweep | host | partial (r12) |
| X14 | Native replies survive a let-go or refused answer | Snapshot reads with `native.later` | js runtime | `js/src/parking.rs` (uncommitted framework edit) + `T3ReadGate.swift` |
| X15 | Key equivalents under a non-Latin input source | ⌘B, ⌘K and menu chords under Korean 2-Set | host | `R10Connect.swift` re-issues chords |
| X16 | `autocorrect="off"` also turns off smart quotes, dashes, text replacement | Exact bytes typed in composer and Files editor | host | `t3-plain-text` hook |
| X17 | Popover side areas and `position-try` flips | Hover cards and tooltips that flip near edges | contract/host | fixed placement |
| X18 | SVG path `d` animation | Morphing icons | host | cross-fade |
| X19 | Timers/clock in data sources | Debounces, cooldowns (450 ms, 10 s) | policy (LLP 1092 accepted, not built) | time passed as arguments, Contract tasks |
| X20 | Rich-text editing with atomic inline nodes; caret/selection read and range replace; paste interception; undo groups | Composer (Tiptap): @ / $ menus, chips, history recall, large paste → file | framework feature (DEFERRED "no rich value type") | native NSTextView composer (`T3Composer*.swift`) |
| X21 | Two-way WebSocket for data modules | WebSocket RPC to the T3 server, device input | framework feature (LLP 1016.000: receive-only) | Swift transport (`T3Transport.swift`, `T3Fleet.swift`) |
| X22 | Reactive layout facts (size/position, text width) and row visibility | Composer overlay reservation, menu placement, timeline minimap | framework feature | `t3-frame`, `t3-anchor`, `t3-turn` hooks |
| X23 | Scroll restore by key on a top-level list; `scroll-margin`; animated native scrollIntoView | Per-thread scroll position, minimap/citation jumps | framework feature | `R9Input.swift`, `T3TimelineTurns.swift` |
| X24 | Hover re-hit-test under a still pointer after layout | Row under the pointer after ⌘Z / list change | host | `t3-rehover` hook |
| X25 | Keyboard: keyup / modifiers-held fact, `KeyboardEvent.code` and `repeat`, capture-phase handler, compositionend on a chord | ⌘ hold hints, ⌘Q hold, Send-button modifiers, key recorder, surface launcher | framework feature | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `R9Input` |
| X26 | App menu control: standard items (Paste as Text, Speech, Help), hide host Go/Develop, page zoom, submenus, menu at the pointer | Electron application menu, context menus | host | `T3Menus.swift`, `R8KeysMenus.swift`, `T3Sidebar.swift` |
| X27 | Window chrome: title-row height and traffic-light inset; frame restore after final style | `hiddenInset` title bar, window frame across launches | host | `T3WindowChrome.swift`, `R8PointerWindowFrame.swift` |
| X28 | Notification click → app action, Dock badge, window-focus fact | Thread notifications | policy (DEFERRED refuses actions/badges) | `T3Notifications.swift` |
| X29 | `video` from `app:/` files; a PDF viewer element | Composer video preview, PDF attachments | framework feature | AVPlayerView, PDFView natively |
| X30 | TS can announce a topic / invalidate a resource; pixel readback; any-type file picker with bytes and image transcode | Wake reads, image accent colour, attachments | framework feature | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X43 | `aria-checked="mixed"` on a switch or checkbox ([#120](https://github.com/ccheever/exact2/issues/120)) | Scoped switches whose targets disagree (D15) | contract/host | the app draws the mixed thumb; the switch reports unchecked |

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

**Current state.** The clone saves the setting, but nothing reads it (`settings-appearance-look.ts:55`).

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

## X8. Pointer input for native views in the agent

**Reviewed.** `ExactNativeInput` has only `.text` and `.key` (`host/apple/modules/ExactNativeModule.swift:527-530`).

**Why it matters.** The agent cannot click, drag or scroll inside native views (terminal, browser, device screen, PDF/video). These checks need a real-input session.

**Support needed.** Pointer phases (down/move/up, wheel) in `ExactNativeInput`, as forms of `tap` (the DEFERRED agent-API rule allows a new input as a form of `tap`).

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

---

## Already fixed on exact2 main (remove clone workarounds after the rebase)

- `pointer-events: none` on boxes and hit testing of visible overflow (`652a6c865`, `c44607c7d`).
- `cursor` keywords on macOS (`c6136f39d`).
- Popover placement above/below the invoker with `position-area` (`2c6b551ba`).
- Key events carry modifiers; `preventDefault()` claims a key (`f35b3eafc`, `8a0afbeab`).
- Key events bubble to ancestors with a `key` handler, `tabindex` exists, so rows can take keys (`f35b3eafc`, `d672f9372`).
- Awaiting another answer's fetch waits for it on Hermes main placement (`f96641ddd`).
- `title` maps to a native tooltip (`2bfebe63e`).

Rebase notes (clone-side edits, not exact2 asks): LLP 1091 D1 refuses names reached only through another file's `use` (e.g. `markdown.contract` uses `Icon`; `class=Control` without `use`). A `button` is now Chrome's block `<button>` with centred content (`e8bc9c846`). The clone's 9 `role="alertdialog"` overlays are columns without `popover`, so `lower-alertdialog` does not refuse them.

## X20–X30 detail

Source: a map of every clone hook and native component to the exact2 gap behind it, checked on main `d2cb661eb`.
- **X20.** DEFERRED says "no rich value type" (`rules/DEFERRED.md:160`). LLP 1045's editor styles Markdown only. A textarea has no caret/selection events (`selectionchange` is on `text` only), and a macOS textarea fires no copy/cut/paste (`docs/contract-grammar.md:681-695`). Main added `KeyboardEvent` with `preventDefault()` (`8a0afbeab`, `f35b3eafc`), so Enter-to-send and menu keys no longer need native code.
- **X21.** "receive-only WebSocket … no frame is ever sent" (`docs/reference.md:405-408`).
- **X22.** `frame()` now reads viewport space (`e73605835`), but works only in actions and is not reactive (`docs/contract-for-humans.md:1150-1156`).
- **X23.** Main added `wheel`, ScrollEvent extents, scroll anchoring and `scrollIntoView(id, block, behavior)` (`2bfebe63e`, `09fc9b0d4`, `cef67560c`, `fbcc4ecb2`). Missing: restore by key for a top-level list (LLP 1070:261), offsets, and smooth landing on native hosts.
- **X24.** Hover comes only from NSTrackingArea enter/exit when the pointer moves (`Mac/NodeViewMac.swift:321-354`).
- **X25.** Shortcuts match `charactersIgnoringModifiers` (`Mac/ShortcutsMac.swift:50-64`); `aria-keyshortcuts` buttons hear chords before a focused element's `key` handler (`docs/contract-grammar.md:775-777`).
- **X26.** The host menu bar is fixed (`Mac/DevMenuMac.swift:141-181`). App chords now win over host items (`9824f0e3a`). Submenus are out (LLP 1021:614).
- **X27.** `viewport-fit=cover` has no title-row or traffic-light setting (`ExactMac/main.swift:209-223`); the frame autosave is restored before the final style (`:310-313`).
- **X28.** `showNotification` exists (`4754c6d9e`), but DEFERRED refuses notification actions and badges (`rules/DEFERRED.md:367-369`); no focus fact (`runner/src/page.rs:21-36`).
- **X29.** `video` takes only http(s) or bundled assets (`Mac/NodeViewMac.swift:402-407`); `image` takes `app:/`.
- **X30.** TS `native` has only available/call/watch/later (`js/src/prelude.js:820-842`); Canvas readback is refused (LLP 1056:387); a file `input` needs a literal `accept`.

## Settings scope and the theme editor: declared differences

Task `20261005-settings-scoped-controls-and-theme-editor` (D15, D16).
- **Mixed switch, accessibility (X43, [#120](https://github.com/ccheever/exact2/issues/120)).** `ScopedSwitch` (`settings-scoped-switch.contract`) draws the reference's mixed state (thumb centred at 70 % on the unchecked track). `aria-checked` takes only a boolean, so VoiceOver hears "off" where the reference says "mixed".
- **Header drag from a button (X13).** A Contract `pan` takes a drag that starts on a nested button once it passes the slop; the reference ignores pointer-downs on the header's buttons, inputs and links. A tap on Minimize or Close still presses.
- **Window resize clamp (X22, [#127](https://github.com/ccheever/exact2/issues/127)).** No window resize event reaches a component; a window-sized, inert, clipped tracker inside the panel hears `resize=` instead, and its action shrinks the size, then clamps and stores the place as the reference's listener does.
- **Inspect app colors (X30, [#116](https://github.com/ccheever/exact2/issues/116)).** Not built (plan decision U18 pending).

## Not exact2 asks (stay in the app module)

Keychain credentials, SSH tunnels, VideoToolbox/SceneKit device views, the terminal (WKWebView running REF's Ghostty WASM; no exact2 change needed), notifications, SnapShot capture, the offscreen Mermaid web view, agent export plumbing.
