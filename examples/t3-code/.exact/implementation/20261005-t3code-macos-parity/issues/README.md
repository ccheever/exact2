# Issues for 20261005-t3code-macos-parity

User rules (2026-10-05): a capability that exact2 does not support is filed as an **issue
only**; this plan writes no framework PR. Each issue explains in detail why it arose (the T3
Code behavior, what exact2 does today with evidence, where the clone hits it) and why it must
be resolved (parity impact). The goal is a complete clone, so a declared deviation is not an
end state: every issue ends resolved upstream and adopted in the app (verified by
`issue-close`), or closed by the user's decision. Policy issues (DEFERRED rules) need
Charlie's waiver; scope issues (X38–X41) need the user's decision to build or close.

Every file here is a **local draft**: not reproduced on the pinned `main`, not searched
upstream, not published. `issue-open` reproduces each one, checks for duplicates and prepares
the report; publication needs the user's approval.

Kinds: `framework-gap` (missing support), `framework-policy` (a DEFERRED rule refuses it),
`scope-decision` (excluded product scope). "(unconfirmed)" means the library does not cover
the capability and nobody has checked it yet: `issue-open` first confirms the gap and closes
the draft if the capability already works. "(unconfirmed)" means the library does not cover
the capability and nobody has checked it yet: `issue-open` first confirms the gap and closes
the draft if the capability already works. "Blocks" lists the tickets whose work or
acceptance rows wait for the issue or carry its difference until it is resolved.

