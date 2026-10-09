---
name: 20261009-blocked-desktop-update-controls
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Blocked: the reference's no-feed update controls (sidebar "Check for updates", Settings nav button, Update track)

## Outcome

This record holds two audit findings that touch the T3 desktop update feed's UI. The user closed that scope on
2026-10-08 (X40). The closed record said the clone's disabled updater equals the reference with no feed. The audit shows
that the reference with no feed still draws three controls that the clone does not have. This record does not build
them. It says why, and what decision would unblock them.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build (no update feed). Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| SH-6 | The sidebar footer has Settings, Pull Requests, Usage and a disabled "Check for updates". `SidebarUpdatePill` renders whenever the app is Electron (`SidebarUpdatePill.tsx:111-113`) and is disabled when it cannot check (`:153-157`). | The footer has Settings, Pull Requests and Usage only. | Launch either app; compare the sidebar's bottom row. | `target/t3-audit/evidence/shell/SH-6-ref.png`, `SH-6-clone.png`, `shell/r-initial.aria.txt` |
| S1-13 | General › About's Update track (Stable/Nightly) is an enabled select. The Settings nav footer has a "Check for updates" icon button beside Back. General › Check for Updates is disabled (no feed). | The Update track select is disabled. The nav footer has only Back. Check for Updates is disabled, as in the reference. | Settings › General › About; look at the Settings nav footer. | `target/t3-audit/evidence/settings-1/S1-13-ref.png`, `S1-13-clone.png` |

## Why it cannot be built

- **The user's scope decision (binding).** On 2026-10-08 the user closed X38–X41 ("close all"). X40 is "The T3 desktop
  update feed and its UI: check, download, install, channels" (`../issues/closed/20261005-x40-app-update-feed.md`,
  status `closed-by-decision`). `plan.md:106-108` records it: `app-update-feed` is "closed by that decision, not built".
  [app-update-feed](closed/20261005-app-update-feed.md) "Next action" (lines 117-121): "The clone has no desktop update
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

## Current clone state

The sidebar footer has three buttons; the Settings nav footer has Back; the Update track select is disabled
(`settings-a-about.ts` keeps the disabled path); the menus' item and alert match the reference.

## Declared difference

Until the user decides: no sidebar "Check for updates" control, no Settings nav "Check for updates" button, and a disabled
Update track select. The 2026-10-08 scope decision is the reason.

## Decision needed

Decide one of:
- (a) **Build the no-feed controls** as "match the original": a disabled sidebar "Check for updates" control (tooltip
  "Check for updates"), a disabled "Check for updates" button in the Settings nav footer, and an enabled Update track
  select. Also decide what the select stores with no feed (a saved preference with no effect, or as the reference does).
  No feed, no download, no install: X40 stays closed. This reopens this record as a planned task.
- (b) **Keep them out**: this record closes as a declared difference. Add the row to `EXACT2-GAPS.md` and correct
  `app-update-feed` lines 19 and 91 (the reference with no feed does draw these controls).

## What would unblock it

The user's choice (a) above. With (a), the work is clone-side: `sidebar.contract` (footer), the Settings nav footer
(`app-settings.contract`), `settings-a-about.ts`. Reference: `apps/web/src/components/sidebar/SidebarUpdatePill.tsx:111-160`,
`desktopUpdate.logic.ts:28-97`, `components/settings/SettingsPanels.tsx:278-470`.

## Next action

The coordinator builds option (a) as a **draft** PR (the user's rule of 2026-10-09: a choice that needs the user goes
up as a draft PR). Merging it means (a); closing it means (b), and then this record closes as a declared difference.
