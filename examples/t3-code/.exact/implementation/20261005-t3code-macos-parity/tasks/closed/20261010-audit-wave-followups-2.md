---
name: 20261010-audit-wave-followups-2
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-audit-wave-followups-2
pr_url: https://github.com/ccheever/exact2/pull/376
verified_commit: fea35ef11246fb4bc65b6765fd181d8402793948
---

# More differences the audit fix agents found outside their tasks (second set)

## Outcome

The later audit fix PRs (#367, #368, #369) reported five more differences from the reference, outside their findings,
each on the base and the branch alike. This task fixes them as the reference does. ([First set](20261010-audit-wave-followups.md).)

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FV-1 | The Scheduled tasks base-branch picker left-aligns its ref names. | They are centred (a button centres its text, the cause S2-9's row alignment fixed for the icon submenu). | settings-rows-and-labels (#367) | Settings › Scheduled tasks › New task › Base branch; compare the list. |
| FV-2 | The base-branch picker pages its refs (`usePaginatedBranches`): "Showing 100 of 122 refs", a server-side search, and a next page. | It lists the first 100 refs only: no count line, the search filters only those 100, no next page. | settings-rows-and-labels (#367) | A project with more than 100 refs (a lane fixture); open the picker; search for a ref past the first page. |
| FV-3 | With an active custom theme that has only one palette, the other appearance uses the default theme's palette; Create theme on Light seeds from `#fcfcfc` (`ThemeSettings.tsx:667-670`). | `themeRoles` falls back to the theme's own palette (`settings-appearance.ts:32`): a dark-only theme paints the window dark in mode System on a light Mac, and Create theme on Light seeds from `prefs.themeLight` (`#0a0a0a`). | app-color-scheme (#369) | Install a dark-only theme; mode System on a light Mac; compare the window; Create theme. |
| FV-4 | The theme editor's submit button reads "Save changes" for an edit, "Merge into “<name>”" when a create's name matches an installed theme, and "Add <light|dark> palette" when it adds the other palette (`ThemeEditorPanel.tsx:1252-1266`). | "Save theme" for an edit and "Create theme" otherwise. | app-color-scheme (#369) | Edit a theme; create one named like an installed theme; add the other palette. |
| FV-5 | (check) The Settings › Connections exposure and Tailscale failure toasts carry the same text as the reference's (whether the Electron IPC prefix "Error invoking remote method …" shows). | They show the persistence error's bare message (`connections-network.ts:347,366`); #368's Update track toast now carries the reference's prefix. | desktop-update-controls (#368) | Force a write failure for Network access and for Tailscale in both apps (a lane with a read-only settings file); compare the toast text. Change nothing if the reference shows the bare message there. |

## Scope and exclusions

Included: the five rows. Excluded: framework changes.

## Context and guidance

- FV-1, FV-2: reference `WorktreeBaseBranchPicker.tsx`, `usePaginatedBranches`; clone `settings-scheduled.contract`,
  `scheduled-view.ts` (the per-visit refs cache from #366 and the `selected` lookup from #367).
- FV-3, FV-4: reference `ThemeSettings.tsx`, `ThemeEditorPanel.tsx`; clone `settings-appearance.ts` (`themeRoles`),
  `settings-appearance-editor.ts`, `theme-editor-session.ts`.
- FV-5: reference `apps/desktop/src` IPC error wrapping and the web toast for these two settings; clone
  `connections-network.ts`.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FV-1 | agent drive | before / after / reference image |
| FV-2 | Bun test with a fake 122-ref list; agent drive on a lane fixture | before / after / reference image |
| FV-3 | Bun test of `themeRoles` and the create seed; agent drive with `prefer` light | before / after / reference image |
| FV-4 | Bun test of the label rule | before / after / reference image |
| FV-5 | reference comparison; a Bun test if the text changes | text |

## What the reference does (read and driven, lane `audit-wave-followups-2`, base port 16400)

- FV-1, FV-2: `BranchPickerRefItem` starts the name at the left. `WorktreeBaseBranchPicker` reads
  `usePaginatedBranches`: pages of 100 by cursor, the search sent to the server (`sanitizeNewRefName`, matched
  case-insensitively anywhere in the name, `GitVcsDriverCore.filterBranchesForListQuery`), the next page when a scroll
  lands within 96 px of the list's end, and the status line `error ?? "Loading refs..." / "Loading more refs..." /
  "Showing N of M refs"`. Driven over a 122-ref fixture (`many-refs`: trunk, main, topic-001…120): "Showing 100 of 122
  refs", the line gone after the scroll (topic-119, topic-120, main at the end), `TOPIC-12` finds topic-120.
- FV-3: the finding's premise holds for the library's Use only. A one-palette theme chosen from its card is put on its
  own half (`assignHalf`, `singleAppearanceOf`), and the other appearance keeps its theme (the default). A theme made
  the whole theme (the editor's create calls `setTheme`) resolves every mode to its own appearance
  (`resolveThemeAppearance`, `themePalette.ts:1541-1567`): driven, a dark-only "Dusk" created in mode System with the
  system emulated light keeps the window dark, and Create theme then opens on Dark seeded from Dusk; the new theme's
  Light palette is the default (`#fcfcfc`, `#1b4ed8`), never Dusk's dark one (`ThemeEditorPanel.tsx:375-390`). The clone
  now does both.
- FV-4: the label rule is `ThemeEditorPanel.tsx:1252-1266`: an edit says "Save changes", or "Merge into “<name>”" when it
  is renamed onto another installed theme (the finding placed Merge on a create; the source places it on an edit); a
  create says "Create theme" (paintbrush), or "Add <appearance> palette" (plus) when its name names an installed theme
  (by derived id or label, any case). Typing such a name flips the draft to the free appearance (`:449-462`). Driven:
  "Save changes", "Add light palette" (flipped to Light), "Merge into “Dusk”".
- FV-5: the reference shows the IPC prefix. With `desktop-settings.json` in a read-only directory, Network access says
  "Error invoking remote method 'desktop:set-server-exposure-mode': DesktopServerExposureModePersistenceError: Failed
  to persist desktop server exposure mode network-accessible." in the toast and the row; the bridge's
  setTailscaleServeEnabled rejects with "Error invoking remote method 'desktop:set-tailscale-serve-enabled':
  DesktopTailscaleServePersistenceError: Failed to persist desktop Tailscale Serve settings (enabled: true, port:
  443)." (Tailscale is not running on this Mac, so its dialog cannot be reached; the text is the bridge call the
  dialog makes.)

## Cause and fix

- FV-1: the ref row is a `button`, which centres its text; the name now has `text-align="left"`
  (`settings-scheduled.contract`).
- FV-2: `scheduled-view.ts` keeps, per editor visit, pages per project and search (`r5-composer-paging.ts`'s
  `firstPage`/`morePages`/`refsStatus`, as the composer's pickers do) and the by-name lookups; the editor sends its
  project, base and search to the root as one `taskPicker` value (`taskBase`, reset when an editor opens or closes);
  the list is the server's answer (no client filter), hooked `scroll:task-refs` for `R5ComposerScroll.swift`; the shell
  clock asks again while the page is wanted (`r6-polish-refs.ts`, the `scheduled` resource takes `shellClock`); the
  status line follows the Start from origin row; the search starts empty when the picker opens, a project is chosen
  or a ref is picked. Review round 2: the shown list (project and search, usePaginatedBranches' `targetKey`) starts
  again from its first page whenever it changes (`cursors = INITIAL_BRANCH_CURSORS`): a kept list shows its first page
  (kept as `first`) with `ends` moved to the current scroll count, so another list's scrolls never page it; a search not
  read yet answers "Loading refs..." over no refs (`branches.isPending && branches.data === null`) and wakes `t3.notify`,
  which the page watches for that answer, so it is asked again at once for the read; a base another kept list of the
  project has needs no lookup.
- FV-3: `themeRoles` and the editor's seed (`rolesOf`) and the terminal theme take the default palette for an
  appearance a custom theme lacks; previews keep the theme's own look (`paletteMode`, `previewColorsOf ??
  previews[0]`). `effectiveMode` resolves the drawn mode as `resolveThemeAppearance` does; `palette()`, `look().mode`
  (so the root's `scheme`) and the window (`devicePresentation`'s appearance, `windowAppearanceMode`) use it. The
  `theme` device setting puts a one-palette custom theme on its own half (`settings-core.ts`). Review round 2: while the
  theme editor is open the whole app wears its draft in the appearance being edited, whatever the mode
  (`applyThemeColorPreview` toggles the `dark` class to it): `paintOf` (`settings-appearance-look.ts`) gives `look()`,
  `windowAppearanceMode(client)` and the Settings palette (`settings-core-view.ts`) the same prefs, themes and mode; the
  editor opening or closing is window state with no command, so `settingsCore` wakes `t3.notify` when the worn mode
  changes and the data source's read sends the window's appearance.
- FV-4: `submitLabel` and `mergeTargetOf` (`settings-appearance-editor.ts`); the name reaches the draft per keystroke as
  a stamped `themelocal:name` op (never behind another command), and `followName` flips the appearance; `saveDraft`
  merges an edit renamed onto another theme (collision error "“X” already has a <mode> palette. Pick another name.");
  the editor view carries `saveIcon`.
- FV-5: `ipcFailure` wraps DesktopServerExposure's failures (`setMode`, `setTailscaleServeEnabled`) as Electron's IPC
  does, and the three tagged errors carry their `name`. The clone's own checks (the port field) and the restart in
  place (the reference relaunches) keep their bare messages.

No new declared difference.

## Acceptance results

Review round 2 re-drove both builds with the same steps plus two screenshots (the list wheeled to its end after the
next page; the editor cancelled): the branch bundle at the merge of `a6e31eed7` (storage `awf2-after-2`) and the
before (storage `awf2-before-4`); every image below is from that drive. Round 1: agent mode on the branch bundle (lane
`audit-wave-followups-2`, base port 16400, 1280x840, storage `awf2-after-1`,
`prefer prefers-color-scheme light`), one session; the before build is the feature tip `2dc9b0043` built in
`t3-code-awf2-before` (lane `audit-wave-followups-2-before`, storage `awf2-before-3`), because
`t3-code-evidence-base` is detached at `950e8e2e5`, 27 commits behind (#366, #367 and #369 changed this code since);
the reference over CDP (`1280x840`, mode System, the system emulated light for FV-3/FV-4). Both clone lanes had
`enableProviderUpdateChecks: false` (the update toast covered New task) and `desktop-settings.json` in a read-only
directory (FV-5). Each image is before | after | reference.

| Row | Result | Evidence |
| --- | --- | --- |
| FV-1 | pass (agent mode): the ref names start at the left edge, the tags at the right, as the reference; before: centred. | [fv1-fv2-picker-open.png](https://raw.githubusercontent.com/ccheever/exact2/a20e9cd25881b1b291490166f6f05a94b1acfd75/audit-wave-followups-2/r2-fv1-fv2-picker-open.png) |
| FV-2 | pass (agent mode + Bun test): "Showing 100 of 122 refs" under Start from origin; a wheel to the list's end loads the next page and the line goes, and a second wheel shows its end, topic-114…topic-120 and main, as the reference (before: topic-099 is the last); `topic-12` finds topic-120 past the first page (before: "No refs found."). Bun tests (fake 122-ref server): the reads (limit 100, cursor 100, `query` sanitized), "Loading more refs...", the error line, the search across pages; a search not read yet answers "Loading refs..." over no refs and wakes `t3.notify` once, then shows its answer; a search read before shows at once; with two projects of 122 refs, P1 scrolled to its end then P2 chosen shows P2's first page with "Showing 100 of 122 refs" and reads nothing (before the fix: "Loading more refs..." and a cursor-100 read), P1 again shows its first page, and a scrolled search cleared shows the first page, no read. | [fv1-fv2-picker-open.png](https://raw.githubusercontent.com/ccheever/exact2/a20e9cd25881b1b291490166f6f05a94b1acfd75/audit-wave-followups-2/r2-fv1-fv2-picker-open.png), [fv2-next-page.png](https://raw.githubusercontent.com/ccheever/exact2/c7aada1f130f8041a0289571be2f201fa897a0ab/audit-wave-followups-2/r2-fv2-next-page.png), [fv2-search.png](https://raw.githubusercontent.com/ccheever/exact2/18acef22fbc479705f14175f28097049eed859ae/audit-wave-followups-2/r2-fv2-search.png) |
| FV-3 | pass (agent mode + Bun test): Dusk (dark only) created in mode System on a light system: the whole window is dark (the toast and the chrome too), as the reference; before: Dusk's dark tokens over a light scheme (a white toast). Create theme opens on Dark from Dusk (both); its Light draft starts from `#fcfcfc`/`#1b4ed8` and the whole app wears it, light, as the reference (before: Dusk's `#0a0a0a`, all dark); Cancel puts the stored Dusk back, dark. Bun test: the draft's appearance is the mode of `look()`, the window and the Settings palette in System and in Dark, and the editor opening and closing wakes the data source. Use of a one-palette theme sets only its half (Bun test). | [fv3-window.png](https://raw.githubusercontent.com/ccheever/exact2/3ea405eb171526cb65fddd7658cd1ee72df8ec58/audit-wave-followups-2/r2-fv3-window.png), [fv3-create.png](https://raw.githubusercontent.com/ccheever/exact2/c6a779283ab1f6244a64c469b0bb9dd7661d52de/audit-wave-followups-2/r2-fv3-create.png), [fv3-create-light.png](https://raw.githubusercontent.com/ccheever/exact2/411d658fba65deb7de79936667f2f0308d76f950/audit-wave-followups-2/r2-fv3-create-light.png), [fv3-cancel.png](https://raw.githubusercontent.com/ccheever/exact2/d18cb16b6d7756b634f2003c60e211d8f8d38ada/audit-wave-followups-2/r2-fv3-cancel.png) |
| FV-4 | pass (agent mode + Bun test): Create named "dusk" flips to Light, reads "Add light palette" with the plus, and the app turns light with the draft, as the reference; Edit Dusk reads "Save changes"; Edit Noon renamed Dusk reads "Merge into “Dusk”"; before: "Create theme" (no flip) and "Save theme". The merge itself and its collision error are Bun tests. | [fv4-add-palette.png](https://raw.githubusercontent.com/ccheever/exact2/675a949bb74bf2a78136b721d6137c8f9114137a/audit-wave-followups-2/r2-fv4-add-palette.png), [fv4-edit.png](https://raw.githubusercontent.com/ccheever/exact2/eb977e14061d3f8116a539643eabf28c750dffd8/audit-wave-followups-2/r2-fv4-edit.png), [fv4-merge.png](https://raw.githubusercontent.com/ccheever/exact2/d9c311876d75b3282d95492fca8c7d72615224fd/audit-wave-followups-2/r2-fv4-merge.png) |
| FV-5 | pass (agent mode + Bun test): Network access: the toast and the row read the reference's text, prefix included; before: "Failed to persist desktop server exposure mode network-accessible." Tailscale HTTPS: the reference bridge's text, by Bun test (no Tailscale on this Mac for either app). | [fv5-network-access.png](https://raw.githubusercontent.com/ccheever/exact2/04259bf4c0e1dcd8596ec97bd637dde203194122/audit-wave-followups-2/r2-fv5-network-access.png) |

Tests: `audit-wave-followups-2.test.ts` (new, 16 tests); `connections-network.test.ts` (the no-network-address error
reads with the IPC prefix); `settings-labels.test.ts` (S2-6's source lines follow `taskPicker` and `shellClock`).

## Found during this task (not this task; for the coordinator)

- Settings › Connections: after a failed Network access change, the "Enable network access?" dialog came back over
  the Appearance page later in the same session (before build, lane `audit-wave-followups-2-before`, storage
  `awf2-before-2`: Connections › Network access › Restart and enable with a read-only `desktop-settings.json`, then
  Appearance › Create theme). The reference closes it for good.
- `t3-code-evidence-base` is detached at `950e8e2e5`, 27 commits behind the feature tip.

## Real-input batch steps

None: every row is agent mode.

## Progress

2026-10-10: built (one commit), the reference probed and driven, the branch bundle and a tip bundle (before) built, the
before driven (three runs to settle the steps: the update toast, then the dialog closing with Escape), the branch
driven once, images composed and uploaded; merged `origin/feat(example)/t3-code` at `256c189d8` (#371, clean); final
checks on the merge head; the draft PR opened.
2026-10-10, review round 2 (four should-fix findings): the picker's shown list restarts from its first page and
"Loading refs..." for a search not read yet (FV-2); the whole app, the window too, wears the theme editor's draft
appearance (FV-3); the next-page image re-shot at the list's end; merged `origin/feat(example)/t3-code` at `a6e31eed7`
(clean); the bundle rebuilt, both builds driven once with the round-2 steps, images composed and uploaded (`r2-*`); final
checks on the merge head.

## Next action

Review and merge.

## Delivery

Merged on 2026-10-10 as `fea35ef11` (#376, squash) after an independent review and its repair round.