| Issue | Capability | Kind | Blocks | Status |
| --- | --- | --- | --- | --- |
| [X1](20261005-x01-chromium-cdp-browser-surface.md) | An embedded browser engine that an app can drive (Browser surface) | framework-policy + framework-gap | browser-surface, right-panel-tab-menu, t3-connect-sign-in | draft |
| [X2](20261005-x02-app-developer-tools.md) | A developer-tools inspector for the app's own UI (View › Toggle Developer Tools) | framework-policy | app-developer-tools, desktop-shell-details, terminal-surface | draft |
| [X3](20261005-x03-root-font-size.md) | An app-settable root font size, the base of `rem` | framework-gap | interface-font-size, interface-font-size-conversion | draft |
| [X4](20261005-x04-bundle-helper-executables.md) | Helper executables and large resource trees in the `.app` | framework-gap | embedded-server-runtime, portable-app-download, this-machine-network-access | draft |
| [X5](20261005-x05-url-scheme-delivery.md) | A custom-scheme URL delivered to a data source or module when no route takes it | framework-gap | app-activation, managed-codex-chatgpt, provider-sign-in-and-install, t3-connect-sign-in | draft |
| [X6](20261005-x06-module-quit-shutdown.md) | A module hook at quit that can delay termination for a bounded time | framework-gap | app-activation, app-update-feed, embedded-server-runtime, managed-codex-chatgpt, telemetry | draft |
| [X7](closed/20261005-x07-ats-keys.md) | App Transport Security keys cannot be set from `app.json`, so a rendered HTML preview cannot load `http://` assets from a named host | framework-gap | media-actions | fixed upstream (main #173), adopted by adopt-main-fixes-shell (PR #181) |
| [X8](20261005-x08-agent-pointer-native-views.md) | Pointer input (down, move, up, wheel) for native views in the agent driver | framework-gap | browser-surface, diff-review-engine, floating-device-player, right-panel-tab-menu, settings-scoped-controls-and-theme-editor, sign-in-terminals, terminal-drawer, terminal-integrations, terminal-layout, terminal-surface | fix built (local branch; see file) |
| [X9](20261005-x09-root-component-across-files.md) | A child component cannot own a resource, and the root component cannot span files, so the app's data layer is capped at one 1,500-line file | framework-gap | auto-balance, client-activity-reporting, composer-fidelity, diff-review-engine, hot-file-split, interface-font-size, legacy-sidebar, live-automations-and-clones, local-primary-environment, managed-codex-chatgpt, media-actions, pr-code-tab, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, right-panel-tab-menu, server-update-banner, settings-scoped-controls-and-theme-editor, terminal-drawer, terminal-integrations, terminal-layout, thread-commands-and-keys, upstream-timeline-and-markdown, upstream-ui-sync, usage-pooled-view, usage-reset-and-feedback | fix built (local branch; see file) |
| [X10](20261005-x10-text-rendering-parity.md) | Text renders differently from Chrome in five separate ways (ellipsis, code wrap, balance, placeholder, weight) | framework-gap | interface-font-size, live-automations-and-clones, provider-settings-upkeep, shiki-residuals, upstream-timeline-and-markdown, usage-pooled-view | draft |
| [X11](20261005-x11-shadow-blur-parity.md) | Negative-spread box shadows draw too faint, and `backdrop-filter` blurs only the parent's paint, not the window below | framework-gap | auto-balance, composer-fidelity, managed-codex-chatgpt, provider-sign-in-and-install, server-update-banner, settings-scoped-controls-and-theme-editor, usage-pooled-view, usage-reset-and-feedback | draft |
| [X12](20261005-x12-textarea-field-sizing.md) | A textarea with `field-sizing: content` is sized from its plain string, not from what the native text view draws | framework-gap | composer-fidelity | draft |
| [X13](20261005-x13-hover-keys-during-pan.md) | Hover and key events keep flowing while a pan gesture owns the press | framework-gap | diff-review-engine, floating-device-player, legacy-sidebar, live-automations-and-clones, pr-code-tab, pr-handoffs-and-quick-actions, pr-links-previews-and-routing, provider-settings-upkeep, round12-wrapup, settings-scoped-controls-and-theme-editor, terminal-layout, upstream-timeline-and-markdown, usage-pooled-view, usage-reset-and-feedback | draft |
| [X14](20261005-x14-parked-native-reply.md) | A let-go answer's in-flight native request survives and hands its reply to the next answer | framework-gap | clone-on-exact2-main, managed-codex-chatgpt | draft |
| [X15](closed/20261005-x15-non-latin-key-equivalents.md) | Chords and menu equivalents match the physical key under a non-Latin input source | framework-gap | desktop-shell-details, terminal-layout, terminal-surface, thread-commands-and-keys | upstream #110 closed (main #168); workaround kept |
| [X16](closed/20261005-x16-smart-substitutions-off.md) | `autocorrect="off"` on a macOS textarea turns off spelling correction only, so AppKit still rewrites quotes, dashes and text | framework-gap | composer-fidelity, diff-review-engine, pr-writing-and-metadata | upstream #111 fixed (main #160); adopted |
| [X17](20261005-x17-popover-position-try.md) | Popover anchoring with every side/align area and automatic flip and shift (`position-area`, `position-try`) | framework-gap | auto-balance, composer-fidelity, legacy-sidebar, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, server-update-banner, upstream-ui-sync, usage-pooled-view, usage-reset-and-feedback | draft |
| [X18](closed/20261005-x18-svg-path-animation.md) | An SVG path's `d` cannot be animated, so icons cross-fade where T3 Code morphs them | framework-gap | composer-fidelity | upstream #123 fixed (main #188); not adopted |
| [X19](20261005-x19-data-source-timers.md) | Timers and a clock inside data sources (`setTimeout`, `setInterval`, `Date.now`) | framework-policy | auto-balance, client-activity-reporting, embedded-server-runtime, environment-routes, live-automations-and-clones, pr-code-tab, pr-conversation-and-refresh, pr-links-previews-and-routing, reference-logic-test-ports, reference-logic-tests-done-areas, server-update-banner, telemetry, this-machine-network-access, usage-pooled-view, usage-reset-and-feedback | fix built (local branch; see file) |
| [X20](20261005-x20-rich-text-editing.md) | No way to build an editor with inline atomic chips, caret/selection access, range replacement, paste interception and undo grouping | framework-policy | composer-fidelity, diff-review-engine, terminal-integrations, thread-commands-and-keys | fix built, phase 1 (local branch; see file) |
| [X21](20261005-x21-two-way-websocket.md) | A two-way WebSocket for data modules (send, message, close, backpressure) | framework-gap | auto-balance, browser-surface, client-activity-reporting, embedded-server-runtime, environment-routes, live-automations-and-clones, local-primary-environment, managed-codex-chatgpt, pr-code-tab, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-links-previews-and-routing, pr-writing-and-metadata, provider-settings-upkeep, provider-sign-in-and-install, remote-scopes-and-update-commands, server-update-banner, settings-scoped-controls-and-theme-editor, sign-in-terminals, t3-connect-sign-in, terminal-drawer, this-machine-network-access, thread-commands-and-keys, usage-pooled-view, usage-reset-and-feedback | fix built (local branch; see file) |
| [X22](20261005-x22-reactive-layout-facts.md) | Reactive layout facts (size, position, text width, row visibility) | framework-gap | browser-surface, composer-fidelity, diff-review-engine, floating-device-player, pr-handoffs-and-quick-actions, pr-links-previews-and-routing, settings-scoped-controls-and-theme-editor, shiki-residuals | fix built (local branch; see file) |
| [X23](20261005-x23-scroll-restore-offsets.md) | Scroll restoration by key, scroll padding/margin, animated scrollIntoView, and same-frame scroll offset compensation | framework-gap | diff-review-engine, pr-code-tab, pr-handoffs-and-quick-actions, round12-wrapup | draft |
| [X24](closed/20261005-x24-still-pointer-rehover.md) | Hover state follows layout changes under a stationary pointer | framework-gap | diff-review-engine, floating-device-player, legacy-sidebar, pr-code-tab, upstream-timeline-and-markdown | fixed upstream (main #174), adopted by adopt-main-fixes-shell (PR #181) |
| [X25](20261005-x25-keyboard-keyup-code-capture.md) | Keyboard facts for Contract: keyup, modifiers held, `code`, `repeat`, capture phase, composition end on a chord | framework-gap | browser-surface, desktop-shell-details, diff-review-engine, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks, pr-writing-and-metadata, right-panel-tab-menu, sign-in-terminals, terminal-drawer, terminal-layout, terminal-surface, thread-commands-and-keys | draft |
| [X26](20261005-x26-app-menu-control.md) | App menu control (declared application menu, hide host Go/Develop, zoom, submenus, menu at a point) | framework-gap | app-developer-tools, app-update-feed, browser-surface, desktop-shell-details, legacy-sidebar, media-actions, pr-handoffs-and-quick-actions, right-panel-tab-menu, ssh-password-and-remote-open, terminal-integrations, terminal-layout | draft |
| [X27](20261005-x27-window-chrome.md) | Title-row height and traffic-light inset, frame restore after the final style, full-screen state fact | framework-gap | desktop-shell-details | partly fixed upstream (main #164: frame restore, adopted by adopt-main-fixes-shell, PR #181); title-row and full-screen facts still missing |
| [X28](20261005-x28-notification-actions-badges.md) | Notification click → app action, Dock badge, window-focus fact (DEFERRED refuses actions and badges) | framework-policy | client-activity-reporting | draft |
| [X29](20261005-x29-video-pdf-app-files.md) | `video` and `audio` from app-written local files, and a PDF viewer element | framework-gap | media-actions | draft |
| [X30](20261005-x30-ts-announce-readback-picker.md) | Data-module topic announce and resource invalidation; pixel readback; any-type file pick with bytes and image transcode | framework-gap | composer-fidelity, media-actions, pr-conversation-and-refresh, settings-scoped-controls-and-theme-editor | draft |
| [X31](20261005-x31-deferred-window-readiness.md) | Defer the first window until the app says it is ready | framework-gap (unconfirmed) | local-primary-environment, portable-app-download | draft |
| [X32](20261005-x32-sticky-positioning-in-lists.md) | `position: sticky` inside a scroll container and a virtualized list | framework-gap (unconfirmed) | diff-review-engine, pr-code-tab, pr-conversation-and-refresh | draft |
| [X33](20261005-x33-transcript-selection-range.md) | Selected text, its source message and UTF-16 offsets, and its end rectangle from the rendered transcript | framework-gap (unconfirmed) | diff-review-engine | upstream #132 closed (main #171, part 1); parts 2–3 open on main |
| [X34](20261005-x34-inline-span-frame.md) | Hover and frame of an inline link or span inside rendered Markdown text | framework-gap (unconfirmed) | pr-links-previews-and-routing | upstream #133 closed (main #178, agent hover); inline frame open on main |
| [X35](closed/20261005-x35-secure-text-entry.md) | Secure (password) text entry in Contract | framework-gap (unconfirmed) | managed-codex-chatgpt, provider-settings-upkeep, provider-sign-in-and-install, ssh-password-and-remote-open | upstream #134 closed (main #167); SSH workaround kept |
| [X36](20261005-x36-data-runtime-intl-locale.md) | Locale-aware `Intl` and the system locale in the data runtime | framework-gap (unconfirmed) | desktop-shell-details, reference-logic-tests-done-areas | draft |
| [X37](20261005-x37-distribution-signing.md) | Developer ID signing, notarization and a pre-seal hook in the host build | framework-gap (unconfirmed) | portable-app-download | draft |
| [X38](20261005-x38-t3-connect-clerk-sign-in.md) | T3 Connect, Clerk sign-in, relay connections, hosted pairing and the `t3code://` handoff | scope-decision | t3-connect-sign-in | draft |
| [X39](20261005-x39-telemetry.md) | Telemetry: product analytics, OTLP export and the host-telemetry pipes | scope-decision | app-update-feed, t3-connect-sign-in, telemetry | draft |
| [X40](20261005-x40-app-update-feed.md) | The T3 desktop update feed and its UI: check, download, install, channels | scope-decision | app-update-feed, server-update-banner, telemetry | draft |
| [X41](20261005-x41-wsl-environments.md) | WSL backends (Windows only) | scope-decision | wsl-environments | draft |
| [X42](20261005-x42-text-blur-filter.md) | `filter: blur()` on text and boxes (blurred redacted account text) | framework-gap (unconfirmed) | provider-sign-in-and-install | draft |
| [X43](closed/20261005-x43-tristate-switch-mixed.md) | A tri-state (`mixed`) accessibility value on a switch | framework-gap (unconfirmed) | settings-scoped-controls-and-theme-editor | upstream #120 closed, not planned; workaround kept |
| [X44](20261005-x44-remote-image-policy.md) | Remote `image` loading policy (credentials, referrer, redirects, size cap, cache, load state) and remote SVG | framework-gap (unconfirmed) | provider-settings-upkeep | fixed upstream (main #177: load/error, fetch policy documented), adopted by adopt-main-fixes-shell (PR #181); SVG on Apple still an error |
| [X45](20261005-x45-app-relaunch.md) | An app cannot relaunch itself | framework-gap (unconfirmed) | local-primary-environment, this-machine-network-access | not fixed: main #170 only moves reload()'s log to stderr; no process relaunch (U4 rows stay blocked) |

## Upstream issues (filed 2026-10-06)

Each was reproduced on exact2 `4c893fef6` before filing. Not filed: X13 (unverified on macOS; the web behavior is designed), X42 (already supported on main), X38–X41 (product scope, not framework gaps).

| Gap | Upstream | Title |
| --- | --- | --- |
| X01 | [#100](https://github.com/ccheever/exact2/issues/100) | Policy decision: an embedded browser surface an app can drive as a shell |
| X02 | [#101](https://github.com/ccheever/exact2/issues/101) | Policy decision: a developer-tools inspector for an app's own UI |
| X03 | [#102](https://github.com/ccheever/exact2/issues/102) | An app cannot set the root font size that `rem` resolves against |
| X04 | [#103](https://github.com/ccheever/exact2/issues/103) | macOS bundle cannot carry helper executables or large resource trees |
| X05 | [#104](https://github.com/ccheever/exact2/issues/104) | Custom-scheme URLs reach an app only through a navigation root, and lossily |
| X06 | [#105](https://github.com/ccheever/exact2/issues/105) | macOS: a native module gets no quit hook, and destroy() never runs at quit |
| X07 | [#106](https://github.com/ccheever/exact2/issues/106) | macOS: app.json cannot set ATS keys, so an iframe cannot load http:// from a named host — **adopted** (main #173; PR #181) |
| X08 | [#107](https://github.com/ccheever/exact2/issues/107) | macOS agent mouse: no middle or triple click, wheel point, held modifiers |
| X09 | [#108](https://github.com/ccheever/exact2/issues/108) | Contract: a child component cannot own a resource or mutation |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Text vs Chrome: macOS code-wrap breaks, placeholder color, balance, smoothing |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | macOS backdrop-filter misses content outside the parent; no saturate() |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | field-sizing: content ignores a hooked native text view's laid-out height |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | A watched topic that changes faster than native.later replies starves the resource |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | aria-keyshortcuts chords do not fire when the input source types non-Latin letters. Closed by main #168. **Not adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): #168 covers declared chords and the host's command items only; `R10Connect.swift` stays for the menu's standard items, the terminal's web view and the module's key monitors |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | macOS textarea with autocorrect="off" still turns ' into ‘ ’ and -- into —. Fixed by main #160. **Adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): the `t3-plain-text` switch-off is gone; every textarea has `autocorrect="off"` |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Popovers cannot flip near a window edge: no position-try-fallbacks, few position-area values |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | SVG path `d` cannot transition; macOS jumps where the web morphs. Fixed by main #188. **Not adopted**: the feature branch does not yet contain it |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Data modules have no timers: `setTimeout` and `Date.now` are refused |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | textarea editing: caret moves, setRangeText, beforeinput, undo, atomic ranges |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Data modules cannot send on a WebSocket on native hosts (receive-only) |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Layout facts beyond size: visibility, live position, container/anchor CSS |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Scroll: restore a top-level list by key, scroll-padding/margin, smooth jumps, plain-scroll anchoring |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | macOS: hover does not follow layout changes under a stationary pointer — **adopted** (main #174; PR #181) |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Keyboard: add keyup, KeyboardEvent.code and .repeat, held modifiers, capture phase |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | macOS: let an app declare its menu bar items, and support context-menu submenus |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | macOS window shrinks 32 pt per relaunch; no title-bar area or full-screen fact — **partly adopted** (main #164 frame restore; PR #181); title-row and full-screen facts missing |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Notification click to app action, app badge, and window-focus fact (policy) |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Show a PDF: no PDF element, and `iframe` cannot show an `app:/` or bundled PDF |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Any-type file input and image pixel readback/re-encode in data modules (policy) |
| X31 | [#117](https://github.com/ccheever/exact2/issues/117) | macOS: no way to hold the first window until the app says it is ready |
| X32 | [#131](https://github.com/ccheever/exact2/issues/131) | Sticky section header over several rows of a virtualized list |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Text selection: macOS button press clears it; no end rect or clear/set. Closed; main #171 fixed the button press only. Nothing to adopt ([#180](https://github.com/ccheever/exact2/pull/180)): Cite's `retainFocus` is the reference's own; no end rectangle or `clearSelection()` on main |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | macOS: inline text run has no frame(), and agent hover skips inline runs. Closed; main #178 fixed the agent hover only. Nothing to adopt ([#180](https://github.com/ccheever/exact2/pull/180)); `frame()` of an inline run is still missing |
| X35 | [#134](https://github.com/ccheever/exact2/issues/134) | Password input: agent tree prints its value; no autocomplete attribute. Closed by main #167. **Partly adopted** ([#180](https://github.com/ccheever/exact2/pull/180)): the Contract password inputs are masked with no clone change; the SSH dialog keeps its native secure field, since a Contract field would carry the password through app state and the data module, which #167 leaves outside its guarantee |
| X36 | [#118](https://github.com/ccheever/exact2/issues/118) | `Intl.Locale` and its week info are missing from the macOS data runtime |
| X37 | [#119](https://github.com/ccheever/exact2/issues/119) | `exact release` skips nested executables and has no app-declared entitlements |
| X43 | [#120](https://github.com/ccheever/exact2/issues/120) | `aria-checked` refuses `"mixed"` on switch and checkbox roles. Closed, not planned: the app keeps its mixed thumb; the switch reports unchecked |
| X44 | [#121](https://github.com/ccheever/exact2/issues/121) | Remote `image`: no `load` event, silent SVG failure, undocumented fetch policy — **adopted** (main #177; PR #181); SVG on Apple still an error |
| X45 | [#122](https://github.com/ccheever/exact2/issues/122) | macOS `reload()` reboots the session in-process; no process relaunch — closed after main #170, which adds no process relaunch; not adopted (U4 relaunch rows stay blocked) |
| X47 | [#179](https://github.com/ccheever/exact2/issues/179) | macOS: a custom pressable box takes keyboard focus but draws no focus ring (filed 2026-10-07; the feature branch dropped its local focus-mask patch in `4cdb8aa63`, so custom buttons show no ring until main fixes it) |
| side (from X7) | [#135](https://github.com/ccheever/exact2/issues/135) | macOS: a bundled iframe page cannot load any `http:` sub-resource, even on loopback — still open |
| side (from X3) | [#136](https://github.com/ccheever/exact2/issues/136) | Web JS target bakes `rem` and `em` to px at build time |
| side (from X3) | [#137](https://github.com/ccheever/exact2/issues/137) | A bound string `font-size` ("20px") works on web but macOS silently unsets it — closed (main #159 refuses it at compile time); the clone compiles |

Local draft: [macOS native module termination](20261006-native-module-termination.md) — reproduced by SSH acceptance; app workaround, upstream report not published.
