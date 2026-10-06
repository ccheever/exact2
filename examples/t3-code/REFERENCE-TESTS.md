# Reference tests mapped to the clone

Every test file of T3 Code `1e2ecbd975` in the four packages the clone draws logic from, with
one class each, the clone module and test it belongs to, and what is left. Task:
`.exact/implementation/20261005-t3code-macos-parity/tasks/20261005-reference-logic-tests-done-areas.md`.
Port tickets are cut from this map; this map ports no test.

Reproduce: the title script `test-map.mjs` is apparatus pending decision U2, so it is not in the
tree; it is on the evidence branch,
<https://github.com/ccheever/exact2/blob/t3-code-evidence/reference-logic-tests-done-areas/test-map.mjs>.
`T3_REF=<reference checkout> T3_REPO=<this repo> bun test-map.mjs check examples/t3-code/REFERENCE-TESTS.md`
lists the reference files (`git ls-files`, or a walk of an exported tree without `.git`), checks
one row and one class per file, a note on every row and every named ticket file, and compares
every `done-equivalent` row's titles with its clone test (`compare <ref-file> <clone-test>` lists
missing and clone-only titles).

Cases are the `describe`/`it`/`test` calls in the file (titles, `.each` counted once).

| Package | Files |
| --- | --- |
| `apps/web/src` | 431 |
| `packages/client-runtime/src` | 121 |
| `packages/shared/src` | 76 |
| `apps/desktop/src` | 106 |

734 file rows, plus two group rows (`packages/contracts`, `apps/server`).

| Class | Files |
| --- | --- |
| done-equivalent | 5 |
| port | 236 |
| swift | 90 |
| n/a-ui | 99 |
| n/a-server | 20 |
| n/a-excluded | 150 |
| later-ticket | 134 |

Classes: `done-equivalent` (the clone test has every reference title); `port` (pure logic a port
ticket moves into a `bun:test` file with the reference titles; Notes list the conversions);
`swift` (the behavior is in an app Swift module; the column names the AppKit binary under
`macos/tests/`, or `n/a-electron`); `n/a-ui` (DOM or React rendering, or Effect plumbing with no
pure part, proven by drives); `n/a-server`; `n/a-excluded` (Browser, Clerk, T3 Connect,
telemetry, update feed, WSL, Linux, Windows, mobile only); `later-ticket` (another ticket builds
the area and moves the row).

Clone modules for `port` rows come from the clone's file headers and from exported names the
reference test imports; a module with a different name is noted with its reference name. A
`port` row's test is the existing clone test that imports the module.

`later-ticket` rows per ticket: `20261005-settings-scoped-controls-and-theme-editor` 12, `20261005-local-primary-environment` 10, `20261005-diff-review-engine` 10, `20261005-upstream-timeline-and-markdown` 9, `20261005-this-machine-network-access` 7, `20261005-provider-settings-upkeep` 7, `20261005-terminal-layout` 7, `20261005-embedded-server-runtime` 6, `20261005-desktop-shell-details` 6, `20261005-pr-conversation-and-refresh` 6, `20261005-pr-code-tab` 5, `20261005-terminal-surface` 5, `20261005-app-activation` 4, `20261005-media-actions` 4, `20261005-pr-links-previews-and-routing` 4, `20261005-managed-codex-chatgpt` 3, `20261005-ssh-password-and-remote-open` 3, `20261005-terminal-integrations` 3, `20261005-pr-writing-and-metadata` 3, `20261005-provider-sign-in-and-install` 2, `20261005-auto-balance` 2, `20261005-shiki-residuals` 2, `20261005-usage-pooled-view` 2, `20261005-server-update-banner` 2, `20261005-upstream-ui-sync` 2, `20261005-remote-scopes-and-update-commands` 1, `20261005-composer-fidelity` 1, `20261005-pr-handoffs-and-quick-actions` 1, `20261005-pr-header-actions-and-stacks` 1, `20261005-thread-commands-and-keys` 1, `20261005-environment-routes` 1, `20261005-terminal-drawer` 1, `20261005-usage-reset-and-feedback` 1.

Plan gaps (no clone counterpart found and no ticket owns the work):

- `apps/desktop/src/permissions/MacPermissionHelper.test.ts`: the clone asks for permission without the reference helper window
- `apps/desktop/src/permissions/MacSettingsWindow.test.ts`: (same as MacPermissionHelper)
- `apps/web/src/components/chat/composerScrollGesture.test.ts`: no clone counterpart found and no ticket owns it
- `apps/web/src/components/chat/pageScrollController.test.ts`: no clone counterpart found and no ticket owns it
- `apps/web/src/components/permissions/usePermissionStatus.test.ts`: (same as MacPermissionHelper)
- `apps/web/src/composer-undo-grouping.test.ts`: no clone counterpart found and no ticket owns it
- `apps/web/src/workspaceBasenameLookup.test.ts`: no clone module found

Conversions named in Notes: `vite-plus/test`/`@effect/vitest` → `bun:test`; fake timers → a
`now` argument; `vi.mock` → the clone's fake native harness (`composer-controls-fixture.ts`);
Effect values, branded ids and `effect/DateTime` → plain results, strings and ISO text;
`Intl`/locale rows depend on issue X36; "render cases n/a-ui" marks the rendering cases of a
mixed file.

