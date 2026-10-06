---
name: 20261005-desktop-shell-details
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Desktop shell details: file pickers, system locale, ⌘W and quit hold, full screen

## Outcome

The window and shell behaviors that the original desktop app has and the clone lacks or does halfway work as in the reference: "Open in Finder" in the project icon
picker, a theme-file picker with the reference's start folder and size limit, timestamps in the Mac's locale, a held ⌘W that does not close several things, the quit hold
that hides the window by opacity and leaves full screen first, and the title-row inset that disappears in full screen. (The SSH password prompt and remote Open are
`20261005-ssh-password-and-remote-open`.)

## Scope and exclusions

Included (each row is missing or partial; line numbers are from the mc-orch tree on 2026-10-05):

1. **Project icon picker, "Open in Finder".** The clone's picker has no footer action (`settings-b-icons.contract`, the `project-favicon-picker` dialog). Add the trailing footer button (`Open in <Finder>`, from
   `getLocalFileManagerName`; disabled while the panel is open) that opens an `NSOpenPanel` for one image (the extensions of `WORKSPACE_IMAGE_PREVIEW_EXTENSIONS`, `packages/shared/src/filePreview.ts:76`) starting at the
   project's workspace root; a pick closes the dialog and selects that absolute path; a failure shows the toast "Could not open image picker". Offered only for projects of the primary environment, when the local
   environment is on and the path style is not Windows-only (`ProjectSettingsPanel.tsx`, `canPickExternalProjectFavicon`); the native call returns none when the local environment is off
   (`apps/desktop/src/ipc/methods/window.ts:247-271`). Until `20261005-local-primary-environment` merges, "primary" is the clone's existing loopback stand-in (`isLoopback`, `settings-b-fleet.ts`).
2. **Theme-file picker.** The clone's `openText` (`T3ContextMenu.swift`, `openText`) caps files at 1 MiB and 40 files, silently skips big ones, and starts anywhere. Match `pickThemeFiles`
   (`apps/desktop/src/ipc/methods/window.ts:378-421`): JSON filter, multiple selection, start in `~/.vscode/extensions` when it exists, a 256 KiB cap that returns `{name, size, text: ""}` for an oversized file and
   `{name, size: 0, text: ""}` for an unreadable one, none on cancel. Port `MAX_THEME_FILE_BYTES` and `describeOversizedThemeFile` (`ThemeImportDialog.tsx`) and the batch failure text `<file>: too large` /
   `<file>: <reason>` joined with " — " into `settings-appearance-import.ts` (the `choose` branch). The single-file message is "That file is N KB. Theme files are only a few KB, so this one was not read (limit 256 KB)."
