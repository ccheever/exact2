---
name: 20261010-settings-headings
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-settings-headings'
pr_url: https://github.com/ccheever/exact2/pull/404
verified_commit: null
---

# Settings section and row titles as headings

## Outcome

In T3 Code (`1e2ecbd975`) the Settings pages' section titles and some row titles are headings. On Settings › General,
"New threads" is level 2, and Model, Permissions, Workspace and Submodules are level 3. In the clone they are plain text,
and its Settings accessibility tree has no headings. Both builds of round 8 matched, so main did not cause this; it
predates the round. Found by [adopt-main-fixes-r8](closed/20261010-adopt-main-fixes-r8.md) (#402).

## Steps

1. Read the reference's Settings pages (`apps/web/src/components/settings/`) and list every heading with its level, page
   by page. Confirm the list on the running reference over CDP (`target/t3-audit/ref-app.sh`, the accessibility tree).
2. Give the clone's matching titles `role="heading"` with the reference's `aria-level`. Change nothing visual: no font,
   size, weight or spacing (the titles already look as the reference's do). Keep `headings.test.ts`'s rule that every
   `role="heading"` says its level.
3. Extend `headings.test.ts` (or add a Settings row to it) so it fails on the base and passes after.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Each Settings page's headings match the reference's list | AX tree read on the clone (agent `tree`) against the reference's list | text |
| No visual change | one agent drive of Settings pages | before / after images, identical |
| Guard | the test fails on the base and passes after | test output |

## The reference's headings, page by page

Read from `settingsLayout.tsx` (SettingsSection's title is an `<h2>`, `sr-only` with `hideTitle`; SettingsRow's title an
`<h3>`), FoldedSettingsSection (an `<h2>` holding its trigger), ThemeSettings.tsx (two `<h3>`s), and confirmed on the
running reference over CDP (every `h1`–`h6`/`role=heading` and the ARIA snapshot, lane `settings-headings`, 1280×840).
Toasts (the reference's "Update Available", the clone's) are not Settings headings and are left out.

| Page | Reference headings | Clone before (evidence base `c03d7e908`) |
| --- | --- | --- |
| General | 9 h2 (New threads, Organization, Behavior, Projects & threads, Confirmations, Text generation, About, Diagnostics, Legacy features) and 39 h3 (each row title; "Version 0.0.45" holds the version) | none |
| Appearance | h2 Colors & themes (sr-only), Interface, Motion, Typography; h3 Color scheme, Themes, and each row title (12 h3) | none |
| Project (project scope) | h2 Project (sr-only), New threads, Actions, Danger; h3 Name, Project icon, Model, Workspace, Actions, Remove project | Model h3 and the sr-only Project h2 missing |
| Scheduled Tasks | h2 Scheduled tasks; h2 the environment (sr-only for one environment); h3 No scheduled tasks | the environment h2 missing (and it was level 3 when shown) |
| Connections | h2 the machine, Environments; h3 Local environment, Version, Network access, Tailscale HTTPS | extra h3 "No saved remote environments" (EmptyTitle is a `<div>`) |
| Keybindings, SnapShots, Providers, Integrations, Source Control, Storage, Archive, Diagnostics, Open source licenses | as the clone's | matched already |

Two rows show only on a Nightly build (NightlyMobileBeta.tsx's "Mobile app", SettingsPanels.tsx's "Environment
identification"); the clone is Nightly and the reference Alpha, so they are SettingsRow `<h3>`s the reference does not
draw here: the clone gives them h3 as well.

## Built

- `settings-rows.contract`: CoreSections' section title is `role="heading" aria-level=2`; Legacy features gets an
  sr-only h2 beside its trigger; CoreRowView's row title is `aria-level=3` (General, Appearance, and the Project page's
  Model row); the Version heading is named `Version <version>` and the value beside it is `aria-hidden`, as the
  reference's `<h3>` holds both.
- `settings-kit.contract`: `SettingsSrHeading`, a heading at a given level that draws nothing (absolute, 1×1, clipped,
  transparent, no pointer events), for the reference's `sr-only` titles.
- `settings-appearance.contract`: sr-only h2 "Colors & themes"; "Color scheme" and "Themes" are h3.
- `settings-projects.contract`: sr-only h2 "Project" before the Name card.
- `settings-scheduled.contract`: the environment title is h2 (was 3), sr-only when one environment shows.
- `connections.contract`: "No saved remote environments" is no heading.
- Nothing visual: every new heading writes its `font-size` (and the row titles `font-weight=500`); the section titles'
  weight is title2/title3's 400, the weight they had (LLP 1115 D3 sets only size and weight on a heading).
- Declared (EXACT2-GAPS.md, "Settings headings: a heading beside its button"): the Mac host exposes a heading only on
  text, so Legacy features' h2 sits beside its button instead of holding it. New framework gap, unnumbered, not filed.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Each Settings page's headings match the reference's list | pass: 14 of 14 AX reads match after (9 of 14 before); Diagnostics' AX tree stops at the reader's element limit, so its last four h2s are read from the plain tree (all 13 `role="heading"` level 2) | [after](https://raw.githubusercontent.com/ccheever/exact2/2dbf11eacc115614b9b9225b3af516332912d43c/settings-headings/evidence-after-headings.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/da101fb30a3aab429fbd104aa6cb5421c83ae898/settings-headings/evidence-before-headings.txt) |
| No visual change | pass: outside the toast corner (each drive shows a different toast) the same drive's window screenshots are pixel-identical on 10 of 14 pages; the rest differ only in a newer build's model label ("Medium · 1M"), live Diagnostics data and ≤10/255 text noise on one untouched description line; the 49 General headings are pixel-identical | [pixel diff](https://raw.githubusercontent.com/ccheever/exact2/c724c7703afbffcd635106e5ab4aeef3e75deb16/settings-headings/pixel-diff.txt); [General](https://raw.githubusercontent.com/ccheever/exact2/72b504a130e75087d2ed303f3cfdbbca2de54b0c/settings-headings/settings-headings-general.png), [Appearance](https://raw.githubusercontent.com/ccheever/exact2/57e40cfe94023a589b2c58cb6a51fcb99107a601/settings-headings/settings-headings-appearance.png), [Project](https://raw.githubusercontent.com/ccheever/exact2/1f5456b4fbbdc4bb254108a5433fd602ce791ad7/settings-headings/settings-headings-projects.png), [Scheduled Tasks](https://raw.githubusercontent.com/ccheever/exact2/aa44bf78247b86cd299d30f65a6f135840a649c4/settings-headings/settings-headings-scheduled-tasks.png), [Connections](https://raw.githubusercontent.com/ccheever/exact2/c5481da66b6fb86a99112bdbe3efc583f2dd4c6c/settings-headings/settings-headings-connections.png) |
| Guard | pass: `headings.test.ts` "Settings section titles are h2 and row titles h3, as the reference" fails on the base contracts (`settings-rows.contract`'s section title has no role) and passes after | [base run](https://raw.githubusercontent.com/ccheever/exact2/e7965f46e363783e178292454410ecf0daf59b7b/settings-headings/evidence-guard-base.txt); after: 3 pass |

The before build is the evidence worktree at `c03d7e908` (657 commits behind this branch's base `9447c1c75`), as the
brief names it; the after build is this branch. One live drive of the after build (all pages, one launch).

## Tests

- `headings.test.ts`: new test "Settings section titles are h2 and row titles h3, as the reference"; the existing rule
  that every `role="heading"` says its level still holds (167 heading lines, 162 before).

## Real-input batch steps

None: the AX tree is the platform's (`tree --ax`, AppKit), read in agent mode.

## Next action

Coordinator review of #404.
