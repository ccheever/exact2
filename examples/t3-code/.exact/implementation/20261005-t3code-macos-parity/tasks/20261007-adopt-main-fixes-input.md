---
name: 20261007-adopt-main-fixes-input
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-input
pr_url: null
verified_commit: null
---

# The clone adopts main's input fixes (#110, #111, #132, #133, #134)

## Outcome

Where a fix that main merged on 2026-10-06 covers what the clone needs, the clone's workaround is
gone and the clone behaves as T3 Code does. Where it does not, the workaround stays and the docs
say exactly what is still missing. `EXACT2-GAPS.md`, the issues README's "Upstream issues" table,
the issue records and the task documents that cite these issues are current.

## Scope and exclusions

| Issue | Plan id | main PR | Clone sites |
| --- | --- | --- | --- |
| [#110](https://github.com/ccheever/exact2/issues/110) | X15 | #168 (`841e1b152`) | `R10Connect.swift` (chord re-issue), `T3Menus.swift` (⌘W repeat and ⌘Q hold key codes), `R9Input.swift` (`chordKey`) |
| [#111](https://github.com/ccheever/exact2/issues/111) | X16 | #160 (`c73345026`) | `t3-plain-text` hook (`app.json`, Files editor, diff comment, script command), `T3PanelsNative.swift`, `T3ComposerEditor.swift` |
| [#132](https://github.com/ccheever/exact2/issues/132) | X33 | #171 (`19d335484`) | Cite (`diff-cite.contract`, `diff-citations.ts`) |
| [#134](https://github.com/ccheever/exact2/issues/134) | X35 | #167 (`e931c1d7c`) | `ssh-prompt.contract`, `ssh-auth.ts`, `T3SshAuth.swift` |
| [#133](https://github.com/ccheever/exact2/issues/133) | X34 | #178 (`8c0c6331e`) | none in code; `pr-links-previews-and-routing` waits on it |
| [#120](https://github.com/ccheever/exact2/issues/120) | X43 | closed, not planned | `settings-scoped-switch.contract` (mixed thumb) |

Also recorded: [#179](https://github.com/ccheever/exact2/issues/179) (X47, filed 2026-10-07: a
custom pressable box draws no focus ring; X46 was already the assets build step).

Excluded: framework changes; reopening or commenting on GitHub issues (the coordinator does it);
real-input checks (Korean 2-Set, a physical keyboard's pauses): unverified (attended).

## Context and guidance

Brief: wave 4 addenda 9 and 10 ("Adoption tasks"). Each fix was confirmed on this branch with
`git merge-base --is-ancestor <merge> HEAD`. Every site was found with `grep -rn` of the issue
number and its X id in `examples/t3-code`, outside `evidence/`. Reference source: T3 Code
`1e2ecbd975` (`AssistantSelectionToolbar.tsx`, `SshPasswordPromptDialog.tsx`, the password
inputs' `autoComplete`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged main | origin/main into `feat(example)/t3-code` | `4cdb8aa63` | Merged | the five merge commits are ancestors of HEAD |
| merged task PR | terminal drawer | #175 (`1a50d0df3`) | rebased on it before the PR | no new site for these issues in its diff |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| #111 / X16 | **adopted** (behavior change) | #160 maps `autocorrect="off"` to no smart quotes, dashes or text replacement. The `t3-plain-text` switch-off is removed from the Files editor, the diff comment, the script command and the composer (which already had `autocorrect="off"`). The Files editor's hook is renamed `t3-file-editor`: it still takes the focus its press began. The commit message, theme JSON, scheduled prompt and scoped-setting textareas had nothing and now carry `autocorrect="off"`, so they keep their bytes as T3 Code's browser textareas do. `text-entry.test.ts` checks every editable textarea. Link and data detection, text completion and smart insert/delete are AppKit's defaults again. |
| #110 / X15 | **not adopted** | #168 matches `aria-keyshortcuts` and the host's own command menu items (`ShortcutHost` targets) by physical key. It does not reach menu items with another target (Copy, Paste, Undo, Select All, Quit, Close Window, the clone's Reload and Paste as Text), the terminal's WKWebView, or the module's own key monitors that read the event's characters (the composer's queued-edit chords, the SnapShot shortcut). `R10Connect`'s re-issue covers all of them, so it stays; `T3Menus` and `R9Input` keep their key-code fallbacks (they read raw events). Comments now say so. |
| #134 / X35 | **partly adopted** | #167 masks a password field's value in `tree`, `layout` and the `type` reply, and `autocomplete` sets the AutoFill content type. The provider and Bitbucket password inputs are masked with no clone change. The SSH dialog keeps its native secure field: main's docs leave the app's state slots and data module outside the guarantee, and a Contract field would carry the password through a component state slot, the answering action and the data module's native request; the native field keeps it out of all three (the SSH acceptance's secret audit relied on that). The drive's `state` did not list the theme dialog's component-local draft, so a `state` reply may not show such a slot; the data-module path remains. |
| #132 / X33 | **not adopted** (nothing to remove) | #171 keeps the selection when a button is pressed. Cite's button already kept it with `retainFocus=true`, which is the reference's `onPointerDown` `preventDefault()`, not a workaround. Still missing on main: the selection's end rectangle and `clearSelection()` (the reference clears the selection after citing; the clone leaves it). |
| #133 / X34 | **not adopted** (nothing to remove) | #178 makes the macOS agent hover inline runs. The clone has no inline-link hover card. Unblocked for `pr-links-previews-and-routing`: an inline link's hover can be built and agent-driven on macOS. Still missing: `frame()` of an inline run, so the card has nothing to anchor to. |
| #120 / X43 | closed, not planned | The mixed thumb stays; the switch reports unchecked. The docs say "closed, not planned". |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Typed bytes kept | lane server 16360 (isolated HOME, `T3CODE_TELEMETRY_ENABLED=false`), fresh pairing link | Settings › Appearance › Add theme; type `"0.1" -- 'a'` key by key with real pauses into the Theme JSON box | BEFORE: AppKit's curly quotes and dash; AFTER: the bytes as typed | macOS 1280×840 | `tree add-theme-json`, before/after shot |
| Every textarea | — | `bun test examples/t3-code/text-entry.test.ts` | every editable textarea has `autocorrect="off"` | — | test (fails on the base contracts: 4 textareas) |
| Files editor focus kept | — | AppKit `r5-panels` | the press-to-focus tests pass with the renamed hook | macOS | 6/0 |
| Workarounds kept still work | — | AppKit `r10-connect`, `menus`, `ssh` | pass | macOS | counts below |
| Korean 2-Set chords | real keyboard | ⌘B, ⌘K, ⌘C, ⌘W under 2-Set | as before | macOS | unverified (attended) |

## Progress

Implemented 2026-10-07 on the feature branch at `1a50d0df3` (rebased from `4cdb8aa63` after #175).
Verification: unverified.

Commits: X16 adoption; comments on the kept X15, X35 and X43 workarounds; docs; this document.

Not run: real-input rows (Korean 2-Set chords, a physical keyboard's typing pauses), oracle and
trace-diff rows (desktop-oracle-and-trace will not be built), 840×620 and dark appearance live.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | on `1a50d0df3` | see Checks below | drive records below | — |

## Next action

Review the PR. An attended session checks ⌘B/⌘K/⌘C/⌘W under Korean 2-Set and real typing in the
theme JSON, commit message and scheduled prompt boxes.
