---
name: 20261010-settings-headings
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

## Next action

Start now.
