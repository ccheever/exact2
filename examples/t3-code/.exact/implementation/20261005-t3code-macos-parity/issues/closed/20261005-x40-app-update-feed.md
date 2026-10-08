---
name: 20261005-x40-app-update-feed
plan: 20261005-t3code-macos-parity
status: closed-by-decision
kind: scope-decision
blocks: [20261005-app-update-feed, 20261005-server-update-banner, 20261005-telemetry]
upstream_url: null
reproduced_on: null
---

# X40: The T3 desktop update feed and its UI: check, download, install, channels

## Summary

T3 Code updates itself from T3's release feed. It checks 15 s after launch and every 4 min, lets the user download and install an update, offers a Stable and a Nightly track, and lets a connected remote client trigger the same update.
The clone must never read T3's feed, because that feed ships Electron builds of a different app. The clone shows the update rows in their disabled state and a "Check for Updates..." menu item that explains why nothing happens.
This issue records the pending decision: keep that disabled state as final (close), or build the update flow on a feed the clone owns. Ticket `20261005-app-update-feed` is blocked until then.

## Why this issue arose

### The T3 Code behavior
- Feed and gate. The packaged app reads `Resources/app-update.yml` (the installed Nightly bundle has this file, seen by `ls` on 2026-10-05) and parses its `provider` entry (`apps/desktop/src/updates/DesktopUpdates.ts:198-210`).
  Release builds attach `latest*.yml`, `nightly*.yml` and `*.blockmap` files to the GitHub release (`docs/operations/release.md:39`); the maintainers' preview builds carry no feed (`release.md:17`).
  Updates are off, with a reason text, when: no feed is configured ("Automatic updates are not available because no update feed is configured."), the app is not packaged ("Automatic updates are only available in packaged production builds."),
  the environment variable `T3CODE_DISABLE_AUTO_UPDATE` is set ("Automatic updates are disabled by the T3CODE_DISABLE_AUTO_UPDATE setting."), or on Linux without an AppImage or `.deb` (`DesktopUpdates.ts:256-270`; `apps/desktop/src/app/DesktopConfig.ts:59`).
- Schedule and mode. First check after 15 s, then every 4 min (`DesktopUpdates.ts:50-51,706-721`). Auto-download and install-on-quit are both off (`:933-934`): the user starts each step.
- State machine (`apps/desktop/src/updates/updateMachine.ts:18-210`): `disabled`, `idle`, `checking`, `up-to-date`, `available`, `downloading` (progress broadcast in 10 % steps, `DesktopUpdates.ts` `shouldBroadcastDownloadProgress`), `downloaded`, `error` with a context of `check`, `download` or `install`.
  A failed check keeps a downloaded update installable; a failed download returns to `available` when a version is known, else to `error`; a failed install returns to `downloaded` with context `install`. Release notes are parsed into at most 6 groups of 8 items of 220 characters (`releaseNotes.ts:20-22`).
- Channels. `latest` and `nightly` (`packages/contracts/src/ipc.ts:92,107`). The default is `nightly` when the version looks like `x.y.z-nightly.YYYYMMDD.N`, else `latest` (`updateChannels.ts:1-18`).
  Nightly allows prerelease and downgrade (`DesktopUpdates.ts:391-402`); an update that does not match the selected channel is ignored (`:744-747`).
  Changing the track persists the choice, resets the state, and runs a check with downgrade allowed; it is refused while another update action runs: "Cannot change the desktop update channel to <channel> while an update <action> action is in progress." (`:88,970-1011`).
- Install. Confirmation text "Install update <version> and restart T3 Code?" followed by "Any running tasks will be interrupted. Make sure you're ready before continuing." (`apps/web/src/components/desktopUpdate.logic.ts:104`).
  An install request that arrives while a check runs waits up to 90 s for it, then is refused. An admitted install sets `quitting`, writes an update restart marker, stops every backend (5 s timeout each, concurrently) and calls `quitAndInstall` (silent, run after install)
  (`DesktopUpdates.ts:52,606-650`).
