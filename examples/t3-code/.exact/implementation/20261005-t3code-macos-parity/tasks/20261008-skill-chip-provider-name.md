---
name: 20261008-skill-chip-provider-name
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-skill-chip-provider-name'
pr_url: https://github.com/ccheever/exact2/pull/288
verified_commit: f00a02704
---

# Composer skill chips show the provider's display name

## Outcome

Found by #257 ([editable-font-prompt-preview](20261007-editable-font-prompt-preview.md), "Follow-up, not
in this task"): the composer's `$skill` chips always title-cased the raw name ("Imagegen"), where the
reference shows the provider's own display name ("Image Gen").

## The reference (`1e2ecbd975`)

- `ComposerPromptEditorTiptap.tsx:675-686` `skillLabelFor(name)`: the skill whose `name` equals the
  token (without its `$`) in the editor's `skills` prop; found, its label is
  `formatProviderSkillDisplayName(skill)`, else `formatProviderSkillDisplayName({ name })`.
  `ComposerSkillNodeView` (`:281-307`) draws that label (and `Skill <label>` as its accessible name).
- `client-runtime/src/providerSkills.ts:22` `formatProviderSkillDisplayName`: `displayName` trimmed,
  else the name title-cased on spaces, `:`, `_` and `-`.
- The `skills` prop is `ChatComposer.tsx:2173` `selectedProviderSkills`:
  `resolveProviderSkillsForCwd(selectedProviderStatus, gitCwd)`, the selected provider's skills for the
  thread's workspace. The settings preview passes `EMPTY_SKILLS` (`SettingsFontPreviews.tsx:47`), so its
  sample chip stays "Frontend Design".
- The server fills `displayName` from Codex's `skill.interface.displayName` (`CodexProvider.ts:318`),
  OpenCode's and Cursor's names.

## What was built

- `composer-editor-menu.ts` `skillChipLabels(skills)`: name → `skillDisplayName(skill)` (the existing
  `formatProviderSkillDisplayName` port), the first skill of a name winning as `find` does.
- `composer-editor.ts`: `editorSync` sends `skills: skillChipLabels(workspaceValues(provider, cwd,
  'skills'))`, the same list the `$` menu's rows come from (the workspace's own discovered list when
  there is one, else the provider's).
- `T3Composer.swift` hands it to `T3ComposerStyler.skills`, which restyles when it changes;
  `displayLabel(_:)` is now the styler's own: a skill chip reads its label from `skills`, else the
  title-cased name as before. The pill's width follows the label. The prompt preview's editor gets no
  `skills`, as the reference's preview passes none.
- One difference, kept on purpose: the reference fixes a chip's label when the chip is made (the
  node's `skillLabel` attribute, `composer-rich-text-doc.ts:113-118`) and makes it again only when the
  document is rebuilt from the value; the clone's styler reads the label each time it restyles, so a
  chip typed before the provider's skill list arrived (or kept across a provider switch) takes the
  label the list now gives. With the list loaded first, as in every drive, the two are the same.

## Acceptance and reproduction

