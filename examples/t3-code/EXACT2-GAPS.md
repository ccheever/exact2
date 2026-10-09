# exact2 support needed for T3 Code parity

Date: 2026-10-05; re-checked 2026-10-06 on the pin; on 2026-10-08 every unfiled item and the rest of every partly fixed issue was reproduced on main `0365ad1a4` and filed (or closed), see the issues README's "Upstream issues (filed 2026-10-08)"; X7, X24, X27 and X44 updated 2026-10-07 after main #173, #174, #164 and #177 (adopt-main-fixes-shell); X8, X14 and #135 updated 2026-10-07 after main #186, #183 and #184 (adopt-main-fixes-r3, main `cff90b364`); X4, X5, X6, X10, X20, X23, X29, X36 and X37 updated 2026-10-07 after main #215, #201, #200, #208, #209, #210, #205, #204 and #199 (adopt-main-fixes-r4, main `463acda68`); X11, X25, X26, X27 and X28 updated 2026-10-07 after main #221, #220, #223, #226 and #219 (adopt-main-fixes-r5, main `261dd4e10`). Re-checked 2026-10-08 on main `e200397ec` (adopt-main-fixes-r6): main closed none of these issues since `1f19b2400`; X23 and X49 updated for main's partial steps, X61 added. X59, X60 and X61 filed 2026-10-08 as #300, #301 and #302 (reproduced on main `febb2c5fb`); the scope items X38–X41 were closed by the user the same day (not framework gaps, not listed here). Charlie decided 40 of these issues on 2026-10-08 (recorded per row, and in each X file's "Decided upstream (2026-10-08)" section); framework issues are fixed on separate branches from main, outside the T3 work. Reclassified 2026-10-08 against upstream (the issues README, "Charlie's decisions and upstream state, by bucket"): a row that waits names its bucket: "done on main" (round 7 adopts it), "fixed in open main PR #N", "waits for Charlie's next core investment, #N", "approved, no fix in progress: waits for main fix of #N", "waits for an owner ruling on #N", a permanent declared difference, or "attempt withdrawn, no fix in progress". The clone now builds on exact2 main `c12832e82` (the pin); the items were first checked against main `d2cb661eb`. The reference app is T3 Code `f870c419fc` (HEAD `1e2ecbd975`).
The current state of every item on the pin is the table "Current state on the pin" below; what main fixed and which workaround was removed is "Already fixed on exact2 main".
Each item lists the T3 feature it blocks, what we reviewed, the current state, why it does not work, and the exact2 support it needs.
REF = `~/Documents/work/3.open-source/t3code`. X2 = exact2 main.

## Summary

