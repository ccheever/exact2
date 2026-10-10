# Issues for 20261005-t3code-macos-parity

Since 2026-10-09 main tracks Exact's open gaps as files in its `issues/` folder, and its
[`docs/issues.md`](https://github.com/ccheever/exact2/blob/main/docs/issues.md) says "Do not track the same live state in
two places". So a plan record whose gap is a main issue file is closed here with status `moved-to-main` (user decision,
2026-10-10): it sits in `closed/` and keeps only the T3 history (why the gap arose, the clone's workaround, the
evidence), and the main file is the gap's status. This README maps each such record to its main file, the T3 tasks it
blocks and what the clone does meanwhile. It copies no status: read the main file for that.

The user's rules stand: a capability Exact does not support is filed as an issue, and this plan writes no framework PR
(2026-10-05); framework fixes land on main-based branches outside the T3 work, and a blocked T3 task resumes after the
fix merges to `main` and a main-adoption round brings it in (2026-10-08).

## Plan records moved to main

The main issue links point at `main`; the number after each is the GitHub issue main transferred into that file. Tasks
in bold are open (in `../tasks/`); the rest are closed and carry the workaround.

| X | Main issue | T3 tasks | Clone workaround or declared difference |
| --- | --- | --- | --- |
| [X5](closed/20261005-x05-url-scheme-delivery.md) | [20261009-protocol-handler-url-templates.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-protocol-handler-url-templates.md) ([#268](https://github.com/ccheever/exact2/issues/268)) | [app-activation](../tasks/closed/20261005-app-activation.md), [managed-codex-chatgpt](../tasks/closed/20261005-managed-codex-chatgpt.md), [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md), [t3-connect-sign-in](../tasks/closed/20261005-t3-connect-sign-in.md) | None needed: no scheme is registered, and no consumer is left since T3 Connect (X38) was dropped and U10 decided |
| [X6](closed/20261005-x06-module-quit-shutdown.md) | [20261009-macos-orderly-sigterm-quit-hold.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-orderly-sigterm-quit-hold.md) ([#269](https://github.com/ccheever/exact2/issues/269)) | [app-activation](../tasks/closed/20261005-app-activation.md), [app-update-feed](../tasks/closed/20261005-app-update-feed.md), [embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md), [managed-codex-chatgpt](../tasks/closed/20261005-managed-codex-chatgpt.md), [telemetry](../tasks/closed/20261005-telemetry.md) | The embedded server stops in the synchronous `destroy()` (SIGTERM, then SIGKILL after 2 s); the pid-file reaper and the `atexit` stop stay |
| [X10](closed/20261005-x10-text-rendering-parity.md) | [20261009-balanced-text-placeholder-color.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-balanced-text-placeholder-color.md) ([#266](https://github.com/ccheever/exact2/issues/266)) | [interface-font-size](../tasks/closed/20261005-interface-font-size.md), [live-automations-and-clones](../tasks/closed/20261005-live-automations-and-clones.md), [provider-settings-upkeep](../tasks/closed/20261005-provider-settings-upkeep.md), [shiki-residuals](../tasks/closed/20261005-shiki-residuals.md), [upstream-timeline-and-markdown](../tasks/closed/20261005-upstream-timeline-and-markdown.md), [usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md) | None: balance and the placeholder colour are not drawn; font smoothing is a permanent declared difference |
| [X11](closed/20261005-x11-shadow-blur-parity.md) | [20261010-macos-backdrop-beyond-parent.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-macos-backdrop-beyond-parent.md) (case B of [#129](https://github.com/ccheever/exact2/issues/129)) | [auto-balance](../tasks/closed/20261005-auto-balance.md), [composer-fidelity](../tasks/closed/20261005-composer-fidelity.md), [managed-codex-chatgpt](../tasks/closed/20261005-managed-codex-chatgpt.md), [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md), [server-update-banner](../tasks/closed/20261005-server-update-banner.md), [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md), [usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md), [usage-reset-and-feedback](../tasks/closed/20261005-usage-reset-and-feedback.md) | Flattened opaque glass (composer card, picker, toasts, PR tooltips, dialogs); `saturate()` from main #232 not adopted (user decision) |
| [X17](closed/20261005-x17-popover-position-try.md) | [20261009-popover-css-flip-fallbacks.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-popover-css-flip-fallbacks.md) ([#112](https://github.com/ccheever/exact2/issues/112)) | [auto-balance](../tasks/closed/20261005-auto-balance.md), [composer-fidelity](../tasks/closed/20261005-composer-fidelity.md), [legacy-sidebar](../tasks/closed/20261005-legacy-sidebar.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [pr-header-actions-and-stacks](../tasks/closed/20261005-pr-header-actions-and-stacks.md), [pr-links-previews-and-routing](../tasks/closed/20261005-pr-links-previews-and-routing.md), [pr-writing-and-metadata](../tasks/closed/20261005-pr-writing-and-metadata.md), [provider-settings-upkeep](../tasks/closed/20261005-provider-settings-upkeep.md), [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md), [server-update-banner](../tasks/closed/20261005-server-update-banner.md), [upstream-ui-sync](../tasks/closed/20261005-upstream-ui-sync.md), [usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md), [usage-reset-and-feedback](../tasks/closed/20261005-usage-reset-and-feedback.md) | Fixed placement per call site; hover layers place with `hoverFlip` (#307); no new per-site flip arithmetic |
| [X19](closed/20261005-x19-data-source-timers.md) | [20261009-typescript-failure-parity.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-typescript-failure-parity.md) ([#124](https://github.com/ccheever/exact2/issues/124)) | [auto-balance](../tasks/closed/20261005-auto-balance.md), [client-activity-reporting](../tasks/closed/20261005-client-activity-reporting.md), [embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md), [environment-routes](../tasks/closed/20261005-environment-routes.md), [live-automations-and-clones](../tasks/closed/20261005-live-automations-and-clones.md), [pr-code-tab](../tasks/closed/20261005-pr-code-tab.md), [pr-conversation-and-refresh](../tasks/closed/20261005-pr-conversation-and-refresh.md), [pr-links-previews-and-routing](../tasks/closed/20261005-pr-links-previews-and-routing.md), [reference-logic-test-ports](../tasks/closed/20261005-reference-logic-test-ports.md), [reference-logic-tests-done-areas](../tasks/closed/20261005-reference-logic-tests-done-areas.md), [server-update-banner](../tasks/closed/20261005-server-update-banner.md), [telemetry](../tasks/closed/20261005-telemetry.md), [this-machine-network-access](../tasks/closed/20261005-this-machine-network-access.md), [usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md), [usage-reset-and-feedback](../tasks/closed/20261005-usage-reset-and-feedback.md) | A permanent declared difference: time passed as arguments, gated tasks, and Swift timers in the module |
| [X20](closed/20261005-x20-rich-text-editing.md) | [20261009-field-selection-beforeinput-range-edit.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-field-selection-beforeinput-range-edit.md) ([#275](https://github.com/ccheever/exact2/issues/275)) | [composer-fidelity](../tasks/closed/20261005-composer-fidelity.md), [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md), [terminal-integrations](../tasks/closed/20261005-terminal-integrations.md), [thread-commands-and-keys](../tasks/closed/20261005-thread-commands-and-keys.md) | The native `NSTextView` composer (`T3Composer*.swift`); its atomic chips are a permanent declared difference (#276 not planned) |
| [X21](closed/20261005-x21-two-way-websocket.md) | [20261009-runner-owned-bidirectional-streams.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-runner-owned-bidirectional-streams.md) ([#126](https://github.com/ccheever/exact2/issues/126)) | [auto-balance](../tasks/closed/20261005-auto-balance.md), [browser-surface](../tasks/closed/20261005-browser-surface.md), [client-activity-reporting](../tasks/closed/20261005-client-activity-reporting.md), [embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md), [environment-routes](../tasks/closed/20261005-environment-routes.md), [live-automations-and-clones](../tasks/closed/20261005-live-automations-and-clones.md), [local-primary-environment](../tasks/closed/20261005-local-primary-environment.md), [managed-codex-chatgpt](../tasks/closed/20261005-managed-codex-chatgpt.md), [pr-code-tab](../tasks/closed/20261005-pr-code-tab.md), [pr-conversation-and-refresh](../tasks/closed/20261005-pr-conversation-and-refresh.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [pr-header-actions-and-stacks](../tasks/closed/20261005-pr-header-actions-and-stacks.md), [pr-links-previews-and-routing](../tasks/closed/20261005-pr-links-previews-and-routing.md), [pr-writing-and-metadata](../tasks/closed/20261005-pr-writing-and-metadata.md), [provider-settings-upkeep](../tasks/closed/20261005-provider-settings-upkeep.md), [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md), [remote-scopes-and-update-commands](../tasks/closed/20261005-remote-scopes-and-update-commands.md), [server-update-banner](../tasks/closed/20261005-server-update-banner.md), [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md), [sign-in-terminals](../tasks/closed/20261005-sign-in-terminals.md), [t3-connect-sign-in](../tasks/closed/20261005-t3-connect-sign-in.md), [terminal-drawer](../tasks/closed/20261005-terminal-drawer.md), [this-machine-network-access](../tasks/closed/20261005-this-machine-network-access.md), [thread-commands-and-keys](../tasks/closed/20261005-thread-commands-and-keys.md), [usage-pooled-view](../tasks/closed/20261005-usage-pooled-view.md), [usage-reset-and-feedback](../tasks/closed/20261005-usage-reset-and-feedback.md) | The app-local Swift transport (`T3Transport`, `T3Protocol`, `T3Fleet`) with a polled inbox |
| [X22](closed/20261005-x22-reactive-layout-facts.md) | [20261009-intersection-visibility-event-design.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-intersection-visibility-event-design.md) ([#127](https://github.com/ccheever/exact2/issues/127)) | [browser-surface](../tasks/closed/20261005-browser-surface.md), [composer-fidelity](../tasks/closed/20261005-composer-fidelity.md), [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md), [floating-device-player](../tasks/closed/20261005-floating-device-player.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [pr-links-previews-and-routing](../tasks/closed/20261005-pr-links-previews-and-routing.md), [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md), [shiki-residuals](../tasks/closed/20261005-shiki-residuals.md) | Native measuring hooks (`t3-frame`, `t3-anchor`, `t3-measure`, `t3-turn`), a permanent declared difference (#127 decision) |
| [X23](closed/20261005-x23-scroll-restore-offsets.md) | [20261009-scroll-padding-settlement-restore.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-scroll-padding-settlement-restore.md) ([#277](https://github.com/ccheever/exact2/issues/277)) | [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md), [pr-code-tab](../tasks/closed/20261005-pr-code-tab.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [round12-wrapup](../tasks/closed/20261005-round12-wrapup.md) | `R9Input.swift` restores each thread's position and `T3TimelineTurns.swift` holds jumps; top-list restoration stays the app's |
| [X25](closed/20261005-x25-keyboard-keyup-code-capture.md) | [20261009-capture-phase-key-events.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-capture-phase-key-events.md) ([#140](https://github.com/ccheever/exact2/issues/140)) | [browser-surface](../tasks/closed/20261005-browser-surface.md), [desktop-shell-details](../tasks/closed/20261005-desktop-shell-details.md), [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [pr-header-actions-and-stacks](../tasks/closed/20261005-pr-header-actions-and-stacks.md), [pr-writing-and-metadata](../tasks/closed/20261005-pr-writing-and-metadata.md), [right-panel-tab-menu](../tasks/closed/20261005-right-panel-tab-menu.md), [sign-in-terminals](../tasks/closed/20261005-sign-in-terminals.md), [terminal-drawer](../tasks/closed/20261005-terminal-drawer.md), [terminal-layout](../tasks/closed/20261005-terminal-layout.md), [terminal-surface](../tasks/closed/20261005-terminal-surface.md), [thread-commands-and-keys](../tasks/closed/20261005-thread-commands-and-keys.md) | Swift key monitors (`T3Sidebar`, `T3ComposerIntent`, `R8KeysLauncher`, `T3KeyRecorder`, `RightPanelTabsInput`); ⌘W repeat and the ⌘Q hold stay native |
| [X26](closed/20261005-x26-app-menu-control.md) | [20261009-contract-menu-bar-extensions.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-contract-menu-bar-extensions.md) ([#141](https://github.com/ccheever/exact2/issues/141)) | [app-developer-tools](../tasks/closed/20261005-app-developer-tools.md), [app-update-feed](../tasks/closed/20261005-app-update-feed.md), [browser-surface](../tasks/closed/20261005-browser-surface.md), [desktop-shell-details](../tasks/closed/20261005-desktop-shell-details.md), [legacy-sidebar](../tasks/closed/20261005-legacy-sidebar.md), [media-actions](../tasks/closed/20261005-media-actions.md), [pr-handoffs-and-quick-actions](../tasks/closed/20261005-pr-handoffs-and-quick-actions.md), [right-panel-tab-menu](../tasks/closed/20261005-right-panel-tab-menu.md), [ssh-password-and-remote-open](../tasks/closed/20261005-ssh-password-and-remote-open.md), [terminal-integrations](../tasks/closed/20261005-terminal-integrations.md), [terminal-layout](../tasks/closed/20261005-terminal-layout.md) | Menu-bar surgery in `R8KeysMenus.swift` and `T3Menus.swift`; Shift+F10 and the module's own `NSMenu`s (`T3Sidebar`, `T3ContextMenu`) |
| [X27](closed/20261005-x27-window-chrome.md) | [20261009-titlebar-env-display-mode.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-titlebar-env-display-mode.md) ([#267](https://github.com/ccheever/exact2/issues/267)) | [desktop-shell-details](../tasks/closed/20261005-desktop-shell-details.md) | `T3WindowChrome.swift` (the 52 pt title row) and `T3FullScreen.swift` (the full-screen fact) |
| [X28](closed/20261005-x28-notification-actions-badges.md) | [20261009-notification-default-click-design.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-notification-default-click-design.md) ([#224](https://github.com/ccheever/exact2/issues/224)) | [client-activity-reporting](../tasks/closed/20261005-client-activity-reporting.md) | `T3Notifications.swift` handles the click; the Dock badge is a permanent declared difference; focus from `hasFocus` (main #219, adopted) |
| [X29](closed/20261005-x29-video-pdf-app-files.md) | [20261009-app-file-iframe-loading.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-app-file-iframe-loading.md) ([#273](https://github.com/ccheever/exact2/issues/273)) | [media-actions](../tasks/closed/20261005-media-actions.md) | `AVPlayerView` and `PDFView` in the module; `PDFView` is a permanent declared difference |
| [X30](closed/20261005-x30-ts-announce-readback-picker.md) | [20261009-any-type-file-input-ruling.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-any-type-file-input-ruling.md) ([#116](https://github.com/ccheever/exact2/issues/116)) | [composer-fidelity](../tasks/closed/20261005-composer-fidelity.md), [media-actions](../tasks/closed/20261005-media-actions.md), [pr-conversation-and-refresh](../tasks/closed/20261005-pr-conversation-and-refresh.md), [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md) | `r10Wake` announce, `T3ImageAccent` averaging, `NSOpenPanel` attach; pixel readback and transcode are a permanent declared difference |
| [X33](closed/20261005-x33-transcript-selection-range.md) | [20261009-text-selection-rectangles-commands.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-text-selection-rectangles-commands.md) ([#274](https://github.com/ccheever/exact2/issues/274)) | [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md) | Cite shows under the selection's block, not at its end; `retainFocus` keeps the selection, which is not cleared after citing |
| [X34](closed/20261005-x34-inline-span-frame.md) | [20261009-apple-inline-run-frame-layout.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-apple-inline-run-frame-layout.md) ([#272](https://github.com/ccheever/exact2/issues/272)) | [pr-links-previews-and-routing](../tasks/closed/20261005-pr-links-previews-and-routing.md) | The PR-link hover card anchors to the link chip's own box (`hoverTipAtFrame`), not the inline run |
| [X37](closed/20261005-x37-distribution-signing.md) | [20261009-macos-release-digest-entitlements.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-release-digest-entitlements.md) ([#270](https://github.com/ccheever/exact2/issues/270)) | [portable-app-download](../tasks/closed/20261005-portable-app-download.md) | Ad-hoc signing (U11); no consumer left since portable-app-download was dropped |
| [X45](closed/20261005-x45-app-relaunch.md) | [20261009-native-process-relaunch.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-native-process-relaunch.md) ([#271](https://github.com/ccheever/exact2/issues/271)) | [local-primary-environment](../tasks/closed/20261005-local-primary-environment.md), [this-machine-network-access](../tasks/closed/20261005-this-machine-network-access.md) | The embedded server restarts in place and the window reconnects; the U4 relaunch rows stay blocked |
| [X48](closed/20261007-x48-runtime-font-family.md) | [20261009-runtime-installed-font-family.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-runtime-installed-font-family.md) ([#318](https://github.com/ccheever/exact2/issues/318)) | **[installed-font-picker](../tasks/20261007-installed-font-picker.md)** | None: the picker keeps its generic-only catalog, and the task stays blocked |
| [X49](closed/20261007-x49-progress-value-accessibility.md) | [20261009-aria-range-accessibility-values.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-aria-range-accessibility-values.md) ([#279](https://github.com/ccheever/exact2/issues/279)) | [provider-sign-in-and-install](../tasks/closed/20261005-provider-sign-in-and-install.md), **[provider-sign-in-verification-followup](../tasks/20261008-provider-sign-in-verification-followup.md)** | A drawn bar with `role="progressbar"` and the percentage in `aria-description` |
| [X50](closed/20261008-x50-agent-module-data-storage.md) | [20261009-agent-native-module-storage-roots.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-agent-native-module-storage-roots.md) ([#284](https://github.com/ccheever/exact2/issues/284)) | [pr-conversation-and-refresh](../tasks/closed/20261005-pr-conversation-and-refresh.md) | `T3Storage.dataRoot(agent:)` re-roots module data under `TMPDIR` in agent mode; fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) |
| [X51](closed/20261008-x51-popover-click-passthrough.md) | [20261009-macos-popover-pointer-passthrough.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-popover-pointer-passthrough.md) ([#281](https://github.com/ccheever/exact2/issues/281)) | [theme-color-picker](../tasks/closed/20261007-theme-color-picker.md), [fix-hover-cards](../tasks/closed/20261008-fix-hover-cards.md) | `press` + `retainFocus` on the colour popover, so the click ends there; fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) |
| [X52](closed/20261008-x52-macos-form-controls-tab-order.md) | [20261009-macos-controls-default-tab-order.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-controls-default-tab-order.md) ([#280](https://github.com/ccheever/exact2/issues/280)) | [dialog-shortcut-focus](../tasks/closed/20261008-dialog-shortcut-focus.md) | None needed (the dialog's other Tab stops are in order); fix in open PR [#327](https://github.com/ccheever/exact2/pull/327), then re-drive the Custom snooze Tab cycle |
| [X53](closed/20261008-x53-state-driven-modal-focus.md) | [20261009-gui-dialog-action-commands.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-gui-dialog-action-commands.md) ([#282](https://github.com/ccheever/exact2/issues/282)) | [dialog-shortcut-focus](../tasks/closed/20261008-dialog-shortcut-focus.md) | Each dialog traps Tab itself (#310's focus plumbing); fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) (partial: macOS and the JS/wasm web) |
| [X54](closed/20261008-x54-focus-within-subtree.md) | [20261009-bubbling-focus-events.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-bubbling-focus-events.md) ([#283](https://github.com/ccheever/exact2/issues/283)) | [pr-writing-and-metadata](../tasks/closed/20261005-pr-writing-and-metadata.md) | "Show full comment" and the pencil are Tab stops of their own |
| [X55](closed/20261008-x55-macos-status-alert-progress-roles.md) | [20261009-macos-role-modal-accessibility.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-role-modal-accessibility.md) ([#278](https://github.com/ccheever/exact2/issues/278)) | [portable-app-download](../tasks/closed/20261005-portable-app-download.md) | None: no clone view waits on it (portable-app-download was dropped) |
| [X57](closed/20261008-x57-overflowing-centred-line.md) | [20261009-apple-overflow-line-alignment.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-apple-overflow-line-alignment.md) ([#291](https://github.com/ccheever/exact2/issues/291)) | [pr-list-title-clip](../tasks/closed/20261008-pr-list-title-clip.md) | `text-align="left"` on the pull request surfaces, where the reference has `text-left` |
| [X60](closed/20261008-x60-number-field-semantics.md) | [20261009-macos-number-field-behavior.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-number-field-behavior.md) ([#301](https://github.com/ccheever/exact2/issues/301)) | [provisional-decisions-parity](../tasks/closed/20261008-provisional-decisions-parity.md) | `tsStep` steps the Tailscale port field; letters are not filtered and show with the error |
| [X61](closed/20261008-x61-field-focus-ring-opt-out.md) | [20261009-field-outline-none.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-field-outline-none.md) ([#302](https://github.com/ccheever/exact2/issues/302)) | [editable-font-prompt-preview](../tasks/closed/20261007-editable-font-prompt-preview.md), [adopt-main-fixes-r6](../tasks/closed/20261008-adopt-main-fixes-r6.md) | None: the ring shows on the composer, the prompt preview and bare fields (a visible difference) |
| [X62](closed/20261008-x62-hover-outside-the-box.md) | [20261009-macos-subtree-hover.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-subtree-hover.md) ([#322](https://github.com/ccheever/exact2/issues/322)) | [fix-hover-cards](../tasks/closed/20261008-fix-hover-cards.md) | A window-level hover layer (`hover-layer.contract`, #307), which stays for its clipping fix; fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) |
| [X63](closed/20261008-x63-textarea-undo-menu.md) | [20261009-macos-textarea-native-undo.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-textarea-native-undo.md) ([#315](https://github.com/ccheever/exact2/issues/315)) | **[fix-misc-batch](../tasks/closed/20261008-fix-misc-batch.md)** | The clone's own Edit › Undo and Redo (`R8KeysMenus.swift`, #306) act on the focused text view's manager; fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) |
| [X64](closed/20261008-x64-shrunk-paragraph-keeps-old-raster.md) | [20261009-macos-small-text-stale-raster.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-small-text-stale-raster.md) ([#316](https://github.com/ccheever/exact2/issues/316)) | [fix-provider-auth-state](../tasks/closed/20261008-fix-provider-auth-state.md) | The provider row's and editor's status texts are keyed by status (#312); fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) |
| [X65](closed/20261008-x65-scroll-empty-area-press.md) | [20261009-macos-empty-scroll-pointer.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-empty-scroll-pointer.md) ([#317](https://github.com/ccheever/exact2/issues/317)) | [popover-escape-parity](../tasks/closed/20261008-popover-escape-parity.md) | A full-height `usage-ground` under the Usage page's scroll content |
| [X66](closed/20261008-x66-popover-from-action-and-toggle.md) | [20261009-popover-action-commands.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-popover-action-commands.md) ([#319](https://github.com/ccheever/exact2/issues/319)) | [fix-keyboard-focus](../tasks/closed/20261008-fix-keyboard-focus.md) | `KeyMenuOpen`'s invisible invokers and the keyboard-open counts (#310) |
| [X69](closed/20261009-x69-paint-role-node-query.md) | [20261010-paint-role-node-list-read.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-paint-role-node-list-read.md) (main #386) | **[blocked-theme-usage-highlight](../tasks/20261009-blocked-theme-usage-highlight.md)** | None: the theme editor's usage outline and count are a declared difference |
| [X70](closed/20261010-x70-aria-disabled-focusable.md) | [20261010-aria-disabled-focusable-control.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-aria-disabled-focusable-control.md) (main #386) | [blocked-desktop-update-controls](../tasks/closed/20261009-blocked-desktop-update-controls.md) | The disabled "Check for updates" control is skipped by Tab; its label, dimmed look, tooltip and no-op press match (#368) |
| [X71](closed/20261010-x71-pointermove-without-press-capture.md) | [20261010-pointer-events-reach-ancestors.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-pointer-events-reach-ancestors.md) (main #386) | [audit-wave-followups-4](../tasks/closed/20261010-audit-wave-followups-4.md) | Painted menus move the highlight only when the pointer enters a row (#383) |
| [X72](closed/20261010-x72-apple-intl-half-even.md) | [20261010-apple-intl-number-format-half-even.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-apple-intl-number-format-half-even.md) (main #394) | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) | `formatUsd` (`pages-usage.ts`) rounds the shortest digits half away from zero itself, with Bun's ICU `Intl.NumberFormat` as its test oracle |
| [X73](closed/20261010-x73-macos-popover-escape-after-blur.md) | [20261010-macos-popover-escape-after-blur.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-macos-popover-escape-after-blur.md) (main #401) | **[realinput-1010c-fixes](../tasks/closed/20261010-realinput-1010c-fixes.md)** (RC-3) | The Filters submenu's dropped focus lands on a 1-pt rest box inside the popover (`pr-filters-rest`, `pages-prs.contract` `PrFiltersMenu`), not on nothing, so the next Escape closes Filters (#399) |
| [X74](closed/20261010-x74-macos-heading-inside-button.md) | [20261010-macos-a-heading-inside-a-button.md](https://github.com/ccheever/exact2/blob/main/issues/20261010-macos-a-heading-inside-a-button.md) (main #405) | [settings-headings](../tasks/closed/20261010-settings-headings.md) | Legacy features' level-2 heading is an sr-only heading right before its button (`SettingsSrHeading`, `settings-kit.contract`; `settings-rows.contract` `CoreSections`), not inside it (#404) |
| X67 (no record) | [20261009-compiler-small-stack-depth.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-compiler-small-stack-depth.md) ([#320](https://github.com/ccheever/exact2/issues/320)) | [view-depth-under-test-stack](../tasks/closed/20261009-view-depth-under-test-stack.md), **[clone-on-exact2-main](../tasks/20261005-clone-on-exact2-main.md)** | The clone's views were flattened under main's 2 MiB test-thread stack (#382) |
| X68 (no record) | [20261009-elements-from-point-read.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-elements-from-point-read.md) ([#321](https://github.com/ccheever/exact2/issues/321)) | [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md) (U18) | The theme editor's Inspect is not built (U18) |
| side, from X26 (no record) | [20261009-macos-menu-stalls-native-work.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-macos-menu-stalls-native-work.md) ([#292](https://github.com/ccheever/exact2/issues/292)) | [adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md) | `T3MenuTurn` keeps native work running while an `NSMenu` tracks; fix in open PR [#327](https://github.com/ccheever/exact2/pull/327) (partial) |
| side (no record) | [20261009-dotted-dashed-border-style.md](https://github.com/ccheever/exact2/blob/main/issues/20261009-dotted-dashed-border-style.md) ([#287](https://github.com/ccheever/exact2/issues/287)) | [this-machine-network-access](../tasks/closed/20261005-this-machine-network-access.md) | A solid underline under the "+N" toggle |

## Records still open here

| X | Gap | T3 tasks | Tracked by |
| --- | --- | --- | --- |
| [X44](20261005-x44-remote-image-policy.md) | A remote SVG `image` on Apple (the rest of [#121](https://github.com/ccheever/exact2/issues/121)); no main `issues/` file | [provider-settings-upkeep](../tasks/closed/20261005-provider-settings-upkeep.md) | Main PR [#239](https://github.com/ccheever/exact2/pull/239); the ACP registry SVG icons stay a declared difference |

## Records closed here

| X | Outcome |
| --- | --- |
| [X1](closed/20261005-x01-chromium-cdp-browser-surface.md) | [#100](https://github.com/ccheever/exact2/issues/100) closed upstream, not planned (2026-10-08); the Browser surface is the clone's own `WKWebView` (path B) |
| [X2](closed/20261005-x02-app-developer-tools.md) | [#101](https://github.com/ccheever/exact2/issues/101) closed upstream (2026-10-09) after main #309; the clone's web views are inspectable in development (#326); View › Toggle Developer Tools is a declared difference |
| [X3](closed/20261005-x03-root-font-size.md) | #102 fixed by main #185; adopted |
| [X4](closed/20261005-x04-bundle-helper-executables.md) | #103 fixed by main #215; adopted by embedded-server-runtime |
| [X7](closed/20261005-x07-ats-keys.md) | #106 fixed by main #173; adopted (PR #181) |
| [X8](closed/20261005-x08-agent-pointer-native-views.md) | #107 fixed by main #186; adopted (adopt-main-fixes-r3, real-input checks) |
| [X9](closed/20261005-x09-root-component-across-files.md) | [#108](https://github.com/ccheever/exact2/issues/108) closed upstream, not planned (2026-10-09); the root rewrite (#332) was the remedy |
| [X12](closed/20261005-x12-textarea-field-sizing.md) | [#130](https://github.com/ccheever/exact2/issues/130) closed upstream, not planned (2026-10-09); a declared difference |
| [X13](closed/20261005-x13-hover-keys-during-pan.md) | Not reproduced on main `0365ad1a4` (2026-10-08); not filed |
| [X14](closed/20261005-x14-parked-native-reply.md) | #109 fixed by main #183; adopted (`T3ReadGate.swift` removed) |
| [X15](closed/20261005-x15-non-latin-key-equivalents.md) | #110 closed by main #168; `R10Connect.swift` stays for what #168 does not cover |
| [X16](closed/20261005-x16-smart-substitutions-off.md) | #111 fixed by main #160; adopted |
| [X18](closed/20261005-x18-svg-path-animation.md) | #123 fixed by main #188; partly adopted (keyframed `d` stays refused) |
| [X24](closed/20261005-x24-still-pointer-rehover.md) | #139 fixed by main #174; adopted (PR #181) |
| [X31](closed/20261005-x31-deferred-window-readiness.md) | [#117](https://github.com/ccheever/exact2/issues/117) closed upstream, not planned (2026-10-08); the connecting state is a declared difference (U5) |
| [X32](closed/20261005-x32-sticky-positioning-in-lists.md) | [#131](https://github.com/ccheever/exact2/issues/131) closed upstream, not planned (2026-10-09); pinned diff file headers are a declared difference |
| [X35](closed/20261005-x35-secure-text-entry.md) | #134 closed by main #167; the SSH dialog keeps its native secure field |
| [X36](closed/20261005-x36-data-runtime-intl-locale.md) | #118 fixed by main #204; adopted |
| [X38](closed/20261005-x38-t3-connect-clerk-sign-in.md), [X39](closed/20261005-x39-telemetry.md), [X40](closed/20261005-x40-app-update-feed.md), [X41](closed/20261005-x41-wsl-environments.md) | Scope decisions, closed by the user (2026-10-08, "close all"), not built |
| [X42](closed/20261005-x42-text-blur-filter.md) | Supported on main `0365ad1a4`; not filed |
| [X43](closed/20261005-x43-tristate-switch-mixed.md) | #120 closed upstream, not planned; the app keeps its mixed thumb |
| [X56](closed/20261008-x56-press-lost-on-focus-restyle.md) | Not reproduced on main `0365ad1a4` (2026-10-08); not filed |
| [X58](closed/20261008-x58-scroll-lost-after-window-refocus.md) | Not reproduced on main `0365ad1a4` (2026-10-08); not filed |
| [X59](closed/20261008-x59-line-clamp-first-layout-ellipsis.md) | [#300](https://github.com/ccheever/exact2/issues/300) fixed by main #305; adopted with round 7 (#384, `afe62dfeb`) |
| [Native module termination](closed/20261006-native-module-termination.md) | Folded into #105 and fixed by main #200; adopted (adopt-main-fixes-r4) |

The closed #260 branch numbered two of its own drafts X50 and X54 (distribution build paths). Those drafts are not on
this branch; here X50 and X54 are the records above.

## Filing history

What each record was filed as, with its title at filing. The outcome is in the tables above and, for an open gap, in its main file.

### Filed on GitHub, 2026-10-06

Each was reproduced on exact2 `4c893fef6` before filing. Not filed: X13 (unverified on macOS; the web behavior is designed), X42 (already supported on main), X38–X41 (product scope, not framework gaps).

| Gap | Upstream | Title |
| --- | --- | --- |
| X01 | [#100](https://github.com/ccheever/exact2/issues/100) | Policy decision: an embedded browser surface an app can drive as a shell |
| X02 | [#101](https://github.com/ccheever/exact2/issues/101) | Policy decision: a developer-tools inspector for an app's own UI |
| X03 | [#102](https://github.com/ccheever/exact2/issues/102) | An app cannot set the root font size that `rem` resolves against |
| X04 | [#103](https://github.com/ccheever/exact2/issues/103) | macOS bundle cannot carry helper executables or large resource trees |
| X05 | [#104](https://github.com/ccheever/exact2/issues/104) | Custom-scheme URLs reach an app only through a navigation root, and lossily |
| X06 | [#105](https://github.com/ccheever/exact2/issues/105) | macOS: a native module gets no quit hook, and destroy() never runs at quit |
| X07 | [#106](https://github.com/ccheever/exact2/issues/106) | macOS: app.json cannot set ATS keys, so an iframe cannot load http:// from a named host |
| X08 | [#107](https://github.com/ccheever/exact2/issues/107) | macOS agent mouse: no middle or triple click, wheel point, held modifiers |
| X09 | [#108](https://github.com/ccheever/exact2/issues/108) | Contract: a child component cannot own a resource or mutation |
| X10 | [#128](https://github.com/ccheever/exact2/issues/128) | Text vs Chrome: macOS code-wrap breaks, placeholder color, balance, smoothing |
| X11 | [#129](https://github.com/ccheever/exact2/issues/129) | macOS backdrop-filter misses content outside the parent; no saturate() |
| X12 | [#130](https://github.com/ccheever/exact2/issues/130) | field-sizing: content ignores a hooked native text view's laid-out height |
| X14 | [#109](https://github.com/ccheever/exact2/issues/109) | A watched topic that changes faster than native.later replies starves the resource |
| X15 | [#110](https://github.com/ccheever/exact2/issues/110) | aria-keyshortcuts chords do not fire when the input source types non-Latin letters |
| X16 | [#111](https://github.com/ccheever/exact2/issues/111) | macOS textarea with autocorrect="off" still turns ' into ‘ ’ and -- into — |
| X17 | [#112](https://github.com/ccheever/exact2/issues/112) | Popovers cannot flip near a window edge: no position-try-fallbacks, few position-area values |
| X18 | [#123](https://github.com/ccheever/exact2/issues/123) | SVG path `d` cannot transition; macOS jumps where the web morphs |
| X19 | [#124](https://github.com/ccheever/exact2/issues/124) | Data modules have no timers: `setTimeout` and `Date.now` are refused |
| X20 | [#125](https://github.com/ccheever/exact2/issues/125) | textarea editing: caret moves, setRangeText, beforeinput, undo, atomic ranges |
| X21 | [#126](https://github.com/ccheever/exact2/issues/126) | Data modules cannot send on a WebSocket on native hosts (receive-only) |
| X22 | [#127](https://github.com/ccheever/exact2/issues/127) | Layout facts beyond size: visibility, live position, container/anchor CSS |
| X23 | [#138](https://github.com/ccheever/exact2/issues/138) | Scroll: restore a top-level list by key, scroll-padding/margin, smooth jumps, plain-scroll anchoring |
| X24 | [#139](https://github.com/ccheever/exact2/issues/139) | macOS: hover does not follow layout changes under a stationary pointer |
| X25 | [#140](https://github.com/ccheever/exact2/issues/140) | Keyboard: add keyup, KeyboardEvent.code and .repeat, held modifiers, capture phase |
| X26 | [#141](https://github.com/ccheever/exact2/issues/141) | macOS: let an app declare its menu bar items, and support context-menu submenus |
| X27 | [#113](https://github.com/ccheever/exact2/issues/113) | macOS window shrinks 32 pt per relaunch; no title-bar area or full-screen fact |
| X28 | [#114](https://github.com/ccheever/exact2/issues/114) | Notification click to app action, app badge, and window-focus fact (policy) |
| X29 | [#115](https://github.com/ccheever/exact2/issues/115) | Show a PDF: no PDF element, and `iframe` cannot show an `app:/` or bundled PDF |
| X30 | [#116](https://github.com/ccheever/exact2/issues/116) | Any-type file input and image pixel readback/re-encode in data modules (policy) |
| X31 | [#117](https://github.com/ccheever/exact2/issues/117) | macOS: no way to hold the first window until the app says it is ready |
| X32 | [#131](https://github.com/ccheever/exact2/issues/131) | Sticky section header over several rows of a virtualized list |
| X33 | [#132](https://github.com/ccheever/exact2/issues/132) | Text selection: macOS button press clears it; no end rect or clear/set |
| X34 | [#133](https://github.com/ccheever/exact2/issues/133) | macOS: inline text run has no frame(), and agent hover skips inline runs |
| X35 | [#134](https://github.com/ccheever/exact2/issues/134) | Password input: agent tree prints its value; no autocomplete attribute |
| X36 | [#118](https://github.com/ccheever/exact2/issues/118) | `Intl.Locale` and its week info are missing from the macOS data runtime |
| X37 | [#119](https://github.com/ccheever/exact2/issues/119) | `exact release` skips nested executables and has no app-declared entitlements |
| X43 | [#120](https://github.com/ccheever/exact2/issues/120) | `aria-checked` refuses `"mixed"` on switch and checkbox roles |
| X44 | [#121](https://github.com/ccheever/exact2/issues/121) | Remote `image`: no `load` event, silent SVG failure, undocumented fetch policy |
| X45 | [#122](https://github.com/ccheever/exact2/issues/122) | macOS `reload()` reboots the session in-process; no process relaunch |
| X47 | [#179](https://github.com/ccheever/exact2/issues/179) | macOS: a custom pressable box takes keyboard focus but draws no focus ring |
| side (from X7) | [#135](https://github.com/ccheever/exact2/issues/135) | macOS: a bundled iframe page cannot load any `http:` sub-resource, even on loopback |
| side (from X3) | [#136](https://github.com/ccheever/exact2/issues/136) | Web JS target bakes `rem` and `em` to px at build time |
| side (from X3) | [#137](https://github.com/ccheever/exact2/issues/137) | A bound string `font-size` ("20px") works on web but macOS silently unsets it |
| X28 (rest) | [#224](https://github.com/ccheever/exact2/issues/224) | [Policy] Notifications: a click runs an app action, and an app badge (rest of #114) |
| X11 (rest) | [#225](https://github.com/ccheever/exact2/issues/225) | [Design] macOS backdrop-filter: blur content beyond the parent's subtree, and saturate() (rest of #129) |
| X26 (keyboard) | [#235](https://github.com/ccheever/exact2/issues/235) | [Feature] macOS: Shift+F10 and the context-menu key open a focused node's contextPopover |
| side (build) | [#234](https://github.com/ccheever/exact2/issues/234) | [Bug] macOS build: the linked-SDK check fails for an app whose name ends in parentheses |

### Filed on GitHub, 2026-10-08

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
| X20 (rest) | [#276](https://github.com/ccheever/exact2/issues/276) | [Policy] Atomic inline ranges (chips) over a text field's plain-string value (rest of #125) |
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
| side (agent driver) | [#285](https://github.com/ccheever/exact2/issues/285) | [Bug] Agent driver: `clock +N real` can ask an Apple host for a time just behind its clock ("the clock cannot go backwards") |
| side (native executor) | [#286](https://github.com/ccheever/exact2/issues/286) | [Bug] Native executor: a request a source makes inside its answer fails at the ordered admission limit (16), where the web queues it |
| side (borders) | [#287](https://github.com/ccheever/exact2/issues/287) | [Feature] `border-style`: `dotted` and `dashed` |
| X57 | [#291](https://github.com/ccheever/exact2/issues/291) | [Bug] macOS: a centred line wider than its box is centred and clipped at its start (CSS start-aligns it) |
| side (from X26) | [#292](https://github.com/ccheever/exact2/issues/292) | [Bug] macOS: while a context or button menu is open, main-queue work stalls, so native module calls stop until it closes |
| X59 | [#300](https://github.com/ccheever/exact2/issues/300) | [Bug] macOS: a `line-clamp` text mounted after launch paints its last line without the ellipsis until a restyle |
| X60 | [#301](https://github.com/ccheever/exact2/issues/301) | [Bug] macOS: `input type="number"` is a plain text field (ArrowUp/ArrowDown do not step, letters are accepted) |
| X61 | [#302](https://github.com/ccheever/exact2/issues/302) | [Feature] `outline: none` on `input` and `textarea`, to remove the focus ring Exact draws on a bare field |

### Filed on GitHub, 2026-10-08, second round

Each was reproduced on main `b896050d7` in agent mode before filing (evidence under `file-x48-x68/` on `t3-code-evidence`).
X62's and X66's files came with #307 and #310 (both merged into the T3 branch on 2026-10-08). X64's file arrived with #312's merge and carries #316. X67 (the compiler's stack overflow in main's examples test)
and X68 (the element and paint lookup for the theme editor's Inspect) have no local file.

| Gap | Upstream | Title |
| --- | --- | --- |
| X48 | [#318](https://github.com/ccheever/exact2/issues/318) | [Design] `font-family` from a string at run time, so a font picker can apply any installed family |
| X62 | [#322](https://github.com/ccheever/exact2/issues/322) | [Bug] macOS: hover is not pointerenter/pointerleave — entering a descendant leaves its ancestors, and a node outside its parent's box hears no pointer |
| X63 | [#315](https://github.com/ccheever/exact2/issues/315) | [Bug] macOS: ⌘Z and Edit › Undo do nothing in a plain `textarea` (the undo reaches the window's manager, not the field's) |
| X64 | [#316](https://github.com/ccheever/exact2/issues/316) | [Bug] macOS: a paragraph that shrinks below the text-raster size keeps painting its old lines |
| X65 | [#317](https://github.com/ccheever/exact2/issues/317) | [Bug] macOS: a press on a `scroll`'s empty area reaches no node, so neither it nor an ancestor hears `pointerdown` |
| X66 | [#319](https://github.com/ccheever/exact2/issues/319) | [Feature] `showPopover(id)`, `hidePopover(id)`, `togglePopover(id)` from an action and a popover `toggle` event (the popover sibling of #282) |
| X67 (no local file) | [#320](https://github.com/ccheever/exact2/issues/320) | [Bug] Contract compiler: unoptimized, 103 nested view sites overflow a 2 MiB thread (the parser admits 255), so the examples sweep aborts the whole test binary |
| X68 (no local file) | [#321](https://github.com/ccheever/exact2/issues/321) | [Feature] `elementsFromPoint(x, y)`: every node at a point, so an inspect layer can take the click and still name what it covers (theme editor Inspect) |

### Filed on main, 2026-10-10

Main PR [#386](https://github.com/ccheever/exact2/pull/386) (merged as `e281d83e9`) filed these as main `issues/` files
directly; they were never GitHub issues.

| Gap | Main issue | Title |
| --- | --- | --- |
| X11 (case B) | `issues/20261010-macos-backdrop-beyond-parent.md` | macOS: `backdrop-filter` blurs only its parent's subtree, so a glass panel one wrapper deeper shows the content behind it sharp |
| X69 | `issues/20261010-paint-role-node-list-read.md` | No read lists the rendered nodes a paint role reaches, so a theme editor cannot count or outline a colour's uses |
| X70 | `issues/20261010-aria-disabled-focusable-control.md` | `aria-disabled`: a control that reads as disabled and still takes the focus |
| X71 | `issues/20261010-pointer-events-reach-ancestors.md` | Pointer events do not bubble: a node's `pointermove` takes its ancestors' `pointerdown`, and on macOS a press in a popover reaches no ancestor |

### Filed on main, 2026-10-10, second round

Main PR [#394](https://github.com/ccheever/exact2/pull/394) filed this as a main `issues/` file, reproduced on main
`a88a90cd6` in a one-file app (the app's Hermes on macOS against Chrome). Until then it was `EXACT2-GAPS.md`'s local
draft X71, renumbered X72 because the plan's X71 is the pointer-events gap above.

| Gap | Main issue | Title |
| --- | --- | --- |
| X72 | `issues/20261010-apple-intl-number-format-half-even.md` | Apple: `Intl.NumberFormat` rounds half to even, so USD 0.825 prints $0.82 where the web prints $0.83 |

### Filed on main, 2026-10-10, third round

Main PR [#401](https://github.com/ccheever/exact2/pull/401) filed this as a main `issues/` file, reproduced on main
`a10050516` in a one-file app (agent keys at no focus on macOS against the Exact web in Chrome). Until then it was
`EXACT2-GAPS.md`'s local draft X73 (realinput-1010c RC-3).

| Gap | Main issue | Title |
| --- | --- | --- |
| X73 | `issues/20261010-macos-popover-escape-after-blur.md` | macOS: Escape closes no `popover="auto"` after an action's `blur()`, because the window itself is then the first responder |

### Filed on main, 2026-10-10, fourth round

Main PR [#405](https://github.com/ccheever/exact2/pull/405) (merged as `2002d5a31`) filed this as a main `issues/`
file. It was reproduced on main `d413487a8` in a one-file app, with `tree --ax` on macOS against the Exact web in
Chrome. Until then it was `EXACT2-GAPS.md`'s unnumbered entry "Settings headings: a heading beside its button"
(settings-headings, #404). The same round checked X75, two sends on one mutation in one turn (right-panel-escape,
#403), and did not file it: main's docs declare the behaviour (`EXACT2-GAPS.md` X75).

| Gap | Main issue | Title |
| --- | --- | --- |
| X74 | `issues/20261010-macos-a-heading-inside-a-button.md` | macOS: a heading inside a button is not exposed, because the host makes every button an accessibility leaf |
