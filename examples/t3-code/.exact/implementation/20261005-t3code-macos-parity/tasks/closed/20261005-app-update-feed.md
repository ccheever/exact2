---
name: 20261005-app-update-feed
plan: 20261005-t3code-macos-parity
implementation: dropped
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# App update feed: check, download, install, channels and server-triggered updates

## Outcome

The clone checks a feed that the clone owns, offers the update, downloads it on request, installs it with a restart, lets the user switch between a Stable and a Nightly track, and lets a connected remote client trigger the same update, with the reference app's states, texts and timing.
This ticket starts only after the user decides, in issue X40, to build the update flow on a clone-owned feed. The clone never reads T3's feed. If the decision is to close, this ticket is closed with it and the disabled updater stays as the final state, which equals the reference in a build without a feed.

## Scope and exclusions

Included, if the decision is to build (reference at `1e2ecbd975`; evidence and strings are in [X40](../../issues/closed/20261005-x40-app-update-feed.md)):

1. **Gate and reasons.** Updates are on only with a configured feed, a packaged build, no `T3CODE_DISABLE_AUTO_UPDATE`. Otherwise the state is `disabled` with the reason text: "Automatic updates are not available because no update feed is configured.",
   "Automatic updates are only available in packaged production builds.", "Automatic updates are disabled by the T3CODE_DISABLE_AUTO_UPDATE setting." (`apps/desktop/src/updates/DesktopUpdates.ts:256-270`; the Linux reason does not apply). The clone's existing disabled behavior (`settings-a-about.ts`, `T3Menus.swift:94-120`) is the starting point and must stay identical when the feed is absent.
2. **State machine.** Port the pure reducers of `updateMachine.ts` (`disabled`, `idle`, `checking`, `up-to-date`, `available`, `downloading`, `downloaded`, `error` with `check`, `download` or `install`) and their rules: a failed check keeps a downloaded update installable; a failed download returns to `available` when a version is known, else `error`;
   a failed install returns to `downloaded` with context `install`; download progress is broadcast in 10 % steps and at 100 %.
