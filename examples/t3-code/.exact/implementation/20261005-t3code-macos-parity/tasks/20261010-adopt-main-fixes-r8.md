---
name: 20261010-adopt-main-fixes-r8
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r8'
pr_url: null
verified_commit: 922ba1b5bf7743f86ffcae638be723aed9298275
---

# Main adoption, round 8: merge current main into the T3 branch

## Outcome

`feat(example)/t3-code` takes current `main`. Round 7 ([adopt-main-fixes-r7](closed/20261010-adopt-main-fixes-r7.md),
#384) adopted main `bc357d03c`; outside `examples/t3-code/` the branch's tree equals that commit except the workspace
registration in `Cargo.toml` and `Cargo.lock`. Main has 22 commits after it (`bc357d03c..1f127a788` when this record was
written; take main's tip at merge time). `git rev-list` reports hundreds more because #384 was squash-merged, which
drops main from the branch's ancestry; the content is already here.

#327 (bucket 2) is still open. Its retirements wait for round 9.

## Steps

1. Record the last adopted main without its content: `git merge -s ours bc357d03c`, then `git merge origin/main`.
   Keep the T3 side for `examples/t3-code/` and main's side for framework files. After the merge,
   `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
2. Check what the clone relied on in main's new commits, and record each with its outcome:
   - "A tap that names a node presses that node" (LLP 1012 §1, `a09f48a5e`, `e5d9b46f4`, `abc1eadff`): the agent's
     `tap <target>` now presses the target or refuses when its middle holds another control. Run the clone's agent-driven
     tests and one drive. A `tap` that is now refused moves to the named control or to `tap <target> at <x> <y>`.
   - A heading with no `aria-level` is level 2 (`f9dcdb1af`): check the clone's headings in the macOS accessibility tree
     and any test that reads a level.
   - LLP 1041 six workers for independent HTTP (`94dfb2332`, `1b31848f5`): note whether the clone's HTTP reads change.
     Nothing to change unless a test or the PR panel's one-at-a-time reads depend on the old bound.
   - Canvas host passes (`0685da1f0` … `5ee9ce1a0`, runner `collection_shown`): check whether the clone uses a
     collection that the runner change reaches. If so, the drive covers scrolling it.
   - Docs (`docs/start-here.md`, LLP 1115 D7) and `rules/`: re-read `rules/RULES.md` and `rules/DEFERRED.md` if they
     changed. Record any rule that changes the clone.
3. Main's `issues/` now holds the clone's framework gaps (#386, #394). Nothing to copy; `P/issues/README.md` already
   points there.

## Acceptance

| Row | How to verify |
| --- | --- |
| The branch contains main | `git merge-base --is-ancestor origin/main HEAD` after the merge (at the merge time) |
| The framework tree is main's | the `git diff` of step 1 is empty |
| The five checks pass on the merged tree | CLAUDE.md's five checks once on the final head |
| The clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries; one live agent drive of the main surfaces (shell, thread, composer, right panel, Settings) with before/after screenshots |
| Each item of step 2 has an outcome | the record's table lists each with its commit, or "nothing to change" and why |

## Adopted main

**`d413487a8`** (main's tip at the final merge, 26 commits past round 7's `bc357d03c`):

- `b027ae773`: `git merge -s ours bc357d03c` (round 7's squash-merged main; its content was already here).
- `c595a36b2`: `git merge origin/main` at `1f127a788`, the 22 commits this record names. One conflict, `Cargo.toml`:
  main's members (it added `apps/shelf/web` and `apps/shelf/apple`) plus `examples/t3-code/macos`. `Cargo.lock` merged
  clean; its difference from main is the `t3-code-macos` package entry only.
- `360e7132a`: the clone's one change (headings, below).
- `85a1748e0`: main moved three commits while the round ran (`1a71f1cfb`, `2e913b294`, `a10050516`: VideoArm reads its
  position from the item's timebase), merged before the final checks.
- `922ba1b5b`: the base `5e3253cfc` (#399 realinput-1010c-fixes and the right-panel-escape record), merged before the
  final checks.
- `cdfd670ff`, `308668781`: after the checks and the records commit, main `d413487a8` (#401: the X73 issue file only)
  and the base `a1ade42f9` (X73 moved to main: plan records and one `EXACT2-GAPS.md` line) landed; both are Markdown
  only and merged clean. On `308668781` the Bun suite (4,360 pass, 0 fail) and caps ran again.

On `308668781`: `git merge-base --is-ancestor origin/main HEAD` yes (origin/main `d413487a8`), and so for the base
(`a1ade42f9`); `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` empty. This PR is
squash-merged like the others, so round 9 starts with `git merge -s ours d413487a8`.

## Each item of step 2

| Item | Main commit | Outcome | Clone change | Commit |
| --- | --- | --- | --- | --- |
| A tap that names a node presses that node (LLP 1012 §1) | `a09f48a5e`, `e5d9b46f4`, `abc1eadff` | On the Mac a plain named click, and `tap … hover`, goes through `Agent.addressedAim` (`AgentAddressedTap.swift`): the node's own press at its middle, or beside a control its middle holds (`avoided`), or refused. The clone's three agent-run Contract tests pass on the after build: `sidebar-palette-keys` 5/5, `menu-one-highlight` 5/5 (its `tap … hover` rows included), `browser-launcher-chevron` 3/3. The drive's 12 taps and one `type` answer as they did on the old agent: same node, same point, `delivery: platform`, no `avoided`, no refusal | nothing to move: no tap is refused | — |
| A heading with no `aria-level` is level 2 | `f9dcdb1af` (contract-lower says 2), `cf7196c0d` (`TextInteraction.swift` `headingLevel`: `role="heading"` alone is AXHeading level 2; until now it was AXStaticText) | 160 of the clone's 162 headings wrote a level. R7DevSection's title is DeviceToolsPanel.tsx `Section`'s `<h3>`: now `aria-level=3`. "Import from t3.json" is ProjectActionsSettings.tsx's `MenuGroupLabel`, which Base UI renders `role="presentation"` (checked on the running reference: `<div role="presentation" data-slot="menu-label">`): now `role="presentation"`, so it stays AXStaticText (main alone would have made it a level-2 heading). `hover-layer.test.ts` and `dialog-focus.test.ts` read written levels: unchanged. Drive: the shell's AX headings, New thread (2) and the Nightly toast (2), are the same in both builds; the import menu's label is `text` in both | `r7-device.contract`, `settings-projects.contract`; `headings.test.ts` holds every heading to a level (fails on the merged tree before the change) | `360e7132a` |
| LLP 1041 six workers for independent HTTP | `94dfb2332`, `1b31848f5` | Only a request marked `exactIndependentHttp` uses the independent owners. The clone marks none and makes no HTTP `fetch` of its own (its reads are native-module calls on the ordered lane), so the PR panel's one-at-a-time reads and the 16-ticket bound are unchanged. Each executor now starts six idle independent owners instead of two (process bound 112) | nothing | — |
| Canvas host passes, runner `collection_shown` | `0685da1f0`, `1425ab8a7`, `27ad05cce`, `c811edad8`, `7e785faf1`, `5ee9ce1a0` (QUEUE `a09efff61`, `a60419648`) | `CollectionFill.lean` (feedback flag bit 3) and `Runner::collection_shown` are used only by `host/linux`. The Apple host's report encodes bits 0–2 (`host/apple/Sources/ExactKit/Collection.swift:42`) and never calls `collection_shown`, so `lead()` keeps its old window for the clone's lists (the transcript, diff, PR code, legacy sidebar, connections). The drive still wheels the transcript: its content is 345.9 pt in a 788 pt port, nothing scrolls, the same in both builds | nothing | — |
| Docs and rules | `37cb9c504`, `a88a90cd6`, `ee564ac6f`, `8a32c4b10`, `cf7196c0d`, `fdfb97276` | `rules/` is unchanged since `bc357d03c`. AGENTS.md makes `docs/start-here.md` the one required read for building an app (LLP 1115 D7) and the long guides lookup; `docs/agent-pitfalls.md` gains the tap entry, the simulator choice and the Shelf recipe's iOS entries. No rule changes the clone. `scripts/no-tells.mjs` lists the literal colours and sizes of an app that should leave them to the platform; the clone writes the reference's tokens on purpose (a parity copy of a web app; author > platform), so it is not applied | nothing | — |
| Issues | `e281d83e9` (#386), `1f127a788` (#394), `d413487a8` (#401) | X69, X70, X71, the backdrop beyond the parent, X72 and X73 live in main's `issues/`; nothing to copy | nothing | — |
| `apps/shelf` (LLP 1115 wave 2) | `cf7196c0d` | two new workspace members | `Cargo.toml` keeps `examples/t3-code/macos` | `c595a36b2` |
| VideoArm reads the item's timebase | `1a71f1cfb`, `2e913b294`, `a10050516` | merged last; the clone's video previews (`r6-media-video`, media markdown) play through it; the bundle of `922ba1b5b` builds; not driven (the drive plays no video) | nothing | `85a1748e0` |
| #327 (bucket 2) | open | nothing: round 9 | — | — |

## Found while checking (for the coordinator)

- Settings › General: the reference's section and row titles are headings (its ARIA snapshot: "New threads" level 2;
  "Model", "Permissions", "Workspace", "Submodules" level 3); the clone's are plain text. The Settings AX tree has 282
  elements and no heading in either build, so main's change did not cause it. Not changed in this adoption round: a
  later task can give `settings-core.contract`'s section and row titles `role="heading"` and those levels (the Project
  page's rows already say `aria-level=3`).

## Acceptance results

Evidence: [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/daa80409c7b971f288a317237c10fdea09c296cd/adopt-main-fixes-r8/evidence.txt)
(§1 merge, §2 main's changes, §3 the heading guard, §4 Contract tests, §5 the drive, §6 checks);
[drive.sh](https://raw.githubusercontent.com/ccheever/exact2/a9f6b1949fed74ab5a64261a6629182005990c80/adopt-main-fixes-r8/drive.sh.txt).

| Row | Result | Proof |
| --- | --- | --- |
| The branch contains main | pass: `git merge-base --is-ancestor origin/main HEAD` on `922ba1b5b` with origin/main `a10050516` (`1f127a788` at the first merge), and on `308668781` with `d413487a8` | evidence §1; Adopted main |
| The framework tree is main's | pass: the step 1 diff is empty on `c595a36b2`, `922ba1b5b` and `308668781` | evidence §1; Adopted main |
| The five checks pass on the merged tree | pass on `922ba1b5b`: build, test (3,679 passed, 0 failed, 34 ignored, 95 suites), clippy, fmt, boot: all exit 0 | evidence §6 |
| The clone works | pass: Bun 4,360 pass / 1 skip / 0 fail; strict tsc; contract build; `t3-code-macos` lib 17; AppKit 37 of 37 (mermaid against a scratch T3 server) and `timeline-keyboard`; the live drive of shell, thread, composer, right panel, Settings and Settings › Project: screenshots pixel-identical before and after, trees identical apart from lane ids and the import label's role, no refusal or error in the logs | [shell](https://raw.githubusercontent.com/ccheever/exact2/94922100c7bf17de5b3b3254ac5ff1a0ef013d0b/adopt-main-fixes-r8/1-shell.png), [thread](https://raw.githubusercontent.com/ccheever/exact2/f3b75339268aa85327c49845f0234f522a105981/adopt-main-fixes-r8/2-thread.png), [composer](https://raw.githubusercontent.com/ccheever/exact2/f952a22c8e39efeea149331f6532743fb7000069/adopt-main-fixes-r8/4-composer.png), [right panel](https://raw.githubusercontent.com/ccheever/exact2/9a50bddbede0cd1c45d083b252caea7c650f774e/adopt-main-fixes-r8/5-right-panel.png), [Settings](https://raw.githubusercontent.com/ccheever/exact2/ccf04073f870b58112186b4a83920e2c196e3e3f/adopt-main-fixes-r8/6-settings.png), [Import scripts](https://raw.githubusercontent.com/ccheever/exact2/c97241860a4fb4305d0538dfde3d94c209932a92/adopt-main-fixes-r8/8-import-menu.png); evidence §5, §6 |
| Each item of step 2 has an outcome | pass: the table above | this record |
| Every heading says its level (main's default 2) | pass: `headings.test.ts` fails on `c595a36b2` (`r7-device.contract:181`, `settings-projects.contract:286`) and passes on `360e7132a`; the drive's tree shows the import label `presentation` | evidence §3, §5 |

## Tests

- `headings.test.ts` (new): every `role="heading"` in the clone's `.contract` files writes `aria-level`; the device tools
  section title is level 3 and "Import from t3.json" is `role="presentation"`.
- Agent-run Contract tests on the after build (main's new agent): `sidebar-palette-keys.test.contract` 5,
  `menu-one-highlight.test.contract` 5, `browser-launcher-chevron.test.contract` 3; all pass.

Final checks on `922ba1b5b` (the record follows): `bun test examples/t3-code --timeout 60000` 4,360 pass, 1 skip, 0 fail
(296 files), exit 0; strict `tsc` exit 0; `bun scripts/exact.mjs contract build examples/t3-code/app.contract` exit 0
(5,980 slots, 110,235 nodes); `cargo test -p t3-code-macos --lib` 17 pass; the bundle builds; the AppKit binaries (README
recipe): 36 exit 0 on the first run, `mermaid` failed to load Mermaid without `T3_SERVER` as in earlier rounds and passed
against a scratch `t3 --mode web` server on the lane's spare port 16583 (stopped after), `timeline-keyboard` passes;
`git add -A && bun scripts/caps.mjs` exit 0; `cargo build --all-targets --keep-going` exit 0; `cargo test --lib --bins
--tests --no-fail-fast` exit 0 (3,679 passed, 0 failed, 34 ignored, 95 suites); `cargo clippy --all-targets --keep-going
-- -D warnings` exit 0; `cargo fmt --all -- --check` exit 0; `bun scripts/boot.mjs` exit 0. `app.contract`: 1,341 lines.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | the evidence-base worktree `c03d7e908` (main `bc357d03c`'s host and agent), lane `adopt-main-fixes-r8-before` | the first try stopped at a second `tap toggle-right-panel` (the open panel's toggle is `panel-toggle-right`); the step was fixed and the drive rerun to the end | first column of each image |
| Contract tests | the bundle of `360e7132a`'s sources, lane `adopt-main-fixes-r8` (16580) | 13 of 13 | evidence §4 |
| After drive (the live drive) | the same bundle, lane `adopt-main-fixes-r8` | one try, completed | second column of each image; evidence §5 |
| Reference | `ref-app.sh adopt-main-fixes-r8 16580`, the same provider settings | shots of the same states (its transcript, like the clone's, shows no rows under the provider error at this size); stopped | third column of each image |
| Final bundle | `922ba1b5b` (main `a10050516` and base `5e3253cfc` merged) | builds, exit 0; not driven again (one live drive): the later merges add VideoArm's timebase read and #399's clone fixes, which #399 verified | evidence §6 |

No row needs real input.

## Next action

The coordinator reviews the draft PR, syncs `STATUS.md` and `plan.md` (round 8 done, main `d413487a8` adopted), merges
it, and decides on the Settings headings follow-up above. Round 9 starts with `git merge -s ours d413487a8` and takes
#327's retirements once it lands.
