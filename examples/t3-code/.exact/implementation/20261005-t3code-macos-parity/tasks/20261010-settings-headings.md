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
`<h3>`), SettingsPanels.tsx's `LegacyFeaturesSection` (about line 2112: its `<CollapsibleTrigger>`, a button, holds
`<h2>Legacy features</h2>`; FoldedSettingsSection is not used there), ThemeSettings.tsx (two `<h3>`s), and the
ProjectSettingsPanel.tsx, ProjectDefaultsSettings.tsx and ProjectActionsSettings.tsx sections for the Project page.
Confirmed on the running reference over CDP (every `h1`–`h6`/`role=heading` and the ARIA snapshot, lane
`settings-headings`, 1280×840). The first read had no project chosen, so the Project page showed only its scope notice
and the list came from source; review round 2 chose the project "work" in the scope picker and read the same list
([ref-projects-headings.txt](https://raw.githubusercontent.com/ccheever/exact2/19fa108c33379eb34dcd94861f0a2bc19a8637f1/settings-headings/ref-projects-headings.txt)).
It also read Legacy features' nesting, `button "Legacy features"` > `heading "Legacy features" [level=2]`
([ref-legacy-aria.txt](https://raw.githubusercontent.com/ccheever/exact2/98e34bd659cfadd18ece51a01a5f85fc61828137/settings-headings/ref-legacy-aria.txt)).
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
- Declared (EXACT2-GAPS.md, "Settings headings: a heading beside its button"): the reference's Legacy features trigger is
  a button that holds its `<h2>`. The Mac host makes a button a leaf (`NodeViewMac.accessibilityChildren` is nil when
  `actsAsButton`) and exposes a heading only on text, so the clone's h2 sits beside the button instead of inside it. This
  is a new framework gap with no issue number, and it is not filed because the brief files nothing upstream. It is open:
  see "Decision needed".

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Each Settings page's headings match the reference's list | pass: 14 of 14 AX reads match after (9 of 14 before); Diagnostics' AX tree stops at the reader's element limit, so its last four h2s are read from the plain tree (all 13 `role="heading"` level 2) | [after](https://raw.githubusercontent.com/ccheever/exact2/2dbf11eacc115614b9b9225b3af516332912d43c/settings-headings/evidence-after-headings.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/da101fb30a3aab429fbd104aa6cb5421c83ae898/settings-headings/evidence-before-headings.txt) |
| No visual change | pass (review round 2): in both drives the toasts are gone and nothing is masked. 18 of 19 window screenshots are pixel-identical before and after. They cover all 14 pages, General scrolled to its bottom in four 700pt steps (every General heading), and Legacy features closed and open. The sr-only h2 sits in that section's gap column and moves nothing: the button is at (344.5, 760) 848×32 in both builds' AX trees. The one difference is Diagnostics' live process and resource data, on a page this PR does not change. (Round 1 had a toast in the after shots, so it masked the toast corner. That hid General's Model controls and Appearance's Dark tile, and its General shot showed only the top of the page.) | [pixel diff](https://raw.githubusercontent.com/ccheever/exact2/4f3548514fd4e1a6c79bce7c7c852b23c695984a/settings-headings/pixel-diff-r2.txt), [Legacy features AX](https://raw.githubusercontent.com/ccheever/exact2/33bba53928f9e4fbd90ca39b2eddaae965bd22f6/settings-headings/evidence-legacy-ax-r2.txt); [General](https://raw.githubusercontent.com/ccheever/exact2/73b98e2579ccc30ceb2067201592ceaf68014a29/settings-headings/settings-headings-r2-general.png), [General, Legacy features](https://raw.githubusercontent.com/ccheever/exact2/0fd16287544a79913338718995b8ab72be27de40/settings-headings/settings-headings-r2-general-legacy.png), [Legacy features open](https://raw.githubusercontent.com/ccheever/exact2/bd459c952b7163d85352634b77e53d40b90442b1/settings-headings/settings-headings-r2-general-legacy-open.png), [Appearance](https://raw.githubusercontent.com/ccheever/exact2/06518561341152d61585c145d37b79ae6404da41/settings-headings/settings-headings-r2-appearance.png), [Project](https://raw.githubusercontent.com/ccheever/exact2/ed3de200781767d4a40cf43c20ae8df29343b5f2/settings-headings/settings-headings-r2-projects.png), [Scheduled Tasks](https://raw.githubusercontent.com/ccheever/exact2/4aa596f6d9fd626734db8bcecc3b340aff022095/settings-headings/settings-headings-r2-scheduled-tasks.png), [Connections](https://raw.githubusercontent.com/ccheever/exact2/1af110bd1be8c9cc40bfdcc34cf87b660774669c/settings-headings/settings-headings-r2-connections.png) |
| Guard | pass: `headings.test.ts` "Settings section titles are h2 and row titles h3, as the reference" fails on the base contracts (`settings-rows.contract`'s section title has no role) and passes after | [base run](https://raw.githubusercontent.com/ccheever/exact2/e7965f46e363783e178292454410ecf0daf59b7b/settings-headings/evidence-guard-base.txt); after: 3 pass |

The before build is the evidence worktree at `c03d7e908` (657 commits behind this branch's base `9447c1c75`), as the
brief names it; the after build is this branch. Round 1 made one live drive of the after build, covering all pages in
one launch. Review round 2 made one more drive of each build with the same steps (`target/settings-headings/r2/drive.sh`).
Both drives closed the Nightly beta-app toast with its Dismiss button. The provider update toast arrives at a varying
time, so both lanes' server settings turned it off (`enableProviderUpdateChecks: false`) for the two drives and were
restored afterwards. The reference shots are from the running reference at 1280×840.

## Tests

- `headings.test.ts`: new test "Settings section titles are h2 and row titles h3, as the reference"; the existing rule
  that every `role="heading"` says its level still holds (167 heading lines, 162 before).

## Checks (head `0f4f79fa3`, review round 2; `origin/feat(example)/t3-code` `4c13b440f` is already merged)

`bun test examples/t3-code --timeout 60000` 0 (4363 pass, 1 skip, 0 fail); strict `tsc` 0; `contract build
examples/t3-code/app.contract` 0; `git add -A && bun scripts/caps.mjs` 0; `cargo build --all-targets --keep-going` 0;
`cargo test --lib --bins --tests --no-fail-fast` 0 (3679 passed, 0 failed, 34 ignored); `cargo clippy --all-targets
--keep-going -- -D warnings` 0; `cargo fmt --all -- --check` 0; `bun scripts/boot.mjs` 0. No Rust or Swift changed, so
no `cargo test -p t3-code-macos` or AppKit binary. The bundle was built once before round 2's drives (exit 0).
`app.contract`: 1,341 lines. Later commits change only this record. Round 1's checks (head `91f6ddbd9`) had the same
results.

## Real-input batch steps

None: the AX tree is the platform's (`tree --ax`, AppKit), read in agent mode.

## Not done / not verified

- Legacy features' heading sits beside its button, not inside it. The reference's button holds the h2. The clone's h2 is
  the button's sibling, because the Mac host makes a button a leaf. The heading list and levels match. The brief allows a
  difference only for a framework limit that has an issue number. This one has none and is not filed, because the brief
  files nothing upstream (EXACT2-GAPS.md, "Settings headings: a heading beside its button"). Blocker: the decision below.

## Decision needed

Legacy features' heading: should the coordinator file the framework gap? The Mac host gives a button no accessibility
children (`NodeViewMac.accessibilityChildren` is nil when `actsAsButton`), so a heading cannot sit inside a button. If
it is filed, the EXACT2-GAPS entry takes its number, and the clone can nest the heading once main exposes a button's
heading child. If the difference is accepted, nothing changes here. The rest of the task does not depend on this decision.

## Next action

The coordinator decides the Legacy features heading ("Decision needed"), then reviews #404.
