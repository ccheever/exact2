---
name: 20261010-audit-wave-followups-2
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
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

## Next action

Build after the in-flight tasks touching Settings and themes merge (usage-and-pr-pages, model-picker-parity).
