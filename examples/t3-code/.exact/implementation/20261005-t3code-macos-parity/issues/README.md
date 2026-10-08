# Issues for 20261005-t3code-macos-parity

User rules (2026-10-05): a capability that exact2 does not support is filed as an **issue
only**; this plan writes no framework PR. Each issue explains in detail why it arose (the T3
Code behavior, what exact2 does today with evidence, where the clone hits it) and why it must
be resolved (parity impact). The goal is a complete clone, so a declared deviation is not an
end state: every issue ends resolved upstream and adopted in the app (verified by
`issue-close`), or closed by the user's decision. Policy issues (DEFERRED rules) need
Charlie's waiver (decided 2026-10-08: #100 and #117 closed as not planned; #101, #116, #124 and #224 bounded; see
"Charlie's decisions" below); scope issues (X38–X41) need the user's decision to build or close (decided 2026-10-08: all
four closed, not built).

A file whose row has no upstream link is a **local draft**: not reproduced on the pinned
`main`, not searched upstream, not published. `issue-open` reproduces each one, checks for
duplicates and prepares the report; publication needs the user's approval. The user approved
the 2026-10-08 round ("reproduce, then file as issues; close the ones that don't reproduce"):
the tables below say what each draft became.

Kinds: `framework-gap` (missing support), `framework-policy` (a DEFERRED rule refuses it),
`scope-decision` (excluded product scope). "(unconfirmed)" means the library does not cover
the capability and nobody has checked it yet: `issue-open` first confirms the gap and closes
the draft if the capability already works. "Blocks" lists the tickets whose work or
acceptance rows wait for the issue or carry its difference until it is resolved.

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X1](closed/20261005-x01-chromium-cdp-browser-surface.md) | An embedded browser engine that an app can drive (Browser surface) | framework-policy + framework-gap | browser-surface, right-panel-tab-menu, t3-connect-sign-in | [#100](https://github.com/ccheever/exact2/issues/100) closed upstream, not planned (2026-10-08): path A refused; the user chose path B (a `WKWebView` in the clone's module), so `browser-surface` is clone-side work; **closed** |
| [X2](20261005-x02-app-developer-tools.md) | A developer-tools inspector for the app's own UI (View › Toggle Developer Tools) | framework-policy | app-developer-tools, desktop-shell-details, terminal-surface | [#101](https://github.com/ccheever/exact2/issues/101) narrowed (2026-10-08): development-only Safari inspection of web views, no inspector of the app UI; the View item is a permanent declared difference; `app-developer-tools` builds the inspection in the app (in progress); main #309 merged (`f2f0e7092`): Exact's development iframe web views are inspectable; the clone's own web views still need its gate |
| [X3](closed/20261005-x03-root-font-size.md) | An app-settable root font size, the base of `rem` | framework-gap | interface-font-size, interface-font-size-conversion | upstream #102 closed (main #185); adopted |
| [X4](20261005-x04-bundle-helper-executables.md) | Helper executables and large resource trees in the `.app` | framework-gap | embedded-server-runtime, portable-app-download (dropped 2026-10-08), this-machine-network-access | upstream #103 fixed (main #215); adopted by [embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md) (the release archive as a native resource tree, no parts; the unpack stays: #215 re-signs Mach-O without its entitlements, see file); portable-app-download and this-machine-network-access adopt it next |
| [X5](20261005-x05-url-scheme-delivery.md) | A custom-scheme URL delivered to a data source or module when no route takes it | framework-gap | app-activation, managed-codex-chatgpt, provider-sign-in-and-install, t3-connect-sign-in | upstream #104 closed (main #201 journals an unheard launch URL only); delivery to a data source or module still missing; nothing to adopt ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest filed as [#268](https://github.com/ccheever/exact2/issues/268) ([Design], 2026-10-08, reproduced on main `0365ad1a4`); #268 narrowed (2026-10-08): `protocol_handlers` still needs a navigation root; no clone consumer left |
| [X6](20261005-x06-module-quit-shutdown.md) | A module hook at quit that can delay termination for a bounded time | framework-gap | app-activation, app-update-feed, embedded-server-runtime, managed-codex-chatgpt, telemetry | upstream #105 closed (main #200: `destroy()` at quit and last-window close); adopted for SSH ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)) and by [embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md) (server stopped in `destroy()`: gone 0.79 s after a quit); a bounded quit hold is still missing; the other four tickets adopt it next; rest (a bounded quit hold, SIGTERM) filed as [#269](https://github.com/ccheever/exact2/issues/269) ([Design], 2026-10-08); #269 decided (2026-10-08): SIGTERM as an orderly quit and one shared 5 s hold; waits for main fix of #269; main #313 merged (`a3d61c023`): SIGTERM through the orderly quit; the async hold still open |
| [X7](closed/20261005-x07-ats-keys.md) | App Transport Security keys cannot be set from `app.json`, so a rendered HTML preview cannot load `http://` assets from a named host | framework-gap | media-actions | fixed upstream (main #173), adopted by adopt-main-fixes-shell (PR #181) |
| [X8](closed/20261005-x08-agent-pointer-native-views.md) | Pointer input (down, move, up, wheel) for native views in the agent driver | framework-gap | browser-surface, diff-review-engine, floating-device-player, right-panel-tab-menu, settings-scoped-controls-and-theme-editor, sign-in-terminals, terminal-drawer, terminal-integrations, terminal-layout, terminal-surface | fixed upstream (main #186); adopted by adopt-main-fixes-r3 in AppKit for terminal, device, panel and diff rows (two terminal bugs fixed); tab middle click driven live; agent rows driven live in adopt-main-fixes-r3's final round; real input in [real-input-checks](../tasks/closed/20261007-real-input-checks.md): middle click, cursor shapes, theme editor dragged out of the window, terminal double-click, right-click and drag select pass ([terminal-real-drag](../tasks/closed/20261007-terminal-real-drag.md)); **adopted** |
| [X9](20261005-x09-root-component-across-files.md) | A child component cannot own a resource, and the root component cannot span files, so the app's data layer is capped at one 1,500-line file | framework-gap | auto-balance, client-activity-reporting, composer-fidelity, diff-review-engine, hot-file-split, interface-font-size, legacy-sidebar, live-automations-and-clones, local-primary-environment, managed-codex-chatgpt, media-actions, pr-code-tab, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, right-panel-tab-menu, server-update-banner, settings-scoped-controls-and-theme-editor, terminal-drawer, terminal-integrations, terminal-layout, thread-commands-and-keys, upstream-timeline-and-markdown, upstream-ui-sync, usage-pooled-view, usage-reset-and-feedback | [#108](https://github.com/ccheever/exact2/issues/108) decided (2026-10-08), a different design: the root rewrite is the remedy, request ownership waits for D5; the local A1 branch is superseded, not pursued |
| [X10](20261005-x10-text-rendering-parity.md) | Text renders differently from Chrome in five separate ways (ellipsis, code wrap, balance, placeholder, weight) | framework-gap | interface-font-size, live-automations-and-clones, provider-settings-upkeep, shiki-residuals, upstream-timeline-and-markdown, usage-pooled-view | upstream #128 closed (main #208: Chrome line breaks on macOS); balance, placeholder colour and smoothing still missing; no clone workaround ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest (balance, placeholder colour, smoothing) filed as [#266](https://github.com/ccheever/exact2/issues/266) ([Design], 2026-10-08); #266 decided (2026-10-08): balance and `::placeholder` colour wait for main fix of #266; font smoothing deferred, a permanent declared difference |
| [X11](20261005-x11-shadow-blur-parity.md) | Negative-spread box shadows draw too faint, and `backdrop-filter` blurs only the parent's paint, not the window below | framework-gap | auto-balance, composer-fidelity, managed-codex-chatgpt, provider-sign-in-and-install, server-update-banner, settings-scoped-controls-and-theme-editor, usage-pooled-view, usage-reset-and-feedback | upstream #129 closed (main #221: a backdrop blur mirrors its box at the edges, Chrome's); no clone workaround, the composer drawer's glass edge is clean now; a backdrop beyond the parent's subtree and `saturate()` continue in #225, so the flattened glass stays ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)) |
| [X12](20261005-x12-textarea-field-sizing.md) | A textarea with `field-sizing: content` is sized from its plain string, not from what the native text view draws | framework-gap | composer-fidelity | [#130](https://github.com/ccheever/exact2/issues/130) deferred (2026-10-08): a permanent declared difference (no workaround) |
| [X13](closed/20261005-x13-hover-keys-during-pan.md) | Hover and key events keep flowing while a pan gesture owns the press | framework-gap | diff-review-engine, floating-device-player, legacy-sidebar, live-automations-and-clones, pr-code-tab, pr-handoffs-and-quick-actions, pr-links-previews-and-routing, provider-settings-upkeep, round12-wrapup, settings-scoped-controls-and-theme-editor, terminal-layout, upstream-timeline-and-markdown, usage-pooled-view, usage-reset-and-feedback | not reproduced on main `0365ad1a4` as described, closed (2026-10-08): keys reach a `key` handler during a pan on macOS; no host delivers hover to other nodes during a pan (the web host's pan holds pointer capture); not filed |
| [X14](closed/20261005-x14-parked-native-reply.md) | A let-go answer's in-flight native request survives and hands its reply to the next answer | framework-gap | clone-on-exact2-main, managed-codex-chatgpt | fixed upstream (main #183), adopted by adopt-main-fixes-r3: `T3ReadGate.swift` and the reader tags removed |
| [X15](closed/20261005-x15-non-latin-key-equivalents.md) | Chords and menu equivalents match the physical key under a non-Latin input source | framework-gap | desktop-shell-details, terminal-layout, terminal-surface, thread-commands-and-keys | upstream #110 closed (main #168); workaround kept |
| [X16](closed/20261005-x16-smart-substitutions-off.md) | `autocorrect="off"` on a macOS textarea turns off spelling correction only, so AppKit still rewrites quotes, dashes and text | framework-gap | composer-fidelity, diff-review-engine, pr-writing-and-metadata | upstream #111 fixed (main #160); adopted |
| [X17](20261005-x17-popover-position-try.md) | Popover anchoring with every side/align area and automatic flip and shift (`position-area`, `position-try`) | framework-gap | auto-balance, composer-fidelity, legacy-sidebar, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, server-update-banner, upstream-ui-sync, usage-pooled-view, usage-reset-and-feedback | [#112](https://github.com/ccheever/exact2/issues/112) the next core investment (2026-10-08): waits for main fix of #112 (invoker popovers first; hover layers keep the app's placement) |
| [X18](closed/20261005-x18-svg-path-animation.md) | An SVG path's `d` cannot be animated, so icons cross-fade where T3 Code morphs them | framework-gap | composer-fidelity | upstream #123 fixed (main #188); partly adopted (adopt-main-fixes-r3) |
| [X19](20261005-x19-data-source-timers.md) | Timers and a clock inside data sources (`setTimeout`, `setInterval`, `Date.now`) | framework-policy | auto-balance, client-activity-reporting, embedded-server-runtime, environment-routes, live-automations-and-clones, pr-code-tab, pr-conversation-and-refresh, pr-links-previews-and-routing, reference-logic-test-ports, reference-logic-tests-done-areas, server-update-banner, telemetry, this-machine-network-access, usage-pooled-view, usage-reset-and-feedback | [#124](https://github.com/ccheever/exact2/issues/124) decided (2026-10-08): timers stay refused; time as arguments and gated tasks are a permanent declared difference; the local branch is declined, not pursued; #192 checked: three mount polls became gated tasks ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)) |
| [X20](20261005-x20-rich-text-editing.md) | No way to build an editor with inline atomic chips, caret/selection access, range replacement, paste interception and undo grouping | framework-policy | composer-fidelity, diff-review-engine, terminal-integrations, thread-commands-and-keys | upstream #125 closed (main #209: field paste/copy/cut only); caret events, `setRangeText`, `beforeinput`, undo groups, atomic ranges still missing; native composer kept ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest filed as [#275](https://github.com/ccheever/exact2/issues/275) ([Feature]: selectionchange, setRangeText, beforeinput) and [#276](https://github.com/ccheever/exact2/issues/276) ([Policy]: atomic ranges), 2026-10-08; #276 closed upstream, not planned (2026-10-08): the chips are a permanent declared difference; #275 narrowed: waits for main fix of #275 for plain fields |
| [X21](20261005-x21-two-way-websocket.md) | A two-way WebSocket for data modules (send, message, close, backpressure) | framework-gap | auto-balance, browser-surface, client-activity-reporting, embedded-server-runtime, environment-routes, live-automations-and-clones, local-primary-environment, managed-codex-chatgpt, pr-code-tab, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, remote-scopes-and-update-commands, server-update-banner, settings-scoped-controls-and-theme-editor, sign-in-terminals, t3-connect-sign-in, terminal-drawer, this-machine-network-access, thread-commands-and-keys, usage-pooled-view, usage-reset-and-feedback | [#126](https://github.com/ccheever/exact2/issues/126) the next core investment (2026-10-08), a different design (runner-owned streams, mutation send): waits for main fix of #126; the local branch is not that API, superseded, not pursued |
| [X22](20261005-x22-reactive-layout-facts.md) | Reactive layout facts (size, position, text width, row visibility) | framework-gap | browser-surface, composer-fidelity, diff-review-engine, floating-device-player, pr-handoffs-and-quick-actions, pr-links-previews-and-routing, settings-scoped-controls-and-theme-editor, shiki-residuals | [#127](https://github.com/ccheever/exact2/issues/127) narrowed (2026-10-08): the measuring hooks are a permanent declared difference; anchors go to #112; visibility has no date; the local branch is superseded by main's `resize` (`5949b2b64`), not pursued |
| [X23](20261005-x23-scroll-restore-offsets.md) | Scroll restoration by key, scroll padding/margin, animated scrollIntoView, and same-frame scroll offset compensation | framework-gap | diff-review-engine, pr-code-tab, pr-handoffs-and-quick-actions, round12-wrapup | upstream #138 closed (main #210: plain-scroll anchoring, X23d); X23a–c still missing; fold row unblocked ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest (X23a–c) filed as [#277](https://github.com/ccheever/exact2/issues/277) ([Design], 2026-10-08); scroll-padding now works on virtualized lists only; #277 decided (2026-10-08): padding, smooth jumps and `scrollend` wait for main fix of #277; restoration is the app's (a different design) |
| [X24](closed/20261005-x24-still-pointer-rehover.md) | Hover state follows layout changes under a stationary pointer | framework-gap | diff-review-engine, floating-device-player, legacy-sidebar, pr-code-tab, upstream-timeline-and-markdown | fixed upstream (main #174), adopted by adopt-main-fixes-shell (PR #181) |
| [X25](20261005-x25-keyboard-keyup-code-capture.md) | Keyboard facts for Contract: keyup, modifiers held, `code`, `repeat`, capture phase, composition end on a chord | framework-gap | browser-surface, desktop-shell-details, diff-review-engine, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-writing-and-metadata, right-panel-tab-menu, sign-in-terminals, terminal-drawer, terminal-layout, terminal-surface, thread-commands-and-keys | upstream #140 open; main #220 adds `keyup`, `KeyboardEvent.code` and `.repeat`; nothing to remove: every clone monitor needs a window capture-phase handler or held-modifier state (#140), or lives in a native view or the reference's main-process code ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)); #140 decided (2026-10-08): capture phase waits for main fix of #140; no held-modifier fact (a different design: `key`/`keyup` + `hasFocus`) |
| [X26](20261005-x26-app-menu-control.md) | App menu control (declared application menu, hide host Go/Develop, zoom, submenus, menu at a point) | framework-gap | app-developer-tools, app-update-feed, browser-surface, desktop-shell-details, legacy-sidebar, media-actions, pr-handoffs-and-quick-actions, right-panel-tab-menu, ssh-password-and-remote-open, terminal-integrations, terminal-layout | upstream #141 open (menu bar); main #223 (context-menu submenus) **adopted** for the sidebar's thread and draft menus, now context popovers the agent can drive; main #226 (Edit ▸ Speech, paste variants) adopted: the clone's own Speech removed, Edit's app commands hidden; keyboard-opened row menus stay the module's (#235); Files tree and legacy project menus not converted (follow-up) ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)); #141 decided (2026-10-08): Contract menu extensions wait for main fix of #141, Develop stays in development builds; #235: the ContextMenu key waits for main fix of #235, Shift+F10 stays the module's; main #314 merged (`d236c36d5`): the ContextMenu key; #292 partial in #327 |
| [X27](20261005-x27-window-chrome.md) | Title-row height and traffic-light inset, frame restore after the final style, full-screen state fact | framework-gap | desktop-shell-details | #113 closed with only the frame restore (main #164, adopted by adopt-main-fixes-shell, PR #181); title-row and full-screen facts still missing on main `261dd4e10`; no open upstream issue for the rest ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)); rest (title-row area, full-screen fact) filed as [#267](https://github.com/ccheever/exact2/issues/267) ([Design], 2026-10-08); #267 decided (2026-10-08): Window Controls Overlay `env()` and `displayMode`; waits for main fix of #267 |
| [X28](20261005-x28-notification-actions-badges.md) | Notification click → app action, Dock badge, window-focus fact (DEFERRED refuses actions and badges) | framework-policy | client-activity-reporting | upstream #114 closed (main #219: `exactPage().hasFocus`); **focus fact adopted** for thread notifications, activity reports, the git refresh and SnapShot settings on focus; notification actions and the Dock badge continue in #224 (policy), so `T3Notifications.swift` keeps them ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)); #224 narrowed (2026-10-08): the click waits for main fix of #224; the Dock badge is a permanent declared difference |
| [X29](20261005-x29-video-pdf-app-files.md) | `video` and `audio` from app-written local files, and a PDF viewer element | framework-gap | media-actions | upstream #115 closed (main #205: bundled PDF iframe); a PDF element and `app:/` iframe still missing; PDFView kept ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest (an `app:/` iframe, a PDF element) filed as [#273](https://github.com/ccheever/exact2/issues/273) ([Design], 2026-10-08); #273 decided (2026-10-08): `app:/` iframe only; `PDFView` is a permanent declared difference |
| [X30](20261005-x30-ts-announce-readback-picker.md) | Data-module topic announce and resource invalidation; pixel readback; any-type file pick with bytes and image transcode | framework-gap | composer-fidelity, media-actions, pr-conversation-and-refresh, settings-scoped-controls-and-theme-editor | [#116](https://github.com/ccheever/exact2/issues/116) narrowed (2026-10-08): pixel readback and transcode are a permanent declared difference; an any-type file input may come; announce and the Inspect lookup are not in #116 (Inspect: X68, [#321](https://github.com/ccheever/exact2/issues/321)) |
| [X31](closed/20261005-x31-deferred-window-readiness.md) | Defer the first window until the app says it is ready | framework-gap (unconfirmed) | local-primary-environment, portable-app-download (dropped 2026-10-08) | [#117](https://github.com/ccheever/exact2/issues/117) closed upstream, not planned (2026-10-08): the connecting state is a permanent declared difference (U5 final); **closed** |
| [X32](20261005-x32-sticky-positioning-in-lists.md) | `position: sticky` inside a scroll container and a virtualized list | framework-gap | diff-review-engine, pr-code-tab, pr-conversation-and-refresh | [#131](https://github.com/ccheever/exact2/issues/131) deferred (2026-10-08): pinned diff file headers are a declared difference until #131 lands on main; sticky inside a `scroll` or one row works |
| [X33](20261005-x33-transcript-selection-range.md) | Selected text, its source message and UTF-16 offsets, and its end rectangle from the rendered transcript | framework-gap (unconfirmed) | diff-review-engine | upstream #132 closed (main #171, part 1); parts 2–3 still missing on main `cff90b364`; parts 2–3 filed as [#274](https://github.com/ccheever/exact2/issues/274) ([Feature], 2026-10-08); #274 decided (2026-10-08): waits for main fix of #274 (`removeAllRanges`/`setBaseAndExtent`, not `clearSelection()`) |
| [X34](20261005-x34-inline-span-frame.md) | Hover and frame of an inline link or span inside rendered Markdown text | framework-gap (unconfirmed) | pr-links-previews-and-routing | upstream #133 closed (main #178, agent hover); inline frame still missing on main `cff90b364`; inline `frame()` filed as [#272](https://github.com/ccheever/exact2/issues/272) ([Bug], 2026-10-08); #272 a correctness fix (2026-10-08): waits for main fix of #272 |
| [X35](closed/20261005-x35-secure-text-entry.md) | Secure (password) text entry in Contract | framework-gap (unconfirmed) | managed-codex-chatgpt, provider-settings-upkeep, provider-sign-in-and-install, ssh-password-and-remote-open | upstream #134 closed (main #167); SSH workaround kept |
| [X36](closed/20261005-x36-data-runtime-intl-locale.md) | Locale-aware `Intl` and the system locale in the data runtime | framework-gap (unconfirmed) | desktop-shell-details, reference-logic-tests-done-areas | upstream #118 closed (main #204: `Intl.Locale`, `getWeekInfo()`); adopted ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)) |
| [X37](20261005-x37-distribution-signing.md) | Developer ID signing, notarization and a pre-seal hook in the host build | framework-gap (unconfirmed) | portable-app-download (dropped 2026-10-08) | upstream #119 closed (main #199: nested Mach-O signing); entitlements and pre-seal hook still missing; available, not adopted here ([adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)); rest (entitlements, pre-seal) filed as [#270](https://github.com/ccheever/exact2/issues/270) ([Design], 2026-10-08); #270 narrowed (2026-10-08): no pre-seal hook; no clone consumer |
| [X38](closed/20261005-x38-t3-connect-clerk-sign-in.md) | T3 Connect, Clerk sign-in, relay connections, hosted pairing and the `t3code://` handoff | scope-decision | t3-connect-sign-in | closed by the user's decision (2026-10-08, "close all"): out of scope, not built; [t3-connect-sign-in](../tasks/closed/20261005-t3-connect-sign-in.md) closed with it; not filed |
| [X39](closed/20261005-x39-telemetry.md) | Telemetry: product analytics, OTLP export and the host-telemetry pipes | scope-decision | app-update-feed, t3-connect-sign-in, telemetry | closed by the user's decision (2026-10-08, "close all"): out of scope, not built; [telemetry](../tasks/closed/20261005-telemetry.md) (part 1 stays off in embedded-server-runtime) closed with it; not filed |
| [X40](closed/20261005-x40-app-update-feed.md) | The T3 desktop update feed and its UI: check, download, install, channels | scope-decision | app-update-feed, server-update-banner, telemetry | closed by the user's decision (2026-10-08, "close all"): out of scope, not built; [app-update-feed](../tasks/closed/20261005-app-update-feed.md) closed with it; not filed |
| [X41](closed/20261005-x41-wsl-environments.md) | WSL backends (Windows only) | scope-decision | wsl-environments | closed by the user's decision (2026-10-08, "close all"): out of scope, not built; [wsl-environments](../tasks/closed/20261005-wsl-environments.md) closed with it; not filed |
| [X42](closed/20261005-x42-text-blur-filter.md) | `filter: blur()` on text and boxes (blurred redacted account text) | framework-gap (unconfirmed) | provider-sign-in-and-install | supported on main `0365ad1a4`: `filter="blur(4px)"` blurs text and a box on macOS and the web; closed (2026-10-08), not filed |
| [X43](closed/20261005-x43-tristate-switch-mixed.md) | A tri-state (`mixed`) accessibility value on a switch | framework-gap (unconfirmed) | settings-scoped-controls-and-theme-editor | upstream #120 closed, not planned; workaround kept |
| [X44](20261005-x44-remote-image-policy.md) | Remote `image` loading policy (credentials, referrer, redirects, size cap, cache, load state) and remote SVG | framework-gap (unconfirmed) | provider-settings-upkeep | fixed upstream (main #177: load/error, fetch policy documented), adopted by adopt-main-fixes-shell (PR #181); SVG on Apple still an error; remote SVG on Apple reproduced on main `0365ad1a4`; not filed: open PR [#239](https://github.com/ccheever/exact2/pull/239) covers it (2026-10-08) |
| [X45](20261005-x45-app-relaunch.md) | An app cannot relaunch itself | framework-gap (unconfirmed) | local-primary-environment, this-machine-network-access | not fixed: main #170 only moves reload()'s log to stderr; no process relaunch (U4 rows stay blocked); rest filed as [#271](https://github.com/ccheever/exact2/issues/271) ([Design], 2026-10-08); #271 decided (2026-10-08): waits for main fix of #271, after #269 |

## Desktop audit addition, 2026-10-07

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X48](20261007-x48-runtime-font-family.md) | Apply an installed font family supplied by state/data at runtime | framework-gap | [installed-font-picker](../tasks/20261007-installed-font-picker.md) | reproduced locally on `fbce02624d2e33449ee2cde34497083d6fd47457`; filed as [#318](https://github.com/ccheever/exact2/issues/318) (2026-10-08, reproduced on main `b896050d7`); waits for main fix of #318 |

X48 includes a minimal compiler reproduction and successful literal-family controls.
Installed families already render when named as literals; enumeration can be supplied by
the example's native module. The blocker is applying an arbitrary runtime-selected family,
which is distinct from X10's text-rendering differences. No upstream duplicate search or
publication was performed during this local audit.

## Provider setup addition, 2026-10-07

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X49](20261007-x49-progress-value-accessibility.md) | A progress value for assistive technology (`progress`, `aria-valuenow`) | framework-gap | [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md) (nonblocking: the status text carries the numbers) | upstream [#279](https://github.com/ccheever/exact2/issues/279) ([Feature], 2026-10-08); reproduced on main `0365ad1a4`; main `d82c12252` (adopt-main-fixes-r6's merge of `e200397ec`) adds an indeterminate `progress` only and refuses `value`/`max`, so nothing to adopt; decided (2026-10-08): ARIA values first; waits for main fix of #279 |
| [X51](20261008-x51-popover-click-passthrough.md) | A click inside an open popover also reaching the page under it (macOS) | framework-gap | [theme-color-picker](../tasks/closed/20261007-theme-color-picker.md) (nonblocking: `press` + `retainFocus` on the popover) | upstream [#281](https://github.com/ccheever/exact2/issues/281) ([Bug], 2026-10-08); reproduced in a one-file app on main `0365ad1a4` (a click on the popover over the button under it); a correctness fix (2026-10-08): waits for main fix of #281; fixed by #327 (open on main, 2026-10-08); resumes in the adoption round after it merges |

## Dialog focus addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X52](20261008-x52-macos-form-controls-tab-order.md) | `input type="date"`/`"time"` and `select` as Tab stops on macOS | framework-gap | [dialog-shortcut-focus](../tasks/closed/20261008-dialog-shortcut-focus.md) (nonblocking: Custom snooze's other stops are in order) | upstream [#280](https://github.com/ccheever/exact2/issues/280) ([Bug], 2026-10-08); reproduced on main `0365ad1a4`; a correctness fix (2026-10-08): waits for main fix of #280; fixed by #327 (open on main, 2026-10-08); resumes in the adoption round after it merges |
| [X53](20261008-x53-state-driven-modal-focus.md) | A modal opened from state (`showModal(id)` on macOS and the web): focus in, Tab trapped, focus back | framework-gap | [dialog-shortcut-focus](../tasks/closed/20261008-dialog-shortcut-focus.md) (per-dialog traps meanwhile) | upstream [#282](https://github.com/ccheever/exact2/issues/282) ([Feature], 2026-10-08); reproduced on main `0365ad1a4`; a correctness fix (2026-10-08): waits for main fix of #282; #327 partial (macOS and JS/wasm web; the dialog-close event and other hosts stay open) |

## Usage pooled view addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X56](closed/20261008-x56-press-lost-on-focus-restyle.md) | A mouse click whose mouse-down focuses a button that restyles itself on `focus` loses its press (macOS) | framework-gap (unconfirmed) | none ([usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md) draws no ring of its own; the host ring stands in) | not reproduced on main `0365ad1a4` (a one-file app, six variants, agent clicks); closed (2026-10-08), not filed |

## Pull request writes addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X54](20261008-x54-focus-within-subtree.md) | An ancestor hearing the focus enter its subtree (`focusin`, `:focus-within`) | framework-gap | [pr-writing-and-metadata](../tasks/closed/20261005-pr-writing-and-metadata.md) (nonblocking: "Show full comment" and the pencil are Tab stops of their own) | upstream [#283](https://github.com/ccheever/exact2/issues/283) ([Feature], 2026-10-08); reproduced on main `0365ad1a4`; narrowed (2026-10-08): bubbling `focusin`/`focusout`; the pencil's exit uses `relatedTarget`; waits for main fix of #283 |

## Additions filed or closed in the 2026-10-08 round (X50, X55)

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X50](20261008-x50-agent-module-data-storage.md) | A native module's data folder under the agent's `--storage` (per process today, so module files do not survive an agent relaunch) | framework-gap | [pr-conversation-and-refresh](../tasks/closed/20261005-pr-conversation-and-refresh.md) (nonblocking: agent relaunch checks only) | upstream [#284](https://github.com/ccheever/exact2/issues/284) ([Bug], 2026-10-08); reproduced on main `0365ad1a4`; a correctness fix (2026-10-08): waits for main fix of #284; fixed by #327 (open on main, 2026-10-08); resumes in the adoption round after it merges |
| [X55](20261008-x55-macos-status-alert-progress-roles.md) | Live regions, alerts, modal dialogs and progress bars exposed to accessibility on macOS | framework-gap | [portable-app-download](../tasks/closed/20261005-portable-app-download.md) (the view's accessibility row; dropped 2026-10-08, so nothing waits on it) | upstream [#278](https://github.com/ccheever/exact2/issues/278) ([Bug], 2026-10-08); reproduced on main `0365ad1a4`; record copied from the closed #260 branch; a correctness fix (2026-10-08): waits for main fix of #278 |

The closed #260 branch numbered two of its own drafts X50 and X54 (distribution build paths).
Those drafts are not on this branch; here X50 and X54 are the records above.

## Pull Requests row addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X57](20261008-x57-overflowing-centred-line.md) | A line wider than its box start-aligned whatever `text-align` says (CSS Text 3 §7.1); macOS centres it and cuts its start | framework-gap | none ([pr-list-title-clip](../tasks/closed/20261008-pr-list-title-clip.md): `text-left` on the pull request surfaces; other clone buttons listed in the file) | upstream [#291](https://github.com/ccheever/exact2/issues/291) ([Bug], 2026-10-08); reproduced on main `0365ad1a4`; a correctness fix (2026-10-08): waits for main fix of #291; #327 attempt withdrawn; open |

## Provisional decisions parity addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X60](20261008-x60-number-field-semantics.md) | `input type="number"` on macOS: ArrowUp/ArrowDown stepping within `min`/`max`, refusing characters a number cannot hold | framework-gap | none ([provisional-decisions-parity](../tasks/closed/20261008-provisional-decisions-parity.md) steps the Tailscale port field itself; a typed letter still shows, with the error) | upstream [#301](https://github.com/ccheever/exact2/issues/301) ([Bug], 2026-10-08); reproduced on main `febb2c5fb` in a one-file app (no stepping, letters accepted, ax role `textbox`; the web steps, filters and says `spinbutton`); a correctness fix (2026-10-08): waits for main fix of #301 |

## Pull Requests list live refresh addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X58](closed/20261008-x58-scroll-lost-after-window-refocus.md) | A wheel-scrolled `scroll` keeping its offset after the window is focused again (macOS) | framework-gap (unconfirmed) | [pr-list-live-refresh](../tasks/closed/20261008-pr-list-live-refresh.md) (its scroll row) | not reproduced on main `0365ad1a4` in a one-file app (four variants kept the offset); closed (2026-10-08), not filed |

## Visual parity follow-up addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X59](20261008-x59-line-clamp-first-layout-ellipsis.md) | A `line-clamp` text mounted after launch painting its last line with the ellipsis (macOS paints it cut at a word until a restyle) | framework-gap | none ([visual-parity-followup](../tasks/closed/20261008-visual-parity-followup.md): the collapsed table cell's first layout; a restyle draws the ellipsis) | upstream [#300](https://github.com/ccheever/exact2/issues/300) ([Bug], 2026-10-08); reproduced on main `febb2c5fb` in a one-file app: wider than the draft (any clamped text mounted after launch, first-raster path); the agent's default `screenshot` hides it, `screenshot … window` shows it; closed by main #305 (`9314e7a81`); adopted with round 7 |

## Main adoption round 6 addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X61](20261008-x61-field-focus-ring-opt-out.md) | An app cannot remove the focus ring Exact draws on a bare text field or textarea (no `outline`; `appearance="none"` no longer opts out since main `5b2b77339`) | framework-gap | none ([adopt-main-fixes-r6](../tasks/closed/20261008-adopt-main-fixes-r6.md): the prompt preview gained a ring with the merge; the composer has had one since r4's field sheet) | upstream [#302](https://github.com/ccheever/exact2/issues/302) ([Feature] `outline: none` on `input`/`textarea`, 2026-10-08); reproduced on main `febb2c5fb` and `e200397ec` in one-file apps and in the clone; accepted (2026-10-08): waits for main fix of #302 |

## Real-input batch fixes addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X63](20261008-x63-textarea-undo-menu.md) | Edit › Undo, Edit › Redo, ⌘Z and ⇧⌘Z reaching a plain `textarea`'s own undo history on macOS (`TextArea` overrides `undoManager`; `undo:` lands on `NSWindow`) | framework-gap | none ([fix-misc-batch](../tasks/20261008-fix-misc-batch.md) routes the clone's own Edit › Undo and Redo to the focused text view's manager) | reproduced against the host's `TextArea` on main `fa965d3e2` and with real keys on the clone's base `c0475fbaa`; not #275/#276/#125 (they assume undo reaches the field); filed as [#315](https://github.com/ccheever/exact2/issues/315) (2026-10-08, reproduced on main `b896050d7`); waits for main fix of #315; fixed by #327 (open on main, 2026-10-08); resumes in the adoption round after it merges |
| [X64](20261008-x64-shrunk-paragraph-keeps-old-raster.md) | A wrapped paragraph drawn from a text raster that shrinks below the raster size (16,384 device pixels) keeps painting its old raster on macOS; the tree, `layout` and the capture path have the new text | framework-gap | none ([fix-provider-auth-state](../tasks/closed/20261008-fix-provider-auth-state.md) keys the provider list row's status and the editor's status line by status; other in-place shrinking texts can still show it) | reproduced in a one-file app on main `475043d20` (after #305) and on the clone's base `c0475fbaa`; shares `TextRasterMac.swift` with #291 and #300/#305; draft, not published; filed as [#316](https://github.com/ccheever/exact2/issues/316) on 2026-10-08; fixed by #327 (open on main, 2026-10-08); resumes in the adoption round after it merges |

## Popover-escape-parity addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X65](20261008-x65-scroll-empty-area-press.md) | A press on a `scroll` node's empty area (its port's ground, beyond the content) reaching that node and its ancestors' `pointerdown` on macOS | framework-gap | none ([popover-escape-parity](../tasks/closed/20261008-popover-escape-parity.md) puts a full-height ground under the Usage page's scroll content) | filed as [#317](https://github.com/ccheever/exact2/issues/317) (2026-10-08, reproduced on main `b896050d7`); waits for main fix of #317; was a local draft: seen in the clone's agent drive (real `NSEvent` path); main `2531fb826` unchanged on that path (read); not in #266–#302 or an upstream search; a one-file repro written, not run; #327 attempt withdrawn; open |

Re-checked on main `e200397ec` ([adopt-main-fixes-r6](../tasks/closed/20261008-adopt-main-fixes-r6.md)): since `1f19b2400` main
closed none of this plan's issues (#234 was adopted in round 5). #108, #112, #116, #117, #124, #126, #127, #130, #131,
#140, #141, #224, #235 and #266–#277 are open, and nothing main merged covers them; our drafts #227 and #228 are still
open, and main took no other route. Main's partial steps: `scroll-padding` on a virtualized list (X23, #277) and an
indeterminate `progress` (X49, #279), neither with anything for the clone to adopt.

Round 7 (not run; waits for main fix of X67, the compiler's stack overflow in main's examples test): main #305 closes
#300 (X59) and main #304 closes #285; it unblocks no T3 task today.

## Charlie's decisions (2026-10-08)

Charlie decided 40 of the plan's issues on 2026-10-08 (08:07Z). Each X file ends with a "Decided upstream
(2026-10-08)" section: the quoted decision, what it means for the clone and the next step. In short:
- **Permanent declared differences (the workaround is the design):** #100 (X1, closed), #117 (X31, closed), #124
  (X19), #130 (X12), #131 (X32, until it lands on main), #273 (X29) and #276 (X20); and the parts that #116 (X30:
  readback and transcode), #127 (X22: the measuring hooks), #224 (X28: the Dock badge) and #266 (X10: font
  smoothing) leave out.
- **Waits for main fix of #N** (then an adoption round): #112, #126, #140, #141, #235, #266, #267, #269, #271,
  #272, #274, #275, #277–#284, #286, #287, #291, #292, #301 and #302.
- **Narrowed:** #101, #112, #116, #127, #140, #141, #224, #235, #266, #268, #270 and #275.
- **A different design from the one the issue assumed:** #108 (the root rewrite), #112, #126 (runner-owned
  streams), #140, #141, #274, #277 (app-owned restoration) and #283.
- The four local "fix built" branches are superseded by the decisions or by main and not pursued (recorded only):
  X9 (#108), X19 (#124), X21 (#126) and X22 (main's `resize`, `5949b2b64`).
- Fixed on main since `e200397ec`: #285 (main #304) and #300 (X59, main #305); round 7 adopts them.

Per issue: Charlie's decision (08:07Z) and the disposition in main PR [#327](https://github.com/ccheever/exact2/pull/327), "Fix six native regressions and audit
all open issues" (open on main, 2026-10-08), with main PRs merged the same day. #285 and #300 are closed by main
#304 and #305.

| Issue | X | Charlie, 08:07Z | #327 audit and main merges |
| --- | --- | --- | --- |
| #100 | X1 | closed, not planned: "Keep a browser shell outside core" | — (closed) |
| #101 | X2 | "Allow Safari inspection of development WKWebViews; keep Exact's own inspector deferred" | main #309 merged (`f2f0e7092`): development iframe web views inspectable; issue stays open |
| #108 | X9 | "Use child state/actions to reduce the root now; defer request ownership to the existing D5 design" | request ownership deferred; "Immediate child-state/action root rewrite belongs to #303/T3 example" |
| #112 | X17 | "Choose CSS position-area and flip fallbacks for invoker popovers" | selected next core investment; larger design |
| #116 | X30 | "Keep image codecs/readback deferred; propose only the any-type file-input exception" | policy pending; codecs and readback deferred |
| #117 | X31 | closed, not planned: "Keep the immediate first window" | — (closed) |
| #124 | X19 | "Keep module timers refused; finish failure-parity repair separately" | draft PR #228 incomplete; timers stay refused |
| #126 | X21 | "Choose runner-owned bidirectional streams, not ambient WebSocket globals" | selected next core investment; #227 fixes refusals only |
| #127 | X22 | "Separate intersection visibility from anchor positioning; keep container queries and reactive geometry deferred" | design candidate; deferred parts stay |
| #130 | X12 | "Keep hatch-driven layout feedback deferred" | explicitly deferred |
| #131 | X32 | "Defer virtualized section headers until a list-lifetime design is selected" | deferred pending design; kept open |
| #140 | X25 | "Choose capture-phase key handling; decline a new keyboard fact" | approved; event syntax to design |
| #141 | X26 | "Choose declarative menu extensions in Contract" | approved larger feature; syntax to design |
| #224 | X28 | "Propose default notification clicks first; keep badges and richer actions deferred" | policy pending; most deferred |
| #235 | X26 | "Support the ContextMenu key; treat Shift+F10 as a separate shortcut choice" | main #314 merged (`d236c36d5`): the ContextMenu key; Shift+F10 open |
| #266 | X10 | "Choose balanced text and authored ::placeholder color; defer font smoothing" | approved; smoothing deferred |
| #267 | X27 | "Choose Window Controls Overlay env values and displayMode" | approved bounded platform feature |
| #268 | X5 | "Choose protocol_handlers templates over the existing navigate path" | approved; original criterion narrowed |
| #269 | X6 | "Route SIGTERM through orderly quit; propose one bounded module hold" | main #313 merged (`a3d61c023`): SIGTERM orderly; the async hold needs a ruling |
| #270 | X37 | "Fix signing-before-hashing; choose scoped derived helper entitlements and copied staging files" | approved correctness; entitlements design pending |
| #271 | X45 | "Choose an explicit process relaunch command, after orderly quit is complete" | approved larger lifecycle feature |
| #272 | X34 | "Expose the union of inline-run rectangles through frame and layout" | medium correctness candidate |
| #273 | X29 | "Add scoped app:/ iframe loading first; defer a separate PDF element" | approved scoped feature; PDF surface deferred |
| #274 | X33 | "Add selection rectangles and web-named selection commands" | approved larger text feature |
| #275 | X20 | "Add field selectionchange and beforeinput; pin range-edit semantics first" | approved; semantic choice first |
| #276 | X20 | closed, not planned: "Keep atomic chips out of plain Contract fields" | — (closed) |
| #277 | X23 | "Choose standard scroll padding/margins, native smooth jumps and scrollend; use app-owned top-list restoration" | approved larger feature; amend scope |
| #278 | X55 | "Complete admitted role mappings, implicit live announcements and macOS modal accessibility" | approved correctness; scoped amendment |
| #279 | X49 | "Add ARIA range values first; then determinate progress" | approved feature, medium |
| #280 | X52 | "Put date, time and select controls in the default Tab order" | **fixed by #327** (open on main); resumes in the adoption round after it merges |
| #281 | X51 | "Make the popover top layer consume its own pointer contacts" | **fixed by #327** (open on main); resumes in the adoption round after it merges |
| #282 | X53 | "Carry showModal(id) and close(id) on GUI hosts" | partial in #327: macOS and JS/wasm web; dialog-close event and other hosts open |
| #283 | X54 | "Add bubbling focusin/focusout with enough target information" | approved; acceptance corrected first |
| #284 | X50 | "Use the named agent store for module roots" | **fixed by #327** (open on main); resumes in the adoption round after it merges |
| #286 | side | "Queue capacity-limited source reads before execution" | approved correctness, larger |
| #287 | side | "Admit dotted and dashed borders for the named consumer" | approved feature, medium |
| #291 | X57 | "Start-align overflowing lines consistently in all text paths" | attempt withdrawn; open |
| #292 | side (X26) | "Keep native work running while NSMenu tracks" | partial in #327; full-app cadence evidence pending; open |
| #301 | X60 | "Implement number-field behavior and its adjustable accessible role" | approved correctness, medium |
| #302 | X61 | "Add the requested outline:none suppression" | approved feature, medium |
| #315 | X63 | — (filed after) | **fixed by #327** (open on main); resumes in the adoption round after it merges |
| #316 | X64 | — (filed after) | **fixed by #327** (open on main); resumes in the adoption round after it merges |
| #317 | X65 | — (filed after) | attempt withdrawn; open |
| #318 | X48 | — (filed after) | design pending (LLP 1019 interning) |
| #319 | X66 | — (filed after) | new feature; design not selected |
| #320 | X67 | — (filed after) | attempt withdrawn; open (round 7 stays blocked) |
| #321 | X68 | — (filed after) | new feature; design not selected |
| #322 | X62 | — (filed after) | **fixed by #327** (open on main); resumes in the adoption round after it merges |

Framework issues are fixed on separate branches from `main`, outside the T3 work (user, 2026-10-08). A T3 task blocked
on one resumes after that fix merges to `main` and an adoption round brings it in.


## Hover cards fix addition, 2026-10-08

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X62](20261008-x62-hover-outside-the-box.md) | macOS `hover` that counts a node's overflowing descendants (as `pointerenter`/`pointerleave` do) and follows the hit-test, so a hover card beside its trigger hears the pointer | framework-gap | none ([fix-hover-cards](../tasks/20261008-fix-hover-cards.md) draws hover cards in a window-level layer with Base UI's close delay on the root's hover clock) | filed as [#322](https://github.com/ccheever/exact2/issues/322), fixed by [#327](https://github.com/ccheever/exact2/pull/327) (open); reproduced with a one-file app and a real pointer, and seen again in fix-hover-cards' real-pointer session |
## Upstream issues (filed 2026-10-06)

Each was reproduced on exact2 `4c893fef6` before filing. Not filed: X13 (unverified on macOS; the web behavior is designed), X42 (already supported on main), X38–X41 (product scope, not framework gaps).

| Gap | Upstream | Title |
| --- | --- | --- |
| X01 | [#100](https://github.com/ccheever/exact2/issues/100) | Policy decision: an embedded browser surface an app can drive as a shell — closed upstream, not planned (2026-10-08); the Browser surface is built on a `WKWebView` in the clone's module (user) |
| X02 | [#101](https://github.com/ccheever/exact2/issues/101) | Policy decision: a developer-tools inspector for an app's own UI — narrowed (2026-10-08): development-only web-view inspection; the View item is a declared difference |
| X03 | [#102](https://github.com/ccheever/exact2/issues/102) | An app cannot set the root font size that `rem` resolves against |
| X04 | [#103](https://github.com/ccheever/exact2/issues/103) | macOS bundle cannot carry helper executables or large resource trees — closed by main #215 (`host.macos.resources`); available, not adopted here (embedded-server-runtime, portable-app-download) |
| X05 | [#104](https://github.com/ccheever/exact2/issues/104) | Custom-scheme URLs reach an app only through a navigation root, and lossily — closed by main #201, which only journals an unheard launch URL; nothing to adopt (rest: #268, 2026-10-08) |
| X06 | [#105](https://github.com/ccheever/exact2/issues/105) | macOS: a native module gets no quit hook, and destroy() never runs at quit — main #200 runs `destroy()` at quit and last-window close; **adopted** for SSH (adopt-main-fixes-r4); no bounded quit hold (rest: #269, 2026-10-08) |
| X07 | [#106](https://github.com/ccheever/exact2/issues/106) | macOS: app.json cannot set ATS keys, so an iframe cannot load http:// from a named host — **adopted** (main #173; PR #181) |
| X08 | [#107](https://github.com/ccheever/exact2/issues/107) | macOS agent mouse: no middle or triple click, wheel point, held modifiers — closed by main #186; **partly adopted** (adopt-main-fixes-r3): `tap panel-tab-<id> auxclick` drives the tab's middle click, which closed the wrong tab until this task fixed `RightPanelTabsInput`'s hit test |
| X09 | [#108](https://github.com/ccheever/exact2/issues/108) | Contract: a child component cannot own a resource or mutation — decided (2026-10-08): the root rewrite is the remedy |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Text vs Chrome: macOS code-wrap breaks, placeholder color, balance, smoothing — closed by main #208 (code-wrap breaks only); no clone workaround; balance, placeholder colour and smoothing missing (rest: #266, 2026-10-08) |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | macOS backdrop-filter misses content outside the parent; no saturate() — closed by main #221 (edges as Chrome's; adopted with no clone change, adopt-main-fixes-r5); case B and `saturate()` continue in [#225](https://github.com/ccheever/exact2/issues/225) |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | field-sizing: content ignores a hooked native text view's laid-out height — deferred (2026-10-08): a declared difference |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | A watched topic that changes faster than native.later replies starves the resource — **adopted** (main #183; adopt-main-fixes-r3): `T3ReadGate.swift` and the reader tags are gone |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | aria-keyshortcuts chords do not fire when the input source types non-Latin letters. Closed by main #168. **Not adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): #168 covers declared chords and the host's command items only; `R10Connect.swift` stays for the menu's standard items, the terminal's web view and the module's key monitors |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | macOS textarea with autocorrect="off" still turns ' into ‘ ’ and -- into —. Fixed by main #160. **Adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): the `t3-plain-text` switch-off is gone; every textarea has `autocorrect="off"` |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Popovers cannot flip near a window edge: no position-try-fallbacks, few position-area values — the next core investment (2026-10-08) |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | SVG path `d` cannot transition; macOS jumps where the web morphs — main #188; **partly adopted** (adopt-main-fixes-r3): toast copy, panel maximize, provider lock, steer/queue and table expand morph as morphicons does; code-block and welcome copy still cross-fade (keyframed `d` refused, X19) |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Data modules have no timers: `setTimeout` and `Date.now` are refused — timers stay refused (2026-10-08) |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | textarea editing: caret moves, setRangeText, beforeinput, undo, atomic ranges — closed by main #209 (field paste/copy/cut only); the native composer stays (adopt-main-fixes-r4) (rest: #275, #276, 2026-10-08) |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Data modules cannot send on a WebSocket on native hosts (receive-only) — the next core investment (2026-10-08): runner-owned streams |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Layout facts beyond size: visibility, live position, container/anchor CSS — narrowed (2026-10-08) |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Scroll: restore a top-level list by key, scroll-padding/margin, smooth jumps, plain-scroll anchoring — closed by main #210 (plain-scroll anchoring only); no clone workaround for it; the PR fold row is unblocked (rest: #277, 2026-10-08). Main `2faf6c190` (adopt-main-fixes-r6's merge of `e200397ec`) takes `scroll-padding` on a virtualized `list` only; the reference's scroll-padding sites are plain scrollers in the clone, so nothing to adopt |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | macOS: hover does not follow layout changes under a stationary pointer — **adopted** (main #174; PR #181) |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Keyboard: add keyup, KeyboardEvent.code and .repeat, held modifiers, capture phase — keyup, `code` and `repeat` landed in main #220; open for a capture-phase handler and held-modifier state; nothing to remove yet (adopt-main-fixes-r5) |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | macOS: let an app declare its menu bar items, and support context-menu submenus — submenus landed in main #223 and Edit ▸ Speech in #226, both **adopted** (adopt-main-fixes-r5: the sidebar's menus are context popovers); open for the menu bar |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | macOS window shrinks 32 pt per relaunch; no title-bar area or full-screen fact — closed after main #164 (frame restore, adopted; PR #181); title-row and full-screen facts still missing on `261dd4e10` (adopt-main-fixes-r5) (rest: #267, 2026-10-08) |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Notification click to app action, app badge, and window-focus fact (policy) — closed by main #219 (`exactPage().hasFocus`, **adopted** in adopt-main-fixes-r5); actions and badges continue in [#224](https://github.com/ccheever/exact2/issues/224) |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Show a PDF: no PDF element, and `iframe` cannot show an `app:/` or bundled PDF — closed by main #205 (bundled PDF iframe only); `PDFView` kept (adopt-main-fixes-r4) (rest: #273, 2026-10-08) |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Any-type file input and image pixel readback/re-encode in data modules (policy) — narrowed (2026-10-08) |
| X31 | [#117](https://github.com/ccheever/exact2/issues/117) | macOS: no way to hold the first window until the app says it is ready — closed upstream, not planned (2026-10-08): the connecting state stays |
| X32 | [#131](https://github.com/ccheever/exact2/issues/131) | Sticky section header over several rows of a virtualized list — deferred (2026-10-08): a declared difference until it lands on main |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Text selection: macOS button press clears it; no end rect or clear/set. Closed; main #171 fixed the button press only. Nothing to adopt ([#180](https://github.com/ccheever/exact2/pull/180)): Cite's `retainFocus` is the reference's own; no end rectangle or `clearSelection()` on main (rest: #274, 2026-10-08) |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | macOS: inline text run has no frame(), and agent hover skips inline runs. Closed; main #178 fixed the agent hover only. Nothing to adopt ([#180](https://github.com/ccheever/exact2/pull/180)); `frame()` of an inline run is still missing (rest: #272, 2026-10-08) |
| X35 | [#134](https://github.com/ccheever/exact2/issues/134) | Password input: agent tree prints its value; no autocomplete attribute. Closed by main #167. **Partly adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): the Contract password inputs are masked with no clone change; the SSH dialog keeps its native secure field, since a Contract field would carry the password through app state and the data module, which #167 leaves outside its guarantee |
| X36 | [#118](https://github.com/ccheever/exact2/issues/118) | `Intl.Locale` and its week info are missing from the macOS data runtime — fixed by main #204; **adopted** (adopt-main-fixes-r4) |
| X37 | [#119](https://github.com/ccheever/exact2/issues/119) | `exact release` skips nested executables and has no app-declared entitlements — closed by main #199 (nested signing); entitlements and a pre-seal hook missing; available, not adopted here (portable-app-download) (rest: #270, 2026-10-08) |
| X43 | [#120](https://github.com/ccheever/exact2/issues/120) | `aria-checked` refuses `"mixed"` on switch and checkbox roles. Closed, not planned: the app keeps its mixed thumb; the switch reports unchecked |
| X44 | [#121](https://github.com/ccheever/exact2/issues/121) | Remote `image`: no `load` event, silent SVG failure, undocumented fetch policy — **adopted** (main #177; PR #181); SVG on Apple still an error (rest (remote SVG on Apple): open PR #239, not filed, 2026-10-08) |
| X45 | [#122](https://github.com/ccheever/exact2/issues/122) | macOS `reload()` reboots the session in-process; no process relaunch — closed after main #170, which adds no process relaunch; not adopted (U4 relaunch rows stay blocked) (rest: #271, 2026-10-08) |
| X47 | [#179](https://github.com/ccheever/exact2/issues/179) | macOS: a custom pressable box takes keyboard focus but draws no focus ring (filed 2026-10-07; the feature branch dropped its local focus-mask patch in `4cdb8aa63`, so custom buttons show no ring until main fixes it). **Adopted**: main #189 is in the branch since adopt-main-fixes-r3, and real Tab presses draw the ring on custom pressable buttons (Settings nav), which the base does not ([real-input-checks](../tasks/closed/20261007-real-input-checks.md)); no clone workaround existed |
| side (from X7) | [#135](https://github.com/ccheever/exact2/issues/135) | macOS: a bundled iframe page cannot load any `http:` sub-resource, even on loopback — closed by main #184 (merged by adopt-main-fixes-r3); nothing to adopt: no clone page is served from `assets/` |
| side (from X3) | [#136](https://github.com/ccheever/exact2/issues/136) | Web JS target bakes `rem` and `em` to px at build time |
| side (from X3) | [#137](https://github.com/ccheever/exact2/issues/137) | A bound string `font-size` ("20px") works on web but macOS silently unsets it — closed (main #159 refuses it at compile time); the clone compiles |
| X28 (rest) | [#224](https://github.com/ccheever/exact2/issues/224) | [Policy] Notifications: a click runs an app action, and an app badge (rest of #114) — open; `T3Notifications.swift` keeps them |
| X11 (rest) | [#225](https://github.com/ccheever/exact2/issues/225) | [Design] macOS backdrop-filter: blur content beyond the parent's subtree, and saturate() (rest of #129) — closed when main #232 merged (`7fa3fa5b7`, in adopt-main-fixes-r5's merge of `1f19b2400`): `saturate()` beside `blur()` in an ordered backdrop filter. #232 says cross-parent sampling on macOS remains open, and no open issue tracks it. The flattened glass stays: adopting `saturate()` is not this round's (user decision) |
| X26 (keyboard) | [#235](https://github.com/ccheever/exact2/issues/235) | [Feature] macOS: Shift+F10 and the context-menu key open a focused node's contextPopover — filed 2026-10-07 (adopt-main-fixes-r5); the keyboard-opened row menus stay `T3Sidebar.swift`'s |
| side (build) | [#234](https://github.com/ccheever/exact2/issues/234) | [Bug] macOS build: the linked-SDK check fails for an app whose name ends in parentheses — filed 2026-10-07 (adopt-main-fixes-r5); closed by main #240 (`1f19b2400`, merged into adopt-main-fixes-r5): the check reads the executable whatever its name, and "T3 Code (Exact)" builds with no shim (adopted; no workaround left) |

## Upstream issues (filed 2026-10-08)

Each was reproduced on main `0365ad1a4` (X59–X61: `febb2c5fb`, evidence in `file-x59-x61/` at `649f5649d` on `t3-code-evidence`) with a minimal public-API app before filing; the files involved are unchanged on main `e200397ec`. `[Policy]` and `[Design]` issues carry one "Decision needed" comment. Not filed:
- X13: not reproduced as described.
- X56: not reproduced in a one-file app.
- X58: not reproduced in a one-file app.
- X42: supported on main.
- X44's rest: open PR #239 covers it.
- X38–X41: scope decisions, closed by the user ("close all", 2026-10-08).
- The timeline-keyboard test recipe: a clone recipe problem. ExactKit's `package` access needs `-package-name`; it is not a framework API.

| Gap | Upstream | Title |
| --- | --- | --- |
| X5 (rest) | [#268](https://github.com/ccheever/exact2/issues/268) | [Design] Deliver a custom-scheme URL, whole, to an app with no navigation root (rest of #104) |
| X6 (rest) | [#269](https://github.com/ccheever/exact2/issues/269) | [Design] macOS: a bounded quit hold for native module work, and SIGTERM as an orderly quit (rest of #105) |
| X10 (rest) | [#266](https://github.com/ccheever/exact2/issues/266) | [Design] Text rows: `text-wrap: balance`, an authored placeholder colour, `-webkit-font-smoothing` (rest of #128) |
| X20 (rest) | [#275](https://github.com/ccheever/exact2/issues/275) | [Feature] Text fields: selectionchange on caret moves, setRangeText and a cancelable beforeinput (rest of #125) |
| X20 (rest) | [#276](https://github.com/ccheever/exact2/issues/276) | [Policy] Atomic inline ranges (chips) over a text field's plain-string value (rest of #125) — closed upstream, not planned (2026-10-08) |
| X23 (rest) | [#277](https://github.com/ccheever/exact2/issues/277) | [Design] Scroll: restore a top-level virtualized list, scroll-padding and scroll-margin on any scroller, native smooth element jumps and scrollend (rest of #138) |
| X27 (rest) | [#267](https://github.com/ccheever/exact2/issues/267) | [Design] macOS: a title-bar area beside the traffic lights, and a full-screen fact (rest of #113) |
| X29 (rest) | [#273](https://github.com/ccheever/exact2/issues/273) | [Design] Show a PDF: an `iframe` of an `app:/` file, and a PDF element fitted to its width (rest of #115) |
| X33 (rest) | [#274](https://github.com/ccheever/exact2/issues/274) | [Feature] Text selection: its rectangles, and clearing or setting it on a text (rest of #132) |
| X34 (rest) | [#272](https://github.com/ccheever/exact2/issues/272) | [Bug] macOS: frame() of an inline text run is unavailable, and agent layout omits inline runs (rest of #133) |
| X37 (rest) | [#270](https://github.com/ccheever/exact2/issues/270) | [Design] macOS release: entitlements for the app and its nested code, and a pre-seal step (rest of #119) |
| X45 (rest) | [#271](https://github.com/ccheever/exact2/issues/271) | [Design] A host command that relaunches the app's process (rest of #122) |
| X49 | [#279](https://github.com/ccheever/exact2/issues/279) | [Feature] A progress value for assistive technology: determinate `progress` (`value`, `max`) or `aria-valuenow` |
| X50 | [#284](https://github.com/ccheever/exact2/issues/284) | [Bug] Agent drives: a native module's data folder ignores `--storage`, so its files do not survive a relaunch (macOS) |
| X51 | [#281](https://github.com/ccheever/exact2/issues/281) | [Bug] macOS: a click inside an open popover also presses the page control under it |
| X52 | [#280](https://github.com/ccheever/exact2/issues/280) | [Bug] macOS: date, time and select inputs are not Tab stops |
| X53 | [#282](https://github.com/ccheever/exact2/issues/282) | [Feature] `showModal(id)` and `close(id)` from an action on macOS and the web |
| X54 | [#283](https://github.com/ccheever/exact2/issues/283) | [Feature] `focusin`/`focusout` (or `:focus-within`): an ancestor hears the focus enter its subtree |
| X55 | [#278](https://github.com/ccheever/exact2/issues/278) | [Bug] macOS: progressbar, status, alert and modal dialog roles are not exposed to accessibility |
| side (agent driver) | [#285](https://github.com/ccheever/exact2/issues/285) | [Bug] Agent driver: `clock +N real` can ask an Apple host for a time just behind its clock ("the clock cannot go backwards") — closed by main #304 (`235164b3a`); round 7 drops the branch's `QUEUE.md` entry |
| side (native executor) | [#286](https://github.com/ccheever/exact2/issues/286) | [Bug] Native executor: a request a source makes inside its answer fails at the ordered admission limit (16), where the web queues it |
| side (borders) | [#287](https://github.com/ccheever/exact2/issues/287) | [Feature] `border-style`: `dotted` and `dashed` |
| X57 | [#291](https://github.com/ccheever/exact2/issues/291) | [Bug] macOS: a centred line wider than its box is centred and clipped at its start (CSS start-aligns it) |
| side (from X26) | [#292](https://github.com/ccheever/exact2/issues/292) | [Bug] macOS: while a context or button menu is open, main-queue work stalls, so native module calls stop until it closes |
| X59 | [#300](https://github.com/ccheever/exact2/issues/300) | [Bug] macOS: a `line-clamp` text mounted after launch paints its last line without the ellipsis until a restyle — closed by main #305 (`9314e7a81`); adopted with round 7 |
| X60 | [#301](https://github.com/ccheever/exact2/issues/301) | [Bug] macOS: `input type="number"` is a plain text field (ArrowUp/ArrowDown do not step, letters are accepted) |
| X61 | [#302](https://github.com/ccheever/exact2/issues/302) | [Feature] `outline: none` on `input` and `textarea`, to remove the focus ring Exact draws on a bare field |

## Upstream issues (filed 2026-10-08, second round)

Each was reproduced on main `b896050d7` in agent mode before filing (evidence under `file-x48-x68/` on `t3-code-evidence`).
X62 and X66 have their draft files on the branches in flight (#307, #310), so only their numbers are recorded here;
those branches update their own files. X64's file arrived with #312's merge and carries #316. X67 (the compiler's stack overflow in main's examples test)
and X68 (the element and paint lookup for the theme editor's Inspect) have no local file.

| Gap | Upstream | Title |
| --- | --- | --- |
| X48 | [#318](https://github.com/ccheever/exact2/issues/318) | [Design] `font-family` from a string at run time, so a font picker can apply any installed family |
| X62 (on #307's branch) | [#322](https://github.com/ccheever/exact2/issues/322) | [Bug] macOS: hover is not pointerenter/pointerleave — entering a descendant leaves its ancestors, and a node outside its parent's box hears no pointer — fixed by #327 (open on main) |
| X63 | [#315](https://github.com/ccheever/exact2/issues/315) | [Bug] macOS: ⌘Z and Edit › Undo do nothing in a plain `textarea` (the undo reaches the window's manager, not the field's) — fixed by #327 (open on main) |
| X64 | [#316](https://github.com/ccheever/exact2/issues/316) | [Bug] macOS: a paragraph that shrinks below the text-raster size keeps painting its old lines — fixed by #327 (open on main) |
| X65 | [#317](https://github.com/ccheever/exact2/issues/317) | [Bug] macOS: a press on a `scroll`'s empty area reaches no node, so neither it nor an ancestor hears `pointerdown` — #327 attempt withdrawn; open |
| X66 (on #310's branch) | [#319](https://github.com/ccheever/exact2/issues/319) | [Feature] `showPopover(id)`, `hidePopover(id)`, `togglePopover(id)` from an action and a popover `toggle` event (the popover sibling of #282) |
| X67 (no local file) | [#320](https://github.com/ccheever/exact2/issues/320) | [Bug] Contract compiler: unoptimized, 103 nested view sites overflow a 2 MiB thread (the parser admits 255), so the examples sweep aborts the whole test binary — #327 attempt withdrawn; open |
| X68 (no local file) | [#321](https://github.com/ccheever/exact2/issues/321) | [Feature] `elementsFromPoint(x, y)`: every node at a point, so an inspect layer can take the click and still name what it covers (theme editor Inspect) |

Local draft: [macOS native module termination](closed/20261006-native-module-termination.md) — reproduced by SSH acceptance; folded into #105 and fixed by main #200; the app workaround (T3Ssh's `willTerminateNotification` observer) is removed and the live check passed: both SSH tunnels end at ⌘W, an Apple Event quit and ⌘Q in both builds (adopt-main-fixes-r4). **Adopted.**
