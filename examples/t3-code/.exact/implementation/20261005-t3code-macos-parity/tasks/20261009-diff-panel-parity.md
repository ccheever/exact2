---
name: 20261009-diff-panel-parity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-diff-panel-parity
pr_url: https://github.com/ccheever/exact2/pull/360
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

## Cause and fix

- **PA-12 (cause found; clone, not host).** The composer's Send button declares `aria-keyshortcuts="Meta+Enter …"` at
  all times (`composer-editor-intent.ts` `sendChords`, "⌘↩ always reaches it"). By Exact's rule an `aria-keyshortcuts`
  button hears its chord before any `key` handler and takes the key (`docs/contract-grammar.md` "Shortcuts";
  `Presenter.routeKey` runs `shortcuts.perform` before `keyDown`; a disabled match is consumed). So ⌘↩ in the draft's
  textarea went to Send (disabled with an empty composer: consumed) and `DiffDraftCard`'s `key` handler never heard it.
  The reference's send chords answer only with the composer focused (`composerFocus` in its keybindings; the comment
  draft's own `onKeyDown`, `isCommentSubmitShortcut`). Fix: the draft's textarea reports `focus`/`blur`
  (`diffreview focus|blur`), and while a shown draft holds the focus `composer.sendChords` is empty
  (`diff.ts` `draftHoldsCommandEnter`, `composer-presentation.ts`). Not the capture-phase gap (X25, #140): no framework
  change is needed. The Files preview's comment draft has the same cause and is not in this task's findings: it passes
  the focus ops but does not track them yet (`r4-surfaces-files.contract`).
- **PA-6.** `diff-base-ref.ts` ports `baseRefChoices.ts` (pairs, remote-only, filter) and the DiffPanel Combobox data:
  `vcs.listRefs` local and remote (`includeMatchingRemoteRefs`, the query, limit 100) at the preview's cwd, the head left
  out. The branch selection carries `baseRef` (diffPanelStore `selectBranchBaseRef`; Uncommitted keeps it for Changes)
  and `review.getDiffPreview` asks with it. `diff.contract` `DiffCompare` / `DiffBasePicker` / `DiffBaseRow`: "<head> →"
  (truncates first), the trigger "Change comparison target. Currently <base>", a 18rem popover with "Search refs...",
  Branch / Remote, "No matching refs.", Automatic, the "Use remote version of" switch and the "Remote only" check;
  Escape closes only the popup (`aria-modal` scope).
- **PA-7.** The scope menu always lists Latest turn and Turn; with no turns Latest turn only closes the menu and Turn
  opens nothing, as the reference.
- **PA-8.** Matching the original needed two orders, not one: Pierre's own header counts read "-d +a" (DOM order,
  `createFileHeaderElement.js`), while a large source read file by file (DiffPanel `lazySource`) hides them and draws
  `DiffStatLabel` ("+a -d", both always, compact, 4ch columns). The audit's reference showed its own large checkout,
  so it saw only the second. The clone now draws `DiffStatLabel` for a large source and keeps Pierre's order and zero
  rules otherwise (`diff.ts` `headerStat`); the panel total is compact ("+2.4k -1") as `DiffStatLabel` is.

## Acceptance results

Lane `diff-panel-parity` (base port 16840): the `work` project under the reference home's `worktrees/` (so the reference
server accepts its cwd), an `origin` remote (main, feature/audit, remote-only `release/2026-10`) and a 2,400-line
committed file, so Changes is a large source and Uncommitted is not; providers switched off in the clone homes (no
provider toasts, no turns). Before = `t3-code-evidence-base` (`950e8e2e5`), after = `35d40ee97`, reference = T3 Code
`1e2ecbd975` Electron. Images: before | after | reference.