Lane (not committed, `target/skill-chip-provider-name/lane`): the app's own "This machine" server on
isolated `HOME`, `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*`, `T3_LOCAL_HOME` (`t3-before`, `t3-after`),
`T3_LOCAL_PORT` 16310 (before) / 16311 (after), `T3CODE_TELEMETRY_ENABLED=false`, the staged t3
0.0.46-nightly unpacked as `T3_LOCAL_RUNTIME_DIR`; `bin/` holds `codex` (Homebrew's 0.151.0) and `node`.
Codex is signed in with a placeholder API key (`codex login --with-api-key`, file store; no account,
nothing sent), so the server lists Codex's system skills: `imagegen` "Image Gen", `openai-docs`
"OpenAI Docs", `review-agent` "Review Agent", … (read from `codex app-server` `skills/list`). T3 marks
Codex 0.151.0 unsupported (a banner), which does not stop the skill list. One git repository as the
project. Drive ([drive.mjs](https://raw.githubusercontent.com/ccheever/exact2/38171350f8fdb9e22c97e8eb426fff6788e67b91/skill-chip-provider-name/drive.mjs.txt)),
agent mode at 1280×840: first-run wizard, add the project, type
`Use $imagegen with $openai-docs then $review-agent and $frontend-design ` in the composer, screenshot;
then `$ima` for the `$` menu. Before: the base build (`t3-code-evidence-base` at `07dcef1ab`).

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| A known skill's chip reads its display name | pass (live + unit) | [01](https://raw.githubusercontent.com/ccheever/exact2/5b3793b5c89a05c37544daa20346bb1d59cf2116/skill-chip-provider-name/01-composer-skill-chips.png): before "Imagegen", "Openai Docs"; after "Image Gen", "OpenAI Docs"; XCTest `testSkillChipsReadTheProvidersDisplayName` | — |
| A name the provider does not list is title-cased | pass (live + unit) | 01: `$frontend-design` "Frontend Design" in both; the same XCTest and `skillChipLabels` test | — |
| The workspace's own list wins over the provider's | pass (unit) | "the editor is synced with the selected provider's labels, the workspace's own list first" (`composer-editor.test.ts`) | — |
| A skill the provider drops goes back to the title-cased name | pass (unit) | the XCTest's second `editorSync` (empty `skills`) | — |
| Names match exactly | pass (unit) | the XCTest (`Frontend-Design` does not label `$frontend-design`); `skillChipLabels` keeps `ImageGen` as `ImageGen` | — |
| The pill is as wide as its new label | pass (unit + live) | the XCTest: `chipWidth` grows and the text's kern at the chip is the new width (fails if the sync does not restyle: checked by removing the `didSet`); 01 shows the pills fitting | — |
| The `$` menu row and the chip agree | pass (live) | [02](https://raw.githubusercontent.com/ccheever/exact2/b571c9c50c0663ffd11381f379a5271a62942523/skill-chip-provider-name/02-skill-menu.png): the row read "Image Gen" before and after (control); the chip now matches it | — |
| The settings preview keeps "Frontend Design" | pass (unit) | `PromptPreviewEditorTests.testThePreviewDrawsTheSamplesChipsWithItsOwnEditor`: after the composer is synced with `frontend-design` "Composer Only", the preview still reads "Frontend Design" | — |
| Tests fail on the base | pass | [XCTest on base](https://raw.githubusercontent.com/ccheever/exact2/a055e7495d9cd29ae7efdc0c46d8c1c4da444245/skill-chip-provider-name/xctest-on-base.txt) ("Imagegen" ≠ "Image Gen", the width unchanged; the test compiled against the base styler's static `displayLabel`); [bun test on base](https://raw.githubusercontent.com/ccheever/exact2/8d3ee6015b133b978b36ddbe026b9381fbf1e1e2/skill-chip-provider-name/bun-test-on-base.txt) (`editorSync` carried no `skills`) | — |
| Visual oracle and trace | not run | — | user decision 2026-10-06: the desktop oracle and trace tools are not built; before/after pairs instead |

Found, not in this task: the reference's skill chip opens a popover on press (its label, the
description or "No description is available for this skill.", and "View instructions" for a skill with a
path) and is named `Skill <label>` to assistive technology; the clone's composer chips have neither.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| before 1–4 (agent, base) | `07dcef1ab` | 1: tapped before the first-run wizard showed (inert window); 2: the default screenshot form came back blank, window shots since; 3: typed `$openai-docs,` (a comma is no chip boundary, as in the reference's regex); 4: the record ("Imagegen", "Openai Docs") | [record-before](https://raw.githubusercontent.com/ccheever/exact2/3951bcce35c99fca9a7dfd294e0cc33ecc21712f/skill-chip-provider-name/record-before.txt) | — |
| after (agent, branch; the one live session) | this branch before commit | every live row passes | [record-after](https://raw.githubusercontent.com/ccheever/exact2/cf74a17f57e04986f0a5b8e601fba0129c7cc826/skill-chip-provider-name/record-after.txt), 01, 02 | — |
| checks | this branch | `bun test examples/t3-code` 3038 pass / 1 skip / 0 fail (base 3036 + 2); strict tsc clean; contract build 3844 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11 pass; composer AppKit binary 52 / 0 (base 51 + 1); caps within; five checks: build exit 0, test 3,383 passed / 0 failed / 33 ignored (94 binaries), clippy and fmt clean, boot allowed paths only; verify runner attempt 2 (after the review's tests) passed, `source_unchanged: true` | `target/skill-chip-provider-name/verify/attempt-2` (not committed) | — |
| independent review | the staged diff | no blocking findings. Taken: exact-name, restyle (kern) and preview-isolation tests; the live relabel recorded as a difference. Not taken: building the map from `markdownSkills` (`r4-timeline-chips.ts`) instead of the menu's `skillDisplayName` port, which would pull that module's dependencies into the menu module | — | — |

## Progress

2026-10-08: implemented, verified (`f00a02704`; runner attempt 2 passed and the committed tree matches it;
independent review without blocking findings) and opened as draft PR #288.

## Next action

Coordinator: review and merge the draft PR. The chip popover and its accessible name (above) are a
separate gap for the plan to schedule.
