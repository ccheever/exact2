# exact2 support needed for T3 Code parity

Date: 2026-10-05; re-checked 2026-10-06 on the pin; X7, X24, X27 and X44 updated 2026-10-07 after main #173, #174, #164 and #177 (adopt-main-fixes-shell); X8, X14 and #135 updated 2026-10-07 after main #186, #183 and #184 (adopt-main-fixes-r3, main `cff90b364`); X4, X5, X6, X10, X20, X23, X29, X36 and X37 updated 2026-10-07 after main #215, #201, #200, #208, #209, #210, #205, #204 and #199 (adopt-main-fixes-r4, main `463acda68`); X11, X25, X26, X27 and X28 updated 2026-10-07 after main #221, #220, #223, #226 and #219 (adopt-main-fixes-r5, main `261dd4e10`). The clone now builds on exact2 main `c12832e82` (the pin); the items were first checked against main `d2cb661eb`. The reference app is T3 Code `f870c419fc` (HEAD `1e2ecbd975`).
The current state of every item on the pin is the table "Current state on the pin" below; what main fixed and which workaround was removed is "Already fixed on exact2 main".
Each item lists the T3 feature it blocks, what we reviewed, the current state, why it does not work, and the exact2 support it needs.
REF = `~/Documents/work/3.open-source/t3code`. X2 = exact2 main.

## Summary