| Id | Result | Proof |
| --- | --- | --- |
| PA-6 | pass | [picker](https://raw.githubusercontent.com/ccheever/exact2/5852263d117bd5a75e007481a0a96c49c0724b91/diff-panel-parity/pa6-base-ref-picker.png), [no match](https://raw.githubusercontent.com/ccheever/exact2/4367f0e886a06972ac6fdeaab2f2e3e330fea1ab/diff-panel-parity/pa6-no-matching-refs.png), [after picking main](https://raw.githubusercontent.com/ccheever/exact2/d3b0dddc142a59559b31a36c753074f3e9224ca9/diff-panel-parity/pa6-picked-main.png); agent tree: "Comparing feature/audit against origin/main" → pick main "Currently main" → remote switch "Currently origin/main" → Automatic "Currently origin/main"; Escape closed only the popup (panel kept) |
| PA-7 | pass | [scope menu](https://raw.githubusercontent.com/ccheever/exact2/2b4335c78a473f90c303d01d372453a489c19924/diff-panel-parity/pa7-scope-menu.png): Changes, Uncommitted, Latest turn, Turn on a thread with no turns; unit test: Latest turn with no turns closes the menu without a request or error |
| PA-8 | pass | [Changes (large source)](https://raw.githubusercontent.com/ccheever/exact2/abb7b7936a40391cd5a0570f7c32bb84089be184/diff-panel-parity/pa8-header-stats.png): `src/app.ts` "+5 -1", `data/rows.txt` "+2.4k -0", total "+2.4k -1"; [Uncommitted (Pierre)](https://raw.githubusercontent.com/ccheever/exact2/c7a4f93f3ec4ea68e2635a5159ef5c278675b44d/diff-panel-parity/pa8-uncommitted-pierre-order.png): "-1 +1", "+1", as the reference |
| PA-12 (agent) | pass | [⌘Enter saves](https://raw.githubusercontent.com/ccheever/exact2/24e4d2fc8636690c845e8a2fa6c9f4be4fc18708/diff-panel-parity/pa12-cmd-enter-saves.png): platform-delivered Meta+Enter saved "audit note" (inline card) and inserted the "app.ts L7" chip; before, the draft stayed open. Tree: `send-message` keys `''` while the draft is focused, `Meta+Enter Meta+Alt+Enter` before and after |
| PA-12 (real keys) | open | needs real input: see the batch steps below |

Known residual (framework, declared): the search field draws the native focus ring around it (X61, #302) where the
reference draws only the underline.

## Tests

`diff-panel-parity.test.ts` (7): base-ref choices and filter, the picker rows (switch / remote only / no match / other
cwd), the comparison through `T3Client.command` (listRefs payloads, the pick's `baseRef`, Uncommitted keeps it,
Automatic), Latest turn with no turns, header counts both ways and the compact total, and the Send chords while a draft
holds the focus (blur, closed panel, save).

Checks on `35d40ee97` (the feature branch merged at `6e2040c58`; later commits change this record only):
`bun test examples/t3-code --timeout 60000` exit 0 (3,655 pass, 1 skip, 0 fail, 262 files); strict `tsc` on `app.ts`
exit 0; `contract build examples/t3-code/app.contract` exit 0 (5,752 slots, 46 resources, 90,189 nodes;
`app.contract` 1,230 lines); `git add -A && bun scripts/caps.mjs` exit 0; the five checks: `cargo build --all-targets
--keep-going` 0, `cargo test --lib --bins --tests --no-fail-fast` 0 (3,521 pass, 0 fail, 34 ignored), `cargo clippy
--all-targets --keep-going -- -D warnings` 0, `cargo fmt --all -- --check` 0, `bun scripts/caps.mjs` 0,
`bun scripts/boot.mjs` 0. No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries were
not run. Bundle: `bun host/apple/build.mjs t3-code-macos --bundle` once; one live agent drive (no retry).
The same checks ran again on `55573e65b` (same code; a continuation after a usage-limit stop kept no logs), all
exit 0 with the same counts.

## Real-input batch steps

PA-12, real ⌘Enter (screen unlocked; lane `diff-panel-parity`, build of this branch):
1. `EXACT_ROOT=/Users/daehyeonmun/orca/workspaces/exact2/t3-code-diff-panel-parity` and run the bundle
   (`T3_LOCAL_HOME=<lane>/clone-t3-home T3_LOCAL_PORT=16842 bun host/apple/build.mjs t3-code-macos --bundle --run`).
2. Open "Audit work thread", press Changes in the details card, expand `src/app.ts`, hover line 7 and click its "+".
3. Type `audit note` and press ⌘Return with real keys. Pass: the card "audit note" shows under line 7 and the composer
   gets the "app.ts L7" chip; nothing is sent (the lane's clone home has its providers switched off).

## Next action

Coordinator: review draft PR (link in the frontmatter), run the real-key row in the next batch, merge.
