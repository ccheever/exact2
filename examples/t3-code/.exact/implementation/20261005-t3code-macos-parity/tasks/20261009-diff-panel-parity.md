---
name: 20261009-diff-panel-parity
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

# Diff panel: base-ref comparison, scope menu rows, header stats order, and ⌘Enter on a comment draft

## Outcome

- In the Changes scope with a base ref, the Diff header shows "<head> → <base>" and the "Change comparison target" picker.
- The scope menu always lists Latest turn and the Turn submenu.
- File headers read additions first.
- ⌘Enter saves a line comment draft. The closed diff-review-engine task recorded this as working (regression).

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

Fixture note: in the audit the reference server refused the lane's `work` cwd ("configured workspace root") and showed
its own checkout's diff; the clone showed the `work` project. Compare controls, not data. Use a lane whose reference
server accepts the project (add it as a workspace root) for PA-6.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PA-6 | In the Changes scope with a base ref, the header shows "<head> → <base>" and a combobox "Change comparison target. Currently <base>": "Search refs...", Branch and Remote columns, Automatic, a "Use remote version of …" switch, "Remote only", "No matching refs.". | The header has the scope button and the controls only. No comparison label or base-ref picker; the source has none of these strings (`diff-lazy.ts` carries `baseRef` with no UI). | Open the Diff on a server thread whose branch has a default-branch base (work › Audit work thread › Changes). | `target/t3-audit/evidence/panel/PA-6-ref.png`, `PA-6-clone.png`, `panel/dumps/r-aria-diff-work.txt` |
| PA-7 | The scope menu always lists Changes, Uncommitted, Latest turn and a Turn submenu. | With no turns the menu lists only Changes and Uncommitted (`diff.contract:211-216`). | work › Audit work thread › Changes › click "Diff scope: Changes". | `target/t3-audit/evidence/panel/PA-7-ref.png`, `PA-7-clone.png` |
| PA-8 | Each file header reads "+14 −9" (additions first), as the panel total does. | File headers read "−1 +5" (deletions first); the panel total reads "+19 −1" (`diff-rows.contract:118-121`). | Open the Diff and compare `src/app.ts`'s header. | `target/t3-audit/evidence/panel/PA-8-ref.png`, `PA-8-clone.png` |
| PA-12 | Type a comment and press ⌘Enter: it saves; the inline card and the composer chip ".gitignore L19 (before)" appear. | After typing "audit note" and a platform-delivered Meta+Enter, the draft stays open with its text. The Comment button does save it and inserts the chip "app.ts L7". | Diff › expand `src/app.ts` › "Comment on line 7" › type text › ⌘Enter. | `target/t3-audit/evidence/panel/PA-12-ref.png`, `PA-12-clone.png`, `panel/dumps/clone-drive-12.jsonl` |

## Scope and exclusions

Included: the four findings above.

Excluded:
- Pinned diff file headers: a declared difference until #131 lands (X32).
- A drag across line numbers (pr-code-tab's real-input row).
- Framework code. PA-12's cause is not known. If it is in the host (a capture-phase key, X25,
  [#140](https://github.com/ccheever/exact2/issues/140), main file `issues/20261009-capture-phase-key-events.md`), record it
  for the coordinator with a one-file repro and keep the row open.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src/components`):
- PA-6: `DiffPanel.tsx:716-842`.
- PA-7: `DiffPanel.tsx:676-713`.
- PA-8: `diffs/StyledDiffCodeView.tsx:216` (header counts, additions first).
- PA-12: `diffs/DiffCommentAnnotation.tsx`; `diffs/commentSubmitShortcut.ts`.

Clone (`examples/t3-code`): `diff.contract` (header row; scope menu `:211-216`), `diff-lazy.ts`, `diff-rows.contract:118-121`,
`diff-comments.contract:15-21`. The branch picker of the composer strip (Search refs, Branch/Remote) can be reused for
PA-6.

Regression: [diff-review-engine](closed/20261005-diff-review-engine.md) lists "type; ⌘↵" in its "Diff line comment"
acceptance row (line 101) and "⌘↵ sends" in its build notes (line 133).

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PA-6 | The header shows "<head> → <base>". The picker lists refs, Automatic, the remote switch and "No matching refs." for a bad query; a pick changes the compared base. | `pa6-base-ref-picker.png` | agent |
| PA-7 | On a thread with no turns, the scope menu lists Latest turn and Turn. | `pa7-scope-menu.png` | agent |
| PA-8 | `src/app.ts`'s header reads additions first. | `pa8-header-stats.png` | agent |
| PA-12 | ⌘Enter in a comment draft saves it: the inline card and the composer chip appear. Agent drive first; then real keys. | `pa12-cmd-enter-saves.png` | agent, then needs_real_input (real ⌘Enter) |

## Next action

Prepare a branch from `feat(example)/t3-code`. Find PA-12's cause first. Build and unit-test. Then do one batched live
drive at the end for every row's before/after pair, and the real-key row when the screen is unlocked. Close every row in
this PR, or record the blocker of a row that cannot pass.
