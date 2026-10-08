---
name: 20261008-adopt-main-fixes-r6
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: passed
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r6'
pr_url: https://github.com/ccheever/exact2/pull/297
verified_commit: null
---

# The clone adopts main's fixes, round 6 (main `e200397ec`; no issue of ours closed, X61 found)

## Outcome

The task branch records main `1f19b2400` in its ancestry (round 5 was squash-merged) and merges main `e200397ec`
(367 commits ahead of the feature branch), both as merge commits. **The PR must be merged with a merge commit, not
squashed**, or main's ancestry is lost again; if it is squashed, the next round repeats the `-s ours` step with
`e200397ec`. Since `1f19b2400` main closed none of the plan's issues (round 5 already adopted #234), so no clone
workaround could be removed. Main's own changes needed one clone fix: its native text fields (LLP 1104) dropped the
field sheet, and seven tall bare fields would have drawn their text 3–7 pt above centre; each now pads its content box
to its line. Main's focus ring now draws on `appearance="none"` textareas too, which the clone cannot opt out of: filed
locally as X61 (X57–X60 were taken by #289, #265, #294 and an open branch while this ran). Two partial steps on main (a virtualized list's `scroll-padding`, an indeterminate `progress`) leave
nothing to adopt. Main moved on to `3adf67106` (4 commits, none for our issues) while this ran; the next round takes it.

## Scope and exclusions

| Item | Plan id | Main change | Clone sites |
| --- | --- | --- | --- |
| Record main `1f19b2400` | — | `git merge -s ours` | none (framework files matched, see Dependencies) |
| Merge main `e200397ec` | — | merge commit | `Cargo.toml` member line, `QUEUE.md`, `docs/agent-pitfalls.md` kept both sides |
| Native text fields drop the field sheet | — (LLP 1104) | `508579727`, `5b2b77339` | 7 fields in `legacy-sidebar`, `pages-pr-edit`, `r4-git-publish` (2), `settings-b-actions` (2), `timeline-plan` |
| Focus ring with no opt-out | X61 (new) | `5b2b77339` | `settings-prompt-preview.contract`, `composer.contract` (no edit) |
| `scroll-padding` on a virtualized list | X23 (#277) | `2faf6c190`, `d6ded2e7d` | none (checked) |
| Indeterminate `progress` | X49 (#279) | `d82c12252` | none (checked) |
| Every other issue of ours | X9, X11, X17, X19, X21, X22, X25, X26, X28, X31, … | none | none (re-checked) |

Excluded: framework changes (the clone never patches the framework; X61 is reported, not filed), main's commits after
`e200397ec` (`3adf67106`: first-frame confirmation, a QUEUE line, one layout compute pass), and a clone workaround for X61.

## Context and guidance

Brief: the coordinator's round-6 adoption task (2026-10-08) and its common brief. Main's log since `1f19b2400` names only
#214 and #253 (neither ours); main's merged PRs since round 5 are #240 (adopted), #253 and #214. Every upstream issue the
records cite was read with `gh` (state, and comments since round 5: none), as were our open drafts #227 and #228. The
filing agent's new issues are cited as they appeared: #266–#277 (the rests of closed issues), #279 (X49), #280 (X52),
#281 (X51), #282 (X53), #283 (X54), #284 (X50), #285 (the `clock +N real` QUEUE line). Earlier rounds:
[adopt-main-fixes-r5](closed/20261007-adopt-main-fixes-r5.md) (#236), [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md) (#218).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| ancestry | main `1f19b2400` (round 5 was squash-merged) | `1f19b2400` | the branch's framework files equal `1f19b2400`'s apart from the clone's registration lines and the QUEUE/pitfalls entries; then `git merge -s ours` | `git diff 1f19b2400 07dcef1ab -- . ':(exclude)examples/t3-code'`: `.gitignore` (the clone's 9 lines), `Cargo.lock` (the `t3-code-macos` entry), `Cargo.toml` (the member), `QUEUE.md` (1 entry), `docs/agent-pitfalls.md` (3 entries), nothing else; `53358e0a9` |
| merged main | origin/main into the task branch | `e200397ec` | merge commit | `74114cde1`: `Cargo.toml` takes main's members line plus `examples/t3-code/macos`; `QUEUE.md` and `docs/agent-pitfalls.md` keep both sides (the `clock +N real` entry stays: main's `scripts/agent.mjs` still computes `to` without `s.now`; it is #285 now). Afterwards the only framework difference from `e200397ec` is the same five files |
| feature branch | `feat(example)/t3-code` | `d3df2c426` (#265, #288, #289), then `c0475fbaa` (#295, #294) | merged before the PR, and again when it moved | merge commits `c5641a15e` and the next: the issues README, `EXACT2-GAPS.md`, `STATUS.md` and the X23 and X49 files keep both sides (#295 filed the rests and X49 as #277 and #279 meanwhile). The other tasks took X57 (#289), X58 (#265), X59 (#294) and X60 (an open branch), so this task's draft (X57, the coordinator's "next free number" when it started) is **X61**; images 02, 03, 05 and record 06 were re-uploaded with it. No field the merged tasks added relies on the field sheet; the clone checks were run again on the merges (Attempts 3) |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| Native text fields (LLP 1104, main `508579727`): the field sheet is gone | **fixed in the clone** (`f920f9cab`) | Main's r4 sheet put 6 pt top and bottom padding (and a fill, border, radius, ink, disabled opacity 0.5 and the `fieldStyle` ring mark) under every field's own rows. Main now makes a field native unless a background, border or radius row devolves it, and the bare box gets no sheet. A script over every `input`/`textarea` node and its classes found no clone field that turns native (each has a background or border row, is outside D1's types, or is `appearance="none"`), none that relied on the sheet's fill, border, radius or ink, and nine that set no vertical padding. A bare macOS field draws its line at the top of its content box (`NSTextFieldCell` top-aligns a tall title rect; ExactKit centres only native fields), so text in a taller content box sits high: probe A measured 0.0 pt off centre at 20 pt, −3.0 at 26, −5.0 at 30, −7.0 at 34. Seven fields would have moved 3–7 pt up; each now pads its content box to its 1.25rem line, as the clone's other fields do: legacy project title, pull request title, git publish repository and remote, project action name and preview URL, the plan's workspace path. The two thread rename fields (22 pt) had an 8 pt content box under the sheet (probe A: the line is clipped there) and a 20 pt one now, centred with no change |
| Disabled fields lost the sheet's opacity 0.5 | **no change** | Probe B: AppKit draws a disabled bare field's authored text lighter on its own (darkest 0.155 → 0.413, about 0.69 opacity on white). The base multiplied that by 0.5; now it is AppKit's alone, nearer the reference's 0.64 (`has-disabled:opacity-64`, which also dims the control's border; the clone dims the text only, before and after) |
| X61 (new): Exact's focus ring cannot be removed | **reported** (local draft, not filed); no workaround | Main `5b2b77339` draws the ring on every focused bare field and textarea, `appearance="none"` included; Contract has no `outline` (`contract vocab outline`). The Appearance prompt preview used `appearance="none"` to have no ring and now has one (image 02). The composer had the same ring on the base already (r4's sheet ring; image 03), where the reference shows none. A one-file app reproduces it (image 05). A module-side workaround would hide an ExactKit layer by its private name, so it waits for X61 |
| Native buttons (LLP 1104 step 3) | **nothing to do** | A `button` is native only with a literal `appearance="auto"` (`controls.rs` `native_button`: no attribute is bare); the clone writes none |
| `fetch` bodies (main `cfb4931da`, `4865dcd5c`) | **nothing to do** | A body on a GET or HEAD now rejects; the clone's only data-module `fetch` (`r5-composer-paging.ts`) is a GET with no body |
| X23 (#138 closed, rest #277) | **nothing to adopt** | Main `2faf6c190`/`d6ded2e7d`: `scroll-padding-*` on a `list virtualized=true` only; on a `scroll` it is refused (`lower-scroll-padding`, checked with a scratch build). The reference's scroll-padding sites are not virtualized lists in the clone. `R9Input.swift` and `T3TimelineTurns.swift` stay (X23a, X23c, and X22 for the turns in view) |
| X49 (#279, filed 2026-10-08) | **nothing to adopt** | Main `d82c12252` adds `progress` as the indeterminate activity indicator; `value` and `max` are refused ("Exact does not draw yet", checked with a scratch build), and `aria-valuenow` is not carried. The drawn bar stays |
| #224 (X28 rest) | **kept** (open) | No notification action or badge on main; `T3Notifications.swift` keeps the badge, click and sounds |
| #225 (X11 rest; closed by #232 in round 5) | **kept** | No macOS backdrop change since `1f19b2400`; cross-parent sampling is untracked; adopting `saturate()` is still the user's decision from round 5 |
| #140 (X25) | **kept** (open) | No capture-phase handler or held-modifier state on main |
| #141 (X26) and #235 | **kept** (both open) | `MenusMac.swift` changed only for native-button invokers (`commandfor` show/toggle, D13 anchors, subtitles); no app-declared menu bar, no Shift+F10 / context-menu key. The round-5 finding stands: context and button menus still pop up inside `DispatchQueue.main.async` (`MenusMac.swift:174, 208`) |
| #117 (X31) | **kept** (open) | No first-window hold; U5's connecting state stays (provisional, as before) |
| #108 (X9) | **kept** (open) | No resources in child components; `app.contract` stays the owner |
| #124 (X19), #126 (X21) | **kept** (open; our drafts #227 and #228 still open) | The prelude changed only for `fetch` byte bodies; no timers or WebSocket send in data modules, and no other route |
| #112, #116, #127, #130, #131, #266–#276 | **kept** (open) | Nothing on main covers them |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Result | Proof |
| --- | --- | --- | --- | --- | --- | --- |
| Ancestry recorded | — | `git merge -s ours 1f19b2400` after the framework diff | no file changes; the diff is only the clone's registration lines and entries | — | pass | Dependencies; `53358e0a9` |
| Main merged | — | `git merge origin/main` | merge commit; framework files are main's | — | pass | `74114cde1` |
| Clone builds on the merged base before own changes | — | the clone checks | green | macOS | pass | Attempts 1 |
| Each issue checked | — | `gh issue view`, main's log and diff | adopt or keep with the open issue named | — | pass | Decisions per issue |
| Clone fixes for what main broke | — | probe A; contract build; live session | tall fields keep centred text | macOS | pass | `f920f9cab`; image 01; record 06 |
| Live macOS session | lane: staged T3 release on 127.0.0.1:16210, project demo; fresh agent storage per build | pair through the wizard, composer focus, details card, Publish repository, Settings › Project › Add action, Settings › Appearance | the merged base behaves as the base; pairs where the UI changes | macOS 1280×840 | pass | images 01–04, record 06 |
| Records | — | — | task record, issues README and upstream table, X23, X49, X61, `EXACT2-GAPS.md`, `STATUS.md` | — | pass | this PR |
| Checks | — | clone checks, AppKit binaries, the five checks | green | macOS | see Attempts | Attempts 2 |

## Progress

Implemented and verified 2026-10-08; draft PR [#297](https://github.com/ccheever/exact2/pull/297). One live session per build in agent mode (the coordinator's override: the screen is
unlocked; no real input was needed, so the real-input lock was not taken). No user decision was taken provisionally.
The task stays under `tasks/` for the coordinator's records sync; the PR stays a draft.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (merged base, no clone change) | `74114cde1` | `bun test examples/t3-code` 3036 pass / 1 skip / 0 fail (3037 tests, 238 files); a first run under the parallel cold `cargo` build had 2 timeouts in `tools/github-lane/lane.test.ts` (5 s), which pass alone (14/0) and in the rerun. Strict `tsc` (ES2023) clean. `contract build` 3844 slots, 20 derives, 46 resources, 74,797 nodes. `cargo test -p t3-code-macos --lib` 11 pass. `bun host/apple/build.mjs t3-code-macos --bundle` builds with no shim | `target/r6/*.log` (not committed) | the field padding (fixed in attempt 2) |
| 2 (final) | `f920f9cab` + records | see Checks below. Live: before `07dcef1ab` 04:40–04:42 UTC, after 04:42–04:43 UTC, the same ops; op errors 1 before (a toast already gone) and 0 after; app logs flag no error, refusal or failure in either | images 01–05, record 06 | none |
| 3 (feature branch merged) | `c5641a15e` (`d3df2c426` merged), then the merge of `c0475fbaa` | On `c5641a15e`: `bun test examples/t3-code` 3048 pass / 1 skip / 0 fail (3049 tests, 239 files); strict `tsc` clean; `contract build` 3845 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11 pass; caps within every budget. On the merge of `c0475fbaa` (#294 and #295 change no Swift and add no field): `bun test` 3049 pass / 1 skip / 0 fail (3050 tests); strict `tsc` clean; `contract build` 3860 slots, 20 derives, 46 resources, 75,017 nodes; `cargo test -p t3-code-macos --lib` 11 pass; caps within every budget. AppKit binaries on `c5641a15e`: 33 binaries, 343 XCTest tests, 0 failures, 1 skipped; `snapshot` aborted once at its "actual current layout translates printable letter" precondition (the input source changed under it: the real-input batch shares this Mac) and passed all 12 checks when re-run alone; `mermaid` 9 render failures without `T3_SERVER` (the lane was stopped; it passed with the lane in attempt 2). The five cargo checks were not re-run: the merge touched only `examples/t3-code`, outside `default-members` | `target/r6/appkit/*.log` (not committed) | none |

Checks (final source, `f920f9cab` + records): `bun test examples/t3-code` 3036 pass / 1 skip / 0 fail (3037 tests, 238 files); strict `tsc` (ES2023) clean; `contract build examples/t3-code/app.contract` 3844 slots, 20 derives, 46 resources, 74,797 nodes; `cargo test -p t3-code-macos --lib` 11 pass; the macOS bundle builds with no shim. AppKit binaries (README recipe, `target/r6/appkit-tests.sh`): 33 binaries, 343 XCTest tests, 0 failures, 1 skipped (`ssh` live); `snapshot` 12 checks pass; `mermaid` fails its 9 render checks without `T3_SERVER` and passes all 10 with `T3_SERVER=http://127.0.0.1:16210` (the lane's staged release serves its web bundle); `timeline-keyboard` (its own recipe) not run. Five checks: `cargo build --all-targets --keep-going` ok; `cargo test --lib --bins --tests --no-fail-fast` 3521 passed, 0 failed, 34 ignored (94 test binaries); `cargo clippy --all-targets --keep-going -- -D warnings` ok; `cargo fmt --all -- --check` ok; `git add -A && bun scripts/caps.mjs` all budgets within cap; `bun scripts/boot.mjs` ok.

## Evidence

| Scenario | Before (`07dcef1ab`) | After |
| --- | --- | --- |
| Add Action dialog, Name and Preview URL filled (image 01) | text centred (the sheet's 6 pt padding) | text centred (the branch's padding; without it 5 pt high, probe A) |
| Settings › Appearance, prompt preview focused (image 02) | no ring (`appearance="none"`) | a 2 pt blue ring (main `5b2b77339`, X61) |
| Composer focused (image 03) | 2 pt blue ring (r4's sheet ring) | the same ring (main's ring layer; 256 pixels differ along it); unchanged by the merge |
| Main window after pairing, project demo (image 04) | as below | identical (no pixel differs by more than 16 of 255) |
| X61 repro, after only (image 05) | — | a one-file app on main `e200397ec`: bare input, bare textarea and `appearance="none"` textarea each ringed when focused |

- `https://raw.githubusercontent.com/ccheever/exact2/2df397a51fab848a1131cf9777e774bee7cc48fd/adopt-main-fixes-r6/01-action-dialog-fields-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/cd188e76817234afac0b9a27815f9e5aa9f11f3a/adopt-main-fixes-r6/02-prompt-preview-focus-ring-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/07f6bb6623c2ae2eec5a97d5ace76f25b37be75a/adopt-main-fixes-r6/03-composer-focus-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/37965abc1cdb14cf26ea1ec42de1ff861caed168/adopt-main-fixes-r6/04-main-window-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/973b62c5dd38738397b1d6ed81614d0426d0ada1/adopt-main-fixes-r6/05-x61-ring-probe-after-only.png`
- Session record (lane, pids, ops, probes A and B, the X61 repro): `https://raw.githubusercontent.com/ccheever/exact2/6a28e79c68a9c8e1243e51d46741e500bafbb573/adopt-main-fixes-r6/06-session-record.txt`

Findings for follow-ups (not this task):
- The details card's Publish repository wizard cannot reach its Repository step in a lane with no Git provider account
  (every provider shows Setup Required), so the two git-publish fields' padding is proven by probe A and the build, not
  by a screenshot.
- `stage-runtime.mjs`'s staging smoke takes the first free port in 16900–16999 whatever lane the caller has.
- The keyboard hint under Add Action's Keybinding wraps after "UseBackspace" with no space, on both builds.
- The after build's shot of the publish dialog (not used) shows Forgejo's Setup Required hint, which opens on hover.
  The agent's taps were not over that card; the real-input batch was moving the real cursor on the same screen at the
  time, so the shot is not compared.

## Real-input batch steps

None: every row of this task ran in agent mode.

## Next action

Review the PR (merge commit, not squash). Not done, each with its blocker:
- **The prompt preview's and the composer's focus ring** (and the ring on the clone's other bare fields where the
  reference draws its own focus look). Blocker: X61 (local draft; filing is the coordinator's).
- **Every workaround the plan keeps** (X9, X11, X19, X21, X22, X23, X25, X26, X27, X28, X31, …): their issues are open
  on main `e200397ec` (Decisions per issue).
- **Main `3adf67106`** (4 commits after `e200397ec`, none for our issues): the next round.