- UI. Settings → General → About: the Version row's button reads by state "Update <v> ready to download", "Downloading update N%", "Update <v> downloaded. Click to restart and install.", "Download failed for <v>. Click to retry.", "Install failed for <v>. Click to retry.", "Up to date"
  (`desktopUpdate.logic.ts:28-47,73-97`); the row "Update track" says "Use stable releases or nightly builds. Switch back anytime." (`apps/web/src/components/settings/SettingsPanels.tsx:278-470`).
  Error toasts: "Could not check for updates", "Could not download update", "Could not install update", "Could not change update track" (`SettingsPanels.tsx:308,331,370,387,398`).
  The sidebar pill has tooltips "Update available", "Checking for updates…", "Check for updates" (`apps/web/src/components/sidebar/SidebarUpdatePill.tsx:146-152`). After a download the toast "Update downloaded" says "Restart the app from the update button to install it." and
  links "Read more" to `https://github.com/pingdotgg/t3code/releases/tag/v<version>` (`desktopUpdate.toast.tsx:49-60`; `desktopUpdate.logic.ts:5-25`). The app menu and the Help menu hold "Check for Updates..."; with updates off it shows an alert "Automatic updates are not available right now."
  with the reason text (`apps/desktop/src/window/DesktopApplicationMenu.ts:85-100,164,261`). The bridge methods are `getUpdateState`, `setUpdateChannel`, `checkForUpdate`, `downloadUpdate`, `installUpdate`, `onUpdateState` (`ipc.ts:1242-1247`).
- Server-triggered update. When a remote client updates a server that a desktop app hosts, the server asks the desktop app over the control pipe (fd 5, issue X39 part 3) to prepare a download, then to commit the install after the client confirms it got the token.
  Limits: 2 checks and 3 downloads per run, prepared update expires after 5 min (`apps/desktop/src/updates/remoteUpdateFlow.ts:25-26`, `DesktopRemoteUpdates.ts:29`). `docs/user/updating.md` says that "Update server" for a desktop-hosted server "closes and relaunches the desktop app on the host".
  The notice side of this is in `20261005-server-update-banner`.
- The reference has a mock feed for tests: `T3CODE_DESKTOP_MOCK_UPDATES` and `T3CODE_DESKTOP_MOCK_UPDATE_SERVER_PORT` (`DesktopConfig.ts:60-61`; `DesktopUpdates.ts:915-921`; `scripts/mock-update-server.ts`).
- Reference tests: `updateMachine.test.ts`, `DesktopUpdates.test.ts`, `updateChannels.test.ts`, `releaseNotes.test.ts`, `DesktopRemoteUpdates.test.ts`, `remoteUpdateFlow.test.ts`; web `desktopUpdate.logic.test.ts`, `desktopUpdate.toast.test.tsx`, `SidebarUpdatePill` tests.

### What exact2 does today
- Not in `EXACT2-GAPS.md`. Bundled library (`20261005-platforms-v3`): app updates and delivery are **not covered: unknown**.
- The repository's `CLAUDE.md` describes exact2's own delivery (LLP 1030.000): `bun scripts/deploy.mjs <app>` publishes signed bundles per stream, and "a native host opens its update store at launch and checks after first pixel" (`state.delivery` shows what it did). This is the framework's feed for apps built on it. It is not T3's feed.
- Observed in the clone on 2026-10-05: `settings-a-about.ts:1-61` maps the About rows onto exact2 delivery facts (`stream`, `staged`). The stream `embedded` means no update store, which the file calls "the reference's disabled updater": the Version button is disabled with the note "Up to date",
  and the Update track select is shown and disabled; when an update is staged the button reads "Install" and asks for confirmation first. `modules/apple/T3Menus.swift:94-120` adds "Check for Updates..." and answers it with the alert "Automatic updates are not available right now." plus the reason, unless the bundle's receipt says the composition is `updating`.
  The same file notes that the module has no hook into the host's check. I found no sidebar update pill or download toast in the clone: a search for "Update available", "Checking for updates" and "Check for updates" matched only provider-update and connection-status text, besides `settings-a-about.ts`.

