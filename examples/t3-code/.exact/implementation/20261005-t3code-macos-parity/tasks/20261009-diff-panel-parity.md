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

### Review round (2026-10-10)

An independent review of draft PR #360 found four problems; the round's own drive found two more.

- **PA-12: a stale focus.** The Apple host sends no blur when a focused view leaves the tree (LLP 1008), so after a
  keyboard thread switch (the panel closes, the draft stays) the Diff could reopen on a thread whose diff lacks the
  draft's line while `draftFocused` was still true, and ⌘↩ in the composer stopped sending. The focus is now the thread
  key it was taken on (`DiffState.draftFocus`, `diff.ts` `noteDraftFocus`), and `draftHoldsCommandEnter` also requires
  the card to be drawn (`draftDrawn`: the panel open, the draft's scope showing, its file listed with its patch in and
  expanded, its end line one the file numbers). A card that mounts again takes the focus by `autofocus` and reports it.
- **PA-6: the search field's keys.** Base UI Combobox list navigation, checked against the reference over CDP: an
  opening starts at the selected item, ↓/↑ move the highlight (loop, and past either end to none), typing clears it,
  Return picks the highlighted item or, with none, only closes; the pointer moves the same highlight and leaving an item
  clears it (`resetOnPointerLeave`). The selected item keeps its own tint under the highlight (`data-selected`). In the
  clone: `DiffBasePicker` keeps the highlight by id per opening (`session`), `dbStep` is the navigation, the search's
  `key` takes ↓/↑ (`preventDefault`, `scrollIntoView` nearest) and its `submit` picks. A plain Enter `aria-keyshortcuts`
  button is not heard in a text field (`ShortcutsMac.permits`), and a popover cannot be hidden from an action (X66,
  #319), so Return empties the popover until the trigger opens it again, as the details branch picker does
  (`r4-git.contract`); the focus goes back to the trigger.
- **PA-6: a stale ref list.** `loadBaseRefs` drops every answer but the newest read's (a counter), so an older cwd's
  answer after a thread switch no longer overwrites the newer one. The unread `loading` flag is gone (the reference
  draws no loading state in this popup).
- **PA-12 with real keys:** still open; it is the coordinator's real-input batch (steps below).
- **Found in this round's drive: a long base overlapped the stats.** `origin/release/2026-10` drew over "+2.4k -1".
  DiffPanel's comparison group is `overflow-hidden` and its trigger a shrink-0 `Button`, so the reference clips it; the
  clone's group now clips the label and trigger (the popup stays outside the clip).
- **Found in this round's drive: the selected ref's tint.** See the keys above (`light-dark(#27272a14, #f5f5f514)`, as
  the clone's other selects).

## Decision needed

With a query, the reference's static Automatic row shifts its keyboard index: "rel" then ↓ lights Automatic, Return on
it selects nothing and leaves the popup open, and a second ↓ goes to no item, so the matching refs cannot be reached
from the keyboard ([reference and clone](https://raw.githubusercontent.com/ccheever/exact2/1a979bef7005de4b1cdc20c6cecd7aa6aa1e76ee/diff-panel-parity/decision-query-keys.png); CDP,
`rk-05`..`rk-11` in the lane's shots). The clone follows Base UI's model over the filtered items
instead (with a query Automatic is not an item, so ↓ lights the first matching ref and Return picks it). Keep this, or
copy the reference's behavior?

## Acceptance results

Lane `diff-panel-parity` (base port 16840): the `work` project under the reference home's `worktrees/` (so the reference
server accepts its cwd), an `origin` remote (main, feature/audit, remote-only `release/2026-10`) and a 2,400-line
committed file, so Changes is a large source and Uncommitted is not; providers switched off in the clone homes (no
provider toasts, no turns). Before = `t3-code-evidence-base` (`950e8e2e5`), after = `35d40ee97` (first round) and
`1c41d6434` (review round), reference = T3 Code `1e2ecbd975` Electron. Images: before | after | reference.

| Id | Result | Proof |
| --- | --- | --- |
| PA-6 | pass | [picker](https://raw.githubusercontent.com/ccheever/exact2/5852263d117bd5a75e007481a0a96c49c0724b91/diff-panel-parity/pa6-base-ref-picker.png), [no match](https://raw.githubusercontent.com/ccheever/exact2/4367f0e886a06972ac6fdeaab2f2e3e330fea1ab/diff-panel-parity/pa6-no-matching-refs.png), [after picking main](https://raw.githubusercontent.com/ccheever/exact2/d3b0dddc142a59559b31a36c753074f3e9224ca9/diff-panel-parity/pa6-picked-main.png); agent tree: "Comparing feature/audit against origin/main" → pick main "Currently main" → remote switch "Currently origin/main" → Automatic "Currently origin/main"; Escape closed only the popup (panel kept) |
| PA-7 | pass | [scope menu](https://raw.githubusercontent.com/ccheever/exact2/2b4335c78a473f90c303d01d372453a489c19924/diff-panel-parity/pa7-scope-menu.png): Changes, Uncommitted, Latest turn, Turn on a thread with no turns; unit test: Latest turn with no turns closes the menu without a request or error |
| PA-8 | pass | [Changes (large source)](https://raw.githubusercontent.com/ccheever/exact2/abb7b7936a40391cd5a0570f7c32bb84089be184/diff-panel-parity/pa8-header-stats.png): `src/app.ts` "+5 -1", `data/rows.txt` "+2.4k -0", total "+2.4k -1"; [Uncommitted (Pierre)](https://raw.githubusercontent.com/ccheever/exact2/c7a4f93f3ec4ea68e2635a5159ef5c278675b44d/diff-panel-parity/pa8-uncommitted-pierre-order.png): "-1 +1", "+1", as the reference |
| PA-12 (agent) | pass | [⌘Enter saves](https://raw.githubusercontent.com/ccheever/exact2/24e4d2fc8636690c845e8a2fa6c9f4be4fc18708/diff-panel-parity/pa12-cmd-enter-saves.png): platform-delivered Meta+Enter saved "audit note" (inline card) and inserted the "app.ts L7" chip; before, the draft stayed open. Tree: `send-message` keys `''` while the draft is focused, `Meta+Enter Meta+Alt+Enter` before and after |
| PA-12 (real keys) | open | needs real input: see the batch steps below |
| PA-6 keys (review) | pass | [↓ ↓](https://raw.githubusercontent.com/ccheever/exact2/fddba6fa96dab33541841b1f9387c829eaf204d6/diff-panel-parity/pa6-picker-arrow-keys.png): Automatic (selected, tinted) and origin/feature/audit lit, as the reference; [↑ then Return](https://raw.githubusercontent.com/ccheever/exact2/c1d3488ce73900b2d3a60aa9fba757e98ba43f0a/diff-panel-parity/pa6-picker-return-picks.png): "Comparing feature/audit against main" in both apps. Agent tree on `1c41d6434`: "rel" ↓ Return gives origin/release/2026-10; "zzz" Return keeps the base and empties the popover; Automatic restores origin/main |
| PA-6 long base (review) | pass | [clipped](https://raw.githubusercontent.com/ccheever/exact2/f8ac03c156dbfa3dee58f968befb245405b0aefc/diff-panel-parity/pa6-long-base-clipped.png): "→origin/release/2026-" clipped beside "+2.4k -1", as the reference; before the fix it drew over the stats |
| PA-12 stale focus (review) | pass | unit test (no visible change): another thread, a diff without the draft's file and a collapsed file each give Send back `Meta+Enter`; a card mounted again and focused takes it. Live regression on `1c41d6434`: `send-message` keys `''` with the draft open, `Meta+Enter Meta+Alt+Enter` after ⌘↩ saved it |
| PA-6 stale refs (review) | pass | unit test: two reads at `/a` then `/b`, `/a` answering last; the picker keeps `/b`'s refs |

Known residual (framework, declared): the search field draws the native focus ring around it (X61, #302) where the
reference draws only the underline.

## Tests

`diff-panel-parity.test.ts` (9): base-ref choices and filter, the picker rows (switch / remote only / no match / other
cwd), the comparison through `T3Client.command` (listRefs payloads, the pick's `baseRef`, Uncommitted keeps it,
Automatic), only the newest ref read landing (review), Latest turn with no turns, header counts both ways and the
compact total, the Send chords while a draft holds the focus (blur, closed panel, save), and a draft that left the tree
without a blur (review: another thread, a diff without its file, its file collapsed). The picker's keys are Contract
(`dbStep`, no unit evaluator for a `fn`): proven by the agent drive.

Checks on `1c41d6434` (review round; the feature branch merged at `3b334f704`; later commits change records only):
`bun test examples/t3-code --timeout 60000` exit 0 (3,700 pass, 1 skip, 0 fail, 265 files); strict `tsc` on `app.ts`
exit 0; `contract build examples/t3-code/app.contract` exit 0 (5,780 slots, 46 resources, 90,284 nodes;
`app.contract` 1,234 lines); `git add -A && bun scripts/caps.mjs` exit 0; the five checks: `cargo build --all-targets
--keep-going` 0, `cargo test --lib --bins --tests --no-fail-fast` 0 (3,521 pass, 0 fail, 34 ignored), `cargo clippy
--all-targets --keep-going -- -D warnings` 0, `cargo fmt --all -- --check` 0, `bun scripts/caps.mjs` 0,
`bun scripts/boot.mjs` 0. No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries were
not run. Bundle: `bun host/apple/build.mjs t3-code-macos --bundle` before the drive and once more for the one retry
(the first drive found the long base's overlap); two live agent drives in all. The first round's checks on `35d40ee97`
were all exit 0 as well (3,655 Bun tests).
The feature branch moved while those ran (#355, #359, #365; `EXACT2-GAPS.md` and `r4-surfaces-files.contract` touched
by both sides, no conflict), so it was merged again (`a07d41b1f`) and the checks a merge can change ran again there:
`bun test examples/t3-code --timeout 60000` 0 (3,714 pass, 1 skip, 0 fail, 265 files), strict `tsc` 0, `contract build`
0 (5,784 slots, 90,324 nodes; `app.contract` 1,234 lines), `git add -A && bun scripts/caps.mjs` 0. This PR changes no
Rust or Swift, so the cargo checks and boot stand from `1c41d6434`.

## Real-input batch steps

PA-12, real ⌘Enter, and the picker's keys in the same session (screen unlocked; lane `diff-panel-parity`, build of this
branch). `A` = `/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit`, `L` = `$A/lanes/diff-panel-parity`.
1. From `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-diff-panel-parity`, with the lane's isolation as
   `$A/clone-drive.sh` sets it: `PATH=$HOME/.bun-1.4.2/bin:$PATH EXACT_APP_DIR=$PWD/examples/t3-code
   T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16842 T3CODE_TELEMETRY_ENABLED=false
   T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 CODEX_HOME=$L/codex
   CLAUDE_CONFIG_DIR=$L/claude XDG_CONFIG_HOME=$L/xdg/config XDG_DATA_HOME=$L/xdg/data XDG_STATE_HOME=$L/xdg/state
   XDG_CACHE_HOME=$L/xdg/cache bun host/apple/build.mjs t3-code-macos --bundle --run`.
2. Open "Audit work thread", press Changes in the details card, expand `src/app.ts`, hover line 7 and click its "+".
3. Type `audit note` and press ⌘Return with real keys. Pass: the card "audit note" shows under line 7 and the composer
   gets the "app.ts L7" chip; nothing is sent (the lane's clone home has its providers switched off).
4. Click the header's "origin/main" trigger, press ↓ ↓ ↑ Return with real keys. Pass: the header reads
   "feature/audit → main". Reopen it and click Automatic to restore origin/main.

## Next action

Coordinator: answer "Decision needed" (the query's keyboard), run the real-key rows in the next batch, review draft PR
(link in the frontmatter), merge.