| ID | Missing in exact2 | T3 feature blocked | Kind | Workaround in the clone |
|---|---|---|---|---|
| X1 | Embedded Chromium + CDP | Browser surface (preview browser, agent browser automation) | policy + build | none (not built) |
| X2 | Developer Tools for the app UI | View › Toggle Developer Tools | policy (DEFERRED) | none |
| X3 | App-settable root font size (`rem` base) (fixed on main #185, adopted) | Interface font size (12–20 px) | framework feature | none: `setRootFontSize` from app.contract `rootFont`; Contract lengths in `rem` (`font-size-map.json`) |
| X4 | Helper executables and large resource trees in the bundle (fixed on main #215: `host.macos.resources`) | Embedded local T3 server | build | fixed by main #215; the release archive ships as a native resource tree and is unpacked at first launch (U3; #215 re-signs Mach-O without entitlements) |
| X5 | Custom URL scheme delivered to the app (#104 closed; main #201 only journals an unheard launch URL) | `t3code://` deep links, provider sign-in return | host | none |
| X6 | Module shutdown time at quit (`destroy()` at quit fixed on main #200; no bounded hold) | Stop the embedded server cleanly; remove the `t3 app` socket | host | fixed by main #200; the server stops in `destroy()` (and from `atexit` for the agent driver's `exit(0)`); T3Ssh's own `willTerminate` observer removed (adopt-main-fixes-r4); the `t3 app` control socket is closed and unlinked in `destroy()` and from `atexit` (app-activation, #254) |
| X7 | ATS keys from `app.json` (fixed on main #173, adopted) | Rendered HTML / web views that load `http://` from host names | build | none: `app.json` `host.macos.appTransportSecurity` |
| X8 | Pointer input for native views in the agent | Agent tests of terminal, browser, device views | agent API | none: main #186, adopted (every pointer row driven by the agent or with real input) |
| X9 | Root component across files; resources in child components | Large apps (`app.contract` near 1,500 lines) | contract | split views, keep state in root |
| X10 | Text rendering parity (wrap points fixed on main #208) | `text-wrap: balance`, placeholder colour, weight | kernel/host | none (visible difference) |
| X11 | Shadow and blur parity (backdrop edges fixed on main #221; `saturate()` on main #232, which closed #225, not adopted yet; a backdrop beyond the parent's subtree is still missing on macOS, untracked) | Dialog/popover shadows, glass composer | host | flattened glass and opaque composer (#225); real backdrops take #221 with no clone change |
| X12 | Textarea field sizing | Composer height with long chips | host | measured height |
| X13 | Hover and key events during a pan | Sidebar row-action sweep | host | partial (r12) |
| X14 | Native replies survive a let-go or refused answer | Snapshot reads with `native.later` | runner | none: fixed for a watched topic's re-ask by main #183 and adopted (`T3ReadGate.swift` removed) |
| X15 | Key equivalents under a non-Latin input source ([#110](https://github.com/ccheever/exact2/issues/110), closed by main #168 for declared chords only) | Menu items the host does not own, the terminal's web view and the module's own key readers under Korean 2-Set | host | `R10Connect.swift` re-issues chords (kept) |
| X16 | `autocorrect="off"` also turns off smart quotes, dashes, text replacement ([#111](https://github.com/ccheever/exact2/issues/111), fixed by main #160) | none now | host | none: every textarea has `autocorrect="off"` (adopted) |
| X17 | Popover side areas and `position-try` flips | Hover cards and tooltips that flip near edges | contract/host | `position-area` top/bottom; fixed placement for end-aligned and flipping layers |
| X18 | SVG path `d` animation | Morphing icons | host | cross-fade |
| X19 | Timers/clock in data sources | Debounces, cooldowns (450 ms, 10 s) | policy (LLP 1092 accepted, not built) | time passed as arguments, Contract tasks |
| X20 | Rich-text editing with atomic inline nodes; caret/selection read and range replace; undo groups (field paste/copy/cut fixed on main #209) | Composer (Tiptap): @ / $ menus, chips, history recall, large paste → file | framework feature (DEFERRED "no rich value type") | native NSTextView composer (`T3Composer*.swift`), its paste handling included |
| X21 | Two-way WebSocket for data modules | WebSocket RPC to the T3 server, device input | framework feature (LLP 1016.000: receive-only) | Swift transport (`T3Transport.swift`, `T3Fleet.swift`) |
| X22 | Reactive layout facts (size/position, text width) and row visibility | Composer overlay reservation, menu placement, timeline minimap | framework feature | `t3-frame`, `t3-anchor`, `t3-turn` hooks |
| X23 | Scroll restore by key on a top-level list; `scroll-margin`; animated native scrollIntoView (plain-scroll anchoring fixed on main #210) | Per-thread scroll position, minimap/citation jumps | framework feature | `R9Input.swift`, `T3TimelineTurns.swift` |
| X24 | Hover re-hit-test under a still pointer after layout (fixed on main #174, adopted) | Row under the pointer after ⌘Z / list change | host | none (the `t3-rehover` hook is gone) |
| X25 | Keyboard: modifiers-held fact and capture-phase handler (#140; `keyup`, `KeyboardEvent.code` and `repeat` on main since #220) | ⌘ hold hints, ⌘Q hold, Send-button modifiers, key recorder, surface launcher | framework feature | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `T3Sidebar` (all need #140) |
| X26 | App menu control: standard items (Paste as Text, Help), hide host Go/Develop, page zoom, a menu opened from the keyboard (#141, #235; context-menu submenus on main #223 and Edit ▸ Speech #226, adopted). Also, a context or button menu opened by `MenusMac` holds back every `native.later` call while it is open: it pops up inside `DispatchQueue.main.async`, where the main queue does not drain (reported in adopt-main-fixes-r5, not filed) | Electron application menu, context menus | host | `T3Menus.swift`, `R8KeysMenus.swift`; `T3Sidebar.swift` for keyboard-opened row menus only; the module's own menus open from a common-modes run-loop turn (`T3MenuTurn`) |
| X27 | Window chrome: title-row height and traffic-light inset; full-screen fact (frame restore fixed on main #164, adopted; the rest still missing on `261dd4e10`, #113 closed) | `hiddenInset` title bar, sidebar inset in full screen | host | `T3WindowChrome.swift`, `T3FullScreen.swift` |
| X28 | Notification click → app action, Dock badge (#224; the window-focus fact is on main since #219, adopted) | Thread notifications | policy (DEFERRED refuses actions/badges) | `T3Notifications.swift` (badge, click, sounds) |
| X29 | `video` from `app:/` files; a PDF viewer element (bundled PDF iframe fixed on main #205) | Composer video preview, PDF attachments | framework feature | AVPlayerView, PDFView natively |
| X30 | TS can announce a topic / invalidate a resource; pixel readback; any-type file picker with bytes and image transcode | Wake reads, image accent colour, attachments | framework feature | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X31 | Hold the first window until the app says it is ready ([#117](https://github.com/ccheever/exact2/issues/117), open) | The reference opens its window only once the embedded server is ready | host | the first window shows the connecting state until the primary connects (`20261005-local-primary-environment`, decision U5, provisional) |
| X45 | An app process relaunch ([#122](https://github.com/ccheever/exact2/issues/122), closed by main #170, which only moves `reload()`'s log; no relaunch) | A Local environment change relaunches the app (decision U4) | host | `applyLocalSetting` stops or starts the embedded server in place and reconnects; the window stays (`this-machine.ts`); Network access and Tailscale HTTPS restart it in place with the new envelope (`connections-network.ts`) |
| X35 | A password `input` whose value stays out of every agent output ([#134](https://github.com/ccheever/exact2/issues/134), closed by main #167: the tree, `layout` and `type` reply mask the field; the app's state and data module stay outside that) | SSH Password Required dialog | host + agent | native `t3-ssh-password` secure field (`T3SshAuth.swift`, kept); the module reads it on Continue |
| X43 | `aria-checked="mixed"` on a switch or checkbox ([#120](https://github.com/ccheever/exact2/issues/120), closed, not planned) | Scoped switches whose targets disagree (D15) | contract/host | the app draws the mixed thumb; the switch reports unchecked (kept) |
| X46 | A build step that makes app assets before the Apple bundle copies `assets/` | The terminal page (`terminal-host/build.mjs` output) | build | run `bun terminal-host/build.mjs` (app.json `commands.terminal`) before the bundle build; without it the terminal stays blank and its status names the load error |
| X47 | A focus ring on a custom pressable box ([#179](https://github.com/ccheever/exact2/issues/179), filed 2026-10-07) | Keyboard focus on the clone's custom buttons, rows and toggles | host | none: fixed by main #189 (in the branch since adopt-main-fixes-r3); the ring was seen under real Tab presses (`20261007-real-input-checks`) |
| X50 | A native module's data directory under `--storage` (not filed): in agent mode `ExactKit/NativeModule.swift:526` names it `$TMPDIR/exact-agent-<pid>-<runtime>`, per process, whatever `--storage` names, so a module's files (the clone's `t3-code.json`) do not survive an agent relaunch while `app:/data` does | Agent relaunch checks of the pull request panel's kept detail (pr-conversation-and-refresh) | host/agent | the kept file was replayed through the relaunch path offline; a normal launch keeps it in the app's data directory. Repro: `agent.mjs macos --storage s` twice; the second run's `$TMPDIR/exact-agent-<pid>-2/data/t3-code.json` starts empty |
| X49 | A progress value for assistive technology: no `progress` element and no `aria-valuenow`/`aria-valuetext` (local draft, not published) | The Antigravity runtime download bar (`<progress aria-label="Antigravity download">`) | contract/host | a drawn track and fill with `role="progressbar"` and the percentage as `aria-description`; the status text carries the byte counts |
| X51 | A click inside an open popover also reaches the page under it, on macOS (local draft, unconfirmed: not reproduced in a one-file app) | The theme editor's colour popover: a click on the plane pressed the Dark toggle or focused the name field under it | host | the popover box takes `press` and `retainFocus=true` (`theme-color-picker.contract`) |
| X52 | On macOS, `input type="date"`, `type="time"` and `select` are no Tab stops (the web's are; local draft, not published) | Custom snooze: Date, Time and Unit are skipped by Tab | host | none; the dialog's other stops follow the reference order |
| X53 | A modal an app opens from state: `showModal(id)` is carried by the terminal host only (macOS "unknown command", the web refuses it) and `aria-modal` keeps no Tab inside (local draft, not published) | Every T3 dialog (Base UI's focus trap, initial and final focus) | runner/host | per-dialog `key` traps, `autofocus` and `focus()` in AppConfirm, SettingsConfirm, Custom snooze and Add Environment (`20261008-dialog-shortcut-focus`); other dialogs rely on the inert page behind them |
| X54 | An ancestor cannot hear the focus enter its subtree: no `focusin`/`focusout`, no `:focus-within` (local draft, not published) | A long pull request comment opening when Tab reaches a link it holds below the fold; a remark's pencil showing while the remark has the focus | contract/host | none: "Show full comment" is a Tab stop before the held body, and the pencil shows while it is itself focused |


## Current state on the pin (2026-10-06, exact2 `c12832e82`)

Each open item was reproduced for its upstream issue on exact2 `4c893fef6`, which adds only a QUEUE line and a test wait (`22903cc75`) over the pin, so each reproduction holds on the pin. "fix built" means a fix exists on a local `daehyeon/fw-*` branch, not on main. Every workaround below is still in the clone.

| ID | Issue | Current state on the pin | Workaround kept |
|---|---|---|---|
| X3 | [#102](https://github.com/ccheever/exact2/issues/102) (+ #136, #137) | Fixed on main (#185 `setRootFontSize`; #176 the web JS target keeps `rem`; #159 refuses a string on a number-only row), adopted 2026-10-07 (interface-font-size): the clone sets the root font size from the setting and sizes its Contract lengths in `rem` where the reference does. Contract `calc()` takes percent ± px only, so a length that adds a layout px value to a rem one multiplies the root size in (the top bars). | none |
| X4 | [#103](https://github.com/ccheever/exact2/issues/103) | Closed by main #215 (adopt-main-fixes-r4 checked `463acda68`): `app.json` `host.macos.resources: [{from, to}]` copies a tree under `Contents` with modes, internal symlinks, any names and files above 64 MiB, hashed into the binary receipt and signed inside-out. Developer ID signing of the tree is unverified upstream. Merged in embedded-server-runtime (2026-10-07). | archive in `server-runtime/` → `Resources/t3-runtime`, unpacked into `<T3 home>/runtime/versions` |
| X5 | [#104](https://github.com/ccheever/exact2/issues/104) | Closed by main #201, which journals a launch URL that no navigation root hears; a scheme URL still reaches only a navigation root's `navigate` (checked on `463acda68`). | none |
| X6 | [#105](https://github.com/ccheever/exact2/issues/105) | Closed by main #200: `applicationWillTerminate` destroys every session, so `destroy()` runs at ⌘Q, an Apple Event quit and last-window close. No bounded hold at quit (LLP 1069.010 Q4), no SIGTERM handling. Merged in embedded-server-runtime (2026-10-07). | T3Ssh's `willTerminateNotification` observer removed (adopt-main-fixes-r4; live: both SSH tunnels end at ⌘W, Apple Event quit and ⌘Q, as with the observer before); the embedded server's pid file and next-launch reaper (crash only); `atexit` stop for the agent driver's `exit(0)` |
| X7 | [#106](https://github.com/ccheever/exact2/issues/106) (+ #135) | Fixed on main (#173), adopted 2026-10-07 (adopt-main-fixes-shell): `app.json` sets `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent`, and the bundle's rendered HTML loads `http://` from a named host. #135 (an `http:` sub-resource of the app's own `assets/` page) is fixed by main #184 (a bundled page is served at `http://exact.localhost`; loopback `http:` loads, a failed load is logged); nothing to adopt: no clone page is served from `assets/`. | none |
| X8 | [#107](https://github.com/ccheever/exact2/issues/107) | Fixed by main #186 (merged 2026-10-07, adopt-main-fixes-r3): `tap … auxclick`, `clicks 1–3`, `wheel … at x y`, `modifiers` held through a contact or drag, and every agent mouse and wheel event goes through `NSApplication.sendEvent`, so module monitors see it. | partly adopted: the agent rows (terminal, device, panel, diff, tab middle click) are driven; with real input (`20261007-real-input-checks`) middle click, cursor shapes, drags out of the window and terminal double-click, right-click and drag select pass (`20261007-terminal-real-drag`); adopted |
| X9 | [#108](https://github.com/ccheever/exact2/issues/108) | Open on main; fix built. `app.contract` holds every resource. | split views, root keeps state |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Closed by main #208: macOS paragraphs break at Chrome's opportunities (no break after `/`). Still missing on `463acda68`: `text-wrap: balance`, a placeholder colour row, `-webkit-font-smoothing`. | none (visible difference) |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | Closed by main #221 (adopt-main-fixes-r5 merged `261dd4e10`): where macOS blurs a backdrop, the chain mirrors the box at its edges as Chrome does. Still missing: a backdrop beyond the parent's subtree (untracked: [#225](https://github.com/ccheever/exact2/issues/225) closed with main #232, which brings `saturate()` and leaves cross-parent sampling open). | flattened glass (composer card, popovers, toasts, the confirm dialog); the composer drawer and dialog backdrops are real blurs and take #221 unchanged |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | Open. | measured height |
| X13 | not filed | Unverified on macOS (the web behavior is designed). | partial (r12 Escape) |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | Fixed by main #183 (merged 2026-10-07, adopt-main-fixes-r3): a watched topic announced while the resource's request is in flight lets that reply land, then asks once more. A re-ask for new arguments or a `refresh` still forgets the old request (LLP 1016 D5). | none: `T3ReadGate.swift` and the reader tags are removed (adopted); `readDetail` reads per answer, which the different-arguments rule still needs (`r6-pr-actions.ts`) |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | Closed by main #168 (2026-10-07 merge): `aria-keyshortcuts` and the host's command menu items match the physical key. Not covered: menu items without the host's shortcut target (Copy, Paste, Undo, Quit, Close Window, Reload, Paste as Text), the terminal's web view, the module's own key monitors. | `R10Connect.swift` re-issues chords (kept; adopt-main-fixes-input) |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | Fixed by main #160. | none: the hook's switch-off is removed; every textarea has `autocorrect="off"` (adopt-main-fixes-input) |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Partly fixed: `position-area` top, bottom and center work (`2c6b551ba`); no `position-try` flip, no `span-left` or side areas. | end-aligned and flipping menus keep fixed placement |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | Fixed by main #188 (SVG `d` transitions on macOS). Partly adopted (adopt-main-fixes-r3): five icons morph as T3 Code's morphicons does (per-subpath turn and drift on the snappy spring, `d` between same-structure polylines). | the code-block and welcome copy buttons still cross-fade: their return runs as keyframes (X19) and keyframed `d` is refused |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Open on main; fix built. | time as arguments, Contract tasks |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | Closed by main #209: a field's ⌘V/⌘C/⌘X fire `paste`/`copy`/`cut` first, cancelable. Not on main: `selectionchange` on a field, `setRangeText`, `beforeinput`, undo groups, atomic ranges (phase-1 fix built, not merged). | native NSTextView composer, its paste handling included (a Contract `paste` reaches only a Contract field) |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Open on main; fix built. | Swift transport |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Open on main; fix built. | `t3-frame`, `t3-anchor`, `t3-turn` hooks |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Closed by main #210: macOS and iOS anchor a plain `scroll` box (X23d). Not on main: restore by key, `scroll-padding`/`scroll-margin`, smooth native `scrollIntoView` and `scrollend`, `overflow-anchor: none`. | `R9Input.swift`, `T3TimelineTurns.swift` (no X23d workaround existed) |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | Fixed on main (#174), adopted 2026-10-07: the host hit-tests a resting pointer after layout and scrolling. | none (`t3-rehover`, R10Connect's passes and the thread lists' `data-frame` removed) |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Open for a capture-phase handler and held-modifier state. Main #220 (in `261dd4e10`): `keyup` handlers, `KeyboardEvent.code` and `.repeat` on every host; a ⌘ chord commits a composition first. Modifiers on keydown (`8a0afbeab`): Shift+F10 on draft rows and right-panel tabs is Contract. | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `T3Sidebar` (held ⌘, window capture); the tab rename field's Escape in `RightPanelTabsInput.swift`; nothing removable with #220 alone (adopt-main-fixes-r5) |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | Open for the menu bar. Main #223: a menu row naming another menu popover is a submenu (`NSMenuItem.submenu`, nested popover under the agent). Main #226: Edit ▸ Speech, paste variants after Paste, Edit's app commands after Select All. A context popover opens only from a right-click ([#235](https://github.com/ccheever/exact2/issues/235)). | `T3Menus.swift`, `R8KeysMenus.swift` (menu bar; Edit's app commands hidden); the sidebar's right-click menus are context popovers (adopt-main-fixes-r5); `T3Sidebar.swift` for keyboard-opened menus, `T3ContextMenu.swift` for the module's flat menus and the Files tree's Open with ▸ |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | Partly fixed: main #164 restores the frame autosave after the final style (adopted 2026-10-07: `R8PointerWindowFrame.swift` deleted). #113 is closed. Still missing on `261dd4e10` (adopt-main-fixes-r5): no `env(titlebar-area-*)` or title-bar setting, no full-screen fact (`exactPage()` has `hasFocus` since #219, not full screen). No open issue tracks the rest. | `T3WindowChrome.swift` (empty unified toolbar), `T3FullScreen.swift` (full-screen fact through `t3.status`) |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Closed by main #219: `exactPage().hasFocus` is `document.hasFocus()` (macOS: active app and key window), `prefer has-focus` under the agent. Notification actions and badges continue in [#224](https://github.com/ccheever/exact2/issues/224) (policy). | `T3Notifications.swift` (badge, click, sounds); the focus reading is the page's (adopt-main-fixes-r5) |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Closed by main #205: a bundled PDF `iframe` shows WebKit's PDF view. Not on main: an `app:/` iframe and a PDF element with fit-to-width. | AVPlayerView, PDFView (WebKit's PDF view ignores `#toolbar=0&view=FitH` and has its own white surround, where T3 Code shows the page on Chromium's #282828 surface) |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Open (policy). | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X31 | [#117](https://github.com/ccheever/exact2/issues/117) | Open. The host creates the window at launch; no app signal holds it. | the connecting state in the first window (U5, provisional) |
| X45 | [#122](https://github.com/ccheever/exact2/issues/122) | Closed after main #170, which moves `reload()`'s log to stderr; exact2 has no process relaunch. | `applyLocalSetting` restarts the embedded server in place |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Closed; main #171 fixed part 1 (a button press keeps the selection). Parts 2 and 3 are not on main: no selection end rectangle, no `clearSelection()`. Re-checked on `cff90b364`: still missing. | none to remove: Cite's `retainFocus` is the reference's `onPointerDown` `preventDefault()`; after citing the selection stays (the reference clears it) |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | Closed; main #178 fixed the macOS agent's hover on inline runs. `frame()` of an inline run and inline runs in agent `layout` are still missing (re-checked on `cff90b364`). | none (no inline-link hover card is built) |
| X36 | [#118](https://github.com/ccheever/exact2/issues/118) | Fixed by main #204: `Intl.Locale` and `getWeekInfo()` in every Hermes runtime, as Chrome answers them. | none: adopted (adopt-main-fixes-r4); `resolveWeekStartsOn` answers on macOS |
| X37 | [#119](https://github.com/ccheever/exact2/issues/119) | Closed by main #199: `exact release` signs nested Mach-O files and bundles inside-out. Not on main: app-declared entitlements, a pre-seal hook. | none (portable-app-download not built) |
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

**Current state (2026-10-07, embedded-server-runtime).** Main #215 added `host.macos.resources`. The official release archive (77 MB) ships as one file in `Contents/Resources/t3-runtime` and is unpacked into `<T3 home>/runtime/versions/<version>` at first launch (about 3 s), keeping the release's signatures. Shipping the tree itself would let #215 re-sign `t3` without its JIT entitlements; under `exact release`'s hardened runtime it then aborts at start.

**Support needed.** An `app.json` field for helper executables and resource trees that keeps file modes and allows any file name, signed with the bundle. Needed only if the archive workaround fails or is too slow.

**Since main #215 (2026-10-07).** `host.macos.resources: [{from, to}]` is that field; the archive plan is no longer needed. Not adopted yet (the embedded server is not built).

## X5. Custom URL scheme delivered to the app

**T3 feature.** `t3code://` links: Codex sign-in hand-off and the provider sign-in return. The `t3 app <dir>` command opens a workspace.

**Reviewed.** `host.macos.urlSchemes` writes `CFBundleURLTypes` (`host/apple/build.mjs:297-301`). On main, a URL that arrives after boot goes only to a navigation root's `navigate` handler; otherwise the host logs "navigate refused: no navigation root handler" (`ExactMac/main.swift:423-438`, `ExactKit/Session.swift:1330-1345`). `ExactModule` has no URL callback. Governing: LLP 1038 D8.

**Support needed.** When no route takes a scheme URL, deliver it to a data source or the app module as an event.

**Since main #201 (2026-10-07).** A launch URL that no navigation root hears is journaled; it is still not delivered.

## X6. Module shutdown time at quit

**T3 feature.** At quit, the desktop app stops its server: SIGTERM, then SIGKILL after 2 s, and it waits up to 5 s.

**Reviewed.** On main there is no `applicationWillTerminate`; ⌘Q returns `.terminateNow` (`ExactMac/main.swift:400-413`). `destroy()` runs only from `Session.destroy` on `windowWillClose` (`ExactKit/Session.swift:1378-1398`); whether that runs at ⌘Q is unconfirmed. Governing: LLP 1069.010 Q4, D7.

**Support needed.** A module hook at quit that can delay termination for a bounded time.

**Current state (2026-10-07).** Main #200: `applicationWillTerminate` destroys every session, so `destroy()` runs at ⌘Q, an Apple Event quit and last-window close, synchronously and without a hold. The embedded server's stop there (≤ 5 s) measured 0.79 s; the agent driver's end of drive still exits without it, so the module also stops from `atexit`. The bounded hold is still missing.

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
- truncation at word boundaries instead of per character (`text-overflow: ellipsis`) — matched Chrome on `4c893fef6` already, not filed;
- code-line wrap break positions — fixed on main #208 (2026-10-07);
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

Fixes from main's third 2026-10-07 merge (`463acda68`), task `20261007-adopt-main-fixes-r4`:

| Fixed on main | What the clone did | Now |
|---|---|---|
| #105 / X6: `destroy()` at quit and last-window close (#200) | T3Ssh observed `willTerminateNotification` to stop its tunnels and prompts (draft `20261006-native-module-termination`) | observer removed; `T3Module.destroy()` → `T3Ssh.destroy()`; the bounded hold is still missing |
| #118 / X36: `Intl.Locale` and `getWeekInfo()` in Hermes (#204) | `resolveWeekStartsOn` answered `undefined` on macOS; its test accepted that | the test expects Chrome's week start for every tag |
| #128 / X10: Chrome line breaks on macOS (#208) | nothing | nothing to remove; balance, placeholder colour and smoothing still missing |
| #125 / X20: field paste/copy/cut (#209) | native composer | kept: a Contract `paste` reaches only a Contract field; caret events, `setRangeText`, `beforeinput`, undo groups and atomic ranges are not on main |
| #138 / X23d: plain-scroll anchoring (#210) | nothing (the PR header fold is not built) | the fold row of `pr-handoffs-and-quick-actions` is unblocked |
| #115 / X29: bundled PDF iframe (#205) | `PDFView` for the server's signed PDF URL | kept: WebKit's PDF view cannot be asked for the page alone on Chromium's surface; no PDF element on main |
| #104 / X5 (#201), #103 / X4 (#215), #119 / X37 (#199) | nothing | recorded: #201 only journals an unheard launch URL; `host.macos.resources` and nested signing are for embedded-server-runtime and portable-app-download |
| #192 (docs: gated tasks) | three mount polls (`every(250)` / `every(500)`) stood in for a debounce and two event bridges | gated tasks: the PR search applies 250 ms after the last keystroke; a terminal close confirm and a module-opened thread come at once |


Fixes from main's fourth 2026-10-07 merge (`261dd4e10`), task `20261007-adopt-main-fixes-r5`:

| Fixed on main | What the clone did | Now |
|---|---|---|
| #114 / X28: `exactPage().hasFocus` (#219) | `T3Notifications.active` read `NSApp.isActive` (under the agent: any visible window counted as focused); `T3ActivityReporter` read key window, occlusion and hidden state; the workspace card never asked `vcs.refreshStatus` on focus; SnapShot settings read permissions only when opened | the page's `hasFocus`/`visibilityState`: shell notifications, activity reports (`activityFacts`), a git refresh when the window regains the focus, SnapShot settings re-read on focus; badge, click and sounds stay native (#224) |
| #140 / X25: `keyup`, `KeyboardEvent.code`, `.repeat` (#220) | native monitors for held ⌘, window capture, the key recorder | kept: each needs a capture-phase handler or held-modifier state (#140), or is a native view / the reference's main-process code |
| #141 / X26: context-menu submenus (#223) | the sidebar's thread, bulk and draft menus were `NSMenu`s built in `T3Sidebar.swift`; the agent got "dismissed" | context popovers (`ThreadMenu`, `DraftMenu`) with nested submenu popovers; a real right-click shows the same `NSMenu`; the agent opens and chooses them; keyboard-opened menus stay native (#235) |
| #141 / X26: Edit ▸ Speech, paste variants (#226) | `T3Menus` added its own Speech | removed (one Speech, the host's); `R8KeysMenus` hides the app commands #226 files under Edit (⇧⌘G Branch, ⌘D Toggle Diff) |
| #129 / X11: backdrop edges (#221) | nothing (flattened glass where the blur cannot see; real blurs elsewhere) | the composer drawer's bottom edge no longer darkens; the flattened glass stays for #225 |
| #113 / X27: re-checked | — | title-row and full-screen facts still missing; no open issue |

Merge notes (clone-side, `261dd4e10`): hooks are access hatches (`hook=` → `hatch=`, app.json `hatches`, `ExactHatches`/`ExactHatchKey`/`ExactElement.hatch`; 61b33c2ca) and LLP 1081's `-exact-` spellings (`-exact-spring(`, `-exact-tint-color`; 0bd99f606). The macOS build of an app named "T3 Code (Exact)" failed main's linked-SDK check ([#234](https://github.com/ccheever/exact2/issues/234): `otool` read the parenthesized name as an archive member). Main #240 fixed it, and the round then merged main `1f19b2400`: the bundle builds with no shim. That merge needed no clone change, and it also brought #232 (`saturate()` in ordered backdrop filters, which closed #225; cross-parent sampling on macOS stays open per #232, untracked).

## X20–X30 detail

Source: a map of every clone hook and native component to the exact2 gap behind it, checked on main `d2cb661eb`.
- **X20.** Since main #209 a field's ⌘V/⌘C/⌘X fire cancelable `paste`/`copy`/`cut` on macOS and iOS; everything else below is unchanged. DEFERRED says "no rich value type" (`rules/DEFERRED.md:160`). LLP 1045's editor styles Markdown only. A textarea has no caret/selection events (`selectionchange` is on `text` only), and a macOS textarea fires no copy/cut/paste (`docs/contract-grammar.md:681-695`). Main added `KeyboardEvent` with `preventDefault()` (`8a0afbeab`, `f35b3eafc`), so Enter-to-send and menu keys no longer need native code.
- **X21.** "receive-only WebSocket … no frame is ever sent" (`docs/reference.md:405-408`).
- **X22.** `frame()` now reads viewport space (`e73605835`), but works only in actions and is not reactive (`docs/contract-for-humans.md:1150-1156`).
- **X23.** Main added `wheel`, ScrollEvent extents, scroll anchoring and `scrollIntoView(id, block, behavior)` (`2bfebe63e`, `09fc9b0d4`, `cef67560c`, `fbcc4ecb2`). Missing: restore by key for a top-level list (LLP 1070:261), offsets, and smooth landing on native hosts. Since main #210 a plain `scroll` box anchors on macOS and iOS.
- **X24.** Fixed on main (#174, `MouseChainMac.swift` `followPointer`): after a batch that moves boxes, or a scroll, the next display frame hit-tests the resting pointer. Adopted: the `t3-rehover` hook is removed.
- **X25.** Shortcuts match `charactersIgnoringModifiers` (`Mac/ShortcutsMac.swift:50-64`; since #168 a non-Latin character falls back to the physical key); `aria-keyshortcuts` buttons hear chords before a focused element's `key` handler (`docs/contract-grammar.md:775-777`). Since #220 a `keyup` handler and `KeyboardEvent.code`/`.repeat` exist; the grammar's "Hints while ⌘ is held" recipe tracks ⌘ with `key`/`keyup` on an ancestor plus `exactPage().hasFocus`, which hears keys only inside the focused element.
- **X26.** The host menu bar is fixed (`Mac/DevMenuMac.swift`). App chords win over host items (`9824f0e3a`); since #226 Edit has Speech and files paste variants after Paste and other Edit chords after Select All. Context-menu submenus since #223 (LLP 1021 §5.2). A context popover opens only from `rightMouseDown` (`NodeViewMac.swift`), #235.
- **X27.** `viewport-fit=cover` has no title-row or traffic-light setting and the page has no full-screen fact (re-checked on `261dd4e10`). The frame autosave is restored after the final style since main #164 (`ExactMac/main.swift` `finishLaunching`); the app's frame record is removed.
- **X28.** `showNotification` exists (`4754c6d9e`), but DEFERRED refuses notification actions and badges (#224). The focus fact is `exactPage().hasFocus` since #219 (`runner/src/page.rs`, `ExactKit/PageFacts.swift`).
- **X29.** `video` takes only http(s) or bundled assets (`Mac/NodeViewMac.swift:402-407`); `image` takes `app:/`. Since main #205 a bundled PDF `iframe` shows WebKit's PDF view; there is no PDF element.
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
- **Host finding: a changed text can show its old bitmap in an agent screenshot (not filed).** The macOS host draws a paragraph of at least 16,384 device pixels into a bitmap off the main thread (`Mac/TextRasterMac.swift`), keeps the old bitmap up until the replacement lands (`NodeText.swift` `invalidateText`), parks it at its old size when the frame shrinks (`presentTextRaster` overflow branch), and `settleForPicture` waits for images only. #182's drive drew "Update available for" at the width of "Updating 1 machine". The banner title is a keyed node with a box paint, which AppKit draws on the main thread. The framework fix: `settleForPicture` waits for text raster work and redraws before the capture. Seen again by `20261005-usage-reset-and-feedback` (2026-10-08): the `/feedback` banner drew "Sending feedback to Ope" at the width of "Feedback sent to OpenAI" in the agent's screenshot; the notice id now carries the upload status, so each status is its own node.

## This machine (local primary environment): declared differences

Task `20261005-local-primary-environment` (`local-primary.ts`, `this-machine.ts`, `this-machine.contract`).
- **The first window before the server is ready (X31, [#117](https://github.com/ccheever/exact2/issues/117)).** The reference shows no window until its embedded server is ready; the clone's window opens at launch and shows the connecting state until the primary connects (decision U5, provisional, user decision pending).
- **No relaunch after a Local environment change (X45, [#122](https://github.com/ccheever/exact2/issues/122)).** The reference relaunches the app (decision U4); the clone stops or starts the embedded server in place, hands the focus over and reconnects, and the window stays.
- **Tab inside the dialog (LLP 1080.003 §4; X53).** `aria-modal` does not keep Tab inside a modal on macOS; the Local environment dialog keeps it on its two buttons with its own `key` handlers, as the reference's focus trap does, so nothing differs. The same stopgap now covers AppConfirm, SettingsConfirm, Custom snooze and Add Environment (`20261008-dialog-shortcut-focus`).

## Provider settings upkeep: declared differences

Task `20261005-provider-settings-upkeep`, 2026-10-08.
- **ACP registry icons on Apple (X44, [#121](https://github.com/ccheever/exact2/issues/121), residual).** Registry icons are SVG (`https://cdn.agentclientprotocol.com/registry/v1/latest/<id>.svg`); an SVG `image` is a load error on Apple, so every instance icon of an ACP agent shows the ACP glyph there (the fallback the reference shows while loading or after a failure). The allow-list, the agent-id URL and the `load`/`error` states are ported; a PNG on the CDN would draw. The fetch rules of `AcpRegistryIcon.tsx` (no credentials, no referrer, 512 KB, CacheStorage) are the host's image loading, documented by main #177.
- **End-aligned popovers (X17, [#112](https://github.com/ccheever/exact2/issues/112)).** The update details popover (side bottom, align end, w-80) is placed by margins: its left edge sits the popover's width minus the trigger's to the left of the trigger. The Apple host applies a popover's margins only when it names a `position-area` (`host/apple/src/style.rs`, the `crossing` rows), so the popover says `position-area="bottom span-right"`, which is D2's own placement; without it the margin is dropped and the popover starts at the trigger (seen on the 2026-10-08 drive). It does not flip near a window edge, and nothing pulls it back from the left edge: the host clamps the margin box (the trigger's width), not the popover, so a list-row popover is cut off on the left when the trigger sits less than 20rem from it (the Settings nav collapsed in a window narrower than about 1,240 points); the reference's Base UI popover shifts to stay on screen. The web target places it by the browser's anchor positioning and was not driven for this popover.
- **Escape inside Settings.** Among `aria-keyshortcuts` buttons the lowest node wins, so Settings' Back (Escape) answered before a popover's own dismissal and closed Settings. The update popover and the clone's select popups are `aria-modal` while open (only shortcuts inside a modal are heard) and carry an Escape button that hides them and returns focus to the trigger; an open custom model editor makes Back give up Escape (`providerPage.escapeOwned`). Not an exact2 gap (the web's `keydown` also reaches document handlers); recorded because the reference's Base UI popover stops it.
- **Popover motion.** The reference fades and scales the popover in; no popover in the clone animates (the app's popover timing), so reduced motion changes nothing for it. The "Updating" spinner stops under reduced motion, as `motion-safe:` does.

## This machine: Network access, Tailscale HTTPS, Authorized clients — declared differences

Task `20261005-this-machine-network-access` (`server-exposure.ts`, `tailscale.ts`, `pairing-urls.ts`, `auth-access.ts`, `connections-network.ts`, `connections-network*.contract`, `T3LocalNetwork.swift`).
- **No relaunch after a network change (X45, [#122](https://github.com/ccheever/exact2/issues/122)).** The reference persists the setting and relaunches the app ("T3 Code will restart."); the clone persists it, restarts the embedded server in place with the new envelope (`host`, `tailscaleServeEnabled`, `tailscaleServePort`), reconnects this machine and keeps the window. The dialogs keep the reference's words.
- **The "+N" toggle's dotted underline (local draft, not published).** Contract's `border-*-style` takes `none`, `hidden`, `solid` and `inset` only; `border-bottom-style="dotted"` fails the build (`lower-attr-value`). The clone draws a solid 1 px underline in the same muted colour.
- **Tailscale is verified with a stub CLI (decision U9, provisional).** `tailscale status --json` and `serve` are a lane script; the MagicDNS HTTPS endpoint never answers the probe, so "Setup required" → available and Disable are covered by unit tests only, until a tailnet the user provides.

## Not exact2 asks (stay in the app module)

Keychain credentials, SSH tunnels, VideoToolbox/SceneKit device views, the terminal (WKWebView running REF's Ghostty WASM; no exact2 change needed), notifications, SnapShot capture, the offscreen Mermaid web view, agent export plumbing.