### Where the clone hits it
The clone never contacts T3's feed. For the downloadable app the plan has no update path at all (`20261005-portable-app-download` excludes "Sparkle or any update channel"); a recipient downloads a new zip.
The visible differences from a shipped Nightly: the Version button never checks, the Update track cannot change, the menu item only explains, and there is no pill, toast or server-triggered update.
It matches the reference's own state in a build without a feed. If a clone-owned feed exists later, the module cannot start a check or a download, and a staged update can only be shown.

## Why it must be resolved
The goal is a full clone, but the user excluded the T3 update feed, and the spec excludes any update channel for the downloadable app (deployment is out of scope). A disabled updater is a declared difference, not an end state, so the plan needs a decision:
(a) Close. The disabled updater is final. The user accepts that recipients update by downloading a new build. The existing rows and menu item stay as they are.
(b) Build the update flow on a feed the clone owns. This reopens the deployment exclusion, because a feed needs a published origin and signed bundles (exact2 delivery, or another service), and it needs framework support (below).
(c) Read T3's feed. Not proposed: it would offer T3's Electron builds to the clone.
Waiting on this: `20261005-app-update-feed` (blocked) and the server-triggered part of `20261005-server-update-banner`.

## Requested support
Product decision only, unless the user chooses (b). If built, exact2 would need (each to confirm at `issue-open`):
- A module API to the delivery system: check now, read state (available version, download progress, staged version, error), start the download, install and relaunch. The module has no such hook today (`T3Menus.swift`, comment in `checkForUpdates`).
- A way to switch stream from the app (Stable and Nightly map to delivery streams), with the downgrade rule.
- A release-notes link that points at the clone's own notes, not at `pingdotgg/t3code` (the reference link is built from a constant, `desktopUpdate.logic.ts:5-6`).
- A bounded quit before install, so the embedded server stops first (issue X6), and a decision on the server-triggered path (needs a control channel; see X39 part 3).
- A clone-owned origin with signed bundles. This is delivery and is outside this plan.

## How to reproduce
To record at `issue-open`; nothing here is a defect.
1. Run T3 Code (Nightly) with its mock feed (`T3CODE_DESKTOP_MOCK_UPDATES=1` and a lane port) and the repository's `scripts/mock-update-server.ts`. Record each state, text and timing: idle, checking, available, downloading, downloaded, error, channel change.
2. Run the clone's lane build and record the same screens: About rows, menu item, alert.
3. Compare; the differences listed above are the expected result for decision (a).

## Acceptance for the fix
- If closed: the decision is recorded here and in the ticket; the disabled rows and the alert text equal the reference's no-feed state (pixel and text pairs from the oracle with `T3CODE_DISABLE_AUTO_UPDATE=1`).
- If built: `20261005-app-update-feed` passes its rows against the oracle driven by the mock feed: the states and texts above, the 15 s and 4 min cadence with the test clock, the channel-change rules and error text, the install flow with server stop, and the pill, toast and menu behavior.

## App adoption after resolution
If closed: set the ticket to closed with the decision; keep `settings-a-about.ts` and the menu as they are; remove nothing. If built: unblock the ticket, replace the delivery-facts stand-in with the new module API, point release notes at the clone, and add the update rows to the portable build's checks.
`issue-close` checks the decision text, or the passing rows on the pinned `main`.

## Status and next action
Draft; not reproduced; not searched upstream; not published. Decision pending (close, or build on a clone-owned feed).
Next: `issue-open` (record the two runs above and prepare the report for the user's approval; publication only after approval), then ask the user to decide.

## Decision (2026-10-08)

Closed by the user's decision of 2026-10-08 on the four scope issues X38–X41: "close all". The T3 desktop update feed and its UI are not built; the clone never reads T3's feed. The server update banner ([server-update-banner](../../tasks/closed/20261005-server-update-banner.md)) is unaffected.
[app-update-feed](../../tasks/closed/20261005-app-update-feed.md) is closed with it (not built). Not filed upstream: a scope decision, not a
framework gap.