3. **Schedule and mode.** First check 15 s after launch, then every 4 min; auto-download and install-on-quit off; each step starts on the user's action (`DesktopUpdates.ts:50-51,706-721,933-934`). One action at a time (`check`, `download`, `install`, `install-recovery`, `channel`).
4. **Channels.** `latest` and `nightly`. Default is `nightly` when the version looks like `x.y.z-nightly.YYYYMMDD.N`, else `latest` (`updateChannels.ts:1-18`). Nightly allows prerelease and downgrade; an update that does not match the selected channel is ignored (`DesktopUpdates.ts:391-402,744-747`).
   Changing the track persists the choice (in the clone's own settings file, U7), resets the state, and runs a check with downgrade allowed; it is refused while another action runs with the text "Cannot change the desktop update channel to <channel> while an update <action> action is in progress." (`:88,970-1011`).
   How a track maps to a clone-owned feed (delivery streams or separate feed files) is decided in X40.
5. **Release notes.** Parse the notes of an update into at most 6 groups of 8 items of 220 characters (`releaseNotes.ts`, 214 lines of tests). The "Read more" link points at the clone's own release notes, never at `pingdotgg/t3code`.
6. **Install.** Confirmation "Install update <version> and restart T3 Code?" + "Any running tasks will be interrupted. Make sure you're ready before continuing." (`apps/web/src/components/desktopUpdate.logic.ts:104`). An install request that arrives during a check waits up to 90 s, then is refused. An admitted install sets `quitting`,
   writes an update restart marker, stops every backend (5 s timeout each; issue X6 and `20261005-embedded-server-runtime`), then installs and relaunches (`DesktopUpdates.ts:52,606-650`).
7. **UI.** Settings → General → About: the Version button by state ("Update <v> ready to download", "Downloading update N%", "Update <v> downloaded. Click to restart and install.", "Download failed for <v>. Click to retry.", "Install failed for <v>. Click to retry.", "Up to date"; disabled while downloading) and the Update track select
   ("Use stable releases or nightly builds. Switch back anytime."; options Stable and Nightly) (`desktopUpdate.logic.ts:28-97`, `SettingsPanels.tsx:278-470`). Error toasts: "Could not check for updates", "Could not download update", "Could not install update", "Could not change update track" (`SettingsPanels.tsx:308,331,370,387,398`).
   Sidebar pill tooltips "Update available", "Checking for updates…", "Check for updates" (`SidebarUpdatePill.tsx:146-152`). Toast "Update downloaded" / "Restart the app from the update button to install it." with "Read more" (`desktopUpdate.toast.tsx:49-60`).
   Menu "Check for Updates..." in the app menu and the Help menu; with updates off it shows "Automatic updates are not available right now." plus the reason (`DesktopApplicationMenu.ts:85-100,164,261`). The Intel-on-Apple-Silicon warning does not apply (arm64 only).
8. **Server-triggered update.** When a remote client updates a server that this app hosts, the server asks the app over the control pipe (fd 5, issue X39 part 3): `requestDesktopUpdate` (check, download, report), then `commitDesktopUpdate` after the client confirms the token, or `cancelDesktopUpdate`.
   The app reports `desktopUpdateStatus` on attach and on every state change. Limits: 2 checks and 3 downloads per run; a prepared update expires after 5 min (`remoteUpdateFlow.ts:25-26`, `DesktopRemoteUpdates.ts:29`). Outcomes: `ready-to-install`, `up-to-date`, `failed` with a reason.
   The banner and its texts are in `20261005-server-update-banner`; this ticket supplies the app side.
9. **Test feed.** A local mock feed on a lane port for the checks, as the reference does with `T3CODE_DESKTOP_MOCK_UPDATES` and `scripts/mock-update-server.ts`. The clone reads its own variable names; the oracle uses the reference's.

Excluded: T3's feed in any form, hosting or publishing a feed (deployment is out of scope, so the user supplies the origin), Windows and Linux paths, Intel builds, Homebrew or any package-manager update, Sparkle, telemetry, the Browser surface.

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior: `apps/desktop/src/updates/*`, `apps/desktop/src/window/DesktopApplicationMenu.ts`, `apps/web/src/components/desktopUpdate.logic.ts`, `desktopUpdate.toast.tsx`, `components/sidebar/SidebarUpdatePill.tsx`, `components/settings/SettingsPanels.tsx`, `packages/contracts/src/ipc.ts:279-350,1242-1247`, `docs/user/updating.md`.
Library revision: `20261005-platforms-v3`. Selected topics: to select at `prepare`; app delivery, relaunch and Keychain are **unknown in the library**. The framework's own delivery (LLP 1030.000) is the likely source of the clone's feed; what a module may call is an open question in X40.
Consumer framework revision and toolchain: the pin from `20261005-clone-on-exact2-main`.
Oracle: the reference updater runs only in a packaged build ("Automatic updates are only available in packaged production builds."), so the update rows need a packaged reference build with the mock feed variables, on a lane home and port; whether the installed T3 Code (Nightly) may be used for that is a user decision at `prepare`.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`.
Every launch runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 (see `20261005-embedded-server-runtime`).
Port changes for headers: `Effect` services and the Electron updater become a Swift state machine with an injected clock and an injected feed client.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | [X40](../../issues/closed/20261005-x40-app-update-feed.md) | pending | The user decides to build on a clone-owned feed (with the feed source and the framework support), or closes it | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-embedded-server-runtime`, `20261005-desktop-shell-details` and `20261005-server-update-banner` | none | Server stop, menu seams, banner texts and the control pipe exist | pending |
| recorded decision (conditional) | Issue X39 part 3 (the control pipe on fd 5) | none | Item 8 only: built if the user keeps part 3; if part 3 is closed, item 8 is dropped with the reason recorded | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X40](../../issues/closed/20261005-x40-app-update-feed.md) | Decision, and a module API to the delivery system | scope decision; not in the library | blocking | `issue-open`, then the user decides |
| [X39](../../issues/closed/20261005-x39-telemetry.md) | Control pipe (part 3) for server-triggered updates | scope decision | blocking for item 8 only | decide part 3 |
| [X6](../../issues/20261005-x06-module-quit-shutdown.md) | Bounded delay at quit | main #200 (#105) runs `destroy()` at quit; no bounded hold on `463acda68` ([adopt-main-fixes-r4](20261007-adopt-main-fixes-r4.md)) | nonblocking (install stops the server itself) | none |
| [X26](../../issues/20261005-x26-app-menu-control.md) | App menu control | `EXACT2-GAPS.md` X26 | nonblocking (the menu item exists) | none |

## Implementation notes