| ID | Missing in exact2 | T3 feature blocked | Kind | Workaround in the clone |
|---|---|---|---|---|
| X1 | Embedded Chromium + CDP | Browser surface (preview browser, agent browser automation) | policy + build | none yet: [#100](https://github.com/ccheever/exact2/issues/100) closed upstream, not planned (2026-10-08); the Browser surface is built in the clone's own module on a `WKWebView` (path B, user decision 2026-10-08), with X1's path-B differences declared ("Browser surface: declared differences (X1 path B)"); part 1 (tabs, chrome, page states, security posture), part 2 (history and local servers, zoom, appearance, the device toolbar, the preview keys) and part 5 (the `previewAutomation` host, "Open links in", Mute) built, part 4 (profiles, clearing, cookie import, the Browser defaults group) in draft PR #354, part 3 in progress |
| X2 | Developer Tools for the app UI | View › Toggle Developer Tools | policy, decided ([#101](https://github.com/ccheever/exact2/issues/101), 2026-10-08): development-only Safari inspection of web views | none: View › Toggle Developer Tools is a permanent declared difference; the clone's own web views (terminal, rendered HTML, Mermaid) are inspectable in development builds and never in release builds (`T3WebInspection.swift`, `app-developer-tools`, [#326](https://github.com/ccheever/exact2/pull/326)); Exact's `iframe` web views get the same from main [#309](https://github.com/ccheever/exact2/pull/309) (merged to main on 2026-10-08 as `f2f0e7092`: done on main, round 7 adopts it and removes nothing) |
| X3 | App-settable root font size (`rem` base) (fixed on main #185, adopted) | Interface font size (12–20 px) | framework feature | none: `setRootFontSize` from app.contract `rootFont`; Contract lengths in `rem` (`font-size-map.json`) |
| X4 | Helper executables and large resource trees in the bundle (fixed on main #215: `host.macos.resources`) | Embedded local T3 server | build | fixed by main #215; the release archive ships as a native resource tree and is unpacked at first launch (U3; #215 re-signs Mach-O without entitlements) |
| X5 | Custom URL scheme delivered to the app (#104 closed; main #201 only journals an unheard launch URL); rest filed as [#268](https://github.com/ccheever/exact2/issues/268) (2026-10-08) | `t3code://` deep links, provider sign-in return | host | none; #268 narrowed (2026-10-08): delivery still needs a navigation root; approved, no fix in progress; no clone consumer left |
| X6 | Module shutdown time at quit (`destroy()` at quit fixed on main #200; no bounded hold); rest filed as [#269](https://github.com/ccheever/exact2/issues/269) (2026-10-08) | Stop the embedded server cleanly; remove the `t3 app` socket | host | fixed by main #200; the server stops in `destroy()` (and from `atexit` for the agent driver's `exit(0)`); T3Ssh's own `willTerminate` observer removed (adopt-main-fixes-r4); the `t3 app` control socket is closed and unlinked in `destroy()` and from `atexit` (app-activation, #254); SIGTERM through the orderly quit is done on main (#313, `a3d61c023`, 2026-10-08; round 7 brings it in and removes nothing); the one shared 5 s module hold waits for an owner ruling on #269 (LLP 1069.010 Q4) |
| X7 | ATS keys from `app.json` (fixed on main #173, adopted) | Rendered HTML / web views that load `http://` from host names | build | none: `app.json` `host.macos.appTransportSecurity` |
| X8 | Pointer input for native views in the agent | Agent tests of terminal, browser, device views | agent API | none: main #186, adopted (every pointer row driven by the agent or with real input) |
| X9 | Root component across files; resources in child components | Large apps (`app.contract` near 1,500 lines) | contract | split views, keep state in root; the root rewrite (`20261008-app-contract-root-rewrite`) is the remedy [#108](https://github.com/ccheever/exact2/issues/108) chose (2026-10-08, a different design); request ownership waits for LLP 1035.005.000 D5 |
| X10 | Text rendering parity (wrap points fixed on main #208); rest filed as [#266](https://github.com/ccheever/exact2/issues/266) (2026-10-08) | `text-wrap: balance`, placeholder colour, weight | kernel/host | none (visible difference); balance and placeholder colour: approved, no fix in progress: wait for main fix of #266; font smoothing deferred: a permanent declared difference (2026-10-08) |
| X11 | Shadow and blur parity (backdrop edges fixed on main #221; `saturate()` on main #232, which closed #225, not adopted yet; a backdrop beyond the parent's subtree is still missing on macOS, untracked) | Dialog/popover shadows, glass composer | host | flattened glass and opaque composer (#225 closed with main #232; the backdrop beyond the parent's subtree has no open upstream issue); real backdrops take #221 with no clone change |
| X12 | Textarea field sizing | Composer height with long chips | host | none: a permanent declared difference ([#130](https://github.com/ccheever/exact2/issues/130) keeps hatch-driven layout feedback deferred, 2026-10-08); this cell said "measured height", but no such code exists |
| X13 | Hover and key events during a pan | Sidebar row-action sweep | host | partial (r12); not reproduced on main `0365ad1a4` as described (2026-10-08): keys reach a `key` handler during a pan on macOS, and no host delivers hover to other nodes during a pan (the web host pans with pointer capture); local draft closed, not filed |
| X14 | Native replies survive a let-go or refused answer | Snapshot reads with `native.later` | runner | none: fixed for a watched topic's re-ask by main #183 and adopted (`T3ReadGate.swift` removed) |
| X15 | Key equivalents under a non-Latin input source ([#110](https://github.com/ccheever/exact2/issues/110), closed by main #168 for declared chords only) | Menu items the host does not own, the terminal's web view and the module's own key readers under Korean 2-Set | host | `R10Connect.swift` re-issues chords (kept) |
| X16 | `autocorrect="off"` also turns off smart quotes, dashes, text replacement ([#111](https://github.com/ccheever/exact2/issues/111), fixed by main #160) | none now | host | none: every textarea has `autocorrect="off"` (adopted) |
| X17 | Popover side areas and `position-try` flips ([#112](https://github.com/ccheever/exact2/issues/112): Charlie picked it on 2026-10-08 as the next core investment, CSS `position-area` plus flip fallbacks, for invoker popovers first) | Hover cards and tooltips that flip near edges | contract/host | `position-area` top/bottom; fixed placement for end-aligned and flipping layers; hover-opened popups placed and flipped by the window hover layer (`hover-layer.contract` `hoverFlip`, fix-hover-cards), which #112's first slice will not cover; invoker popovers wait for Charlie's next core investment, #112 (no PR yet) |
| X18 | SVG path `d` animation | Morphing icons | host | cross-fade |
| X19 | Timers/clock in data sources | Debounces, cooldowns (450 ms, 10 s) | policy, refused ([#124](https://github.com/ccheever/exact2/issues/124), 2026-10-08) | time passed as arguments, Contract tasks (gated tasks): a permanent declared difference |
| X20 | Rich-text editing with atomic inline nodes; caret/selection read and range replace; undo groups (field paste/copy/cut fixed on main #209); rest filed as [#275](https://github.com/ccheever/exact2/issues/275), [#276](https://github.com/ccheever/exact2/issues/276) (2026-10-08) | Composer (Tiptap): @ / $ menus, chips, history recall, large paste → file | framework feature (DEFERRED "no rich value type") | native NSTextView composer (`T3Composer*.swift`), its paste handling included; #276 closed not planned (2026-10-08): the chips are a permanent declared difference; #275 (plain fields only): approved, no fix in progress: waits for main fix of #275 |
| X21 | Two-way WebSocket for data modules | WebSocket RPC to the T3 server, device input | framework feature (LLP 1016.000: receive-only) | Swift transport (`T3Transport.swift`, `T3Fleet.swift`); waits for Charlie's next core investment, [#126](https://github.com/ccheever/exact2/issues/126) (2026-10-08; no PR; #227 is refusal consistency only): runner-owned streams with a mutation send, a different design from the web's `WebSocket` |
| X22 | Reactive layout facts (size/position, text width) and row visibility | Composer overlay reservation, menu placement, timeline minimap | framework feature | `t3-frame`, `t3-anchor`, `t3-turn` hooks; [#127](https://github.com/ccheever/exact2/issues/127) (2026-10-08): the measuring hooks are a permanent declared difference; anchors go to #112; visibility has no date |
| X23 | Scroll restore by key on a top-level list; `scroll-padding` on a plain scroller and `scroll-margin`; animated native scrollIntoView (plain-scroll anchoring fixed on main #210; `scroll-padding` on a virtualized list on main since `2faf6c190`, nothing of the clone's uses it); rest filed as [#277](https://github.com/ccheever/exact2/issues/277) (2026-10-08) | Per-thread scroll position, minimap/citation jumps, menu keyboard paging | framework feature | `R9Input.swift`, `T3TimelineTurns.swift`; #277 (2026-10-08): padding, smooth jumps and `scrollend`: approved, no fix in progress: wait for main fix of #277; per-thread restoration is the app's (a different design) |
| X24 | Hover re-hit-test under a still pointer after layout (fixed on main #174, adopted) | Row under the pointer after ⌘Z / list change | host | none (the `t3-rehover` hook is gone) |
| X25 | Keyboard: modifiers-held fact and capture-phase handler (#140; `keyup`, `KeyboardEvent.code` and `repeat` on main since #220) | ⌘ hold hints, ⌘Q hold, Send-button modifiers, key recorder, surface launcher | framework feature | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `T3Sidebar` (all need #140); #140 (2026-10-08): capture phase: approved, no fix in progress: waits for main fix of #140; no held-modifier fact, so held ⌘ is rebuilt on `key`/`keyup` + `hasFocus` (a different design) |
| X26 | App menu control: standard items (Paste as Text, Help), hide host Go/Develop, page zoom, a menu opened from the keyboard (#141, #235; context-menu submenus on main #223 and Edit ▸ Speech #226, adopted). Also, a context or button menu opened by `MenusMac` holds back every `native.later` call while it is open: it pops up inside `DispatchQueue.main.async`, where the main queue does not drain (reported in adopt-main-fixes-r5; filed 2026-10-08 as [#292](https://github.com/ccheever/exact2/issues/292), reproduced against main `0365ad1a4`'s source with a headless run-loop variant) | Electron application menu, context menus | host | `T3Menus.swift`, `R8KeysMenus.swift`; `T3Sidebar.swift` for keyboard-opened row menus only; the module's own menus open from a common-modes run-loop turn (`T3MenuTurn`); #141 (2026-10-08): Contract menu extensions: approved, no fix in progress: wait for main fix of #141; Develop stays in development builds (declared there); #235: the ContextMenu key is done on main (#314, `d236c36d5`; round 7: the sidebar's ContextMenu key moves to the host's context popover); Shift+F10 waits for an owner ruling on #235 and stays in `T3Sidebar.swift`; #292: partial in open main PR #327 (Charlie) |
| X27 | Window chrome: title-row height and traffic-light inset; full-screen fact (frame restore fixed on main #164, adopted; the rest still missing on `261dd4e10`, #113 closed); rest filed as [#267](https://github.com/ccheever/exact2/issues/267) (2026-10-08) | `hiddenInset` title bar, sidebar inset in full screen | host | `T3WindowChrome.swift`, `T3FullScreen.swift`; approved, no fix in progress: waits for main fix of #267 (Window Controls Overlay `env()` and `displayMode`, 2026-10-08) |
| X28 | Notification click → app action, Dock badge (#224; the window-focus fact is on main since #219, adopted) | Thread notifications | policy (DEFERRED refuses actions/badges) | `T3Notifications.swift` (badge, click, sounds); #224 (2026-10-08): the click waits for an owner ruling on #224 (a click-only DEFERRED exception); the badge is a permanent declared difference |
| X29 | `video` from `app:/` files; a PDF viewer element (bundled PDF iframe fixed on main #205); rest filed as [#273](https://github.com/ccheever/exact2/issues/273) (2026-10-08) | Composer video preview, PDF attachments, a PDF in the Files surface (right-panel-launcher-and-files) | framework feature | AVPlayerView, PDFView natively; `PDFView` is a permanent declared difference (#273, 2026-10-08) |
| X30 | TS can announce a topic / invalidate a resource; pixel readback; any-type file picker with bytes and image transcode | Wake reads, image accent colour, attachments | framework feature | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach`; #116 (2026-10-08): readback and transcode stay native, a permanent declared difference; the any-type file input waits for an owner ruling on #116 (a DEFERRED waiver); the Inspect lookup is X68 ([#321](https://github.com/ccheever/exact2/issues/321)), and U18 waits for an owner ruling on #321 (no design selected); the topic announce has no upstream issue |
| X31 | Hold the first window until the app says it is ready ([#117](https://github.com/ccheever/exact2/issues/117), closed upstream, not planned, 2026-10-08) | The reference opens its window only once the embedded server is ready | host | the first window shows the connecting state until the primary connects (`20261005-local-primary-environment`): a permanent declared difference (#117 closed: "Show an honest connecting state, then the result"; U5 final) |
| X32 | A section header pinned over several rows of a virtualized list (sticky inside a `scroll` and inside one row works since LLP 1083, e.g. `pages-pr-summary.contract:282`) ([#131](https://github.com/ccheever/exact2/issues/131), deferred 2026-10-08) | Pinned diff file headers (diff-review-engine, pr-code-tab) | kernel/contract | none: a declared difference until #131 lands on main (user decision, 2026-10-08); #131 waits for an owner ruling on a list-lifetime design; a file's header scrolls away with its rows |
| X45 | An app process relaunch ([#122](https://github.com/ccheever/exact2/issues/122), closed by main #170, which only moves `reload()`'s log; no relaunch); rest filed as [#271](https://github.com/ccheever/exact2/issues/271) (2026-10-08) | A Local environment change relaunches the app (decision U4) | host | `applyLocalSetting` stops or starts the embedded server in place and reconnects; the window stays (`this-machine.ts`); Network access and Tailscale HTTPS restart it in place with the new envelope (`connections-network.ts`); approved, no fix in progress: waits for main fix of #271, after #269's hold ruling (2026-10-08) |
| X35 | A password `input` whose value stays out of every agent output ([#134](https://github.com/ccheever/exact2/issues/134), closed by main #167: the tree, `layout` and `type` reply mask the field; the app's state and data module stay outside that) | SSH Password Required dialog | host + agent | native `t3-ssh-password` secure field (`T3SshAuth.swift`, kept); the module reads it on Continue |
| X43 | `aria-checked="mixed"` on a switch or checkbox ([#120](https://github.com/ccheever/exact2/issues/120), closed, not planned) | Scoped switches whose targets disagree (D15) | contract/host | the app draws the mixed thumb; the switch reports unchecked (kept) |
| X46 | A build step that makes app assets before the Apple bundle copies `assets/` | The terminal page (`terminal-host/build.mjs` output) | build | run `bun terminal-host/build.mjs` (app.json `commands.terminal`) before the bundle build; without it the terminal stays blank and its status names the load error |
| X47 | A focus ring on a custom pressable box ([#179](https://github.com/ccheever/exact2/issues/179), filed 2026-10-07) | Keyboard focus on the clone's custom buttons, rows and toggles | host | none: fixed by main #189 (in the branch since adopt-main-fixes-r3); the ring was seen under real Tab presses (`20261007-real-input-checks`) |
| X50 | A native module's data directory under `--storage` ([#284](https://github.com/ccheever/exact2/issues/284), filed 2026-10-08): in agent mode `ExactKit/NativeModule.swift:526` names it `$TMPDIR/exact-agent-<pid>-<runtime>`, per process, whatever `--storage` names, so a module's files (the clone's `t3-code.json`) do not survive an agent relaunch while `app:/data` does | Agent relaunch checks of the pull request panel's kept detail (pr-conversation-and-refresh) | host/agent | the kept file was replayed through the relaunch path offline; a normal launch keeps it in the app's data directory. Repro: `agent.mjs macos --storage s` twice; the second run's `$TMPDIR/exact-agent-<pid>-2/data/t3-code.json` starts empty; **fixed in open main PR #327** (Charlie, full; decided 2026-10-08): T3 waits and resumes in the main-adoption round after it merges; then check `T3Storage.dataRoot(agent:)` (`T3Protocol.swift`) against the named store's roots |
| X49 | A progress value for assistive technology: no `progress` element and no `aria-valuenow`/`aria-valuetext` ([#279](https://github.com/ccheever/exact2/issues/279), filed 2026-10-08; main `d82c12252` added an indeterminate `progress` only and refuses `value`/`max`) | The Antigravity runtime download bar (`<progress aria-label="Antigravity download">`) | contract/host | a drawn track and fill with `role="progressbar"` and the percentage as `aria-description`; the status text carries the byte counts; approved, no fix in progress: waits for main fix of #279 (decided 2026-10-08) |
| X51 | A click inside an open popover also reaches the page under it, on macOS ([#281](https://github.com/ccheever/exact2/issues/281), filed 2026-10-08; reproduced in a one-file app on main `0365ad1a4`) | The theme editor's colour popover: a click on the plane pressed the Dark toggle or focused the name field under it | host | the popover box takes `press` and `retainFocus=true` (`theme-color-picker.contract`); **fixed in open main PR #327** (Charlie, full; decided 2026-10-08): T3 waits and resumes in the main-adoption round after it merges; then `press` + `retainFocus` leave the popover |
| X52 | On macOS, `input type="date"`, `type="time"` and `select` are no Tab stops (the web's are; [#280](https://github.com/ccheever/exact2/issues/280), filed 2026-10-08) | Custom snooze: Time and Unit are skipped by Tab (its Date is a button with a calendar popover since `20261009-shell-sidebar-palette-keys`, as the reference's is) | host | none; the dialog's other stops follow the reference order; **fixed in open main PR #327** (Charlie, full; decided 2026-10-08): T3 waits and resumes in the main-adoption round after it merges; then re-drive the Custom snooze Tab cycle (`sidebar-overlays.contract`) |
| X53 | A modal an app opens from state: `showModal(id)` is carried by the terminal host only (macOS "unknown command", the web refuses it) and `aria-modal` keeps no Tab inside ([#282](https://github.com/ccheever/exact2/issues/282), filed 2026-10-08) | Every T3 dialog (Base UI's focus trap, initial and final focus) | runner/host | per-dialog `key` traps, `autofocus` and `focus()` in AppConfirm, SettingsConfirm, Custom snooze and Add Environment (`20261008-dialog-shortcut-focus`; Custom snooze opened from the title menu gives the focus back to the title, `20261007-title-custom-snooze`); other dialogs rely on the inert page behind them; **partial in open main PR #327** (Charlie; decided 2026-10-08): `showModal(id)`/`close(id)` on macOS and the JS/wasm web hosts; the dialog-close event and other hosts stay open (#324, closed unmerged, had the close event); T3 waits |
| X54 | An ancestor cannot hear the focus enter its subtree: no `focusin`/`focusout`, no `:focus-within` ([#283](https://github.com/ccheever/exact2/issues/283), filed 2026-10-08) | A long pull request comment opening when Tab reaches a link it holds below the fold; a remark's pencil showing while the remark has the focus | contract/host | none: "Show full comment" is a Tab stop before the held body, and the pencil shows while it is itself focused; approved, no fix in progress: waits for main fix of #283 (decided 2026-10-08) |
| X55 | macOS exposes no `progressbar` role on a drawn box, no `status` or `alert`, and no modal `dialog` to accessibility; the `progress` element and an explicit `aria-live` work ([#278](https://github.com/ccheever/exact2/issues/278), filed 2026-10-08) | The first-launch view's progress bar, stage line, error and dialog (portable-app-download, dropped 2026-10-08) | host | none; approved, no fix in progress: waits for main fix of #278 (decided 2026-10-08) |
| X56 | A mouse click whose mouse-down focuses a `button` that restyles itself from its `focus` handler runs no `press`, on macOS (not reproduced on main `0365ad1a4` in a one-file app, six variants; local draft closed 2026-10-08, not filed) | The Usage page's pooled segments: a click on an unfocused segment did not open its popover (Enter did) | host | the segment draws no ring of its own; the host's focus ring (X47, main #189) stands in |
| X57 | A line wider than its box keeps its `text-align` on macOS: under `center` (a button's UA value, inherited by its text; `right` takes the same offset, not driven) it starts left of the box, so its start is cut and an ellipsis sits early; CSS Text 3 §7.1 and the web host start-align it ([#291](https://github.com/ccheever/exact2/issues/291), filed 2026-10-08; reproduced on main `0365ad1a4` in a one-file app) | The Pull Requests row's title after a rename or on a long first read ("cribe the vowel count…") | host | `text-align="left"` where the reference says `text-left` (the row, the timeline group toggle, the copyable branch and command) and on the base freshness mark and the filter submenu's value (`20261008-pr-list-title-clip`), and on the right panel's tab title, which a browser tab fills with a page title ("op-up test: sign…" before, `20261005-browser-surface` real input), and on the command palette's option rows (a project pick's path, "s/daehyeonmun/orca/…" before, `20261009-shell-sidebar-palette-keys`); other clone buttons still inherit the centre (listed in the issue); attempt withdrawn, no fix in progress: #327 withdrew its attempt and #291 stays open (decided 2026-10-08) |
| X58 | A wheel-scrolled `scroll` keeping its offset after the window is focused again: on macOS the Pull Requests list goes back to its top when the window's return re-renders it (not reproduced on main `0365ad1a4` in a one-file app, four variants; local draft closed 2026-10-08, not filed) | The Pull Requests list read again on focus (pr-list-live-refresh): a reader who had scrolled down is put back at the top; a re-read without a focus change and the minute tick keep the place | host | none |
| X59 | A `line-clamp` text mounted after launch paints its last kept line cut at a word with no ellipsis until a restyle, on macOS: the first raster skips the clamp's last line (layout is right; the agent's default `screenshot` hides it, `screenshot … window` shows it) ([#300](https://github.com/ccheever/exact2/issues/300), filed 2026-10-08; reproduced on main `febb2c5fb` in a one-file app) | A collapsed Markdown table cell right after "Collapse table cells" (visual-parity-followup) | host | none: a later restyle (a colour-scheme change) draws the ellipsis; base `07dcef1ab` shows the same; done on main (#305, `9314e7a81`): round 7 adopts it |
| X60 | `input type="number"` on macOS is a plain text field: ArrowUp/ArrowDown do not step within `min`/`max`, letters are accepted, and the accessibility role is `textbox` (the web steps, refuses them and says `spinbutton`) ([#301](https://github.com/ccheever/exact2/issues/301), filed 2026-10-08; reproduced on main `febb2c5fb` in a one-file app) | The Tailscale HTTPS port field (provisional-decisions-parity) | host | the dialog's key handler steps the value itself (`tsStep`, clamped to 1…65535); a typed letter still shows, with the error under it; approved, no fix in progress: waits for main fix of #301 (decided 2026-10-08) |
| X61 | An app cannot remove the focus ring Exact draws on a focused bare `input` or `textarea`: Contract has no `outline`, and `appearance="none"` no longer opts out since main `5b2b77339` ([#302](https://github.com/ccheever/exact2/issues/302), filed 2026-10-08 as a feature request for `outline: none` on `input`/`textarea`; reproduced on main `febb2c5fb`) | The composer and the Appearance prompt preview, which T3 Code draws with `outline-none` | contract/host | none: the ring shows on the composer (as on the base) and now on the prompt preview (adopt-main-fixes-r6); approved, no fix in progress: waits for main fix of #302 (decided 2026-10-08) |
| X62 | macOS hover ignores a node's overflowing descendants: a box outside its parent's box gets no tracking (`.inVisibleRect` is clipped by every ancestor), moving into an absolutely positioned child sends the parent a leave, and overlapping hover nodes hand the hover back and forth each move; the web's `pointerenter`/`pointerleave` count every descendant (reproduced with a one-file app and a real pointer on the feature branch's framework, unchanged on main `9314e7a81`; [#322](https://github.com/ccheever/exact2/issues/322), fixed in open main PR [#327](https://github.com/ccheever/exact2/pull/327) (Charlie, full); T3 waits) | Hover cards: the base branch's freshness card (#262) and the Usage segment's popover (#263) closed as the pointer moved into them; "N scopes" the same | host | the window's hover layer (`hover-layer.contract`, fix-hover-cards): hover cards, and tooltips inside clipping scroll areas, drawn at the window level (the Usage popover in its page's scroll content), with Base UI's closeDelay on the root's hover clock so the pointer can cross; hover nodes inside a card report to it; a press outside the Usage page drops a hover-opened popover, as an enter can still reach a segment the pointer is not on. Once #327 is adopted only this X62 part of the layer's reason goes: the layer stays for the clipping (#251, #248) and as the reference's portal |
| X63 | Edit › Undo, Edit › Redo, ⌘Z and ⇧⌘Z do nothing in a plain `textarea` on macOS: `TextArea` keeps its own undo manager and does not answer `undo:`/`redo:`, so the Edit items' action reaches `NSWindow`, which undoes the window's manager; only a Markdown editor routes ⌘Z itself ([#315](https://github.com/ccheever/exact2/issues/315), filed 2026-10-08, reproduced on main `b896050d7`) | The composer and the Appearance prompt preview (#298 bug 3) | host | `R8KeysMenus` (the clone's own Edit › Undo, which lays the reference's menu bar over the host's) undoes and redoes the focused text view's own history (`20261008-fix-misc-batch`); **fixed in open main PR #327** (Charlie, full): T3 waits and resumes in the main-adoption round after it merges; then #306's Edit › Undo routing in `R8KeysMenus.swift` goes |
| X64 | A wrapped paragraph drawn from a text raster keeps painting its old raster after an update shrinks it below the raster size (16,384 device pixels) on macOS; the tree, `layout` and the capture path have the new text ([#316](https://github.com/ccheever/exact2/issues/316), filed 2026-10-08; reproduced in a one-file app on main `475043d20`, after #305) | The Settings › Providers list row stayed "Not authenticated" after Reconnect (#298 bug 19); the editor's status detail ran under the name field after Disconnect | host | the list row's status and the editor's status line are keyed by their status, so a new status is a new node (`20261008-fix-provider-auth-state`); **fixed in open main PR #327** (Charlie, full): T3 waits and resumes in the main-adoption round after it merges; then #312's status-keyed redraw goes |
| X65 | A press on a `scroll` node's empty area (its port's ground, beyond the content) reaches no node on macOS, so no `pointerdown` runs on the `scroll` node or an ancestor; on the web the scroll element takes it and it bubbles ([#317](https://github.com/ccheever/exact2/issues/317), filed 2026-10-08, reproduced on main `b896050d7`) | Light dismiss of a pinned usage-segment popover by a press on the Usage page's ground (`20261008-popover-escape-parity`) | host | a full-height column (`usage-ground`) under the Usage page's scroll content; attempt withdrawn, no fix in progress: #327 withdrew its attempt and #317 stays open |
| X66 | A popover cannot be shown or hidden from an action (`showPopover`/`hidePopover`/`togglePopover` are not host commands) and has no `toggle` event; the only way today is an invisible `popovertargetaction` invoker laid over the trigger and pressed by a scoped `aria-keyshortcuts` (the popover sibling of #282; filed as #319, which waits for an owner ruling on its design; reproduced in two one-file apps on main `9314e7a81`) | Base UI Menu's ↓/↑ on a closed trigger (open and focus the first or last item); state that follows a menu's open state (the sidebar row's hover actions kept shown while its snooze menu is open, #298 bug 4); #290's light dismiss and #307's in-scroll Usage card | contract/host | `KeyMenuOpen` (`menu-keys.contract`): two invisible invokers over each popover menu's trigger, armed by its focus; a keyboard opening's count whose sign is the end (`kmBump`); menus the data module mounts read the end their trigger's keys set; the snooze row pins while the focus is in the menu or the pointer is on a menu row (`20261008-fix-keyboard-focus`); a combobox's Return in its search field (a plain Enter shortcut is not heard in a text field) empties its popover until the trigger opens it again: the details branch picker (`r4-git.contract`) and the Diff comparison target (`diff.contract` `DiffBasePicker`, diff-panel-parity) |


## Current state on the pin (2026-10-06, exact2 `c12832e82`)

Each open item was reproduced for its upstream issue on exact2 `4c893fef6`, which adds only a QUEUE line and a test wait (`22903cc75`) over the pin, so each reproduction holds on the pin. "fix built" means a fix exists on a local `daehyeon/fw-*` branch, not on main. Every workaround below is still in the clone.

| ID | Issue | Current state on the pin | Workaround kept |
|---|---|---|---|
| X3 | [#102](https://github.com/ccheever/exact2/issues/102) (+ #136, #137) | Fixed on main (#185 `setRootFontSize`; #176 the web JS target keeps `rem`; #159 refuses a string on a number-only row), adopted 2026-10-07 (interface-font-size): the clone sets the root font size from the setting and sizes its Contract lengths in `rem` where the reference does. Contract `calc()` takes percent ± px only, so a length that adds a layout px value to a rem one multiplies the root size in (the top bars). | none |
| X4 | [#103](https://github.com/ccheever/exact2/issues/103) | Closed by main #215 (adopt-main-fixes-r4 checked `463acda68`): `app.json` `host.macos.resources: [{from, to}]` copies a tree under `Contents` with modes, internal symlinks, any names and files above 64 MiB, hashed into the binary receipt and signed inside-out. Developer ID signing of the tree is unverified upstream. Merged in embedded-server-runtime (2026-10-07). | archive in `server-runtime/` → `Resources/t3-runtime`, unpacked into `<T3 home>/runtime/versions` |
| X5 | [#104](https://github.com/ccheever/exact2/issues/104) | Closed by main #201, which journals a launch URL that no navigation root hears; a scheme URL still reaches only a navigation root's `navigate` (checked on `463acda68`). Rest reproduced on main `0365ad1a4` and filed as [#268](https://github.com/ccheever/exact2/issues/268) (2026-10-08). | none |
| X6 | [#105](https://github.com/ccheever/exact2/issues/105) | Closed by main #200: `applicationWillTerminate` destroys every session, so `destroy()` runs at ⌘Q, an Apple Event quit and last-window close. No bounded hold at quit (LLP 1069.010 Q4), no SIGTERM handling. Merged in embedded-server-runtime (2026-10-07). Rest reproduced on main `0365ad1a4` and filed as [#269](https://github.com/ccheever/exact2/issues/269) (2026-10-08). | T3Ssh's `willTerminateNotification` observer removed (adopt-main-fixes-r4; live: both SSH tunnels end at ⌘W, Apple Event quit and ⌘Q, as with the observer before); the embedded server's pid file and next-launch reaper (crash only); `atexit` stop for the agent driver's `exit(0)` |
| X7 | [#106](https://github.com/ccheever/exact2/issues/106) (+ #135) | Fixed on main (#173), adopted 2026-10-07 (adopt-main-fixes-shell): `app.json` sets `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent`, and the bundle's rendered HTML loads `http://` from a named host. #135 (an `http:` sub-resource of the app's own `assets/` page) is fixed by main #184 (a bundled page is served at `http://exact.localhost`; loopback `http:` loads, a failed load is logged); nothing to adopt: no clone page is served from `assets/`. | none |
| X8 | [#107](https://github.com/ccheever/exact2/issues/107) | Fixed by main #186 (merged 2026-10-07, adopt-main-fixes-r3): `tap … auxclick`, `clicks 1–3`, `wheel … at x y`, `modifiers` held through a contact or drag, and every agent mouse and wheel event goes through `NSApplication.sendEvent`, so module monitors see it. | partly adopted: the agent rows (terminal, device, panel, diff, tab middle click) are driven; with real input (`20261007-real-input-checks`) middle click, cursor shapes, drags out of the window and terminal double-click, right-click and drag select pass (`20261007-terminal-real-drag`); adopted |
| X9 | [#108](https://github.com/ccheever/exact2/issues/108) | Open upstream for D5. Decided 2026-10-08 (a different design): reduce the root with child state and actions now (the root rewrite); request ownership waits for LLP 1035.005.000 D5. The local A1 branch is superseded, not pursued. `app.contract` holds every resource. | split views, root keeps state |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Closed by main #208: macOS paragraphs break at Chrome's opportunities (no break after `/`). Still missing on `463acda68`: `text-wrap: balance`, a placeholder colour row, `-webkit-font-smoothing`. Rest reproduced on main `0365ad1a4` and filed as [#266](https://github.com/ccheever/exact2/issues/266) (2026-10-08). | none (visible difference) |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | Closed by main #221 (adopt-main-fixes-r5 merged `261dd4e10`): where macOS blurs a backdrop, the chain mirrors the box at its edges as Chrome does. Still missing: a backdrop beyond the parent's subtree (untracked: [#225](https://github.com/ccheever/exact2/issues/225) closed with main #232, which brings `saturate()` and leaves cross-parent sampling open). | flattened glass (composer card, popovers, toasts, the confirm dialog); the composer drawer and dialog backdrops are real blurs and take #221 unchanged |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | Open upstream; deferred 2026-10-08 (hatch-driven layout feedback stays out). | none: a permanent declared difference |
| X13 | not filed | Not reproduced on main `0365ad1a4` as described (2026-10-08): an Escape during a pan reaches the column's `key` handler on macOS; hover during a pan reaches no other node on macOS or the web host (which holds pointer capture). Left, not filed: macOS keeps the origin hovered through the pan; neither host re-hovers at release until the pointer moves. | partial (r12 Escape) |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | Fixed by main #183 (merged 2026-10-07, adopt-main-fixes-r3): a watched topic announced while the resource's request is in flight lets that reply land, then asks once more. A re-ask for new arguments or a `refresh` still forgets the old request (LLP 1016 D5). | none: `T3ReadGate.swift` and the reader tags are removed (adopted); `readDetail` reads per answer, which the different-arguments rule still needs (`r6-pr-actions.ts`) |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | Closed by main #168 (2026-10-07 merge): `aria-keyshortcuts` and the host's command menu items match the physical key. Not covered: menu items without the host's shortcut target (Copy, Paste, Undo, Quit, Close Window, Reload, Paste as Text), the terminal's web view, the module's own key monitors. | `R10Connect.swift` re-issues chords (kept; adopt-main-fixes-input) |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | Fixed by main #160. | none: the hook's switch-off is removed; every textarea has `autocorrect="off"` (adopt-main-fixes-input) |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Partly fixed: `position-area` top, bottom and center work (`2c6b551ba`); no `position-try` flip, no `span-left` or side areas. Charlie picked #112 on 2026-10-08 as the next core investment: CSS `position-area` plus flip fallbacks, for invoker popovers first. | end-aligned and flipping menus keep fixed placement; hover-opened popups (tooltips, hover cards) keep the window hover layer's own placement and flip (`hoverFlip`, fix-hover-cards), since #112's first slice covers invoker popovers only; the rest waits for Charlie's next core investment, #112 |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | Fixed by main #188 (SVG `d` transitions on macOS). Partly adopted (adopt-main-fixes-r3): five icons morph as T3 Code's morphicons does (per-subpath turn and drift on the snappy spring, `d` between same-structure polylines). | the code-block and welcome copy buttons still cross-fade: their return runs as keyframes (X19) and keyframed `d` is refused |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Open upstream until #228's parity slice, then closes not planned. Decided 2026-10-08: module timers stay refused. The local branch is declined, not pursued. | time as arguments, Contract tasks: a permanent declared difference |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | Closed by main #209: a field's ⌘V/⌘C/⌘X fire `paste`/`copy`/`cut` first, cancelable. Not on main: `selectionchange` on a field, `setRangeText`, `beforeinput`, undo groups, atomic ranges (phase-1 fix built, not merged). Rest reproduced on main `0365ad1a4` and filed as [#275](https://github.com/ccheever/exact2/issues/275), [#276](https://github.com/ccheever/exact2/issues/276) (2026-10-08). | native NSTextView composer, its paste handling included (a Contract `paste` reaches only a Contract field) |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Open; the next core investment (2026-10-08): runner-owned bidirectional streams with a mutation send. The local branch is not that API, superseded, not pursued. | Swift transport, until the main fix of #126 |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Open, narrowed (2026-10-08): reactive geometry and container queries deferred; anchors go to #112; visibility has no date. The local branch is superseded by main's element `resize` (`5949b2b64`), not pursued. | `t3-frame`, `t3-anchor`, `t3-turn` hooks: a permanent declared difference |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Closed by main #210: macOS and iOS anchor a plain `scroll` box (X23d). Not on main: restore by key, `scroll-padding`/`scroll-margin`, smooth native `scrollIntoView` and `scrollend`, `overflow-anchor: none`. Rest reproduced on main `0365ad1a4` and filed as [#277](https://github.com/ccheever/exact2/issues/277) (2026-10-08). On `e200397ec` a virtualized `list` takes main-axis padding and `scroll-padding` (`2faf6c190`); a plain `scroll` refuses `scroll-padding` (`lower-scroll-padding`). | `R9Input.swift`, `T3TimelineTurns.swift` (no X23d workaround existed) |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | Fixed on main (#174), adopted 2026-10-07: the host hit-tests a resting pointer after layout and scrolling. | none (`t3-rehover`, R10Connect's passes and the thread lists' `data-frame` removed) |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Open for a capture-phase handler and held-modifier state. Main #220 (in `261dd4e10`): `keyup` handlers, `KeyboardEvent.code` and `.repeat` on every host; a ⌘ chord commits a composition first. Modifiers on keydown (`8a0afbeab`): Shift+F10 on draft rows and right-panel tabs is Contract. | `T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `T3Sidebar` (held ⌘, window capture); the tab rename field's Escape in `RightPanelTabsInput.swift`; nothing removable with #220 alone (adopt-main-fixes-r5) |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | Open for the menu bar. Main #223: a menu row naming another menu popover is a submenu (`NSMenuItem.submenu`, nested popover under the agent). Main #226: Edit ▸ Speech, paste variants after Paste, Edit's app commands after Select All. A context popover opens only from a right-click ([#235](https://github.com/ccheever/exact2/issues/235)). | `T3Menus.swift`, `R8KeysMenus.swift` (menu bar; Edit's app commands hidden); the sidebar's right-click menus are context popovers (adopt-main-fixes-r5); `T3Sidebar.swift` for keyboard-opened menus, `T3ContextMenu.swift` for the module's flat menus and the Files tree's Open with ▸ |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | Partly fixed: main #164 restores the frame autosave after the final style (adopted 2026-10-07: `R8PointerWindowFrame.swift` deleted). #113 is closed. Still missing on `261dd4e10` (adopt-main-fixes-r5): no `env(titlebar-area-*)` or title-bar setting, no full-screen fact (`exactPage()` has `hasFocus` since #219, not full screen). No open issue tracks the rest. Rest reproduced on main `0365ad1a4` and filed as [#267](https://github.com/ccheever/exact2/issues/267) (2026-10-08). | `T3WindowChrome.swift` (empty unified toolbar), `T3FullScreen.swift` (full-screen fact through `t3.status`) |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Closed by main #219: `exactPage().hasFocus` is `document.hasFocus()` (macOS: active app and key window), `prefer has-focus` under the agent. Notification actions and badges continue in [#224](https://github.com/ccheever/exact2/issues/224) (policy). | `T3Notifications.swift` (badge, click, sounds); the focus reading is the page's (adopt-main-fixes-r5) |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Closed by main #205: a bundled PDF `iframe` shows WebKit's PDF view. Not on main: an `app:/` iframe and a PDF element with fit-to-width. Rest reproduced on main `0365ad1a4` and filed as [#273](https://github.com/ccheever/exact2/issues/273) (2026-10-08). | AVPlayerView, PDFView (WebKit's PDF view ignores `#toolbar=0&view=FitH` and has its own white surround, where T3 Code shows the page on Chromium's #282828 surface) |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Open (policy), narrowed 2026-10-08: codecs and readback deferred; only an any-type file input is proposed. | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach` |
| X31 | [#117](https://github.com/ccheever/exact2/issues/117) | Closed upstream, not planned (2026-10-08). The host creates the window at launch. | the connecting state in the first window: a permanent declared difference (U5 final) |
| X45 | [#122](https://github.com/ccheever/exact2/issues/122) | Closed after main #170, which moves `reload()`'s log to stderr; exact2 has no process relaunch. Rest reproduced on main `0365ad1a4` and filed as [#271](https://github.com/ccheever/exact2/issues/271) (2026-10-08). | `applyLocalSetting` restarts the embedded server in place |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Closed; main #171 fixed part 1 (a button press keeps the selection). Parts 2 and 3 are not on main: no selection end rectangle, no `clearSelection()`. Re-checked on `cff90b364`: still missing. Rest reproduced on main `0365ad1a4` and filed as [#274](https://github.com/ccheever/exact2/issues/274) (2026-10-08). | none to remove: Cite's `retainFocus` is the reference's `onPointerDown` `preventDefault()`; after citing the selection stays (the reference clears it) |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | Closed; main #178 fixed the macOS agent's hover on inline runs. `frame()` of an inline run and inline runs in agent `layout` are still missing (re-checked on `cff90b364`). Rest reproduced on main `0365ad1a4` and filed as [#272](https://github.com/ccheever/exact2/issues/272) (2026-10-08). | none (no inline-link hover card is built) |
| X36 | [#118](https://github.com/ccheever/exact2/issues/118) | Fixed by main #204: `Intl.Locale` and `getWeekInfo()` in every Hermes runtime, as Chrome answers them. | none: adopted (adopt-main-fixes-r4); `resolveWeekStartsOn` answers on macOS |
| X37 | [#119](https://github.com/ccheever/exact2/issues/119) | Closed by main #199: `exact release` signs nested Mach-O files and bundles inside-out. Not on main: app-declared entitlements, a pre-seal hook. Rest reproduced on main `0365ad1a4` and filed as [#270](https://github.com/ccheever/exact2/issues/270) (2026-10-08). | none (portable-app-download dropped 2026-10-08) |
| X35 | [#134](https://github.com/ccheever/exact2/issues/134) | Closed by main #167: a password field's value is masked in `tree`, `layout` and the `type` reply; `autocomplete` sets the AutoFill content type. The app's state slots and data module stay outside that (main's docs: "the app's own"). | native `t3-ssh-password` field (kept) |
| X43 | [#120](https://github.com/ccheever/exact2/issues/120) | Closed, not planned. | the app's mixed thumb; the switch reports unchecked |
| X47 | [#179](https://github.com/ccheever/exact2/issues/179) | Fixed by main #189, which is in this branch since the `cff90b364` merge (adopt-main-fixes-r3); no clone workaround to remove; real Tab presses draw the ring on the custom Settings nav buttons, which the base `4f523ef5c` does not (2026-10-07, `20261007-real-input-checks`). | none |

X1 ([#100](https://github.com/ccheever/exact2/issues/100)) was closed upstream as not planned on 2026-10-08: the Browser surface is built in the clone's own module on a `WKWebView` (user decision, path B). X2 ([#101](https://github.com/ccheever/exact2/issues/101)) was narrowed on 2026-10-08: View › Toggle Developer Tools stays absent (a declared difference), and development-only inspection of web views is allowed: the clone's own in `app-developer-tools` ([#326](https://github.com/ccheever/exact2/pull/326)), Exact's `iframe` web views in main [#309](https://github.com/ccheever/exact2/pull/309) (merged to main on 2026-10-08 as `f2f0e7092`: done on main, round 7 adopts it).

---

## X1. Chromium and CDP (Browser surface)

**T3 feature.** The desktop Browser surface: tabs, URL bar, back/forward, Annotate (element picker), screenshot and recording, picture-in-picture window, device toolbar (17 presets), zoom 25–500 %, profiles, cookie import (Chromium/Safari/Firefox), per-page DevTools, and the agent automation host (`previewAutomation.*`: snapshot, click, type, press, scroll, evaluate, waitFor, recording). Terminal links and the "Open links in" setting also target it.

**Reviewed.**
- REF `apps/desktop/src/preview/Manager.ts` (5,245 lines): Electron `<webview>` plus `webContents.debugger` CDP. Domains used: Runtime, Network, Log, Page (`createIsolatedWorld`, `screencastFrame`), Emulation, Input (`dispatchMouseEvent`, `dispatchKeyEvent`), Accessibility (`getFullAXTree`), DOM, Target.
- REF `PickPreload.ts` (uses `react-grab`), `PlaywrightInjectedRuntime.ts` (uses `playwright-core`), `BrowserImport/*`, `preview-pip-preload.ts`.
- X2 `rules/DEFERRED.md:380-384` and LLP 1020 §5: the built-in `iframe` refuses `top` topology ("never, absent a product that is a browser shell"), navigation policy, popups, the controller ops, and permissions.
- X2 `host/apple/build.mjs:1021-1045`: module dependencies link only as static libraries or framework slices from `modules/apple/*.xcframework` ("a dynamic library is not linked into the module").
- X2 `host/apple/build.mjs:1229-1244`: the macOS bundle copies only exact's binaries and `assets/`. No `Contents/Frameworks` for third-party frameworks, and no helper apps.

**Current state.** Part 1 of `20261005-browser-surface` (2026-10-09): Browser tabs over a `WKWebView` in the clone's module (`t3-browser`, `T3Browser*.swift`), the chrome row, the page states, the security posture and Safari inspection in development builds; each path-B row it builds is declared in "Browser surface: declared differences (X1 path B)" below. Part 2 (`20261005-browser-surface-navigation`) adds the empty state's Recently used and Local servers, the history store, target resolution, the unreachable page's Details, zoom (WebKit's `pageZoom`), the appearance pages are told to prefer (the web view's `appearance`), the device toolbar (a web view sized to the viewport and drawn scaled to fit) and the preview keys (a key monitor in the module), with its own rows below. Part 3 (annotate, capture and picture in picture) is its own record. Part 4 built profiles, Clear cookies / Clear cache, Settings › Integrations › Browser (profiles and the defaults group) and the cookie import wizard (`20261005-browser-surface-profiles`). Part 5 (2026-10-09) built the `previewAutomation` host (all 14 `preview_*` tools, by injected script and native input), "Open links in" and Mute; its rows are declared below.

**Why it does not work.**
1. Chromium (CEF) needs a dynamic framework plus GPU/renderer/plugin helper apps inside the bundle. The exact2 Apple build cannot embed either.
2. CDP exists only in Chromium. WebKit has no equivalent for Network events, trusted input dispatch, the full accessibility tree, or screencast.
3. The `iframe` cannot be a browser shell (LLP 1020 §5).
4. Recording and PiP need screen capture. ScreenCaptureKit asks for Screen Recording even for the app's own windows (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:814`).

**Support needed (choose one).**
- A. **Chromium path:** let an app module ship a dynamic framework and helper app bundles (copy, sign, `@rpath`), and accept a "browser shell" product in DEFERRED. Large framework work.
- B. **WebKit path (no exact2 change required):** a `t3-browser` native view hosting WKWebView. Deviations from REF: WebKit rendering; Safari Web Inspector instead of DevTools (cannot open from code); console/network capture by injected script (no subresource status); synthetic or NSEvent input instead of CDP input; ARIA snapshot by vendored Playwright script; recording by `takeSnapshot` polling or ScreenCaptureKit with a permission prompt. Helpful exact2 addition: X8.

**Decided (2026-10-08).** #100 closed upstream as not planned ("Keep a browser shell outside core … Do not add Chromium/CEF"), so A is refused; the user chose B. The Browser surface is clone-side work (`20261005-browser-surface`), after the root rewrite.

## X2. Developer Tools for the app UI

**T3 feature.** View › Toggle Developer Tools opens Chromium DevTools for the whole app UI (REF `apps/desktop/src/window/DesktopApplicationMenu.ts:229-251`).

**Reviewed.** X2 `rules/DEFERRED.md:615`: "no devtools UI". The agent API has `tree`, `state`, `layout`, `logs`, `perf` (DEFERRED Agent API).

**Current state.** The menu item is absent. The View menu has Reload, Force Reload and zoom (`R8KeysMenus.swift`, `T3Menus.swift:78-89`). Since `app-developer-tools` ([#326](https://github.com/ccheever/exact2/pull/326)) every web view the module creates (the terminal, the rendered-HTML preview, the offscreen Mermaid renderer) is `isInspectable` in a development build, so Safari's Develop menu lists it; a release build's are not: the packaged build (`distribution.json`), a production-trust bake or a distributed bundle (its receipt), the line main [#309](https://github.com/ccheever/exact2/pull/309) draws for Exact's `iframe` arm (`T3WebInspection.swift`).

**Why it does not work.** The clone UI is drawn natively. There is no DOM for an inspector. DEFERRED refuses a devtools UI.

**Support needed / options.** (1) Keep it absent and document it. (2) Show the item disabled. (3) Mark the app's own WKWebViews (terminal, HTML preview) `isInspectable` in development builds so Safari can inspect them. A real equivalent needs a DEFERRED change.

**Decided (2026-10-08).** #101 allows (3) in development builds only and refuses an inspector of the app UI; (1) is the permanent declared difference. `app-developer-tools` built (3) for the clone's own web views ([#326](https://github.com/ccheever/exact2/pull/326)); main [#309](https://github.com/ccheever/exact2/pull/309) does it for Exact's `iframe` web views and needs no clone change when adopted.

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

**Decided (2026-10-08).** Neither: #108 reduces the root with child state and actions now (`20261008-app-contract-root-rewrite`) and leaves request ownership to LLP 1035.005.000 D5.

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

A textarea sizes to its plain value. A prompt whose chips render wider than the text can be one line taller or shorter than REF. **Support needed:** `field-sizing: content`, or a measured height hook. **Decided (2026-10-08):** #130 keeps hatch-driven layout feedback deferred; the difference is declared, and no measured-height code exists.

## X13. Hover and key events during a pan

**2026-10-08:** not reproduced on main `0365ad1a4` as described; see the current-state row. The paragraph below is the 2026-10-05 reading.

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
- **Window resize clamp (X22, [#127](https://github.com/ccheever/exact2/issues/127)).** No window resize event reaches a component; a window-sized, inert, clipped tracker inside the panel hears `resize=` instead, and its action shrinks the size, then clamps and stores the place as the reference's listener does. Permanent: #127 keeps reactive geometry deferred (2026-10-08).
- **Inspect app colors (X30, [#116](https://github.com/ccheever/exact2/issues/116)).** Not built. Pixel readback stays deferred (#116, 2026-10-08); the element and paint lookup the reference uses is X68 ([#321](https://github.com/ccheever/exact2/issues/321)), filed 2026-10-08: U18 waits for an owner ruling on #321 (no design selected).
- **X46** (new — record at prepare; task `20261005-terminal-surface`). The Apple build copies `assets/` as it is (`host/apple/build.mjs` `copyAppleStaticTrees`); app.json `commands` are verbs for `exact.mjs`, not build steps. Generated files are not committed (rules/RULES.md), so the terminal page and its WASM/font copies (ignored by git) exist only after `bun terminal-host/build.mjs`. A fresh checkout's bundle has no page: the terminal view stays blank and its status (`state.presentation.terminals[].error`) names the failed load. **Support needed:** a manifest-declared pre-bake command (or a bundler entry for app assets) that the Apple, web and delivery builds run.

## Media actions: declared differences

Task `20261005-media-actions` (`media-actions.*`, `media-views.ts`, `T3MediaActions.swift`).
- **Copy image is never "unavailable".** The reference disables Copy image where `navigator.clipboard.write` is missing and says "Image copying is unavailable. Use a secure browser connection or save the image."; the Mac app always has a pasteboard, so the item is enabled whenever the media has a source or an asset.
- **Tooltip style.** The media tooltip is the node's `title` (AppKit's `toolTip`), not the reference's code-style popup; it shows the same text (path, URL or name) and closes when the menu opens.
- **An image that fails to decode (X44, [#121](https://github.com/ccheever/exact2/issues/121)).** Fixed on main (#177) and adopted 2026-10-07: an `image`'s `error` now shows "Image unavailable · <alt>" (chat Markdown), "Image unavailable. The file may have been moved or deleted." (expanded image), "Unable to load workspace image." (Files) and "Unable to load image." (attachment preview). Still different: an SVG never draws on Apple (it is an `error`), where the reference's browser draws it; the Files failure text sits at the top of the panel, where the reference centres it.
- **Rendered HTML over `http://` to a named host (X7, [#106](https://github.com/ccheever/exact2/issues/106), side issue [#135](https://github.com/ccheever/exact2/issues/135)).** Fixed on main (#173) and adopted 2026-10-07: `app.json` allows arbitrary loads in web content, so the bundle loads it as the reference's frame does. #135 (an `http:` sub-resource of a page under the app's own `assets/`) is fixed by main #184; nothing to adopt, since no clone page is served from `assets/`.

## Auto balance: declared differences

Task `20261005-auto-balance` (`load-balancing.ts`, `auto-balance*.ts`, `auto-balance.contract`).
- **Host resource timing (X19, X21; [#124](https://github.com/ccheever/exact2/issues/124)).** The load runs in a command a root task sends when the fetch key changes; receipt time is the window's wall time when the load starts (at most the 5 s deadline before the reply). The 5 s deadline is T3Transport's per-request `timeout`. Permanent for the timing (#124 keeps timers refused, 2026-10-08); the transport waits for Charlie's next core investment, #126.
- **The machine list popover (X17).** Placed with `position-area="top span-right"`; it does not flip near an edge. Waits for main fix of #112 (invoker popovers, the next core investment, 2026-10-08).
- **Stale 20 s is not reproducible from a server.** The sample age uses the client's receipt time (`uses client receipt time when host clocks differ`), so no server reply can make a sample 20 s old; unit tests cover the age rule and the drive uses a busy (0.96) machine.
- **Host finding: a changed text can show its old bitmap in an agent screenshot (not filed).** The macOS host draws a paragraph of at least 16,384 device pixels into a bitmap off the main thread (`Mac/TextRasterMac.swift`), keeps the old bitmap up until the replacement lands (`NodeText.swift` `invalidateText`), parks it at its old size when the frame shrinks (`presentTextRaster` overflow branch), and `settleForPicture` waits for images only. #182's drive drew "Update available for" at the width of "Updating 1 machine". The banner title is a keyed node with a box paint, which AppKit draws on the main thread. The framework fix: `settleForPicture` waits for text raster work and redraws before the capture. Seen again by `20261005-usage-reset-and-feedback` (2026-10-08): the `/feedback` banner drew "Sending feedback to Ope" at the width of "Feedback sent to OpenAI" in the agent's screenshot; the notice id now carries the upload status, so each status is its own node.

## This machine (local primary environment): declared differences

Task `20261005-local-primary-environment` (`local-primary.ts`, `this-machine.ts`, `this-machine.contract`).
- **The first window before the server is ready (X31, [#117](https://github.com/ccheever/exact2/issues/117)).** The reference shows no window until its embedded server is ready; the clone's window opens at launch and shows the connecting state until the primary connects (#117 closed upstream as not planned, 2026-10-08: "Show an honest connecting state, then the result"; a permanent declared difference, U5 final).
- **No relaunch after a Local environment change (X45, [#122](https://github.com/ccheever/exact2/issues/122)).** The reference relaunches the app (decision U4); the clone stops or starts the embedded server in place, hands the focus over and reconnects, and the window stays. Waits for main fix of #271 (after #269).
- **Tab inside the dialog (LLP 1080.003 §4; X53, [#282](https://github.com/ccheever/exact2/issues/282)).** `aria-modal` does not keep Tab inside a modal on macOS; the Local environment dialog keeps it on its two buttons with its own `key` handlers, as the reference's focus trap does, so nothing differs. The same stopgap now covers AppConfirm, SettingsConfirm, Custom snooze and Add Environment (`20261008-dialog-shortcut-focus`).

## Provider settings upkeep: declared differences

Task `20261005-provider-settings-upkeep`, 2026-10-08.
- **ACP registry icons on Apple (X44, [#121](https://github.com/ccheever/exact2/issues/121), residual; reproduced on main `0365ad1a4` 2026-10-08, not filed: open PR [#239](https://github.com/ccheever/exact2/pull/239) decodes SVG on Apple).** Registry icons are SVG (`https://cdn.agentclientprotocol.com/registry/v1/latest/<id>.svg`); an SVG `image` is a load error on Apple, so every instance icon of an ACP agent shows the ACP glyph there (the fallback the reference shows while loading or after a failure). The allow-list, the agent-id URL and the `load`/`error` states are ported; a PNG on the CDN would draw. The fetch rules of `AcpRegistryIcon.tsx` (no credentials, no referrer, 512 KB, CacheStorage) are the host's image loading, documented by main #177.
- **End-aligned popovers (X17, [#112](https://github.com/ccheever/exact2/issues/112)).** The update details popover (side bottom, align end, w-80) is placed by margins: its left edge sits the popover's width minus the trigger's to the left of the trigger. The Apple host applies a popover's margins only when it names a `position-area` (`host/apple/src/style.rs`, the `crossing` rows), so the popover says `position-area="bottom span-right"`, which is D2's own placement; without it the margin is dropped and the popover starts at the trigger (seen on the 2026-10-08 drive). It does not flip near a window edge, and nothing pulls it back from the left edge: the host clamps the margin box (the trigger's width), not the popover, so a list-row popover is cut off on the left when the trigger sits less than 20rem from it (the Settings nav collapsed in a window narrower than about 1,240 points); the reference's Base UI popover shifts to stay on screen. The web target places it by the browser's anchor positioning and was not driven for this popover. Waits for main fix of #112 (invoker popovers flip; the next core investment, 2026-10-08).
- **Escape inside Settings.** Among `aria-keyshortcuts` buttons the lowest node wins, so Settings' Back (Escape) answered before a popover's own dismissal and closed Settings. The update popover and the clone's select popups are `aria-modal` while open (only shortcuts inside a modal are heard) and carry an Escape button that hides them and returns focus to the trigger. Every other owner is one value, `escapeOwned` in `app-settings.contract` (settings-escape-and-nav, 2026-10-09 audit S1-1..S1-3, S2-1): while a General model picker or a core menu is open, the nav search holds a query, a shortcut records or an open custom model editor (`providerPage.escapeOwned`) is shown, Back gives up Escape and the owner's own handler or Cancel takes it. A field whose own `key` handler takes Escape (the Keybindings search, the licence search, the "Add custom model" slug field) owns it only while it has the focus (`escapeFocus`, from the fields' `focus`/`blur`), as the reference's onKeyDown hears it only from the focused input: a search left open with a query and the focus elsewhere lets Escape leave Settings. The "Add custom model" field is held by its Models block's key (`modelAdding`), which a successful add or remove renews (`providers.ts` `modelRevisionsOf`), so the field closes after a successful add as `handleAdd` does; Settings' view is mounted only while Settings is open, so its view state starts over each time, as the reference's route remounts. A focused field the view removes may send no `blur` (macOS `Presenter.send` drops an event for a node that is gone), so each field that closes on its own Escape clears `escapeFocus` itself; the When editor, the font list and the recording field are `aria-modal` while they show (the host's light dismiss closes a popover on Escape), and a Settings dialog (a scheduled task, a project editor, Add device host, the confirms) makes the page inert. Not an exact2 gap (the web's `keydown` also reaches document handlers); recorded because the reference's Base UI popover stops it.
- **Popover motion.** The reference fades and scales the popover in; no popover in the clone animates (the app's popover timing), so reduced motion changes nothing for it. The "Updating" spinner stops under reduced motion, as `motion-safe:` does.

## This machine: Network access, Tailscale HTTPS, Authorized clients — declared differences

Task `20261005-this-machine-network-access` (`server-exposure.ts`, `tailscale.ts`, `pairing-urls.ts`, `auth-access.ts`, `connections-network.ts`, `connections-network*.contract`, `T3LocalNetwork.swift`).
- **No relaunch after a network change (X45, [#122](https://github.com/ccheever/exact2/issues/122)).** The reference persists the setting and relaunches the app ("T3 Code will restart."); the clone persists it, restarts the embedded server in place with the new envelope (`host`, `tailscaleServeEnabled`, `tailscaleServePort`), reconnects this machine and keeps the window. The dialogs keep the reference's words.
- **The "+N" toggle's dotted underline ([#287](https://github.com/ccheever/exact2/issues/287), filed 2026-10-08).** Contract's `border-*-style` takes `none`, `hidden`, `solid` and `inset` only; `border-bottom-style="dotted"` fails the build (`lower-attr-value`). The clone draws a solid 1 px underline in the same muted colour. #287 admits `dotted` and `dashed` (2026-10-08): approved, no fix in progress: waits for main fix of #287.
- **Tailscale is verified with a stub CLI (decision U9: a verification gap, not a function difference).** `tailscale status --json` and `serve` are a lane script; the MagicDNS HTTPS endpoint never answers the probe, so "Setup required" → available and Disable are covered by unit tests only, until a tailnet the user provides. The implementation was checked against the reference line by line (`20261008-provisional-decisions-parity`): the probe is now a 2.5 s deadline for the whole request, the network snapshot is revalidated as the reference's SWR atom (on open when 30 s old, after a change), and the port field takes any digits-only value from 1 to 65535 and steps on ArrowUp/ArrowDown.
- **The Tailscale port field accepts letters (X60, [#301](https://github.com/ccheever/exact2/issues/301)).** The reference's `type="number"` field refuses characters a number cannot hold; exact2's macOS number field is a text field (no filter, no stepping; the clone steps it itself), so a typed letter shows, with "Enter a port from 1 to 65535." under it. Waits for main fix of #301.

## Pull request header actions and stacks: declared differences

Task `20261005-pr-header-actions-and-stacks` (`pages-pr-actions.*`, `pages-pr-stack.*`, `r6-pr-logic.ts`).
- **Menus and the freshness popover do not flip (X17, [#112](https://github.com/ccheever/exact2/issues/112)).** The More menu, the stack menu and the out-of-date base's popover sit under their trigger at its start edge and are clamped to the window (the reference aligns More `end` and the popover `start`, and flips near an edge); in the panel at the window's right edge the clamp lands the More menu where the reference's end alignment does. Waits for main fix of #112.
- **The freshness popover's hover card (fix-hover-cards, X62).** Pointing at the base branch's mark shows the card at once (`openOnHover delay={0}`), drawn by the window's hover layer under the mark (side bottom, align start), where the pointer can move into it and press its buttons; it goes 120 ms after the pointer leaves the mark or the card (the reference's closeDelay, on the root's hover clock). Pressing the mark (or Return on it) opens the same card as a popover, which Escape and a press outside close.
- **Dialog focus (X53, [#282](https://github.com/ccheever/exact2/issues/282)).** The confirmation AlertDialog and the stack's Dialog keep the stopgap the other dialogs use (`20261008-dialog-shortcut-focus`): focus starts on Cancel and Tab cycles their buttons with their own `key` handlers; Escape is Cancel's `aria-keyshortcuts`.

## Pull request hand-offs and quick actions: declared differences

Task `20261005-pr-handoffs-and-quick-actions` (`pages-pr-handoffs.*`, `pages-pr-quick.*`, `T3Sidebar.swift` speed mode).
- **Popover placement (X17, [#112](https://github.com/ccheever/exact2/issues/112)).** The Check out menu (`align="end"`) is placed by margins from its button's start under `position-area="bottom span-right"` (the menu's width less the button's measured width); the row's checks popover and stack menu sit at their trigger's start. None flips near an edge (no `position-try`). Waits for main fix of #112.
- **Quick actions under the agent.** Speed mode is the app's native flags monitor (`NSEvent` local monitor, ⇧ alone, not while an `NSTextView` edits), which the agent's key events do not reach (it delivers to the window). The predicate is the AppKit test's (`macos/tests/sidebar`); a person's ⇧ (or a HID-posted one) shows the buttons.

## Usage pooled view: declared differences

- **Light dismiss (task `20261008-popover-escape-parity`).** A pinned segment popover closes on a primary press that starts outside its card and its two triggers and does not end in the card, as Base UI's does (on the page: a hit test against `frame()`; elsewhere in the window: `T3Window` counts the press). Three host findings shape it. One is a local draft (X65); the other two are unconfirmed, from reading the code:
  - *A press on a scroll view's empty area* ([X65](.exact/implementation/20261005-t3code-macos-parity/issues/20261008-x65-scroll-empty-area-press.md), filed as [#317](https://github.com/ccheever/exact2/issues/317) on 2026-10-08). On macOS a press below or beside a `scroll`'s content (the clip view's ground) reaches no node, so no ancestor's `pointerdown` hears it; on the web the scroll element takes it and it bubbles. Seen in the clone's agent drive (2026-10-08): `tap usage-scroll clicks 1 at 600 700`, below the 604-pt content of a 788-pt port, journalled no `pointerdown`. Worked around: a full-height ground (`usage-ground`) under the content. Reproduced on main `b896050d7` before filing.
  - *A press in a text field* (unconfirmed, no draft). On macOS the field and its field editor (`TextAreaMac.swift` `Field`, `FieldEditor`, `TextArea`) never call `pointerPressed`, so a press in one reaches no ancestor's `pointerdown` (read, not run; on the web it bubbles). Worked around for the fields that can sit over the Usage page (the floating theme editor's Theme name, filter and hex fields): their `focus` counts as an outside press. A Tab into one counts too, where the reference closes the popover only when focus leaves its popup.
  - *A native context menu keeps the pointer held* (unconfirmed, no draft). A right-click on a node with a `contextPopover` (sidebar thread and draft rows, panel tabs) goes through `pointerPressed`, which now holds the pointer on the window's root; the NSMenu opened on the next turn takes the secondary button's up, so `pointerHeld` stays set until the next primary button's up (`MouseChainMac.swift` `pointerPressed` / `pointerReleased`, read, not run: the agent shows a painted menu). Until then no node hears a `pointerdown` (the first press after such a menu does not dismiss a pinned popover, and the colour plane or hue slider misses that first press), and the hit test that follows a resting pointer after a layout change pauses (hover still follows a moving pointer). A real-input check is in the task's steps.
- **The popover's side (X17, [#112](https://github.com/ccheever/exact2/issues/112)).** Worked out from the layout as Base UI's flip lands on an unscrolled page (`popoverSides`); after a scroll a popover near the scroll area's top can still be clipped (no `position-try`). #112's first slice covers invoker popovers only, so this hover-opened layer keeps the app's placement.
- **Hatching of the spent share.** One SVG path of the same 1px stripes 5px apart at 135°: the kernel paints no `repeating-linear-gradient` (it says so at compile time). No visible difference.

## Browser surface: declared differences (X1 path B)

Task `20261005-browser-surface` part 1 (`browser-*.ts`, `browser-surface.contract`, `T3BrowserSession.swift`,
`T3BrowserSessions.swift`, `T3BrowserView.swift`, `T3BrowserFavicon.swift`). The engine is a `WKWebView` in the clone's
module, the user's decision of 2026-10-08 after #100 was closed upstream as not planned; each row is a path-B row of X1's
table, built here. Parts 2–5 add their own rows when they build them.
- **Engine and rendering.** Pages render with WebKit, not Chromium: layout, fonts, form controls, scroll bars and
  standards support follow Safari's engine at the macOS version, and a page that sniffs the engine sees WebKit.
- **User agent.** The page keeps the engine's native user agent, as the reference does (`BrowserSession.ts:198-205`), so
  the string is WebKit's (`AppleWebKit/605.1.15 (KHTML, like Gecko)`, no `Version/… Safari/…` token, no Electron or
  Chrome token); nothing rewrites it.
- **Developer tools.** No "Open DevTools" item: Safari's Web Inspector attaches to a tab's page in a development build
  (`T3WebInspection.mark`, #101/X2) and cannot be opened from code; a release build never marks the page inspectable.
- **Permissions.** The reference grants clipboard-read, clipboard-sanitized-write, notifications and geolocation and denies
  the rest (`BrowserSession.ts:31-40`). WebKit asks a delegate only for the camera and the microphone, which are denied as
  there. A clipboard write needs a user gesture (WebKit's rule) and a clipboard read shows WebKit's own Paste
  confirmation; `WKWebView` has no Notification API, and asks no delegate for geolocation (the app declares no location
  use), so those two are unavailable in the page.
- **Pop-ups.** As `previewWindowOpenAction`: a scripted pop-up to an http(s) URL opens a real window that keeps its
  opener, and its own pop-ups are refused; a `target=_blank` link loads in the tab. WebKit gives no Chromium
  disposition: a request is a pop-up when a script asked for it with window features (a size, a position or a hidden
  bar). As Chromium's pop-up blocker, a script opens a window only from a user gesture.
- **Load failures.** The code is WebKit's (`NSURLErrorDomain`); the description is Chromium's error name where WebKit's
  error has one (`ERR_CONNECTION_REFUSED`, `ERR_NAME_NOT_RESOLVED`, `ERR_CERT_AUTHORITY_INVALID`, …), else WebKit's own
  words. A cancelled load (WebKit -999, Chromium -3) and a load a policy stopped are not failures. WebKit keeps the last
  page under a failed load (Chromium shows its error page at the failed URL); the load-failed page covers it, and Refresh
  and Reload load the failed URL again.
- **Refresh while loading.** The button is named Stop while a page loads and reloads, as the reference's
  `PreviewManager.refresh` does; during a tab's first load it asks for the pending URL again (WebKit's `reload()` would
  reload the committed page).
- **Favicons.** WebKit has no `page-favicon-updated`: a user script in the module's own content world reads the page's
  icon links (or `/favicon.ico`) after each load; the icon is fetched without the tab's cookies (the reference sends them
  for a same-origin icon), so an icon behind a sign-in falls back to the public favicon service, then the globe.
- **Crash recovery.** A crash is `webViewWebContentProcessDidTerminate`; recovery follows `webviewCrashRecovery`
  (three reloads 250, 500 and 1,000 ms apart within 30 s) at the last URL, then the page stays down until Refresh.
- **Hidden tabs.** A tab the panel does not show keeps running, throttled (`inactiveSchedulingPolicy = .throttle`), as a
  background Chromium guest is throttled.
- **Non-web links and downloads (until part 3).** The main frame refuses schemes other than http(s), `about:`, `data:`
  and `blob:` (a `mailto:` link does nothing); a response the page cannot show is not loaded until part 3 sends
  downloads to the artifact directory.
- **The "+" menu's profile list (X66, [#319](https://github.com/ccheever/exact2/issues/319)).** The reference opens
  it on hover of the Browser row (`MenuSubTrigger`); here its chevron opens it, until a popover can open from an action.
- **Storage.** Each environment's profile has its own persistent WebKit data store (identifier derived from the
  environment and the profile), apart from the app's other web views; agent runs keep it in memory.

Part 4 (`20261005-browser-surface-profiles`: `browser-profiles*.ts`, `browser-defaults.ts`, `browser-import*.ts`,
`browser-profiles.contract`, `browser-defaults.contract`, `T3BrowserSessions+Profiles.swift`, `T3BrowserImportIO.swift`)
adds these rows:
- **Profiles and their stores.** Default and every named profile persist in their own `WKWebsiteDataStore(forIdentifier:)`
  per environment (a UUID from the environment and the profile, macOS 14), the reference's `persist:` partitions;
  Incognito is one non-persistent store per environment, discarded when the app quits (its in-memory partition). An
  agent run keeps every store in memory. Removing a profile clears its stores in every known environment; the store's
  folder stays, as the reference leaves its partition's.
- **Clear cookies and Clear cache.** Clear cookies removes WebKit's cookies, local storage, IndexedDB and service worker
  registrations (the reference's `clearStorageData` storages); Clear cache removes the disk, memory and fetch caches
  (`session.clearCache()`). Both act on the tab's environment and profile only.
- **Writing imported cookies.** Each cookie goes through the store's `WKHTTPCookieStore` in batches, and the store is
  read back: a cookie WebKit dropped (expired, malformed) counts as skipped, as a refused `cookies.set` does. A host-only
  cookie keeps no leading dot, a domain cookie keeps it. WebKit has no explicit `SameSite=None` cookie property: an
  imported None cookie and an unspecified one are both written without a policy, which WebKit treats as None (Chromium
  treats an unspecified one as Lax). There is no `flushStore`: WebKit persists the store on its own.
- **Reading the other browsers.** The reference's readers (`BrowserImport/*`) run in Electron's main process on Node;
  the clone's port runs in the data module over the module's primitives (`browserImportIO`: SQLite, CommonCrypto and
  CryptoKit, the Keychain, fcntl). macOS only: Chromium's Linux keyring helper and Windows' DPAPI unwrap are not ported
  (their Linux paths and Firefox's Snap home are), and Firefox's `.parentlock` is probed with `fcntl` in-process where
  the reference spawns python3. The Keychain prompt names this app (in-process `SecItemCopyMatching`), as the reference's
  binding does.
- **Development builds read fixtures only.** Only the packaged build reads the user's browsers and Keychain. Every other
  build, agent runs included, lists no browser unless `T3_BROWSER_IMPORT_HOME` names a fixture home (never the account's
  own home): every path must resolve inside it, its `.t3-browser-import.json` answers the Keychain, and the paths it
  lists under `tccDenied` fail an open with EPERM as TCC does, so the Full Disk Access step can be shown. Full Disk Access
  is never requested from code: Allow opens System Settings › Privacy & Security › Full Disk Access in the packaged build
  and is only recorded in any other.
- **The Full Disk Access check.** The wizard re-reads the grant (Safari's jar opening) whenever the page is drawn; the
  reference also polls every 1.5 s, which a data-module page source cannot (it has no clock).
- **The Browser defaults rows.** The default viewport's menu lists each preset with its size in one label ("iPhone SE
  375 × 667"; the reference right-aligns the size and heads the presets "Standard"), and its width and height are plain
  fields committed on Return or when left (no stepper arrows). A page the module makes (from the launcher, a link, an
  agent or a relaunch) at a fixed viewport is made at the default zoom (the zoom `browserSync` carries for a page the
  module has not reported yet), so its first layout is already at it; its appearance, and a Fill page's zoom, follow
  right after it is made, through part 2's `browserSet`, where the reference passes both at creation. Show device
  toolbar on a fill tab opens at the configured default viewport when it is fixed (part 2's
  `browserResponsiveViewportForToggle`).
- **Where the row shows.** Browser profiles is part of Settings › Integrations, which the clone draws for a connected
  environment; the reference draws its device-local rows without one.

Part 2, `20261005-browser-surface-navigation` (`browser-history.ts`, `browser-targets.ts`, `browser-viewport.ts`,
`browser-navigation.ts`, `browser-stage.contract`, `T3BrowserSession+Navigation.swift`, `T3BrowserView.swift`):
- **Zoom.** WebKit's `pageZoom` on the reference's ladder (25% to 500%): the page's CSS pixels grow, its viewport
  narrows and `devicePixelRatio` follows, as Chromium's `setZoomFactor` does; the web view keeps its zoom across
  navigations and while the tab is hidden.
- **Appearance.** The web view's `appearance`, which WebKit hands the page as `prefers-color-scheme` (the reference
  sets it through CDP `Emulation.setEmulatedMedia`). Light and Dark force it; System clears it, so the page follows the
  window, which follows the app's theme. A forced appearance is the view's own, so WebKit also draws the page's native
  controls (scroll bars, form controls of a page that opts in with `color-scheme`) in it; Chromium's emulation changes
  the media query alone.
- **Fixed viewport.** A web view sized at the viewport times its zoom, in a box whose bounds scale it into the fitted
  footprint (the reference's `transform: scale()`): presentation only, so the page keeps its CSS viewport and the
  screen's device pixels. While a rail is dragged under a locked ratio, the live size skips the area cap's square-root
  bound (Contract has no square root); the size asked for on release is the reference's (`resizeBrowserViewportFromRail`).
- **The device toolbar's fields.** A typed width or height applies on Enter or when its field loses the focus, also when
  the focus moves to the other field (the reference waits until the focus leaves the toolbar); with the ratio locked, a
  typed side is bounded so the other stays within 240 to 3,840 and follows it (`resizeAtAspectRatio`), short of the area
  cap's bound (a square root Contract has not): the size asked for is checked again, as the reference checks it.
  The preset list is a popover list (no typeahead), the Appearance submenu opens by press (X66, #319). An arrow key on a
  rail asks for its step at once (the reference accumulates keys for 150 ms before committing): neither a data source
  nor Contract has a timer, so held keys commit step by step.
- **The preview keys.** A key monitor in the module answers the `previewFocus` chords (⌘R, ⌘L, ⌘= or ⌘+, ⌘-, ⌘0 by
  default; the server's keybindings decide) while the page or the tab's URL field has the focus, before the menus; with
  the page focused ⌘R (or Control-R) always refreshes, as the desktop's `isPreviewRefreshShortcut`. The field counts only
  while it really holds the focus (the window's first responder is inside the field the host tags `browser-url`), and a
  chord is named by the layout's character, else by the key's ANSI code (a non-Latin input source). The reference's
  renderer hears them only from the chrome (a Chromium guest's keys do not reach it), ⌘R aside; here the page's focus
  counts too, as its `isPreviewFocused` intends. Other chords: WebKit hands a key equivalent the page leaves to the
  app's menus (the reference's guest ignores menu shortcuts but the editing ones).
- **Recent sites' icons.** The reference keeps captured favicons per project (`browserFaviconStore`); a Recently used or
  Local servers row shows a live tab's captured icon of the same origin, else the public favicon service for a public
  host, else the globe.

**Part 5 (`20261005-browser-surface-automation`: the `previewAutomation` host, links, Mute).** `browser-automation*.ts`,
`browser-links.ts`, `T3BrowserAutomation*.swift`, the vendored Playwright injected script (`assets/vendor/playwright`,
1.60.0, Apache-2.0, `VENDOR.json`). The host serves all 14 `preview_*` tools; each row is X1 path B's, against the
reference's CDP desktop host (`apps/desktop/src/preview/Manager.ts`):
- **Input.** A page in a window gets native input: `NSEvent` mouse and key events sent to the page's view, trusted in the
  page (`event.isTrusted`), the window's focus lent to the page for the event and given back
  (`runPreviewClickKeepingHostFocus`, for every action). A page in no window (the panel hidden, another thread shown) has
  no native event path: a click is the pointer and mouse events, the focus move and `click` as DOM events, and a key is
  `keydown`/`keyup` with its default edit (typed text, Enter submitting a form or breaking a line, Backspace, Delete), all
  untrusted. The answer waits for the page's own `mouseup`/`keyup` listener (Chromium's `performance.eventCounts`
  receipt does not exist in WebKit), at most 5 s. Human input during an agent action does not interrupt it (the
  reference's `PreviewAutomationControlInterruptedError`), and a key goes to the main frame's focused element, not into a
  focused cross-origin iframe (the reference attaches to its renderer).
- **Evaluate.** In the page's main world with `callAsyncJavaScript`, awaited as `awaitPromise`, the value returned as
  JSON (a DOM node is `{}`, as CDP's `returnByValue`); an expression that is a script (`const a = 1; a + 1`) runs through
  `eval`, so a page whose Content Security Policy forbids `unsafe-eval` refuses it; `returnByValue: false` (a remote
  object) is always by value.
- **Snapshot.** `accessibilityTree` is Playwright's ARIA snapshot of the body (`{format: "playwright-aria-snapshot",
  snapshot}`), not CDP's full AX tree (the server leaves it out of the agent's text either way); the screenshot is
  WebKit's `takeSnapshot` of the page, at most 1,280 pixels wide, also for a page in no window.
- **Console and network.** A page-world user script reports console calls, uncaught errors and unhandled rejections, and
  failed `fetch` and XHR requests (status 400 and up, or a network error) and elements whose load failed (no status), in
  the main frame. Other failed subresources (a stylesheet's `@import`, a font) and subframes have no entry.
- **Locators.** Playwright's injected script runs in the host's own content world with WebKit's options
  (`browserName: "webkit"`), so `role=`, `text=` and CSS locators resolve as Playwright's do on WebKit; a page cannot see
  or replace the script.
- **Appearance.** `preview_set_appearance` sets the page view's appearance (`NSAppearance`), which WebKit reports as
  `prefers-color-scheme`; it also restyles the page's form controls and scroll bars (CDP emulates the media feature only).
  It is part 2's mechanism (`setColorScheme`), so the More menu's Appearance shows what an agent chose.
- **Viewport (part 2).** `preview_resize` takes Fill, a freeform size and the device presets (a preset's size and
  orientation resolved as the reference's `resolvePreviewViewport`), and a tab an agent opens on Fill takes the
  reference's 1280×800. A page the panel shows is sized by part 2's stage; a page it does not show (another thread, the
  panel hidden) is sized to the viewport at its zoom by the host (`T3BrowserViewport`), so the answer measures the size
  asked for either way. A size not rendered in time goes back to the tab's previous one without the reference's check
  that nothing else changed it meanwhile (the module does not see the store's latest setting), and neither resize waits
  in the device toolbar's commit queue (`runBrowserViewportMutation`).
- **Recording (part 3).** `preview_recording_start` answers an execution error (no capture) once the tab is ready, and
  `preview_recording_stop` answers that nothing records, as the reference does when no recording is active.
- **Presentation (part 3).** The reference shows an agent's tab in the floating preview; until part 3 builds it, the tab
  opens in its thread's right panel (`rightPanelStore.openBrowser`), with the same `open: false` suppression.
- **Mute.** WebKit has no public page mute (`_setPageMuted:` and `_isPlayingAudio` are SPI). Mute silences the document's
  `<audio>` and `<video>` elements and keeps them silent; Unmute gives back each element's own muted state. Audible means a
  media element plays with sound the page asked for, muted or not, as Chromium's tab audio state. Web Audio is neither
  muted nor heard, and media in subframes are not covered. WebKit pauses a muted element while its page is out of the
  window (another tab shown, Settings open) and plays it again when the page is shown, so a muted tab's media does not
  advance meanwhile (Chromium's muted background tab plays on); the tab still shows muted.
- **Agent cursor.** Drawn as a layer of the page's view (so the screenshot, which is the page's own paint, leaves it out,
  as the reference's DOM overlay is left out of `capturePage`), with the reference's timings.

## Desktop update controls (no feed): declared differences

Task `20261009-blocked-desktop-update-controls`, option (a), merged as #368 on the user's decision of 2026-10-10 (X40 stays closed).
- **No Tab stop on the disabled "Check for updates" control (framework gap X70, a local draft:
  `.exact/implementation/20261005-t3code-macos-parity/issues/20261010-x70-aria-disabled-focusable.md`; the user keeps
  new framework drafts local).** The reference's
  `SidebarUpdatePill` is `aria-disabled`, so Tab still reaches it (focus ring, no action). Contract has no
  `aria-disabled` (`contract vocab`), and a `disabled` button takes no focus, as a disabled `<button>` on the web; the
  clone's control (`sidebar-icons.contract` `SidebarUpdatePill`) is skipped by Tab. Its label, dimmed state, tooltip and
  no-op press match.

## Not exact2 asks (stay in the app module)

Keychain credentials, SSH tunnels, VideoToolbox/SceneKit device views, the terminal (WKWebView running REF's Ghostty WASM; no exact2 change needed), notifications, SnapShot capture, the offscreen Mermaid web view, agent export plumbing.