| Reference file | Cases | Class | Clone module / test | Notes |
| --- | --- | --- | --- | --- |
| `packages/contracts` | 36 files | n/a-server | — | contracts package; the embedded server is the original's own |
| `apps/server` | 473 files | n/a-server | — | server; the embedded server is the original's own |
| `apps/desktop/src/app/CodexAuthCallback.test.ts` | 5 | later-ticket: 20261005-managed-codex-chatgpt | — | loopback callback for ChatGPT sign-in |
| `apps/desktop/src/app/DesktopAppActivation.test.ts` | 5 | later-ticket: 20261005-app-activation | — | `t3 app` control socket |
| `apps/desktop/src/app/DesktopAppActivationBroker.test.ts` | 7 | later-ticket: 20261005-app-activation | — | activation request queue |
| `apps/desktop/src/app/DesktopAppErrors.test.ts` | 3 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/app/DesktopAppIdentity.test.ts` | 6 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/app/DesktopAssets.test.ts` | 3 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/app/DesktopClerk.test.ts` | 9 | n/a-excluded | — | Clerk |
| `apps/desktop/src/app/DesktopConnectionCatalogStore.test.ts` | 10 | swift | `macos/tests/fleet` | encrypted saved-environment catalog (T3Fleet, T3Credentials) |
| `apps/desktop/src/app/DesktopDetachedActionErrors.test.ts` | 3 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/app/DesktopEarlyElectronStartup.test.ts` | 8 | n/a-excluded | — | Linux password-store preference before Electron starts |
| `apps/desktop/src/app/DesktopEnvironment.test.ts` | 8 | later-ticket: 20261005-embedded-server-runtime | — | state paths and backend root |
| `apps/desktop/src/app/DesktopLifecycle.test.ts` | 4 | later-ticket: 20261005-embedded-server-runtime | — | backend shutdown order at quit |
| `apps/desktop/src/app/DesktopLinuxUrlHandler.test.ts` | 11 | n/a-excluded | — | Linux |
| `apps/desktop/src/app/DesktopObservability.test.ts` | 15 | n/a-excluded | — | telemetry (desktop trace export) |
| `apps/desktop/src/app/DesktopPreReadyFileSystem.test.ts` | 3 | n/a-excluded | — | Windows profile migration |
| `apps/desktop/src/app/DesktopPreReadyPlatform.test.ts` | 6 | n/a-excluded | — | Linux password store and desktop entry |
| `apps/desktop/src/app/DesktopUserData.test.ts` | 2 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/backend/DesktopBackendConfiguration.test.ts` | 34 | later-ticket: 20261005-embedded-server-runtime | — | backend process configuration, spawn and supervision |
| `apps/desktop/src/backend/DesktopBackendManager.test.ts` | 27 | later-ticket: 20261005-embedded-server-runtime | — | backend process configuration, spawn and supervision |
| `apps/desktop/src/backend/DesktopBackendPool.test.ts` | 4 | later-ticket: 20261005-embedded-server-runtime | — | backend process configuration, spawn and supervision |
| `apps/desktop/src/backend/DesktopLocalEnvironmentAuth.test.ts` | 2 | later-ticket: 20261005-local-primary-environment | — | primary environment bootstrap credential |
| `apps/desktop/src/backend/DesktopNetworkInterfaces.test.ts` | 3 | later-ticket: 20261005-this-machine-network-access | — | reachable addresses |
| `apps/desktop/src/backend/DesktopServerExposure.test.ts` | 11 | later-ticket: 20261005-this-machine-network-access | — | Network access mode |
| `apps/desktop/src/backend/tailscaleEndpointProvider.test.ts` | 5 | later-ticket: 20261005-this-machine-network-access | — | Tailscale endpoints |
| `apps/desktop/src/electron/ElectronApp.test.ts` | 9 | later-ticket: 20261005-desktop-shell-details | — | OS locale normalization (system locale timestamps); the rest is Electron plumbing |
| `apps/desktop/src/electron/ElectronDialog.test.ts` | 5 | later-ticket: 20261005-desktop-shell-details | — | native file pickers |
| `apps/desktop/src/electron/ElectronMenu.test.ts` | 9 | swift | `macos/tests/contextmenu` | menu template to native menu, clicked leaf id (T3ContextMenu.swift) |
| `apps/desktop/src/electron/ElectronProtocol.test.ts` | 8 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/electron/ElectronShell.test.ts` | 12 | later-ticket: 20261005-ssh-password-and-remote-open | — | safe external URLs and remote editor deep links (named in that ticket) |
| `apps/desktop/src/electron/ElectronTheme.test.ts` | 3 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/electron/ElectronUpdater.test.ts` | 6 | n/a-excluded | — | update feed |
| `apps/desktop/src/electron/ElectronWindow.module.test.ts` | 1 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/electron/ElectronWindow.test.ts` | 19 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/electron/WindowsForeground.test.ts` | 9 | n/a-excluded | — | Windows |
| `apps/desktop/src/electron/WindowsForegroundFocusThread.test.ts` | 3 | n/a-excluded | — | Windows |
| `apps/desktop/src/electron/WindowsForegroundFocusWorker.test.ts` | 1 | n/a-excluded | — | Windows |
| `apps/desktop/src/ipc/DesktopIpc.test.ts` | 4 | swift | n/a-electron | Electron main-process service; the AppKit host has no counterpart |
| `apps/desktop/src/ipc/methods/localEnvironment.test.ts` | 2 | later-ticket: 20261005-local-primary-environment | — | relaunch on Local environment change (U4) |
| `apps/desktop/src/ipc/methods/notificationBadge.test.ts` | 6 | swift | `macos/tests/notifications` | dock badge count (T3Notifications.swift); Windows overlay cases n/a-excluded |
| `apps/desktop/src/ipc/methods/preview.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/ipc/methods/snapShot.test.ts` | 12 | swift | `macos/tests/snapshot` | capture config read/write through the native picker (T3SnapShot.swift) |
| `apps/desktop/src/ipc/methods/sshEnvironment.test.ts` | 4 | swift | `macos/tests/ssh` | SSH environment descriptor fetch (T3Ssh.swift) |
| `apps/desktop/src/ipc/methods/window.test.ts` | 12 | later-ticket: 20261005-desktop-shell-details | — | full-screen state and local bootstraps; WSL cases n/a-excluded |
| `apps/desktop/src/ipc/methods/wsl.test.ts` | 8 | n/a-excluded | — | WSL |
| `apps/desktop/src/linuxSecretStorage.test.ts` | 12 | n/a-excluded | — | Linux |
| `apps/desktop/src/permissions/MacPermissionHelper.test.ts` | 18 | swift | `macos/tests/snapshot` | macOS capture permission helper; plan gap: the clone asks for permission without the reference helper window |
| `apps/desktop/src/permissions/MacSettingsWindow.test.ts` | 3 | swift | `macos/tests/snapshot` | helper placement in System Settings; plan gap (same as MacPermissionHelper) |
| `apps/desktop/src/preview/AnnotationKeyboard.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/BrowserImport.test.ts` | 7 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/ChromiumCookies.test.ts` | 16 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/ChromiumKeys.module.test.ts` | 1 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/ChromiumKeys.test.ts` | 18 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/CookieDatabase.test.ts` | 4 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/FirefoxCookies.test.ts` | 13 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/LinuxBrowserSecret.test.ts` | 2 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/SafariCookies.test.ts` | 28 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/SafariPermission.test.ts` | 2 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserImport/Sources.test.ts` | 60 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/BrowserSession.test.ts` | 11 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/FaviconCapture.test.ts` | 22 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/Manager.test.ts` | 21 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/PickedElementPayload.test.ts` | 13 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/PlaywrightInjectedRuntime.test.ts` | 1 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/PreviewKeyboard.test.ts` | 13 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/RecordingInput.test.ts` | 7 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/preview/WebviewPreferences.test.ts` | 7 | n/a-excluded | — | Browser surface |
| `apps/desktop/src/settings/DesktopAppSettings.test.ts` | 20 | later-ticket: 20261005-local-primary-environment | — | desktop settings file (U7); update-channel cases n/a-excluded |
| `apps/desktop/src/settings/DesktopClientSettings.diagnostics.test.ts` | 4 | later-ticket: 20261005-local-primary-environment | — | client settings store (U7) |
| `apps/desktop/src/settings/DesktopClientSettings.test.ts` | 10 | later-ticket: 20261005-local-primary-environment | — | client settings store (U7) |
| `apps/desktop/src/settings/DesktopSavedEnvironments.test.ts` | 10 | swift | `macos/tests/fleet` | saved environments and their secrets (T3Fleet, T3Credentials) |
| `apps/desktop/src/shell/DesktopShellEnvironment.test.ts` | 14 | later-ticket: 20261005-embedded-server-runtime | — | login-shell PATH and locale for the server process |
| `apps/desktop/src/snapShot/ActiveWindow.test.ts` | 6 | swift | `macos/tests/snapshot` | frontmost macOS window lookup; non-macOS cases n/a-excluded |
| `apps/desktop/src/snapShot/CaptureShortcutConfig.test.ts` | 21 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/DesktopSnapShot.test.ts` | 83 | swift | `macos/tests/snapshot` | capture service; macOS cases only, Linux/Windows cases n/a-excluded |
| `apps/desktop/src/snapShot/GnomeCaptureSetup.test.ts` | 11 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/HyprlandSnapShot.test.ts` | 10 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/KdeSnapShot.test.ts` | 12 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/LinuxSnapShot.dbus.test.ts` | 1 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/LinuxSnapShot.test.ts` | 21 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/MacModifierPairShortcutProcess.test.ts` | 4 | swift | `macos/tests/snapshot` | modifier-pair shortcut poller (T3SnapshotShortcut.swift) |
| `apps/desktop/src/snapShot/MacSnapShot.test.ts` | 3 | swift | `macos/tests/snapshot` | window capture to PNG (T3SnapshotImage.swift) |
| `apps/desktop/src/snapShot/NativeCaptureFeedback.test.ts` | 5 | swift | `macos/tests/snapshot` | capture flight feedback (T3SnapshotFeedback.swift) |
| `apps/desktop/src/snapShot/NiriSnapShot.test.ts` | 14 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/PortalCaptureShortcut.dbus.test.ts` | 1 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/PortalCaptureShortcut.test.ts` | 18 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/RegionSnapShot.test.ts` | 15 | swift | `macos/tests/snapshot` | region capture bounds and encoding |
| `apps/desktop/src/snapShot/SnapShotAccessibilityProcess.test.ts` | 6 | swift | `macos/tests/snapshot` | accessibility text extraction (T3SnapshotAccessibility.swift) |
| `apps/desktop/src/snapShot/WindowsCaptureFeedback.test.ts` | 3 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/captureConfigEdit.test.ts` | 17 | n/a-excluded | — | Linux or Windows capture backend |
| `apps/desktop/src/snapShot/snapShot.test.ts` | 61 | swift | `macos/tests/snapshot` | capture errors and accessible window text (T3SnapshotAccessibility.swift); Linux cases n/a-excluded |
| `apps/desktop/src/ssh/DesktopSshEnvironment.test.ts` | 4 | swift | `macos/tests/ssh` | host discovery and prompt timeouts (T3Ssh.swift) |
| `apps/desktop/src/ssh/DesktopSshPasswordPrompts.test.ts` | 6 | later-ticket: 20261005-ssh-password-and-remote-open | — | SSH password prompt |
| `apps/desktop/src/telemetry/DesktopTelemetryPublisher.test.ts` | 4 | n/a-excluded | — | telemetry |
| `apps/desktop/src/updates/DesktopRemoteUpdates.test.ts` | 22 | n/a-excluded | — | update feed |
| `apps/desktop/src/updates/DesktopUpdates.test.ts` | 29 | n/a-excluded | — | update feed |
| `apps/desktop/src/updates/releaseNotes.test.ts` | 12 | n/a-excluded | — | update feed |
| `apps/desktop/src/updates/remoteUpdateFlow.test.ts` | 9 | n/a-excluded | — | update feed |
| `apps/desktop/src/updates/updateChannels.test.ts` | 3 | n/a-excluded | — | update feed |
| `apps/desktop/src/updates/updateMachine.test.ts` | 11 | n/a-excluded | — | update feed |
| `apps/desktop/src/window/DesktopApplicationMenu.test.ts` | 5 | swift | `macos/tests/r8-keys` | menu bar, Settings and Paste as Text routing (R8KeysMenus.swift); cases named by `20261005-app-developer-tools` |
| `apps/desktop/src/window/DesktopWindow.test.ts` | 36 | later-ticket: 20261005-desktop-shell-details | — | full screen before quit, bounds restore, guest context menus |
| `apps/desktop/src/window/QuitHold.test.ts` | 30 | swift | `macos/tests/menus` | ⌘Q hold (T3QuitHold in T3Menus.swift); cases named by `20261005-desktop-shell-details` |
| `apps/desktop/src/wsl/DesktopWslBackend.test.ts` | 4 | n/a-excluded | — | WSL |
| `apps/desktop/src/wsl/DesktopWslEnvironment.test.ts` | 55 | n/a-excluded | — | WSL |
| `apps/desktop/src/wsl/DesktopWslServerTree.test.ts` | 11 | n/a-excluded | — | WSL |
| `apps/desktop/src/wsl/wslPathParsing.test.ts` | 27 | n/a-excluded | — | WSL |
| `apps/web/src/appearanceContrast.test.ts` | 5 | port | `settings-appearance-look.ts` / `settings-a.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/appearanceFonts.test.ts` | 22 | port | `settings-appearance.ts` / `settings-core.test.ts` | vite-plus/test → bun:test; canvas metrics → a measure argument; cases named by `20261005-interface-font-size`, `20261005-terminal-drawer` |
| `apps/web/src/authBootstrap.test.ts` | 24 | swift | `macos/tests/transport` | web auth gate; the clone pairs through T3Credentials.swift |
| `apps/web/src/bootstrap.test.ts` | 5 | n/a-ui | — | browser splash and startup errors |
| `apps/web/src/branding.test.ts` | 10 | port | `shell-nightly.ts` / `r3-shell-details.test.ts`, `r4-polish.test.ts` | vite-plus/test → bun:test; desktop branding and channel label |
| `apps/web/src/browser/BrowserDeviceToolbar.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/HostedBrowserWebview.test.tsx` | 2 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserDefaults.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserLinkTarget.test.ts` | 8 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserPointerStore.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserRecording.test.ts` | 29 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserRecordingScope.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserSurfaceStore.test.ts` | 10 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserTargetResolver.test.ts` | 20 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserViewportActions.test.ts` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/browserViewportLayout.test.ts` | 13 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/desktopTabLifetime.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/hostedBrowserWebviewStyle.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/previewRuntimeTabId.test.ts` | 4 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/previewWebviewConfigState.test.ts` | 4 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/recordingCompositor.test.ts` | 9 | n/a-excluded | — | Browser surface |
| `apps/web/src/browser/webviewCrashRecovery.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/browserFaviconLogic.test.ts` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/browserFaviconStore.test.ts` | 16 | n/a-excluded | — | Browser surface |
| `apps/web/src/browserHistoryStore.test.ts` | 41 | n/a-excluded | — | Browser surface |
| `apps/web/src/bundledDev.test.ts` | 2 | n/a-ui | — | Vite dev bundle |
| `apps/web/src/clientPersistenceStorage.test.ts` | 7 | later-ticket: 20261005-local-primary-environment | — | client settings persistence (U7) |
| `apps/web/src/cloud/connectCliAuth.test.ts` | 6 | n/a-excluded | — | T3 Connect |
| `apps/web/src/cloud/dpop.test.ts` | 2 | n/a-excluded | — | T3 Connect |
| `apps/web/src/cloud/linkEnvironment.test.ts` | 8 | n/a-excluded | — | T3 Connect |
| `apps/web/src/cloud/managedAuth.test.ts` | 3 | n/a-excluded | — | T3 Connect |
| `apps/web/src/cloud/publicConfig.test.ts` | 4 | n/a-excluded | — | T3 Connect |
| `apps/web/src/cloud/relayClientInstallDialog.test.ts` | 4 | n/a-excluded | — | T3 Connect |
| `apps/web/src/components/BranchToolbar.logic.test.ts` | 95 | port | `composer-controls-branch.ts` / `composer-controls-branch.test.ts`, `composer-controls.test.ts` | vite-plus/test → bun:test; branded ids → strings; 9 of 95 titles already in r13-threads.test.ts; cases named by `20261005-round12-wrapup` |
| `apps/web/src/components/ChatMarkdown.test.tsx` | 53 | n/a-ui | — | React Markdown rendering; clone rows come from `macos/src/markdown.rs`; cases named by `20261005-terminal-integrations` |
| `apps/web/src/components/ChatView.logic.test.ts` | 149 | port | `composer-controls-commands.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; Effect values → plain results; branded ids → strings; DateTime → ISO text; cases named by `20261005-round12-wrapup`, `20261005-server-update-banner`, `20261005-terminal-drawer`, `20261005-terminal-integrations` |
| `apps/web/src/components/CommandPalette.logic.test.ts` | 40 | port | `palette.ts` / `palette.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; branded ids → strings; Intl/locale (X36) |
| `apps/web/src/components/GitActionsControl.logic.test.ts` | 91 | port | `r4-git-logic.ts` / `r4-git.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/KeybindingsUpdateToast.logic.test.ts` | 4 | port | `r3-protocol-config.ts` / `r3-protocol.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/ProjectFavicon.test.tsx` | 7 | n/a-ui | — | React icon rendering |
| `apps/web/src/components/ProviderUpdateEnvironmentRows.test.tsx` | 2 | later-ticket: 20261005-provider-settings-upkeep | — | provider update rows |
| `apps/web/src/components/ProviderUpdateLaunchNotification.logic.test.ts` | 52 | later-ticket: 20261005-provider-settings-upkeep | — | update advisories and outcomes |
| `apps/web/src/components/RightPanelTabs.test.tsx` | 24 | n/a-ui | — | tab strip rendering; Browser profile cases n/a-excluded; cases named by `20261005-browser-surface`, `20261005-right-panel-tab-menu`, `20261005-round12-wrapup` |
| `apps/web/src/components/ServerUpdateAction.test.tsx` | 18 | later-ticket: 20261005-remote-scopes-and-update-commands | — | update commands per install; cases named by `20261005-auto-balance`, `20261005-server-update-banner` |
| `apps/web/src/components/Sidebar.drag.test.ts` | 38 | port | `sidebar-drop.ts` / `sidebar-extra.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/Sidebar.logic.test.ts` | 168 | port | `sidebar-model.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; Effect values → plain results; branded ids → strings; DateTime → ISO text; Intl/locale (X36); cases named by `20261005-legacy-sidebar`, `20261005-thread-commands-and-keys` |
| `apps/web/src/components/Sidebar.motion.test.ts` | 20 | n/a-ui | — | FLIP row motion |
| `apps/web/src/components/Sidebar.pointer.test.ts` | 12 | port | `sidebar-drop.ts` / `sidebar-extra.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; pointer events → plain event objects |
| `apps/web/src/components/Sidebar.snooze.test.ts` | 9 | port | `sidebar-presentation.ts` / `chat.test.ts` | vite-plus/test → bun:test; Intl/locale (X36) |
| `apps/web/src/components/SidebarStageBackdrop.test.tsx` | 4 | n/a-ui | — | React artwork |
| `apps/web/src/components/ThreadNotificationCoordinator.badge.test.tsx` | 8 | port | `shell-notify.ts` / `shell.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; Effect values → plain results; branded ids → strings; DateTime → ISO text; render cases n/a-ui; React harness → plain calls; badge drawing in `notifications` |
| `apps/web/src/components/ThreadNotificationCoordinator.test.tsx` | 12 | port | `shell-notify.ts` / `shell.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; Effect values → plain results; DateTime → ISO text; render cases n/a-ui; React harness → plain calls |
| `apps/web/src/components/ThreadStatusIndicators.subscriptions.test.tsx` | 1 | n/a-ui | — | React lease lifecycle |
| `apps/web/src/components/ThreadStatusIndicators.test.ts` | 45 | port | `r7-handoff-strip.ts` / `r7-handoff.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `apps/web/src/components/ThreadStatusIndicators.test.tsx` | 6 | n/a-ui | — | React indicator rendering |
| `apps/web/src/components/ThreadTerminalDrawer.test.ts` | 9 | later-ticket: 20261005-terminal-integrations | — | terminal selection menu; cases named by `20261005-terminal-drawer` |
| `apps/web/src/components/chat/AssistantCitationChip.test.tsx` | 5 | n/a-ui | — | React chip lifecycle |
| `apps/web/src/components/chat/AssistantCitationSource.test.ts` | 23 | port | `r5-composer-citation.ts` / `r5-composer-citation.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; branded ids → strings; DOM geometry → measured rects as arguments |
| `apps/web/src/components/chat/ChangedFilesTree.test.tsx` | 7 | n/a-ui | — | React tree rendering |
| `apps/web/src/components/chat/ChatHeader.test.ts` | 4 | port | `shell-commands.ts` / `shell.test.ts`, `shell-r2.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/ComposerBannerStack.test.tsx` | 1 | n/a-ui | — | React banner rendering; cases named by `20261005-server-update-banner` |
| `apps/web/src/components/chat/ComposerCommandMenu.test.tsx` | 6 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test; render cases n/a-ui; render cases n/a-ui |
| `apps/web/src/components/chat/ComposerPendingApprovalActions.test.tsx` | 4 | n/a-ui | — | React approval rendering |
| `apps/web/src/components/chat/ComposerPendingApprovalPanel.test.tsx` | 5 | n/a-ui | — | React approval rendering |
| `apps/web/src/components/chat/ComposerPendingUserInputPanel.test.tsx` | 4 | n/a-ui | — | React question rendering |
| `apps/web/src/components/chat/ComposerPrimaryActions.test.tsx` | 6 | n/a-ui | — | React button rendering |
| `apps/web/src/components/chat/ComposerStashMenu.test.tsx` | 3 | n/a-ui | — | React menu rendering |
| `apps/web/src/components/chat/ContextWindowMeter.logic.test.ts` | 28 | port | `r3-composer-controls-resume.ts` / `r3-composer-controls-resume.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/ExpandedImagePreview.test.ts` | 7 | port | `timeline-attachments.ts` / `timeline-attachments.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/HighlightedCodeLines.test.tsx` | 2 | n/a-ui | — | Shiki HTML rendering |
| `apps/web/src/components/chat/MessagesTimeline.logic.test.ts` | 109 | port | `timeline-rows.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `apps/web/src/components/chat/MessagesTimeline.test.tsx` | 56 | n/a-ui | — | React timeline rendering; cases named by `20261005-upstream-timeline-and-markdown` |
| `apps/web/src/components/chat/ModelPickerContent.test.ts` | 18 | port | `model-catalog.ts` / `composer.test.ts` | vite-plus/test → bun:test; branded ids → strings; shouldOfferModelPickerSetup cases go with provider-sign-in-and-install; cases named by `20261005-provider-sign-in-and-install` |
| `apps/web/src/components/chat/OpenInPicker.test.ts` | 9 | port | `shell-details.ts` / `r3-shell-details.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/ProviderInstanceIcon.test.ts` | 3 | later-ticket: 20261005-provider-settings-upkeep | — | ACP registry icons |
| `apps/web/src/components/chat/ProviderModelPicker.test.tsx` | 9 | n/a-ui | — | React picker rendering |
| `apps/web/src/components/chat/ProviderStatusBanner.test.ts` | 4 | port | `presentation.ts` / new test file | vite-plus/test → bun:test; branded ids → strings; reference name ProviderStatusBanner (clone providerBanner) |
| `apps/web/src/components/chat/ProviderStatusBanner.test.tsx` | 11 | later-ticket: 20261005-provider-sign-in-and-install | — | status banner opens setup |
| `apps/web/src/components/chat/QueuedRunsControl.test.tsx` | 6 | n/a-ui | — | React queue rendering; cases named by `20261005-composer-fidelity` |
| `apps/web/src/components/chat/SnapShotAttachmentDetails.test.tsx` | 5 | n/a-ui | — | React details rendering |
| `apps/web/src/components/chat/ThreadDetailsPanel.test.tsx` | 2 | n/a-ui | — | React panel rendering |
| `apps/web/src/components/chat/ThreadDetailsPrRow.test.tsx` | 1 | n/a-ui | — | React row rendering |
| `apps/web/src/components/chat/ThreadDetailsPrRows.test.tsx` | 2 | n/a-ui | — | React row rendering |
| `apps/web/src/components/chat/ThreadErrorBanner.test.tsx` | 7 | port | `timeline-errors.ts` / `timeline-errors.test.ts` | vite-plus/test → bun:test; render cases n/a-ui; dismissal scoping |
| `apps/web/src/components/chat/ThreadRelationshipsControl.agents.test.tsx` | 4 | n/a-ui | — | React lineage rendering |
| `apps/web/src/components/chat/ThreadRelationshipsControl.test.tsx` | 5 | n/a-ui | — | React lineage rendering |
| `apps/web/src/components/chat/TraitsPicker.test.ts` | 17 | port | `composer-controls-view.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-composer-fidelity` |
| `apps/web/src/components/chat/agentSpawnSummary.test.ts` | 7 | port | `composer-controls-subagent.ts` / `composer-controls-subagent.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/assistantCitationCommentDismissal.test.ts` | 7 | port | `r5-composer-citation.ts` / `r5-composer-citation.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/chatCanvasLayout.test.ts` | 22 | done-equivalent | `chat-canvas-layout.ts` / `chat-canvas-layout.test.ts` | every reference title present (test-map.mjs check); cases named by `20261005-floating-device-player` |
| `apps/web/src/components/chat/composerAttachmentFiles.test.ts` | 21 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/composerContextUndo.test.ts` | 3 | swift | `macos/tests/composer` | chip undo in the text view |
| `apps/web/src/components/chat/composerEventScope.test.ts` | 10 | n/a-ui | — | DOM event scopes |
| `apps/web/src/components/chat/composerMentionDrag.test.ts` | 13 | port | `composer-editor.ts` / `composer-editor-insert.test.ts`, `composer-editor.test.ts` | @effect/vitest → bun:test; Effect values → plain results |
| `apps/web/src/components/chat/composerMenuHighlight.test.ts` | 6 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/composerPromptHistory.test.ts` | 22 | swift | `macos/tests/composer` | prompt recall (T3ComposerText.swift) |
| `apps/web/src/components/chat/composerProviderState.test.tsx` | 28 | later-ticket: 20261005-composer-fidelity | — | implicit Fast-mode default |
| `apps/web/src/components/chat/composerScrollGesture.test.ts` | 12 | swift | `macos/tests/r9-input` | composer collapse on scroll; plan gap: no clone counterpart found and no ticket owns it |
| `apps/web/src/components/chat/composerSlashCommandSearch.test.ts` | 8 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/composerSubmission.test.ts` | 13 | port | `composer-editor.ts` / `composer-editor-insert.test.ts`, `composer-editor.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/draftHeroTransition.test.ts` | 8 | n/a-ui | — | view transition timing |
| `apps/web/src/components/chat/externalLinkContextMenu.test.ts` | 13 | later-ticket: 20261005-media-actions | — | link context menu |
| `apps/web/src/components/chat/folderDrop.test.ts` | 9 | port | `composer-editor.ts` / `composer-editor-insert.test.ts`, `composer-editor.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `apps/web/src/components/chat/modelPickerKeys.test.ts` | 3 | port | `model-catalog.ts` / `composer.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/modelPickerSearch.test.ts` | 9 | port | `model-catalog.ts` / `composer.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/pageScrollController.test.ts` | 11 | swift | `macos/tests/timeline-keyboard` | Page Up/Down hand-off; plan gap: no clone counterpart found and no ticket owns it |
| `apps/web/src/components/chat/pendingDraftWork.test.ts` | 5 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/chat/queuedMessageEdit.test.ts` | 7 | port | `composer-controls-queue.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; branded ids → strings; render cases n/a-ui; cases named by `20261005-composer-fidelity` |
| `apps/web/src/components/chat/restingComposerControlsMeasurement.test.ts` | 5 | port | `r5-composer-measure.ts` / `r5-composer.test.ts` | vite-plus/test → bun:test; DOM measurement → widths as arguments; cases named by `20261005-composer-fidelity` |
| `apps/web/src/components/chat/threadDetailsCardLayout.test.ts` | 12 | done-equivalent | `thread-details-card-layout.ts` / `thread-details-card-layout.test.ts` | every reference title present (test-map.mjs check); cases named by `20261005-floating-device-player` |
| `apps/web/src/components/chat/timelineMinimapItems.test.ts` | 4 | port | `timeline-minimap.ts` / `timeline-minimap.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/chat/timelineScrollAnchoring.test.tsx` | 22 | swift | `macos/tests/r9-input` | scroll anchoring (R9Input.swift); cases named by `20261005-round12-wrapup` |
| `apps/web/src/components/chat/timelineScrollTarget.test.ts` | 10 | n/a-ui | — | DOM overscroll chaining |
| `apps/web/src/components/chat/useComposerFocusState.test.tsx` | 4 | n/a-ui | — | React focus hook |
| `apps/web/src/components/chat/useComposerMenuState.test.tsx` | 3 | n/a-ui | — | React menu hook |
| `apps/web/src/components/chat/useComposerMultilinePrompt.test.ts` | 7 | n/a-ui | — | DOM line measurement |
| `apps/web/src/components/chat/useComposerTriggerState.test.tsx` | 10 | n/a-ui | — | React trigger hook |
| `apps/web/src/components/chat/workspaceFileDrop.test.ts` | 8 | n/a-ui | — | DOM drag events |
| `apps/web/src/components/clerk/MobileClientsUserProfilePage.logic.test.ts` | 5 | n/a-excluded | — | Clerk |
| `apps/web/src/components/clerk/T3ConnectUserProfilePage.test.tsx` | 4 | n/a-excluded | — | Clerk |
| `apps/web/src/components/clerk/authRedirect.test.ts` | 4 | n/a-excluded | — | Clerk |
| `apps/web/src/components/clerk/clerkAppearance.test.ts` | 3 | n/a-excluded | — | Clerk |
| `apps/web/src/components/clerk/electronPasskeys.test.ts` | 3 | n/a-excluded | — | Clerk |
| `apps/web/src/components/cloud/CloudEnvironmentConnectList.test.tsx` | 10 | n/a-excluded | — | T3 Connect |
| `apps/web/src/components/cloud/cloudEnvironmentConnectionPresentation.test.ts` | 4 | n/a-excluded | — | T3 Connect |
| `apps/web/src/components/composerContextPresentation.test.ts` | 3 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/composerFooterLayout.test.ts` | 52 | port | `composer-controls-view.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; cases named by `20261005-composer-fidelity` |
| `apps/web/src/components/composerInlineChip.test.ts` | 6 | port | `r4-timeline-chips.ts` / `r4-timeline-chips.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/composerSelection.test.ts` | 4 | swift | `macos/tests/composer` | visible selection change in the text view |
| `apps/web/src/components/contextPresentationRegistry.test.ts` | 5 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test; render cases n/a-ui |
| `apps/web/src/components/desktop/SnapShotCoordinator.test.ts` | 13 | port | `snapshot-settings.ts` / `snapshot-settings.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; branded ids → strings |
| `apps/web/src/components/desktopUpdate.logic.test.ts` | 36 | n/a-excluded | — | update feed |
| `apps/web/src/components/desktopUpdate.toast.test.tsx` | 4 | n/a-excluded | — | update feed |
| `apps/web/src/components/device/DeviceStreamView.test.tsx` | 3 | swift | `macos/tests/r6-device` | MJPEG pause and Reconnect (R6DeviceStream.swift) |
| `apps/web/src/components/device/deviceFrameLayout.test.ts` | 1 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `apps/web/src/components/device/deviceHubApi.test.ts` | 2 | swift | `macos/tests/r7-device` | R7DeviceTools.swift |
| `apps/web/src/components/device/phoneTrackpad.test.ts` | 4 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `apps/web/src/components/device/useDeviceControls.test.tsx` | 6 | port | `r7-device-tools.ts` / `r7-device-tools.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; branded ids → strings; render cases n/a-ui; React hook → plain calls |
| `apps/web/src/components/diffs/DiffCommentAnnotation.test.tsx` | 5 | later-ticket: 20261005-diff-review-engine | — | diff tree, comments, code view |
| `apps/web/src/components/diffs/DiffFileTree.test.tsx` | 10 | later-ticket: 20261005-diff-review-engine | — | diff tree, comments, code view |
| `apps/web/src/components/diffs/StyledDiffCodeView.test.tsx` | 9 | later-ticket: 20261005-diff-review-engine | — | diff tree, comments, code view |
| `apps/web/src/components/diffs/commentSubmitShortcut.test.ts` | 3 | later-ticket: 20261005-diff-review-engine | — | diff tree, comments, code view |
| `apps/web/src/components/diffs/diffFileTree.logic.test.ts` | 13 | later-ticket: 20261005-diff-review-engine | — | diff tree, comments, code view |
| `apps/web/src/components/files/AttachmentFilePreview.test.tsx` | 4 | n/a-ui | — | React preview recovery |
| `apps/web/src/components/files/FilePreviewPanel.test.ts` | 14 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/files/ProjectFilePicker.logic.test.ts` | 7 | port | `palette-files.ts` / `palette.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/files/fileContentRevision.test.ts` | 4 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/files/fileEditorHighlight.test.ts` | 8 | n/a-ui | — | web editor worker; clone editor is T3PanelsNative.swift |
| `apps/web/src/components/files/fileEditorLanguageReadiness.test.ts` | 6 | n/a-ui | — | web editor languages |
| `apps/web/src/components/files/fileEditorVirtualization.test.ts` | 26 | n/a-ui | — | web editor virtualization |
| `apps/web/src/components/files/fileLineReveal.test.ts` | 3 | swift | `macos/tests/r5-panels` | centered line reveal in the native editor |
| `apps/web/src/components/files/filePath.test.ts` | 11 | port | `r7-polish-crumbs.ts` / `r7-polish.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/files/fileSaveCoordinator.test.ts` | 10 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; Effect values → plain results |
| `apps/web/src/components/files/fileTreeDragMention.test.ts` | 9 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | @effect/vitest → bun:test; Effect values → plain results |
| `apps/web/src/components/files/fileTreeExpansion.test.ts` | 4 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | @effect/vitest → bun:test; Effect values → plain results |
| `apps/web/src/components/files/fileTreePathReconciliation.test.ts` | 4 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/files/projectFilesQueryState.test.ts` | 2 | port | `r4-surfaces-files.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/files/projectFilesQueryState.test.tsx` | 6 | n/a-ui | — | React query refresh |
| `apps/web/src/components/files/useFileSaveCoordinator.test.tsx` | 7 | n/a-ui | — | React StrictMode lifecycle |
| `apps/web/src/components/onboarding/WelcomeWizard.test.tsx` | 3 | n/a-ui | — | React wizard rendering |
| `apps/web/src/components/permissions/usePermissionStatus.test.ts` | 3 | swift | `macos/tests/snapshot` | permission polling; plan gap (same as MacPermissionHelper) |
| `apps/web/src/components/preview/PreviewAutomationHosts.test.tsx` | 7 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/PreviewEmptyState.test.tsx` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/PreviewFaviconIcon.test.tsx` | 1 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/PreviewPanelShell.test.ts` | 8 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/PreviewView.test.tsx` | 11 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/addBrowserSurface.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/agentBrowserCursorLogic.test.ts` | 4 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/closePreviewSession.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/fileExplorerLabel.test.ts` | 6 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/openPreviewSession.test.ts` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/openTerminalLinkInPreview.test.ts` | 8 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewAutomationHostBudget.test.ts` | 12 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewAutomationOpenReadiness.test.ts` | 16 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewAutomationRequestConsumer.test.ts` | 12 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewAutomationTarget.test.ts` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewClickFocus.test.ts` | 9 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewEmptyStateLogic.test.ts` | 5 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewMiniPlayerLayout.test.ts` | 42 | port | `previewMiniPlayerLayout.ts` / `previewMiniPlayerLayout.test.ts` | vite-plus/test → bun:test; 39 of 42 titles ported by floating-device-player; missing: resolvePreviewMiniPlayerSourceSize (2 cases); cases named by `20261005-floating-device-player` |
| `apps/web/src/components/preview/previewNavigationReadiness.test.ts` | 2 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewViewportReadiness.test.ts` | 4 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/previewViewportRollback.test.ts` | 3 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/useDiscoveredLocalServers.test.ts` | 12 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/preview/usePreviewBridge.test.ts` | 2 | n/a-excluded | — | Browser surface |
| `apps/web/src/components/projectScriptEditor.test.tsx` | 6 | n/a-ui | — | React save lifecycle |
| `apps/web/src/components/pullRequest/LinkPullRequestDialog.logic.test.ts` | 12 | later-ticket: 20261005-pr-links-previews-and-routing | — | link dialog input |
| `apps/web/src/components/pullRequest/PullRequestDetailPanel.test.tsx` | 3 | later-ticket: 20261005-pr-handoffs-and-quick-actions | — | checkout and composer hand-off |
| `apps/web/src/components/pullRequest/PullRequestListFilters.test.tsx` | 7 | n/a-ui | — | React filter menu |
| `apps/web/src/components/pullRequest/PullRequestSummaryTab.test.tsx` | 3 | later-ticket: 20261005-pr-conversation-and-refresh | — | summary sections |
| `apps/web/src/components/pullRequest/pullRequestChecks.test.tsx` | 5 | later-ticket: 20261005-pr-conversation-and-refresh | — | checks state |
| `apps/web/src/components/pullRequest/pullRequestDetail.logic.test.ts` | 140 | port | `r6-pr-logic.ts` / `r6-pr.test.ts` | vite-plus/test → bun:test; branded ids → strings; render cases n/a-ui; merge, checkout and action cases may wait for the PR tickets |
| `apps/web/src/components/pullRequest/pullRequestDiff.logic.test.ts` | 17 | later-ticket: 20261005-pr-code-tab | — | diff lines |
| `apps/web/src/components/pullRequest/pullRequestEditing.logic.test.ts` | 17 | later-ticket: 20261005-pr-writing-and-metadata | — | edit permissions |
| `apps/web/src/components/pullRequest/pullRequestFileOrder.logic.test.ts` | 16 | later-ticket: 20261005-pr-code-tab | — | file order |
| `apps/web/src/components/pullRequest/pullRequestFilesViewed.logic.test.ts` | 22 | later-ticket: 20261005-pr-code-tab | — | viewed marks |
| `apps/web/src/components/pullRequest/pullRequestList.logic.test.ts` | 147 | port | `pages-prs.ts` / `pages-prs.test.ts` | vite-plus/test → bun:test; render cases n/a-ui |
| `apps/web/src/components/pullRequest/pullRequestListLines.test.ts` | 3 | port | `r4-surfaces-prs.ts` / `r4-surfaces.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/pullRequest/pullRequestMarkdown.logic.test.ts` | 17 | later-ticket: 20261005-pr-conversation-and-refresh | — | body segmentation |
| `apps/web/src/components/pullRequest/pullRequestPresentation.test.ts` | 10 | port | `r6-pr-logic.ts` / `r6-pr.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/pullRequest/pullRequestProjectAssignment.logic.test.ts` | 19 | later-ticket: 20261005-pr-links-previews-and-routing | — | cross-environment routing |
| `apps/web/src/components/pullRequest/pullRequestProjectFilter.logic.test.ts` | 12 | port | `pages-prs.ts` / `pages-prs.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/pullRequest/pullRequestReactions.logic.test.ts` | 17 | later-ticket: 20261005-pr-writing-and-metadata | — | reactions |
| `apps/web/src/components/pullRequest/pullRequestReviewStore.test.ts` | 5 | later-ticket: 20261005-pr-writing-and-metadata | — | review drafts |
| `apps/web/src/components/pullRequest/pullRequestStackSnapshot.test.ts` | 6 | later-ticket: 20261005-pr-header-actions-and-stacks | — | stacks |
| `apps/web/src/components/pullRequest/pullRequestSummaryScroll.logic.test.ts` | 5 | later-ticket: 20261005-pr-conversation-and-refresh | — | section scroll anchoring |
| `apps/web/src/components/pullRequest/usePullRequestFilesViewed.test.tsx` | 6 | later-ticket: 20261005-pr-code-tab | — | viewed marks lifecycle |
| `apps/web/src/components/settings/AcpRegistryIcon.test.ts` | 7 | later-ticket: 20261005-provider-settings-upkeep | — | ACP registry icons |
| `apps/web/src/components/settings/AcpRegistrySearchStep.test.tsx` | 8 | later-ticket: 20261005-provider-settings-upkeep | — | ACP registry search |
| `apps/web/src/components/settings/AcpSessionManagementSection.test.tsx` | 6 | later-ticket: 20261005-provider-settings-upkeep | — | ACP sessions |
| `apps/web/src/components/settings/AddProviderInstanceDialog.environment.test.tsx` | 7 | n/a-ui | — | React dialog rendering |
| `apps/web/src/components/settings/AddProviderInstanceDialog.test.ts` | 12 | port | `providers.ts` / `providers.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/AddProviderInstanceWizardSteps.test.tsx` | 4 | n/a-ui | — | React wizard rendering |
| `apps/web/src/components/settings/CaptureShortcutConfig.test.tsx` | 10 | n/a-excluded | — | Linux capture shortcut config |
| `apps/web/src/components/settings/ConnectionsSettings.logic.test.ts` | 15 | n/a-excluded | — | WSL rows |
| `apps/web/src/components/settings/EnvironmentIconPicker.test.ts` | 5 | port | `connections.ts` / `connections.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/GitHubRoutingSettings.test.ts` | 3 | port | `connections.ts` / `connections.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/IntegrationsSettings.logic.test.ts` | 10 | n/a-excluded | — | Browser profile data |
| `apps/web/src/components/settings/IntegrationsSettings.test.tsx` | 6 | n/a-ui | — | React rendering; Browser cases n/a-excluded |
| `apps/web/src/components/settings/KeybindingsSettings.logic.test.ts` | 19 | port | `keybinding-view.ts` / `r3-settings-keys.test.ts`, `settings-rest.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/LoadBalancingSettings.test.ts` | 5 | later-ticket: 20261005-auto-balance | — | load preferences |
| `apps/web/src/components/settings/ProjectFaviconPickerDialog.test.tsx` | 4 | later-ticket: 20261005-desktop-shell-details | — | native file picker for the project icon |
| `apps/web/src/components/settings/ProjectIconPickerDialog.test.tsx` | 2 | n/a-ui | — | React picker rendering |
| `apps/web/src/components/settings/ProjectSettingsPanel.logic.test.ts` | 4 | port | `projects-view.ts` / new test file | vite-plus/test → bun:test |
| `apps/web/src/components/settings/ProviderInstanceCard.test.ts` | 10 | port | `providers.ts` / `providers.test.ts` | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-provider-settings-upkeep`, `20261005-provider-sign-in-and-install` |
| `apps/web/src/components/settings/ProviderModelsSection.test.ts` | 5 | port | `settings-b-models.ts` / `settings-b.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/ProviderSettingsForm.test.ts` | 12 | port | `providers.ts` / `providers.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/ProviderSettingsPanel.environment.test.tsx` | 17 | n/a-ui | — | React routing rendering; cases named by `20261005-provider-settings-upkeep`, `20261005-provider-sign-in-and-install` |
| `apps/web/src/components/settings/ProviderSettingsPanel.logic.test.ts` | 21 | port | `providers.ts` / `providers.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/settings/ProviderSetupSection.test.tsx` | 12 | later-ticket: 20261005-provider-sign-in-and-install | — | Antigravity setup |
| `apps/web/src/components/settings/ResourceTelemetryDiagnostics.logic.test.ts` | 10 | port | `settings-a-telemetry.ts` / `settings-a-telemetry.test.ts` | vite-plus/test → bun:test; Effect values → plain results; DateTime → ISO text; resource diagnostics, not analytics |
| `apps/web/src/components/settings/SettingInheritance.test.ts` | 4 | port | `source-control-view.ts` / `settings-rest.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/settings/SettingsPanels.logic.test.ts` | 22 | port | `settings-a-background.ts` / `settings-a.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings |
| `apps/web/src/components/settings/SettingsPanels.restore.test.tsx` | 3 | n/a-ui | — | React restore rendering |
| `apps/web/src/components/settings/SnapShotSettings.logic.test.ts` | 14 | port | `snapshot-settings.ts` / `snapshot-settings.test.ts` | vite-plus/test → bun:test; GNOME/KDE/Hyprland cases n/a-excluded |
| `apps/web/src/components/settings/SnapShotSettings.test.tsx` | 11 | n/a-ui | — | React setup rendering |
| `apps/web/src/components/settings/SnapShotSetupDialog.logic.test.ts` | 25 | n/a-excluded | — | Linux desktop setup steps (GNOME, KDE, Hyprland) |
| `apps/web/src/components/settings/SourceControlWritingSettings.test.tsx` | 7 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | mixed settings across environments |
| `apps/web/src/components/settings/ThemeEditorHost.test.tsx` | 4 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | theme editor |
| `apps/web/src/components/settings/ThemeImportDialog.test.ts` | 4 | port | `settings-appearance-import.ts` / `settings-a.test.ts`, `settings-a-collections.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/browserImportWizard.logic.test.ts` | 36 | n/a-excluded | — | Browser cookie import |
| `apps/web/src/components/settings/colorPickers.test.tsx` | 9 | n/a-ui | — | React color controls |
| `apps/web/src/components/settings/customModelEditor.logic.test.ts` | 10 | later-ticket: 20261005-provider-settings-upkeep | — | custom model options |
| `apps/web/src/components/settings/deviceHostConnectionChecks.test.ts` | 4 | port | `settings-a-hosts.ts` / `settings-a-hosts.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings |
| `apps/web/src/components/settings/deviceHostsSettings.logic.test.ts` | 7 | port | `settings-a-hosts.ts` / `settings-a-hosts.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/settings/pairingUrls.test.ts` | 3 | later-ticket: 20261005-this-machine-network-access | — | pairing URLs |
| `apps/web/src/components/settings/providerStatus.test.ts` | 10 | port | `providers-meta.ts` / `providers.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/settings/scheduledTasksSettings.logic.test.ts` | 12 | port | `scheduled-view.ts` / `settings-a.test.ts` | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-live-automations-and-clones` |
| `apps/web/src/components/settings/scopedSettings.test.ts` | 27 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | scope across environments |
| `apps/web/src/components/settings/settingsLayout.test.tsx` | 6 | n/a-ui | — | DOM layout |
| `apps/web/src/components/settings/settingsScope.test.ts` | 14 | port | `settings-core.ts` / `settings-core.test.ts`, `settings-a-about.test.ts` | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-settings-scoped-controls-and-theme-editor` |
| `apps/web/src/components/settings/settingsScopeAxis.test.ts` | 9 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | scope axes across environments |
| `apps/web/src/components/settings/settingsScopeNavigation.test.ts` | 15 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | scope navigation across environments |
| `apps/web/src/components/settings/settingsSearch.test.ts` | 46 | port | `settings-search.ts` / `settings-core.test.ts` | vite-plus/test → bun:test; branded ids → strings; Intl/locale (X36) |
| `apps/web/src/components/settings/themeEditorStore.test.ts` | 3 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | theme editor |
| `apps/web/src/components/settings/themeInspector.test.ts` | 6 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | theme editor inspector |
| `apps/web/src/components/settings/useSnapShotShortcutRecorder.test.tsx` | 9 | swift | `macos/tests/snapshot` | shortcut recording (T3KeyRecorder.swift) |
| `apps/web/src/components/sidebar/SidebarUpdatePill.test.tsx` | 5 | n/a-excluded | — | update feed release notes |
| `apps/web/src/components/sidebar/SidebarUpdateReleaseNotes.test.tsx` | 5 | n/a-excluded | — | update feed release notes |
| `apps/web/src/components/threadActionMenu.logic.test.ts` | 17 | port | `sidebar-menu.ts` / `sidebar.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/threadSidebarWidth.test.ts` | 8 | port | `r12-sidebar-width.ts` / `r12-sidebar.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/ui/qr-code.test.tsx` | 3 | n/a-ui | — | UI primitive rendering |
| `apps/web/src/components/ui/sidebar.test.tsx` | 2 | n/a-ui | — | UI primitive rendering |
| `apps/web/src/components/ui/switch.test.tsx` | 2 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | mixed switch state (D15, ui/switch.tsx) |
| `apps/web/src/components/ui/toast.logic.test.ts` | 16 | port | `toast.ts` / new test file | vite-plus/test → bun:test |
| `apps/web/src/components/ui/toastHelpers.test.ts` | 2 | n/a-ui | — | UI primitive rendering |
| `apps/web/src/components/usage/UsagePage.refresh.test.tsx` | 5 | n/a-ui | — | React refresh lifecycle; cases named by `20261005-composer-fidelity`, `20261005-usage-pooled-view` |
| `apps/web/src/components/usage/UsagePage.test.tsx` | 5 | n/a-ui | — | React Escape navigation |
| `apps/web/src/components/usage/UsageProviderChart.test.ts` | 15 | port | `pages-usage.ts` / `pages-usage.test.ts`, `pages-usage-r3.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/usage/usageBreakdown.test.ts` | 7 | port | `pages-usage-detail.ts` / `pages-usage-r3.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/usage/usagePagePreferences.test.ts` | 6 | port | `pages-prefs.ts` / `pages-prs.test.ts`, `pages-welcome.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/usage/usagePriceForm.test.ts` | 6 | port | `pages-usage-prices.ts` / `pages-usage-r3.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/components/usage/usagePriceTable.test.ts` | 12 | port | `pages-usage-prices.ts` / `pages-usage-r3.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/usage/usagePriceTargets.test.ts` | 6 | port | `pages-usage-prices.ts` / `pages-usage-r3.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/components/usage/usageShortcuts.test.ts` | 2 | port | `pages-usage.ts` / `pages-usage.test.ts`, `pages-usage-r3.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/composer-editor-mentions.test.ts` | 33 | swift | `macos/tests/composer` | mention segments in the composer text view (T3ComposerText.swift) |
| `apps/web/src/composer-list-continuation.test.ts` | 11 | swift | `macos/tests/composer` | list continuation (T3ComposerText.swift) |
| `apps/web/src/composer-logic.test.ts` | 72 | port | `composer-editor-intent.ts` / `composer-editor-send.test.ts`, `composer-editor-intent.test.ts` | vite-plus/test → bun:test; branded ids → strings; key intents; cursor cases go to the `composer` AppKit binary |
| `apps/web/src/composer-rich-text-doc.test.ts` | 14 | n/a-ui | — | ProseMirror document model; the clone edits an NSTextView |
| `apps/web/src/composer-rich-text.test.ts` | 7 | swift | `macos/tests/composer` | bold/italic markers (T3ComposerStyler.swift) |
| `apps/web/src/composer-undo-grouping.test.ts` | 8 | swift | `macos/tests/composer` | NSTextView undo grouping; plan gap: no clone counterpart found and no ticket owns it |
| `apps/web/src/composerDraftStore.test.ts` | 160 | port | `composer-controls.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; Effect values → plain results; branded ids → strings |
| `apps/web/src/confirmDialog.test.ts` | 6 | n/a-ui | — | React dialog coordinator |
| `apps/web/src/connection/clientMetadata.test.ts` | 4 | n/a-excluded | — | telemetry client metadata |
| `apps/web/src/connection/desktopLocal.test.ts` | 10 | n/a-excluded | — | desktop-local secondary backends (a parallel WSL backend) |
| `apps/web/src/connection/platform.test.ts` | 13 | swift | `macos/tests/ssh` | desktop SSH pairing and bearer cache (T3Ssh.swift, T3Credentials.swift) |
| `apps/web/src/connection/storage.test.ts` | 8 | swift | `macos/tests/fleet` | catalog store and secure storage (T3Fleet.swift) |
| `apps/web/src/contextMenuFallback.test.ts` | 10 | n/a-ui | — | browser fallback menu; the clone uses native menus |
| `apps/web/src/desktopAppActivation.test.ts` | 5 | later-ticket: 20261005-app-activation | — | `t3 app` opens a project thread |
| `apps/web/src/diffFileActions.test.ts` | 8 | port | `diff.ts` / `diff.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/diffPanelStore.test.ts` | 10 | port | `diff.ts` / `diff.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/editorLabels.test.ts` | 6 | port | `shell-details.ts` / `r3-shell-details.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/environmentGrouping.test.ts` | 16 | port | `r6-polish-groups.ts` / `r6-polish.test.ts` | vite-plus/test → bun:test; branded ids → strings; reference name environmentGrouping |
| `apps/web/src/environments/primary/bootstrap.test.ts` | 14 | later-ticket: 20261005-local-primary-environment | — | primary environment bootstrap and auth |
| `apps/web/src/environments/primary/desktopAuth.test.ts` | 3 | later-ticket: 20261005-local-primary-environment | — | primary environment bootstrap and auth |
| `apps/web/src/environments/primary/httpLayer.test.ts` | 3 | later-ticket: 20261005-local-primary-environment | — | primary environment bootstrap and auth |
| `apps/web/src/fileContextMenu.test.ts` | 9 | later-ticket: 20261005-media-actions | — | file context menu items and paths |
| `apps/web/src/filePathDisplay.test.ts` | 6 | port | `timeline-worklog.ts` / `timeline-item-detail.test.ts`, `timeline.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/hooks/showThreadUndoNotice.test.ts` | 9 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; Effect values → plain results |
| `apps/web/src/hooks/threadUndo.test.ts` | 5 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/hooks/useCopyToClipboard.test.ts` | 8 | n/a-ui | — | browser clipboard flavors; the clone writes the pasteboard natively |
| `apps/web/src/hooks/useDefaultTheme.test.ts` | 7 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | environment default theme adoption |
| `apps/web/src/hooks/useEnvironmentDisconnectDelay.test.tsx` | 3 | port | `r8-pointer-reconnect.ts` / `r8-pointer.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; branded ids → strings; render cases n/a-ui; React hook → plain function with a `now` argument |
| `apps/web/src/hooks/useEnvironmentTheme.test.ts` | 12 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | environment themes |
| `apps/web/src/hooks/useEnvironmentThemeSync.test.ts` | 5 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | published theme refresh |
| `apps/web/src/hooks/useHandleNewThread.test.ts` | 5 | port | `r7-handoff-thread.ts` / `r7-handoff.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness |
| `apps/web/src/hooks/useLiveRefresh.test.ts` | 20 | later-ticket: 20261005-pr-conversation-and-refresh | — | live refresh cadence |
| `apps/web/src/hooks/useLocalStorage.test.ts` | 6 | n/a-ui | — | browser storage errors |
| `apps/web/src/hooks/usePullRequestChecksRefresh.test.ts` | 1 | later-ticket: 20261005-pr-conversation-and-refresh | — | checks polling |
| `apps/web/src/hooks/useResizableWidth.test.tsx` | 10 | n/a-ui | — | DOM pointer resize |
| `apps/web/src/hooks/useSettings.test.ts` | 24 | later-ticket: 20261005-local-primary-environment | — | client settings hydration (U7) |
| `apps/web/src/hooks/useTerminalFocus.test.ts` | 2 | later-ticket: 20261005-terminal-layout | — | terminal focus |
| `apps/web/src/hooks/useTheme.test.ts` | 6 | n/a-ui | — | browser theme storage |
| `apps/web/src/hooks/useThreadActions.test.ts` | 10 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/hooks/useThreadActions.undo.test.ts` | 11 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; vi.mock → fake native harness; branded ids → strings |
| `apps/web/src/hooks/useWorkspaceMutationRefresh.test.ts` | 2 | n/a-excluded | — | preview resources (Browser surface) |
| `apps/web/src/hostedPairing.test.ts` | 7 | later-ticket: 20261005-this-machine-network-access | — | hosted pairing links (U8) |
| `apps/web/src/keybindings.test.ts` | 98 | port | `keyboard-dispatch.ts` / new test file | vite-plus/test → bun:test; terminal shortcut cases wait for the terminal tickets; cases named by `20261005-terminal-layout` |
| `apps/web/src/lib/assistantCitationNavigation.test.ts` | 4 | port | `r5-composer-citation.ts` / `r5-composer-citation.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/lib/assistantTextSelection.test.ts` | 34 | n/a-ui | — | DOM selection capture |
| `apps/web/src/lib/attachmentUploadQueue.test.ts` | 22 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; Effect values → plain results; branded ids → strings |
| `apps/web/src/lib/attachmentUploadState.test.ts` | 7 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/lib/backgroundActivityReporter.test.ts` | 5 | swift | `macos/tests/activity` | T3ActivityReporter.swift; cases named by `20261005-client-activity-reporting` |
| `apps/web/src/lib/baseRefChoices.test.ts` | 4 | port | `composer-controls-branch.ts` / `composer-controls-branch.test.ts`, `composer-controls.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/chatThreadActions.test.ts` | 11 | port | `composer-controls-branch.ts` / `composer-controls-branch.test.ts`, `composer-controls.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/lib/chunkReloadGuard.test.ts` | 4 | n/a-ui | — | bundle chunk reload |
| `apps/web/src/lib/color.test.ts` | 4 | port | `settings-b-accent.ts` / `settings-b-accent.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/composerContextRecords.test.ts` | 28 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; cases named by `20261005-terminal-integrations` |
| `apps/web/src/lib/composerContextReferences.test.ts` | 10 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/contextWindow.test.ts` | 7 | port | `composer-controls-view.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; Effect values → plain results; DateTime → ISO text |
| `apps/web/src/lib/desktopPasteAsText.test.ts` | 2 | swift | `macos/tests/r8-keys` | Paste as Text (R8KeysMenus.swift) |
| `apps/web/src/lib/desktopSnapShot.test.ts` | 5 | swift | `macos/tests/snapshot` | capture bridge capability |
| `apps/web/src/lib/diffCollapse.test.ts` | 4 | later-ticket: 20261005-diff-review-engine | — | collapse all |
| `apps/web/src/lib/diffFileContents.test.ts` | 4 | later-ticket: 20261005-diff-review-engine | — | expansion file contents |
| `apps/web/src/lib/diffRendering.test.ts` | 22 | later-ticket: 20261005-diff-review-engine | — | renderable patch cache |
| `apps/web/src/lib/discardComposerDraft.test.ts` | 4 | port | `r11-upstream-drafts.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; vi.mock → fake native harness; branded ids → strings |
| `apps/web/src/lib/elementContext.test.ts` | 6 | n/a-excluded | — | Browser element picker |
| `apps/web/src/lib/imageCompression.test.ts` | 32 | swift | `macos/tests/attach` | thumbnails and re-encoding (T3ComposerAttach.swift) |
| `apps/web/src/lib/incrementalHighlighting.test.ts` | 5 | later-ticket: 20261005-shiki-residuals | — | long texts |
| `apps/web/src/lib/lruCache.test.ts` | 7 | n/a-ui | — | generic cache behind web rendering; the clone has no counterpart to test |
| `apps/web/src/lib/openPullRequestLink.test.ts` | 42 | port | `palette-linkpr.ts` / `palette-linkpr.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/lib/orchestrationV2Timeline.test.ts` | 5 | port | `domain.ts` / `domain.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/previewAnnotation.test.ts` | 4 | n/a-excluded | — | Browser annotate |
| `apps/web/src/lib/projectPaths.test.ts` | 12 | port | `palette-add.ts` / `palette.test.ts` | vite-plus/test → bun:test; cases named by `20261005-app-activation` |
| `apps/web/src/lib/projectScriptKeybindings.test.ts` | 7 | port | `settings-b-actions.ts` / `settings-b.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/runtime.test.ts` | 2 | n/a-excluded | — | telemetry span export |
| `apps/web/src/lib/selectionActions.test.ts` | 25 | n/a-ui | — | DOM selection popup gestures |
| `apps/web/src/lib/snapShotAnimation.test.ts` | 4 | swift | `macos/tests/snapshot` | capture card animation (T3SnapshotFeedback.swift) |
| `apps/web/src/lib/snapShotSetupResume.test.ts` | 2 | port | `snapshot-settings.ts` / `snapshot-settings.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/snapShotShortcut.test.ts` | 14 | port | `snapshot-shortcut.ts` / `snapshot-settings.test.ts`, `snapshot-drafts.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/snapShotSound.test.ts` | 3 | swift | `macos/tests/snapshot` | capture sound |
| `apps/web/src/lib/syntaxHighlighting.test.ts` | 1 | later-ticket: 20261005-shiki-residuals | — | unsupported language fallback |
| `apps/web/src/lib/terminalCloseConfirm.test.ts` | 5 | later-ticket: 20261005-terminal-layout | — | terminal close confirmation; cases named by `20261005-terminal-drawer` |
| `apps/web/src/lib/terminalCloseShortcut.test.ts` | 4 | later-ticket: 20261005-terminal-layout | — | terminal close shortcut |
| `apps/web/src/lib/terminalContext.test.ts` | 6 | later-ticket: 20261005-terminal-integrations | — | terminal context chips |
| `apps/web/src/lib/terminalFocus.test.ts` | 4 | later-ticket: 20261005-terminal-layout | — | terminal focus owner |
| `apps/web/src/lib/threadSort.test.ts` | 6 | port | `legacy-sidebar-model.ts` / `legacy-sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings; 5 of 6 titles in legacy-sidebar.test.ts; missing only the describe "sortThreads" |
| `apps/web/src/lib/turnDiffTree.test.ts` | 8 | port | `timeline-tree.ts` / `timeline-tree.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/lib/utils.test.ts` | 5 | port | `shell-details.ts` / `r3-shell-details.test.ts` | vite-plus/test → bun:test; file manager name |
| `apps/web/src/lib/videoFirstFrame.test.ts` | 4 | swift | `macos/tests/attach` | first-frame poster (T3ComposerAttach.swift) |
| `apps/web/src/lib/visibleAnimation.test.ts` | 5 | n/a-ui | — | IntersectionObserver animation |
| `apps/web/src/localApi.test.ts` | 9 | n/a-ui | — | browser local API facade |
| `apps/web/src/markdown-clipboard.test.ts` | 15 | later-ticket: 20261005-upstream-timeline-and-markdown | — | rendered Markdown copy text; clone parser is Rust (`macos/src/markdown.rs`) |
| `apps/web/src/markdown-github-alerts.test.tsx` | 7 | later-ticket: 20261005-upstream-timeline-and-markdown | — | GitHub alert quotes; clone parser is Rust |
| `apps/web/src/markdown-incremental.test.tsx` | 7 | n/a-ui | — | incremental React Markdown parse cache |
| `apps/web/src/markdown-links.test.ts` | 68 | later-ticket: 20261005-upstream-timeline-and-markdown | — | markdownLinks belongs to that ticket |
| `apps/web/src/markdown-list-indentation.test.tsx` | 8 | later-ticket: 20261005-upstream-timeline-and-markdown | — | list indentation recovery; clone parser is Rust |
| `apps/web/src/modelOrdering.test.ts` | 3 | port | `model-catalog.ts` / `composer.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/modelSelection.test.ts` | 32 | port | `settings-b-models.ts` / `settings-b.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/onboarding/firstRun.logic.test.ts` | 44 | port | `pages-welcome.ts` / `pages-welcome.test.ts`, `pages-r3.test.ts` | vite-plus/test → bun:test; cases named by `20261005-local-primary-environment` |
| `apps/web/src/onboarding/projectImport.logic.test.ts` | 27 | port | `pages-welcome.ts` / `pages-welcome.test.ts`, `pages-r3.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/onboarding/providerReadiness.logic.test.ts` | 25 | port | `pages-welcome.ts` / `pages-welcome.test.ts`, `pages-r3.test.ts` | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-sign-in-terminals` |
| `apps/web/src/onboarding/targetEnvironment.logic.test.ts` | 12 | port | `pages-welcome.ts` / `pages-welcome.test.ts`, `pages-r3.test.ts` | vite-plus/test → bun:test; branded ids → strings; relay cases n/a-excluded |
| `apps/web/src/openVsxThemes.test.ts` | 11 | port | `settings-appearance-import.ts` / `settings-a.test.ts`, `settings-a-collections.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument |
| `apps/web/src/panelAnimations.test.tsx` | 2 | n/a-ui | — | React panel animation suppression |
| `apps/web/src/pendingUserInput.test.ts` | 27 | port | `requests.ts` / `requests.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/pierre-icons.test.ts` | 9 | port | `timeline-files.ts` / `timeline-highlight.test.ts` | vite-plus/test → bun:test; Rust icon table `macos/src/pierre_icons.rs` holds part |
| `apps/web/src/portDiscoveryState.test.ts` | 3 | n/a-excluded | — | Browser local-server discovery |
| `apps/web/src/previewMiniPlayerStore.test.ts` | 7 | done-equivalent | `previewMiniPlayerStore.ts` / `previewMiniPlayerStore.test.ts` | every reference title present (test-map.mjs check); cases named by `20261005-floating-device-player`, `20261005-round12-wrapup` |
| `apps/web/src/previewStateStore.test.ts` | 27 | n/a-excluded | — | Browser surface |
| `apps/web/src/projectIconOptions.test.ts` | 4 | port | `settings-b-icons.ts` / `settings-b-icons.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/projectIdentity.test.ts` | 5 | port | `settings-b-icons.ts` / `settings-b-icons.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/projectScripts.test.ts` | 12 | port | `settings-b-actions.ts` / `settings-b.test.ts` | vite-plus/test → bun:test; cases named by `20261005-terminal-drawer` |
| `apps/web/src/promptStashStore.test.ts` | 17 | port | `composer-editor-stash.ts` / `composer-editor-stash.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/proposedPlan.test.ts` | 21 | port | `timeline-plan.ts` / new test file | vite-plus/test → bun:test |
| `apps/web/src/providerInstances.test.ts` | 42 | port | `providers.ts` / `providers.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/providerModels.test.ts` | 4 | port | `model-catalog.ts` / `composer.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/providerSkillSearch.test.ts` | 8 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/providerUpdateDismissal.test.ts` | 2 | port | `shell-notify.ts` / `shell.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/pullRequestReference.test.ts` | 15 | port | `palette-linkpr.ts` / `palette-linkpr.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/questionAttachments.test.ts` | 5 | port | `requests.ts` / `requests.test.ts` | vite-plus/test → bun:test; vi.mock → fake native harness; branded ids → strings |
| `apps/web/src/remoteOpen.test.ts` | 15 | later-ticket: 20261005-ssh-password-and-remote-open | — | Open in a local editor for remote environments |
| `apps/web/src/reviewCommentContext.test.ts` | 5 | later-ticket: 20261005-diff-review-engine | — | review comment context |
| `apps/web/src/rightPanelStore.test.ts` | 53 | port | `right-panel-tabs.ts` / `right-panel-tabs.test.ts` | vite-plus/test → bun:test; branded ids → strings; 7 of 53 titles already in right-panel-tabs.test.ts; cases named by `20261005-right-panel-tab-menu`, `20261005-terminal-layout` |
| `apps/web/src/rpc/requestLatencyState.test.ts` | 9 | done-equivalent | `shell-slow.ts` / `request-latency.test.ts` | every reference title present (test-map.mjs check) |
| `apps/web/src/session-logic.runtime-diagnostics.test.ts` | 7 | port | `timeline-rows.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `apps/web/src/session-logic.test.ts` | 41 | port | `timeline-rows.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text; render cases n/a-ui |
| `apps/web/src/shortcutModifierState.test.ts` | 8 | n/a-ui | — | React modifier-state hook |
| `apps/web/src/sidebarPendingFileDropStore.test.ts` | 11 | port | `sidebar-drop.ts` / `sidebar-extra.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/state/desktopNetworkAccess.test.ts` | 3 | later-ticket: 20261005-this-machine-network-access | — | Network access state |
| `apps/web/src/state/desktopSshHosts.test.ts` | 10 | port | `settings-b-ssh.ts` / `settings-b-ssh.test.ts` | vite-plus/test → bun:test; Effect values → plain results |
| `apps/web/src/state/desktopUpdate.test.ts` | 4 | n/a-excluded | — | update feed |
| `apps/web/src/state/desktopWslState.test.ts` | 4 | n/a-excluded | — | WSL |
| `apps/web/src/state/entities.test.ts` | 4 | n/a-ui | — | Effect atom subscription; no pure part |
| `apps/web/src/state/paginatedBranches.test.ts` | 7 | swift | `macos/tests/r5-composer` | next-page signal (R5ComposerScroll.swift) |
| `apps/web/src/state/pullRequests.test.ts` | 5 | port | `shell-pr.ts` / `r3-shell-pr.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/state/queries.test.ts` | 2 | port | `r5-composer-paging.ts` / `r5-composer.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `apps/web/src/state/queries.threadSearch.test.tsx` | 2 | n/a-ui | — | React search hook |
| `apps/web/src/state/shell.test.ts` | 6 | n/a-ui | — | Effect atom readiness; no pure part |
| `apps/web/src/state/terminalSessions.test.ts` | 7 | later-ticket: 20261005-terminal-layout | — | terminal session picker; cases named by `20261005-terminal-drawer` |
| `apps/web/src/state/threads.test.ts` | 4 | n/a-ui | — | Effect keep-alive atom; no pure part; cases named by `20261005-local-primary-environment` |
| `apps/web/src/state/usage.test.tsx` | 6 | later-ticket: 20261005-usage-pooled-view | — | usage environment selection |
| `apps/web/src/state/waitForAtomValue.test.ts` | 4 | n/a-ui | — | Effect atom helper; no pure part |
| `apps/web/src/terminal-links.test.ts` | 20 | later-ticket: 20261005-terminal-integrations | — | terminal links; cases named by `20261005-terminal-surface` |
| `apps/web/src/terminal/ghostty/core.test.ts` | 14 | later-ticket: 20261005-terminal-surface | — | Ghostty emulator |
| `apps/web/src/terminal/ghostty/keyCodes.test.ts` | 8 | later-ticket: 20261005-terminal-surface | — | Ghostty emulator |
| `apps/web/src/terminal/ghostty/renderer.test.ts` | 12 | later-ticket: 20261005-terminal-surface | — | Ghostty emulator |
| `apps/web/src/terminal/ghostty/runtimeAbi.test.ts` | 10 | later-ticket: 20261005-terminal-surface | — | Ghostty emulator |
| `apps/web/src/terminal/ghostty/surface.test.ts` | 71 | later-ticket: 20261005-terminal-surface | — | Ghostty emulator |
| `apps/web/src/terminalUiStateStore.test.ts` | 15 | later-ticket: 20261005-terminal-layout | — | tabs and splits; cases named by `20261005-terminal-drawer` |
| `apps/web/src/themeBoot.test.ts` | 15 | n/a-ui | — | index.html boot script |
| `apps/web/src/themePalette.test.ts` | 37 | port | `settings-themes.ts` / `settings-core.test.ts`, `settings-a.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/threadRoutes.test.ts` | 11 | n/a-ui | — | router params; the clone has no URL routes |
| `apps/web/src/threadSelectionStore.test.ts` | 37 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings; render cases n/a-ui; bulk selection and deletion |
| `apps/web/src/threadSync.test.ts` | 6 | port | `composer-controls-view.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test |
| `apps/web/src/timestampFormat.test.ts` | 45 | later-ticket: 20261005-desktop-shell-details | — | timestamps in the Mac's locale |
| `apps/web/src/uiStateStore.test.ts` | 21 | port | `sidebar-state.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings; unread/visit state and project expansion |
| `apps/web/src/versionSkew.test.ts` | 20 | later-ticket: 20261005-server-update-banner | — | version skew card |
| `apps/web/src/vscodeThemeImport.test.ts` | 29 | port | `settings-appearance-import.ts` / `settings-a.test.ts`, `settings-a-collections.test.ts` | vite-plus/test → bun:test; 8 of 29 titles already in r3-settings-import.test.ts |
| `apps/web/src/workspaceBasenameLookup.test.ts` | 13 | port | none found | vite-plus/test → bun:test; bare-filename link lookup; plan gap: no clone module found |
| `apps/web/src/worktreeCleanup.test.ts` | 11 | later-ticket: 20261005-thread-commands-and-keys | — | delete worktree with the thread |
| `apps/web/src/wslPaths.test.ts` | 25 | n/a-excluded | — | WSL |
| `packages/client-runtime/src/authorization/layer.test.ts` | 27 | swift | `macos/tests/transport` | bearer descriptor cache (T3Credentials.swift); relay/account cases n/a-excluded |
| `packages/client-runtime/src/authorization/remote.test.ts` | 11 | swift | `macos/tests/transport` | remote bearer bootstrap (T3Credentials.swift) |
| `packages/client-runtime/src/codexArtifactTemplates.test.ts` | 14 | later-ticket: 20261005-upstream-timeline-and-markdown | — | Codex artifact template blocks |
| `packages/client-runtime/src/codexFileCitations.test.ts` | 8 | later-ticket: 20261005-upstream-timeline-and-markdown | — | Codex file citation links |
| `packages/client-runtime/src/codexMarkdownDirectives.test.ts` | 15 | later-ticket: 20261005-upstream-timeline-and-markdown | — | Codex Markdown directives |
| `packages/client-runtime/src/composerThreadItems.test.ts` | 3 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/connection/compatibility.test.ts` | 5 | port | `r3-protocol-outdated.ts` / `r3-protocol.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/connection/errors.test.ts` | 8 | n/a-excluded | — | relay and DPoP errors (T3 Connect) |
| `packages/client-runtime/src/connection/onboarding.test.ts` | 10 | port | `r9-connect-onboarding.ts` / `r9-connect.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/connection/outdatedHostUpdate.test.ts` | 3 | swift | `macos/tests/transport` | outdated-host update (r3.swift) |
| `packages/client-runtime/src/connection/presentation.test.ts` | 9 | port | `connections.ts` / `connections.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/connection/registry.test.ts` | 41 | swift | `macos/tests/fleet` | environment registry (T3Fleet.swift); relay cases n/a-excluded; cases named by `20261005-environment-routes` |
| `packages/client-runtime/src/connection/resolver.test.ts` | 11 | swift | `macos/tests/transport` | discovery blocks old hosts before opening |
| `packages/client-runtime/src/connection/routes.test.ts` | 16 | later-ticket: 20261005-environment-routes | — | routes per environment |
| `packages/client-runtime/src/connection/supervisor.test.ts` | 60 | swift | `macos/tests/transport` | reconnect ladder and supervision (T3Transport.swift); relay cases n/a-excluded |
| `packages/client-runtime/src/delayedStatus.test.ts` | 4 | port | `composer-controls-view.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument |
| `packages/client-runtime/src/device/androidFoldScene.test.ts` | 4 | swift | `macos/tests/r9-device` | R9DeviceFold.swift |
| `packages/client-runtime/src/device/deviceFraming.test.ts` | 2 | swift | `macos/tests/r7-device` | R7DeviceMotion.swift |
| `packages/client-runtime/src/device/deviceMotion.test.ts` | 8 | swift | `macos/tests/r7-device` | R7DeviceMotion.swift |
| `packages/client-runtime/src/device/duoControl.test.ts` | 4 | port | `r9-device-duo.ts` / `r9-device-duo.test.ts` | vite-plus/test → bun:test; fake timers → `now` argument; native send queue in R9DeviceDuo.swift |
| `packages/client-runtime/src/device/duoScene.test.ts` | 3 | swift | `macos/tests/r9-device` | R9DeviceDuo.swift |
| `packages/client-runtime/src/device/duoSnap.test.ts` | 2 | swift | `macos/tests/r9-device` | R9DeviceDuo.swift |
| `packages/client-runtime/src/device/duoStream.test.ts` | 1 | swift | `macos/tests/r9-device` | R9DeviceDuoView.swift |
| `packages/client-runtime/src/device/duoViewer.test.ts` | 14 | swift | `macos/tests/r9-device` | R9DeviceDuoView.swift |
| `packages/client-runtime/src/device/frame.test.ts` | 2 | swift | `macos/tests/r7-device` | R7DeviceVideo.swift |
| `packages/client-runtime/src/device/model.test.ts` | 6 | swift | `macos/tests/r7-device` | R7DeviceGLB.swift |
| `packages/client-runtime/src/device/modelScene.test.ts` | 3 | swift | `macos/tests/r7-device` | R7DeviceGLB.swift |
| `packages/client-runtime/src/device/phoneInteraction.test.ts` | 5 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `packages/client-runtime/src/device/phoneScene.test.ts` | 7 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `packages/client-runtime/src/device/phoneViewer.test.ts` | 15 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `packages/client-runtime/src/device/renderScheduler.test.ts` | 1 | swift | `macos/tests/r7-device` | R7DeviceClient.swift |
| `packages/client-runtime/src/device/screenshot.test.ts` | 2 | swift | `macos/tests/r6-device` | R6DeviceStream.swift |
| `packages/client-runtime/src/device/shapeProfile.test.ts` | 2 | swift | `macos/tests/r7-device` | R7DevicePhone.swift |
| `packages/client-runtime/src/device/stream.test.ts` | 31 | swift | `macos/tests/r6-device` | R6DeviceStream.swift, R7DeviceVideo.swift |
| `packages/client-runtime/src/device/streamFrames.test.ts` | 1 | swift | `macos/tests/r7-device` | R7DeviceClient.swift |
| `packages/client-runtime/src/environment/endpoint.test.ts` | 4 | port | `connections.ts` / `connections.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/environment/knownEnvironment.test.ts` | 6 | port | `settings-b-fleet.ts` / `settings-b-connections.test.ts`, `settings-b.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/errors/errorTrace.test.ts` | 5 | n/a-ui | — | Effect Cause traversal; no pure part |
| `packages/client-runtime/src/errors/orchestration.test.ts` | 4 | port | `domain.ts` / `domain.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/errors/safeLog.test.ts` | 4 | n/a-excluded | — | telemetry log attributes |
| `packages/client-runtime/src/errors/transport.test.ts` | 14 | port | `r8-pointer-reconnect.ts` / `r8-pointer.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/filePreview.test.ts` | 6 | port | `r5-panels-attach.ts` / `r5-panels.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/handoff.test.ts` | 5 | port | `r7-handoff-thread.ts` / `r7-handoff.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/markdownImages.test.ts` | 6 | later-ticket: 20261005-upstream-timeline-and-markdown | — | Markdown image sources; clone parser is Rust |
| `packages/client-runtime/src/markdownLinks.test.ts` | 19 | later-ticket: 20261005-upstream-timeline-and-markdown | — | named in that ticket |
| `packages/client-runtime/src/mediaReference.test.ts` | 4 | later-ticket: 20261005-media-actions | — | media file references |
| `packages/client-runtime/src/mediaSource.test.ts` | 14 | later-ticket: 20261005-media-actions | — | media sources and Open original |
| `packages/client-runtime/src/operations/commands.performance.test.ts` | 2 | n/a-ui | — | Effect transport budget; no pure part |
| `packages/client-runtime/src/operations/commands.test.ts` | 21 | port | `composer-controls-commands.ts` / `composer-controls.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings; Effect command transport → plain command builders; cases named by `20261005-composer-fidelity` |
| `packages/client-runtime/src/operations/projects.test.ts` | 18 | port | `palette-add.ts` / `palette.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/operations/threadTitle.test.ts` | 6 | port | `composer-editor-title.ts` / `composer-editor-title.test.ts` | @effect/vitest → bun:test; Effect values → plain results |
| `packages/client-runtime/src/platform/orchestrationCache.test.ts` | 4 | n/a-ui | — | Effect cache envelopes; the clone keeps no client cache |
| `packages/client-runtime/src/platform/storageDocument.test.ts` | 17 | swift | `macos/tests/fleet` | connection catalog document (T3Fleet.swift) |
| `packages/client-runtime/src/projectFaviconCache.test.ts` | 16 | port | `r3-sidebar-glyph.ts` / `r3-sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/providerSkills.test.ts` | 22 | later-ticket: 20261005-upstream-ui-sync | — | providerSkills display names belong to that ticket; cases named by `20261005-client-activity-reporting`, `20261005-upstream-timeline-and-markdown` |
| `packages/client-runtime/src/relay/discovery.test.ts` | 8 | n/a-excluded | — | T3 Connect relay |
| `packages/client-runtime/src/relay/errorPresentation.test.ts` | 5 | n/a-excluded | — | T3 Connect relay |
| `packages/client-runtime/src/relay/managedRelay.test.ts` | 14 | n/a-excluded | — | T3 Connect relay |
| `packages/client-runtime/src/relay/managedRelayState.test.ts` | 14 | n/a-excluded | — | T3 Connect relay |
| `packages/client-runtime/src/rpc/client.test.ts` | 17 | swift | `macos/tests/transport` | environment RPC (T3Transport.swift); preview cases n/a-excluded; cases named by `20261005-round12-wrapup` |
| `packages/client-runtime/src/rpc/session.test.ts` | 16 | swift | `macos/tests/transport` | WebSocket session (T3Transport.swift) |
| `packages/client-runtime/src/state/archivedThreads.test.ts` | 2 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/assets.test.ts` | 13 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/attachments.test.ts` | 11 | port | `composer-editor-attach.ts` / `composer-editor-attach.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/state/auth.test.ts` | 3 | later-ticket: 20261005-this-machine-network-access | — | pairing links and clients stream; cases named by `20261005-ssh-password-and-remote-open` |
| `packages/client-runtime/src/state/boundedThreadSnapshotHttp.test.ts` | 5 | swift | `macos/tests/transport` | HTTP snapshot fallback (T3Transport.swift) |
| `packages/client-runtime/src/state/composerDispatch.test.ts` | 6 | port | `composer-controls.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/state/customSnooze.test.ts` | 8 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/state/entities.test.ts` | 25 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/environmentHttpAuth.test.ts` | 16 | swift | `macos/tests/transport` | authenticated HTTP (T3Credentials.swift); relay cases n/a-excluded |
| `packages/client-runtime/src/state/filesystem.test.ts` | 6 | port | `palette-add.ts` / `palette.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/state/itemSupport.test.ts` | 5 | port | `timeline-rows.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/orchestrationV2Projection.test.ts` | 9 | port | `domain.ts` / `domain.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/presentation.test.ts` | 5 | n/a-ui | — | Effect subscriptions; no pure part |
| `packages/client-runtime/src/state/preview.test.ts` | 2 | n/a-excluded | — | Browser surface |
| `packages/client-runtime/src/state/projectCommands.test.ts` | 2 | done-equivalent | `r12-threads-scratch.ts` / `r13-threads.test.ts` | every reference title present (test-map.mjs check); cases named by `20261005-round12-wrapup` |
| `packages/client-runtime/src/state/projectGrouping.test.ts` | 14 | later-ticket: 20261005-auto-balance | — | load balancing; buildProjectGroups cases port against r6-polish-groups.ts |
| `packages/client-runtime/src/state/providerInstanceDisplay.test.ts` | 24 | port | `providers-meta.ts` / `providers.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/state/pullRequestDiffHttp.test.ts` | 3 | later-ticket: 20261005-pr-code-tab | — | PR diff over HTTP |
| `packages/client-runtime/src/state/pullRequests.test.ts` | 15 | later-ticket: 20261005-pr-links-previews-and-routing | — | viewed marks and metadata across environments |
| `packages/client-runtime/src/state/runtime.test.ts` | 32 | n/a-ui | — | Effect atom helpers; no pure part |
| `packages/client-runtime/src/state/server.test.ts` | 25 | later-ticket: 20261005-server-update-banner | — | update restart reconnect |
| `packages/client-runtime/src/state/serverUsage.test.ts` | 2 | port | `pages-usage.ts` / `pages-usage.test.ts`, `pages-usage-r3.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings; Intl/locale (X36) |
| `packages/client-runtime/src/state/sharedSettings.test.ts` | 20 | later-ticket: 20261005-settings-scoped-controls-and-theme-editor | — | shared settings across environments |
| `packages/client-runtime/src/state/shell-sync.test.ts` | 10 | n/a-ui | — | Effect shell sync; no pure part |
| `packages/client-runtime/src/state/shell.test.ts` | 5 | n/a-ui | — | Effect projections; no pure part |
| `packages/client-runtime/src/state/shellReducer.test.ts` | 20 | port | `domain.ts` / `domain.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/state/sourceControl.test.ts` | 2 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/subagentDisplay.test.ts` | 12 | port | `composer-controls-subagent.ts` / `composer-controls-subagent.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/state/terminalSession.test.ts` | 21 | later-ticket: 20261005-terminal-drawer | — | terminal session reducers |
| `packages/client-runtime/src/state/threadCommands.test.ts` | 9 | port | `sidebar-commands.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/threadDetail.test.ts` | 5 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/threadExecution.test.ts` | 38 | port | `composer-controls-subagent.ts` / `composer-controls-subagent.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/threadFeedback.test.ts` | 12 | later-ticket: 20261005-usage-reset-and-feedback | — | Codex /feedback |
| `packages/client-runtime/src/state/threadHistoryController.test.ts` | 3 | n/a-ui | — | Effect registrations; no pure part |
| `packages/client-runtime/src/state/threadHistoryMerge.test.ts` | 10 | port | `domain.ts` / `domain.test.ts` | @effect/vitest → bun:test; Effect values → plain results; DateTime → ISO text |
| `packages/client-runtime/src/state/threadInbox.test.ts` | 5 | later-ticket: 20261005-upstream-ui-sync | — | threadInbox working sort belongs to that ticket |
| `packages/client-runtime/src/state/threadRelationships.test.ts` | 15 | port | `shell-lineage.ts` / `r3-shell-details.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/threadRequests.test.ts` | 5 | port | `requests.ts` / `requests.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/state/threadSearch.test.ts` | 6 | port | `palette.ts` / `palette.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/state/threadShell.test.ts` | 5 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/threadSnoozed.test.ts` | 36 | port | `sidebar-model.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/state/threadSort.test.ts` | 34 | port | `sidebar-model.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test; branded ids → strings; Intl/locale (X36) |
| `packages/client-runtime/src/state/threadSubagents.test.ts` | 10 | n/a-excluded | — | used only by apps/mobile (ThreadDetailScreen) |
| `packages/client-runtime/src/state/threadWorkflows.test.ts` | 13 | port | `composer-controls-queue.ts` / `composer-controls.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/state/threads-atoms.test.ts` | 10 | n/a-ui | — | Effect atoms; no pure part |
| `packages/client-runtime/src/state/threads-sync.test.ts` | 40 | n/a-ui | — | Effect sync; no pure part |
| `packages/client-runtime/src/state/turnItemPresentation.test.ts` | 5 | port | `r11-upstream-retry.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text |
| `packages/client-runtime/src/state/usage.test.ts` | 12 | later-ticket: 20261005-usage-pooled-view | — | manual refresh across environments (ported by that ticket); cases named by `20261005-composer-fidelity` |
| `packages/client-runtime/src/state/vcs.test.ts` | 11 | n/a-ui | — | Effect ref streams; no pure part |
| `packages/client-runtime/src/state/vcsAction.test.ts` | 16 | port | `r4-git-actions.ts` / `r4-git.test.ts` | @effect/vitest → bun:test; Effect values → plain results; branded ids → strings |
| `packages/client-runtime/src/t3ToolSummary.test.ts` | 14 | port | `timeline-t3tools.ts` / `timeline-r3.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/textPaste.test.ts` | 15 | port | `composer-editor-files.ts` / `composer-editor-files.test.ts`, `composer-editor-attach.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/threadPullRequestCompatibility.test.ts` | 7 | later-ticket: 20261005-pr-links-previews-and-routing | — | multi-link capability |
| `packages/client-runtime/src/userMessage.test.ts` | 6 | port | `timeline-rows.ts` / `r11-upstream.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/client-runtime/src/voice-input/controller.test.ts` | 25 | n/a-excluded | — | used only by apps/mobile |
| `packages/client-runtime/src/work-log/commandLabel.test.ts` | 36 | port | `timeline-worklog.ts` / `timeline-item-detail.test.ts`, `timeline.test.ts` | vite-plus/test → bun:test |
| `packages/client-runtime/src/work-log/presentation.test.ts` | 60 | port | `timeline-worklog.ts` / `timeline-item-detail.test.ts`, `timeline.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings; DateTime → ISO text; render cases n/a-ui |
| `packages/client-runtime/src/work-log/scrollAnchor.test.ts` | 6 | swift | `macos/tests/r9-input` | tool-group scroll anchors (R9Input.swift) |
| `packages/client-runtime/src/work-log/toolPresentation.test.ts` | 5 | port | `timeline-tool-icons.ts` / `timeline-tool-icons.test.ts` | @effect/vitest → bun:test; Effect values → plain results |
| `packages/client-runtime/src/work-log/userInput.test.ts` | 4 | port | `timeline-inspect.ts` / new test file | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/Array.test.ts` | 2 | n/a-server | — | not imported by web or desktop |
| `packages/shared/src/DrainableWorker.test.ts` | 2 | n/a-server | — | server worker |
| `packages/shared/src/KeyedCoalescingWorker.test.ts` | 3 | n/a-server | — | server worker |
| `packages/shared/src/Net.test.ts` | 6 | n/a-server | — | server loopback ports |
| `packages/shared/src/agentAwareness.test.ts` | 7 | n/a-server | — | server projection |
| `packages/shared/src/assistantCitations.test.ts` | 21 | port | `r5-composer-citation.ts` / `r5-composer-citation.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/changeRequestUrl.test.ts` | 17 | port | `palette-linkpr.ts` / `palette-linkpr.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/chatList.test.ts` | 6 | swift | `macos/tests/r9-input` | anchored end space of the timeline |
| `packages/shared/src/claudeCompaction.test.ts` | 4 | port | `r3-composer-controls-resume.ts` / `r3-composer-controls-resume.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/cliArgs.test.ts` | 24 | n/a-server | — | server CLI |
| `packages/shared/src/cliRelease.test.ts` | 9 | n/a-server | — | server CLI release archives |
| `packages/shared/src/codexAuthHandoff.test.ts` | 4 | later-ticket: 20261005-managed-codex-chatgpt | — | hosted Codex handoff |
| `packages/shared/src/composerContextClipboard.test.ts` | 7 | swift | `macos/tests/composer` | context metadata on the pasteboard (T3ComposerEditor.swift) |
| `packages/shared/src/composerContextLegacy.test.ts` | 33 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/composerContextLegacySend.test.ts` | 8 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/composerContextReferences.test.ts` | 21 | port | `r4-composer-attachments.ts` / `r4-composer-attachments.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/composerInlineTokens.test.ts` | 19 | swift | `macos/tests/composer` | inline tokens (T3ComposerText.swift) |
| `packages/shared/src/composerPullRequestMatches.test.ts` | 4 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/composerTrigger.test.ts` | 7 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test; trigger detection also in T3ComposerText.swift |
| `packages/shared/src/connectAuth.test.ts` | 5 | n/a-excluded | — | T3 Connect sign-in |
| `packages/shared/src/dateTime.test.ts` | 8 | n/a-server | — | not imported by web or desktop |
| `packages/shared/src/delimitedPreview.test.ts` | 8 | port | `r5-panels-attach.ts` / `r5-panels.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/desktopAppControl.test.ts` | 3 | later-ticket: 20261005-app-activation | — | control socket address |
| `packages/shared/src/devHome.test.ts` | 10 | n/a-server | — | server dev home |
| `packages/shared/src/dpop.test.ts` | 8 | n/a-excluded | — | T3 Connect DPoP |
| `packages/shared/src/favicon.test.ts` | 9 | port | `timeline-tool-icons.ts` / `timeline-tool-icons.test.ts` | @effect/vitest → bun:test; Effect values → plain results; 4 of 9 titles already in timeline-tool-icons.test.ts; cases named by `20261005-upstream-timeline-and-markdown` |
| `packages/shared/src/filePreview.test.ts` | 12 | port | `r5-panels-attach.ts` / `r5-panels.test.ts` | vite-plus/test → bun:test; render cases n/a-ui |
| `packages/shared/src/git.test.ts` | 32 | port | `r4-git-env.ts` / `r4-git.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/gitPatchPath.test.ts` | 13 | later-ticket: 20261005-diff-review-engine | — | patch path quoting |
| `packages/shared/src/hostClassification.test.ts` | 11 | port | `timeline-tool-icons.ts` / `timeline-tool-icons.test.ts` | vite-plus/test → bun:test; cases named by `20261005-environment-routes` |
| `packages/shared/src/image.test.ts` | 10 | port | `composer-editor-title.ts` / `composer-editor-title.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/imageDimensions.test.ts` | 9 | n/a-server | — | server image reads |
| `packages/shared/src/legacyCliLauncher.test.ts` | 1 | n/a-server | — | CLI launcher |
| `packages/shared/src/logging.test.ts` | 9 | n/a-server | — | server log sink |
| `packages/shared/src/model.test.ts` | 21 | port | `r3-composer-controls-model.ts` / `r3-composer-controls-model.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/nodeRuntime.test.ts` | 13 | n/a-server | — | server runtime selection |
| `packages/shared/src/nodeSqliteClient.test.ts` | 8 | n/a-server | — | database |
| `packages/shared/src/oauthScope.test.ts` | 5 | n/a-server | — | server OAuth scopes |
| `packages/shared/src/observability.memory.test.ts` | 1 | n/a-server | — | server tracing |
| `packages/shared/src/observability.test.ts` | 26 | n/a-excluded | — | telemetry |
| `packages/shared/src/orchestrationTiming.test.ts` | 14 | port | `composer-controls-subagent.ts` / `composer-controls-subagent.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/orchestrationV2PendingBackgroundWork.test.ts` | 23 | port | `sidebar-model.ts` / `sidebar-extra.test.ts`, `sidebar.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/orchestrationV2Timeline.test.ts` | 9 | n/a-server | — | imported only by the server |
| `packages/shared/src/otelEnvironment.test.ts` | 10 | n/a-excluded | — | telemetry |
| `packages/shared/src/path.test.ts` | 7 | port | `timeline-files.ts` / `timeline-highlight.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/preview.test.ts` | 12 | n/a-excluded | — | Browser surface |
| `packages/shared/src/previewViewport.test.ts` | 4 | n/a-excluded | — | Browser device toolbar |
| `packages/shared/src/projectFavicon.test.ts` | 4 | port | `r3-sidebar-glyph.ts` / `r3-sidebar.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/projectSettings.test.ts` | 22 | port | `settings-core.ts` / `settings-core.test.ts`, `settings-a-about.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/providerAuthReturnUrl.test.ts` | 3 | later-ticket: 20261005-managed-codex-chatgpt | — | return links on a clone-specific scheme |
| `packages/shared/src/relayAuth.test.ts` | 5 | n/a-excluded | — | Clerk relay auth |
| `packages/shared/src/relayClient.test.ts` | 6 | n/a-excluded | — | T3 Connect relay |
| `packages/shared/src/relayJwt.test.ts` | 4 | n/a-excluded | — | T3 Connect relay |
| `packages/shared/src/relaySigning.test.ts` | 2 | n/a-excluded | — | T3 Connect relay |
| `packages/shared/src/relayTracing.test.ts` | 4 | n/a-excluded | — | T3 Connect relay |
| `packages/shared/src/relayUrl.test.ts` | 3 | n/a-excluded | — | T3 Connect relay |
| `packages/shared/src/remote.test.ts` | 16 | port | `r10-connect-pairing.ts` / `r10-connect.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/schemaJson.test.ts` | 13 | n/a-server | — | server schema helpers |
| `packages/shared/src/schemaYaml.test.ts` | 6 | n/a-server | — | not imported by web or desktop |
| `packages/shared/src/searchRanking.test.ts` | 10 | port | `composer-editor-menu.ts` / `composer-editor.test.ts`, `composer-editor-usage.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/semver.test.ts` | 12 | port | `connections.ts` / `connections.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/serverSettings.test.ts` | 34 | port | `settings-core.ts` / `settings-core.test.ts`, `settings-a-about.test.ts` | vite-plus/test → bun:test; Effect values → plain results; branded ids → strings |
| `packages/shared/src/shell.test.ts` | 41 | n/a-server | — | login-shell probes used by the server |
| `packages/shared/src/sourceControl.test.ts` | 22 | port | `r4-git-logic.ts` / `r4-git.test.ts` | vite-plus/test → bun:test; render cases n/a-ui |
| `packages/shared/src/t3McpToolPresentation.test.ts` | 10 | port | `timeline-t3tools.ts` / `timeline-r3.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/t3ProjectFile.test.ts` | 10 | port | `settings-b-actions.ts` / `settings-b.test.ts` | vite-plus/test → bun:test; Effect values → plain results; t3.json schema |
| `packages/shared/src/terminalLabels.test.ts` | 11 | later-ticket: 20261005-terminal-layout | — | terminal labels; cases named by `20261005-terminal-drawer` |
| `packages/shared/src/thirdPartyLicenses.test.ts` | 10 | n/a-ui | — | the licenses page is drawn from settings-licenses.contract; no manifest decoder |
| `packages/shared/src/threadPullRequests.test.ts` | 30 | port | `shell-pr.ts` / `r3-shell-pr.test.ts` | vite-plus/test → bun:test; branded ids → strings |
| `packages/shared/src/threadReference.test.ts` | 8 | port | `shell-commands.ts` / `shell.test.ts`, `shell-r2.test.ts` | vite-plus/test → bun:test; cases named by `20261005-thread-commands-and-keys` |
| `packages/shared/src/toolActivity.test.ts` | 10 | port | `timeline-worklog.ts` / `timeline-item-detail.test.ts`, `timeline.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/toolOutput.test.ts` | 12 | port | `timeline-worklog.ts` / `timeline-item-detail.test.ts`, `timeline.test.ts` | vite-plus/test → bun:test |
| `packages/shared/src/usageFormat.test.ts` | 11 | port | `pages-usage.ts` / `pages-usage.test.ts`, `pages-usage-r3.test.ts` | vite-plus/test → bun:test; Intl/locale (X36) |
| `packages/shared/src/usageLimits.test.ts` | 49 | port | `composer-controls-usage.ts` / new test file | vite-plus/test → bun:test; branded ids → strings; cases named by `20261005-managed-codex-chatgpt`, `20261005-usage-pooled-view`, `20261005-usage-reset-and-feedback` |
| `packages/shared/src/usageMerge.test.ts` | 29 | port | `pages-usage.ts` / `pages-usage.test.ts`, `pages-usage-r3.test.ts` | vite-plus/test → bun:test; Effect values → plain results; Intl/locale (X36) |
| `packages/shared/src/video.test.ts` | 5 | port | `r4-composer-video.ts` / new test file | vite-plus/test → bun:test |