- New files: `app-update.ts` (reducers, channel rules, release notes, tests), `app-update-view.ts` (About rows, pill, toast), `modules/apple/T3AppUpdate.swift` (feed client, download, install, relaunch, restart marker). Keep `settings-a-about.ts` as the one place for About rows and keep its disabled path.
- The feed client verifies the signature and digest of the download before it offers install; the check has a timeout and never blocks the UI.
- The install path calls the same server stop as quit. The restart marker lets the next launch know it came from an update.
- Dialog and pill: focus enters the confirmation, Escape cancels and returns focus to the button, the pill has an accessible name equal to its tooltip, and the download bar changes in steps under reduced motion.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test` and `cargo test -p t3-code-macos --lib` plus the new AppKit binary: cases of `updateMachine.test.ts`, `DesktopUpdates.test.ts`, `updateChannels.test.ts`, `releaseNotes.test.ts`, `remoteUpdateFlow.test.ts`, `DesktopRemoteUpdates.test.ts`, `desktopUpdate.logic.test.ts`, `desktopUpdate.toast.test.tsx` | Original test names pass | macOS | logs |
| Disabled parity | Clone with no feed; oracle with `T3CODE_DISABLE_AUTO_UPDATE=1` | Open About; use the menu item | Same button state and note, same alert and reason text, no pill | macOS 1280×840 and 840×620, light and dark | png pairs, `tree` |
| State timeline | Mock feed with a newer version; oracle with the reference's mock feed | Launch; wait for the first check; Download; Install (stub installer) | States in order `idle`, `checking`, `available`, `downloading` (progress in 10 % steps), `downloaded`; texts of X40 at each step; first check 15 s after launch with the test clock, then every 4 min | macOS | state transcript, `trace-diff.mjs` |
| Failures | Mock feed that errors on check, on download, on install | Run each | Error toasts of X40; a failed download returns to `available`; a failed install returns to `downloaded` with "Install failed for <v>. Click to retry."; a failed check keeps a downloaded update installable | macOS | transcript, png |
| Channels | Mock feed with Stable and Nightly entries | Switch the track; switch during a download | Persisted choice, state reset, check with downgrade allowed; refusal text while an action runs; an update for the other channel is ignored | macOS | transcript |
| Install flow | Fake server child; stub installer | Install with the server running | Confirmation text; server stopped within 5 s before install; restart marker written; relaunch hook called once | macOS | process list, log |
| Release notes | Notes with HTML entities and over-limit sizes | Open the update details | At most 6 groups, 8 items, 220 characters; entities decoded; "Read more" opens the clone's notes URL | macOS | png |
| Pill, toast and menu | Mock feed | Use the pill, the toast link and both menu items (app, Help) | Tooltips and texts of X40; the menu alert only when disabled | macOS | `tree --ax`, png |
| Dialog and pill accessibility | Confirmation dialog, pill | Keyboard path, Escape, reduced motion | Focus lands in the dialog and returns; the pill has an accessible name; steps instead of animation | macOS | `tree --ax` |
| Server-triggered run | Remote client stub; fake server sending control messages | Send `requestDesktopUpdate`, then `commitDesktopUpdate`; repeat with `cancelDesktopUpdate` and with expiry | Status reports on attach and on change; at most 2 checks and 3 downloads; a prepared update expires after 5 min; outcomes `ready-to-install`, `up-to-date`, `failed` with reason | macOS | fake server log |
| Real update (attended session) | A published test feed that the user owns | Update a real build | The app downloads, installs and relaunches on the new version | macOS | recording |

Task-owned source paths: `app-update.ts`, `app-update-view.ts`, `settings-a-about.ts`, `modules/apple/{T3AppUpdate,T3Menus}.swift`, `app.json` (feed fields as the framework allows), tests under `bun test` and `macos/tests/update`, the mock feed (apparatus, approval needed).
Required environment: Xcode 27.0, pinned Bun, a lane port range 16000-16999, a packaged reference build for the oracle (user's choice), a user-owned test feed origin for the attended row (names only, no secrets).

## Progress

Blocked. Not started.

2026-10-08: closed by the user's decision on X40 (X38–X41 "close all"); not built.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | X40 decision |

## Next action

**Closed by decision (2026-10-08).** The user closed the scope issues X38–X41 ("close all"), so
[X40](../../issues/closed/20261005-x40-app-update-feed.md) is closed as out of scope and this ticket is not built, as its own rule said
("close this ticket if the decision is to close"). The clone has no desktop update feed and no update UI of its own; the
server update banner is unaffected.
