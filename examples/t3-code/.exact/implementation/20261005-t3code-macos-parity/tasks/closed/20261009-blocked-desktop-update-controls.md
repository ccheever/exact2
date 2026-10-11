---
name: 20261009-blocked-desktop-update-controls
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-desktop-update-controls
pr_url: https://github.com/ccheever/exact2/pull/368
verified_commit: 4524d91ed08843fb2368e685fcbfcf93449a251b
---

# The reference's no-feed update controls (sidebar "Check for updates", Settings nav button, Update track): option (a) built, pending the user's decision

## Outcome

This record holds two audit findings that touch the T3 desktop update feed's UI. The user closed that scope on
2026-10-08 (X40). The closed record said the clone's disabled updater equals the reference with no feed. The audit shows
that the reference with no feed still draws three controls that the clone does not have.

2026-10-10: option (a) below is built as a **draft** PR (the user's rule of 2026-10-09: a choice that needs the user
goes up as a draft PR). Merging it means (a); closing it means (b). Until the user decides, this record stays open with
`implementation: implemented-pending-decision`.

Found by the 2026-10-09 desktop audit ([review](../../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build (no update feed). Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| SH-6 | The sidebar footer has Settings, Pull Requests, Usage and a disabled "Check for updates". `SidebarUpdatePill` renders whenever the app is Electron (`SidebarUpdatePill.tsx:111-113`) and is disabled when it cannot check (`:153-157`). | The footer has Settings, Pull Requests and Usage only. | Launch either app; compare the sidebar's bottom row. | `target/t3-audit/evidence/shell/SH-6-ref.png`, `SH-6-clone.png`, `shell/r-initial.aria.txt` |
| S1-13 | General › About's Update track (Stable/Nightly) is an enabled select. The Settings nav footer has a "Check for updates" icon button beside Back. General › Check for Updates is disabled (no feed). | The Update track select is disabled. The nav footer has only Back. Check for Updates is disabled, as in the reference. | Settings › General › About; look at the Settings nav footer. | `target/t3-audit/evidence/settings-1/S1-13-ref.png`, `S1-13-clone.png` |

## Why it needs the user's decision

- **The user's scope decision (binding).** On 2026-10-08 the user closed X38–X41 ("close all"). X40 is "The T3 desktop
  update feed and its UI: check, download, install, channels" (`../issues/closed/20261005-x40-app-update-feed.md`,
  status `closed-by-decision`). `plan.md:106-108` records it: `app-update-feed` is "closed by that decision, not built".
  [app-update-feed](20261005-app-update-feed.md) "Next action" (lines 117-121): "The clone has no desktop update
  feed and no update UI of its own".
- **The closed record's claim does not hold.** `app-update-feed` line 19: "the disabled updater stays as the final
  state, which equals the reference in a build without a feed". Its "Disabled parity" row (line 91) expected "no pill".
  The live reference at the pin, with no feed, shows the disabled sidebar control, the Settings nav button and an enabled
  Update track select (the findings above). So these controls are part of the reference's no-feed state, but they are
  also "its UI" in X40's title.
- No DEFERRED rule or Charlie decision is involved. Only the user's scope decision applies.

## What was compared

- The menus' "Check for Updates..." (app menu and Help) and its alert ("Automatic updates are not available right now."):
  match, from source.
- General › About › Check for Updates: disabled in both.
- The three controls in the findings: present in the reference, absent or disabled in the clone.

## Clone state before this PR (feature tip `950e8e2e5`)

The sidebar footer has three buttons; the Settings nav footer has Back; the Update track select is disabled
(`settings-a-about.ts` keeps the disabled path); the menus' item and alert match the reference.

## Declared difference while the PR is a draft

Until the user decides (on the feature branch): no sidebar "Check for updates" control, no Settings nav "Check for
updates" button, and a disabled Update track select. The 2026-10-08 scope decision is the reason.

## Decision needed

Decide one of:
- (a) **Build the no-feed controls** as "match the original": a disabled sidebar "Check for updates" control (tooltip
  "Check for updates"), a disabled "Check for updates" button in the Settings nav footer, and an enabled Update track
  select. Also decide what the select stores with no feed (a saved preference with no effect, or as the reference does).
  No feed, no download, no install: X40 stays closed. This reopens this record as a planned task.
- (b) **Keep them out**: this record closes as a declared difference. Add the row to `EXACT2-GAPS.md` and correct
  `app-update-feed` lines 19 and 91 (the reference with no feed does draw these controls).

## What the decision unblocks

The user's choice (a) above. With (a), the work is clone-side: `sidebar.contract` (footer), the Settings nav footer
(`app-settings.contract`), `settings-a-about.ts`. Reference: `apps/web/src/components/sidebar/SidebarUpdatePill.tsx:111-160`,
`desktopUpdate.logic.ts:28-97`, `components/settings/SettingsPanels.tsx:278-470`.

## What option (a) builds (2026-10-10)

Reference `1e2ecbd975`: `SidebarUpdatePill.tsx:111-160` (no feed: `canCheckForUpdate` false, so the control is
`aria-disabled`, opacity 60 %, `cursor-not-allowed`, tooltip "Check for updates" side top, `ml-auto` at the end of
`SidebarUtilityMenu`), `SidebarChrome.tsx:167-237` (the same menu in the thread sidebar, the legacy sidebar and, on a
utility page, Back + the control), `SettingsSidebarNav.tsx:351-366` (the Settings nav's footer is that menu),
`SettingsPanels.tsx:278-470` (`handleUpdateChannelChange` → `setUpdateChannel`), `DesktopUpdates.ts:971-1011`
(`setChannel` with no feed: persist the channel, set a base state, check nothing) and `DesktopAppSettings.ts:345-355`
(`setUpdateChannel`: a new channel also sets `updateChannelConfiguredByUser`).

- **SH-6.** `sidebar-icons.contract` `SidebarUpdatePill`: a disabled round RefreshCw button (32 pt, opacity 0.6,
  `cursor="not-allowed"`, `aria-label="Check for updates"`, no press). `sidebar.contract` and `legacy-sidebar.contract`
  end the footer row with it (`margin-left="auto"`), on every page (Settings/Pull Requests/Usage, or Back on a utility
  page). Its tooltip is drawn by `SidebarEdgeTips` (`r4-polish-tip.contract`) from the root hover id `sidebar-update`,
  centred on the control 24 pt in from the sidebar's edge, 4 pt above it, as the other footer tips.
- **S1-13, the Settings nav footer.** `settings-core.contract` `SettingsNav`: the footer is a row of Back (flex 1) and
  the same control; `app-settings.contract` draws its tooltip at the window level (it reaches past the nav), as the
  rail's "Drag to resize sidebar".
- **S1-13, the Update track.** With no update store (the `embedded` delivery stream, every clone build today) the
  select is enabled and reads desktop-settings.json's `updateChannel` (`local-backend.ts` `DesktopSettingsFacts`, its
  default `resolveDefaultDesktopUpdateChannel` of the client's version: Nightly, since the clone is
  `0.0.46-nightly.20261004.1`; the reference build is 0.0.45, so Stable). A pick goes through `desktopSettingsSet
  {updateChannel}` (`T3LocalBackend.swift`, `T3DesktopSettings.settingUpdateChannel` = `setUpdateChannel`), which
  writes the sparse document and publishes the new key; the same track sends nothing; a failed write keeps the track
  and shows the reference's toast "Could not change update track" with what the reference's renderer reads: the write
  failure wrapped as `DesktopUpdates.setChannel` wraps it (`DesktopUpdateChannelPersistenceError`,
  `DesktopUpdates.ts:92-101`, as the clone's exposure and Tailscale persistence errors in `server-exposure.ts`), behind
  Electron's invoke prefix: "Error invoking remote method 'desktop:update-set-channel':
  DesktopUpdateChannelPersistenceError: Failed to persist the latest desktop update channel.". The select is disabled while
  the command runs (`isChangingUpdateChannel`). With a linked Exact update store the track stays the stream's channel,
  shown and not switchable, as before.
- Not built (X40 stays closed): any feed, check, download, install, release notes or channel-driven check. About's
  "Check for Updates" stays disabled with no store, as in the reference with no feed.

## Acceptance results

Before = feature tip `950e8e2e5` (`t3-code-evidence-base`), after = this branch's bundle (one live agent-mode drive),
reference = T3 Code `1e2ecbd975` Electron production build with `T3CODE_DISABLE_AUTO_UPDATE=1` (no feed) over CDP. Each
image: before | after | reference.

| Id | Result | Proof |
| --- | --- | --- |
| SH-6 | pass (agent): the footer ends in the disabled control (tree: `sidebar-check-updates`, label "Check for updates", disabled, no handlers); hovering it at (231, 820) shows "Check for updates" above it (reference: `aria-disabled`, opacity 0.6, not-allowed, centre (231, 820), the same tooltip) | [sh-6-sidebar-footer.png](https://raw.githubusercontent.com/ccheever/exact2/e42830c75e92d4b96177458f2ad30345ed47b56a/desktop-update-controls/sh-6-sidebar-footer.png) |
| S1-13 nav | pass (agent): the Settings nav footer is Back + the disabled control (`settings-check-updates`); hovering it at (232, 820) shows the tooltip over the nav's edge | [s1-13-settings-nav-footer.png](https://raw.githubusercontent.com/ccheever/exact2/58164230fc82b320623181050885314bec68d5a8/desktop-update-controls/s1-13-settings-nav-footer.png) |
| S1-13 track | pass (agent): the Update track is enabled (tree `disabled: false`), opens Stable/Nightly, and a pick saves `{"updateChannel":"latest","updateChannelConfiguredByUser":true}` to the lane's `userdata/desktop-settings.json` and shows Stable (reference: a pick of Nightly saves `"updateChannel":"nightly","updateChannelConfiguredByUser":true` and shows Nightly). Check for Updates stays disabled | [s1-13-update-track-menu.png](https://raw.githubusercontent.com/ccheever/exact2/51a15fed5f333a5390cbc56eacb22a43d6b8cb8c/desktop-update-controls/s1-13-update-track-menu.png), [s1-13-update-track-saved.png](https://raw.githubusercontent.com/ccheever/exact2/5e3ddc82b9e600fc8fa0d9a67c9dca59d3c75e9d/desktop-update-controls/s1-13-update-track-saved.png) (retaken in the review round with the pointer moved to the select first) |
| Tooltip leaves | pass (agent, review fix 2026-10-10): with the tooltip showing, moving the agent's pointer to the Update track hides it; the first round's menu and saved shots kept it because a plain `tap` presses without moving the resting pointer (LLP 1012), so the pointer stayed parked on the control | [s1-13-tooltip-leaves.png](https://raw.githubusercontent.com/ccheever/exact2/b9e99c4a9dd65add166291978a36285274464e31/desktop-update-controls/s1-13-tooltip-leaves.png) |
| S1-13 track, failed save | pass (test, review fix 2026-10-10): the toast's description is the reference's persistence error behind Electron's invoke prefix. Before: "Desktop settings write failed during replace-settings-file at <path>."; after: "Error invoking remote method 'desktop:update-set-channel': DesktopUpdateChannelPersistenceError: Failed to persist the latest desktop update channel." (an error path: no screenshot) | `settings-a-about.test.ts` "a failed save keeps the track …" |
| Real pointer | open: the not-allowed cursor, the tooltip under a real pointer, and the tooltip hiding when the pointer leaves the control (real-input step 1) | — |

Drive record (steps, tree excerpts, both apps' desktop-settings.json):
[drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fd28253e2e5e148c212c0d07d8564da4f0d21e2c/desktop-update-controls/drive-record.txt)
(both rounds; the review round's drive ran on the bundle at the merge with feature tip `284254a72`).

Tests: `settings-a-about.test.ts` (the no-feed select reads the saved track; a pick sends `desktopSettingsSet
{updateChannel}` and the row follows; the same track sends nothing; a failed write keeps the track and shows "Could not
change update track"; a linked stream fixes the track; the toast reads the reference's `DesktopUpdateChannelPersistenceError` behind Electron's invoke prefix; the three
footers draw the disabled control and its tooltips),
`local-backend.test.ts` and `server-exposure.test.ts` (the fifth desktop key and its version default),
`macos/tests/local-backend/settings.swift` (`setUpdateChannel` and its no-op, ported from the reference's
`DesktopAppSettings.test.ts`; `desktopSettingsSet {updateChannel}` persists and publishes, an unknown channel is
refused). Checks: see the PR.

## Declared difference (option a)

- **No Tab stop on the disabled control.** The reference's control is `aria-disabled`, so Tab still reaches it; Contract
  has no `aria-disabled` (`contract vocab`), and a `disabled` button takes no focus, as a disabled `<button>` on the web.
  New framework gap, `EXACT2-GAPS.md` "Desktop update controls". **Issue: number pending.** The 2026-10-07 rule accepts
  a difference only with an issue, and builders file nothing: the coordinator files it from the draft in the PR
  ("Coordinator action": title `[Feature] Contract: aria-disabled, a control that reads disabled and keeps its Tab
  stop`) and puts the number here and in the `EXACT2-GAPS.md` row. No existing issue matches (`aria-disabled`: none;
  #280 is select/date inputs as Tab stops, closed).

## Real-input batch steps

Build this branch's bundle and launch it on the audit lane, not agent mode: `T3_LOCAL_HOME=<A>/lanes/desktop-update-controls/clone-t3-home
T3_LOCAL_PORT=16302 EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle --run`
(`<A>` = `target/t3-audit` of the base checkout; never port 3773 or `~/.t3`).
1. **SH-6, S1-13 under a real pointer.** Rest the pointer on the round arrows at the right end of the sidebar's bottom
   row. Expect: the not-allowed cursor, and "Check for updates" above the control after about half a second. Click it:
   nothing happens. Move the pointer up off the control: the tooltip hides. Open Settings: the same control sits right
   of Back, with the same cursor, tooltip and no effect. Rest on it until the tooltip shows, then move to General ›
   About › Update track and click it: the tooltip is gone before the menu opens (agent mode's `tap` presses without
   moving the pointer, so the agent drive parks it on the control; see the PR's captions).

## Next action

The user decides: merge the draft PR (option a: this record closes as built), or close it (option b: this record closes
as a declared difference; add the row to `EXACT2-GAPS.md` and correct `app-update-feed` lines 19 and 91). Real-input
step 1 goes into the next real-input batch either way it lands.

Coordinator, before a merge (review of 2026-10-10): file the `aria-disabled` issue from the PR's draft and put its number
in "Declared difference (option a)" above and in the `EXACT2-GAPS.md` row (both say "number pending").

## Delivery

Merged on 2026-10-10 as `4524d91ed` (#368, squash). The user decided on 2026-10-10 to build option (a): the no-feed controls are built; the X40 feed stays closed. The aria-disabled gap (the disabled control takes no Tab stop) is the local draft X70, not filed (the user's choice for local drafts). Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