3. **System locale.** A native op returns `Locale.current.identifier` with `_` replaced by `-` (Electron macOS uses the system locale, `electron/ElectronApp.ts:132`). Port `resolveTimestampLocale` and `resolveWeekStartsOn`
   (`apps/web/src/timestampFormat.ts`) with `timestampFormat.test.ts` ("defers to the runtime default when the host reports no locale", "uses a BCP-47 tag reported by the host", "defers to the runtime default rather than
   throwing on an unusable tag", "leaves the default to the caller for a malformed locale", "follows the locale the desktop host reports", "uses the host locale for both the numeric date and wall-clock time"). Replace the clone's
   default-locale formatting (`Intl.DateTimeFormat(undefined, …)` and `toLocale*String()` without a tag: `sidebar-presentation.ts`, `timeline-presentation.ts`, `composer-controls-usage.ts`; search by symbol). Whether the data
   runtime honors an explicit locale tag is [X36](../issues/20261005-x36-data-runtime-intl-locale.md): test it first; if it ignores the tag, format in Swift with the passed locale, accepted only when the output equals
   the reference's for the table in the acceptance row.
4. **⌘W held.** The reference drops auto-repeat ⌘W (no ⌥, no ⇧) before the menu sees it (`apps/desktop/src/window/DesktopWindow.ts:634-662`). Add it to the clone's key monitor (`T3Menus.swift`, `R8KeysMenus.swift` `closeWindow`);
   match by key code (13), not characters, because a Korean 2-Set source reports Hangul ([X15](../issues/20261005-x15-non-latin-key-equivalents.md)). A single deliberate press still closes the right panel first, then the window.
5. **Quit hold.** (a) Conceal by setting the key window's `alphaValue` to 0 and leave full screen first, keeping the window key until the keys are released (`concealPendingQuitWindow`, `DesktopWindow.ts:238-252`; test "leaves
   fullscreen before concealing a pending quit"); the clone orders every window out (`T3Menus.swift`, `quit.conceal`), so held repeats reach the next app. (b) Port the rest of `QuitHold.test.ts` (28 cases, by name, e.g. "conceals a
   completed hold, then quits after release", "waits for slow repeats to stop before quitting", "honors direct mode when the key is released before its mode read settles"); the clone's `macos/tests/menus/main.swift` covers
   about five. They run against `T3QuitHold` (`T3Menus.swift`), which already takes `now` and `schedule`. (c) The reference reads the mode when the key goes down (asynchronously; a failed read quits at once; a stale read is discarded:
   "discards a stale mode resolution from a superseded press"); the clone reads a value set by the `devicePresentation` op (`T3Module.swift`). Keep the synchronous read if every ported case passes; otherwise add the read.
   The reference hint (`QuitHoldOverlay.tsx`) has no animation, so reduced motion changes nothing.
6. **Full-screen state.** A native fact from `NSWindow.didEnter/didExitFullScreenNotification` plus the initial `styleMask` (the module already observes the exit, `T3WindowChrome.swift`), published on a topic; the 90 pt
   traffic-light inset is dropped in full screen and restored on exit (`AppSidebarLayout.tsx`, `--workspace-controls-left`, `MACOS_TRAFFIC_LIGHTS_LEFT_INSET`). Consumers: `settings-core.contract` (`padding-left=90`),
   `r12-sidebar-width.ts` (`MACOS_TRAFFIC_LIGHTS_INSET`, the sidebar minimum width) and every other use of 90 in a title row (search by symbol and value).

Excluded: the Browser surface and its Full Disk Access flow (Browser only), View > Toggle Developer Tools ([X2](../issues/20261005-x02-app-developer-tools.md)), the T3 update menu items, WSL, the Reload/Force Reload and
full-screen menu title differences that come from the host's menus (declared deviations), the SSH password prompt and remote Open (`20261005-ssh-password-and-remote-open`).

## Context and guidance

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (dialog structure), accessibility (focus on open and return, icon labels, reduced motion), design (all states), state-and-data, testing-and-debugging.
Menus, window chrome, `NSOpenPanel`, `NSEvent` monitors and app-local Swift modules are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis. Consumer framework revision: the pin from `20261005-clone-on-exact2-main`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-environment-routes` | none | The plan's order: shared `T3Ssh.swift` and `environmentKey` call sites (this ticket no longer edits `T3Ssh.swift`; the remaining overlap is the connection rows' timestamp formatting) | pending |
| scheduling preference | After `20261005-main-fix-adoption` | none | Menus, popovers and cursors move there | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X26](../issues/20261005-x26-app-menu-control.md) | Menu control | `T3Menus.swift`, `R8KeysMenus.swift` | nonblocking (workaround: native menus) | none |
| [X27](../issues/20261005-x27-window-chrome.md) | Full-screen state fact; chrome | `T3WindowChrome.swift` observes the exit notification | nonblocking (workaround: native notification + topic; result equals the reference) | none |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | `repeat` and `code` for key events | the native monitor reads `isARepeat` and the key code | nonblocking | none |
| [X15](../issues/20261005-x15-non-latin-key-equivalents.md) | Chords under Korean 2-Set | `R10Connect.swift` | nonblocking | test ⌘W under 2-Set |
| [X36](../issues/20261005-x36-data-runtime-intl-locale.md) | Locale-aware `Intl` in the data runtime | not in the library | unknown (workaround: format in Swift; allowed only if equal to the reference) | check at `prepare` |
| [X2](../issues/20261005-x02-app-developer-tools.md) | View › Toggle Developer Tools in the View menu | X2 (DEFERRED "no devtools UI") | nonblocking: the menu item stays absent as a recorded difference until X2 is decided | follow the X2 decision (`20261005-app-developer-tools`) |

## Implementation notes

- New native files `T3Locale.swift`, `T3FullScreen.swift`; edit `T3Menus.swift`, `T3ContextMenu.swift`, `T3WindowChrome.swift` in place. Keep `T3Module.swift` to op prefixes.
- Read the quit mode and the full-screen fact through topics the clone's native-event pattern already uses (a `changed` topic, then a read op).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | bun: `timestampFormat` (6 locale cases), theme size cases; Swift: the 28 `QuitHold` cases in `macos/tests/menus` | Original names pass | macOS | logs |
| Icon picker | Lane project; fixture backend | Open the picker, press the footer action, pick an image; force a panel failure | Panel starts at the project folder; selection applied and dialog closed; the failure toast; action disabled while the panel is open | macOS 1280×840 and 840×620, light and dark | transcript; png pairs vs `target/t3-ui-parity/electron-oracle.mjs` |
| Icon picker keyboard and Escape | Same | Tab to the footer action; Space; with the dialog open press Escape | Focus ring on the action; Escape closes the dialog (and the panel if open) and returns focus to the control that opened the dialog; no animation to reduce | macOS | `tree --ax`, screenshots |
| Theme files | Folder with a 10 KB, a 300 KB and an unreadable file; `~/.vscode/extensions` present or absent (use a scratch `HOME`) | Choose files | Start folder; the oversized one reports "too large" with the reference text, others import; none on cancel | macOS | state, screenshot |
| Locale | A table of locale tags (en-US, ko-KR, de-DE, ja-JP, en-GB, ar-SA) × instants (today, yesterday, last year) | Format the sidebar time, timeline tooltip and week-start in the clone and in the oracle (`toLocale…` run in the oracle's page) | Every string equal; an invalid tag falls back like the reference | macOS | table diff |
| Locale at launch (attended session) | Lane build, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>`; system region set to Korea | Open a thread list and a timeline | Times follow the region as in the oracle | macOS | png pairs |
| ⌘W held (attended session) | Lane build (same variables); right panel open | Hold ⌘W under US and Korean 2-Set | One close only (the panel), none repeated | macOS | recording |
| Quit hold (attended session) | Lane build; hold mode; windowed and full screen | Hold ⌘Q | Hint shows, the window becomes transparent (full screen exits first), quit after release, nothing leaks to the next app | macOS | recording |
| Quit hold, double-click and direct modes | Same | Press ⌘Q twice; set direct mode and press once | Quits as the ported cases state | macOS | AppKit log |
| Full screen | Windowed, then full screen (menu or the agent `key`) | Enter and leave | Inset gone then back; sidebar minimum width matches the oracle in both states | macOS | `layout`, png pairs |

Task-owned source paths: `modules/apple/{T3Menus,T3ContextMenu,T3WindowChrome,T3Locale,T3FullScreen}.swift`, `settings-b-icons.contract`, `settings-appearance-import.ts`, `timestamp-format.ts` (+ tests), `r12-sidebar-width.ts`, `macos/tests/menus`.
Required environment: Xcode 27.0, pinned Bun, the oracle build; a Korean input source for the ⌘W row.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `menus` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
