---
name: 20261014-adopt-main-fixes-r13
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r13'
pr_url: https://github.com/ccheever/exact2/pull/524
verified_commit: 37540190c
---

# Main adoption, round 13 (X87 fixed by #523)

## Outcome

`feat(example)/t3-code` takes current `main`. Round 12 ([adopt-main-fixes-r12](20261013-adopt-main-fixes-r12.md), #520,
merged 2026-10-13 by fast-forward as `9149492d4`) adopted main `0b956d63d`. Since then main has #523 (`898ca3a09`: a
`ChainingScrollView` keeps a held view only while it is in the window), which fixes X87, the SIGABRT round 12's agent
drive found (a press held on the timeline scroller's empty area, then ⌘1 and ⌘2); and Charlie's work since (Ibex's
consumer adoption, `f31c6e2ba` to `6dd91519c`; the intersection fix for sticky targets, `c016978a6`; and, reached
while this round ran, the date and place at launch, `115c3aada` to `95f2fd3e7`, and the focus fix that closes X54's issue,
`1bcf7436f`). The user decided
on 2026-10-11 that every item below is built now, matching the reference T3 Code (no further decision needed unless the
reference itself is ambiguous).

| Id | Clone (the T3 tip `9149492d4`) | What this round does (reference first) |
| --- | --- | --- |
| AD-1 | The branch lacks main's commits after `0b956d63d` (11 at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge or fast-forward that keeps main's ancestry); main's framework, the clone's `examples/t3-code`; the clone adapted to whatever main renamed, now refuses or now presents differently |
| RT-1 | X87: a press held on the timeline scroller's empty area (`tap transcript down`), then ⌘1 and ⌘2 (`thread.jump`), aborts the app at ⌘1's batch (SIGABRT, `NSInternalInconsistencyException`: "<ExactKit.ChainingScrollView> has reached dealloc but still has a super view"); `EXACT2-GAPS.md`, `STATUS.md` and `plan.md` say "not filed" | The reference never aborts on a press and a thread switch. #523 adopted; round 12's agent trigger driven on the before bundle (aborts) and on this round's (runs on), with readouts; the records say fixed on main by #523, adopted in round 13 |
| RT-2 | Main's log since `0b956d63d` against `EXACT2-GAPS.md`'s rows and the follow-up [r12-kept-workarounds](20261013-r12-kept-workarounds.md) | Retire only the trivial workarounds a merged main fix ends (before/after each); list the rest with why |

## Steps

1. Before: build this branch's bundle at the T3 tip `9149492d4` in this worktree before merging main and drive it once
   (lane `adopt-main-fixes-r13-before`, base port 16850: the clone's server on 16852).
2. `git merge --no-ff origin/main` at main's tip; main's side for the framework, the T3 side for `examples/t3-code/`.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
3. Check what the clone relies on in main's new commits (Contract refusals and renames, `contract build` warnings, the
   scripts the clone's tools call, the Mac host changes) and record each with its outcome ("Adopted main", "Each item").
4. RT-1 against #523's issue (`issues/closed/20261013-macos-a-press-held-on-a.md`) and round 12's X87 readouts; RT-2
   against main's log and the reference source; Bun tests for each source change.
5. `EXACT2-GAPS.md` (X87: fixed on main by #523, adopted in round 13), `STATUS.md` (X87's "Found, not in scope" row,
   the adoption entry), `plan.md`.
6. After: this branch's bundle, the same drive (lane `adopt-main-fixes-r13`); before | after images.
7. Merge the latest `origin/feat(example)/t3-code` (and main again if it moved), then the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 2 is empty | text |
| AD-1: the checks pass on the merged tree | CLAUDE.md's checks once on the final head, the workspace check | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, the macOS bundle; ONE agent-mode smoke drive on the before bundle and on this branch's | before / after screenshots |
| RT-1 (X87) | agent drive, before and after: the fixture thread, `tap transcript down` (the timeline scroller's empty area), ⌘1 and ⌘2 (`thread.jump`), the up, the sidebar's Settings, Back; exits and crash frames read from the drive | before / after text and images |
| RT-1 records | `EXACT2-GAPS.md` X87, `STATUS.md`, `plan.md` | text |
| RT-2 | main's log since `0b956d63d`, commit by commit, against `EXACT2-GAPS.md` and the 13 rows of r12-kept-workarounds: each trivial retirement with a Bun guard, `contract build` and before/after; each kept workaround listed with its reason | text (images where a retirement is visible) |
| Real input | "Real-input batch steps" for the coordinator's next session, if any row needs real input | — |

## Reference

- Method: [adopt-main-fixes-r12](20261013-adopt-main-fixes-r12.md) (Steps, "Adopted main", "Each item", checks, Delivery).
- RT-1: the reference never aborts on a press held over its timeline while a thread switch replaces it
  (`apps/web/src/components/chat/MessagesTimeline.tsx`, the thread routes `routes/_chat.$environmentId.$threadId.tsx`;
  `thread.jump.1` to `.9` in `packages/contracts/src/keybindings.ts`, run by `components/Sidebar.tsx`
  (`threadJumpIndexFromCommand`)). Main:
  `issues/closed/20261013-macos-a-press-held-on-a.md` (#523), `host/apple/Sources/ExactKit/Mac/ChainingScrollView.swift`,
  `ScrollRoutingTests.testAnEmptyScrollPortFreedUnderAPressGoesWithItsNode`; round 12's
  [readouts](https://raw.githubusercontent.com/ccheever/exact2/d3c50deffc9d03407c6d3282e4e48f4bf9275513/adopt-main-fixes-r12/readouts.txt)
  (session 2, the bisect).
- RT-2: main `git log 0b956d63d..origin/main`; `EXACT2-GAPS.md` "Closed on main since round 11" and "Partly fixed";
  [r12-kept-workarounds](20261013-r12-kept-workarounds.md) KW-1 to KW-13 with each row's reference lines.

## Adopted main

**`898ca3a09`**, then **`95f2fd3e7`**, **`2ba30e401`** and **`958523176`** (main's tip at the last merge, 20 commits past
round 12's `0b956d63d`):

- `48485695b`: this record's first version.
- Before any merge: the T3 tip `9149492d4`'s bundle built in this worktree and driven (lane `adopt-main-fixes-r13-before`).
- `62a7c3c1d`: `git merge --no-ff origin/main` at `898ca3a09` (11 commits: #523; Ibex's consumer adoption `f31c6e2ba` to
  `6dd91519c`; the sticky-target intersection fix `c016978a6`). No conflict (`Cargo.lock` merged clean). `contract build`
  exit 0 with round 12's plan: 6,470 slots, 120,810 nodes, 48 resources, 33,309,129 bytes, the same warnings (233
  `lower-colour-one-scheme`, 20 `lower-dialog-role` shown). The compiler is unchanged since round 12 (main touched only
  `contract/cli/tests`), so round 12's uncapped analysis stands. This merge's bundle was the live drive's (after).
- `45f7c5045`: the records (`EXACT2-GAPS.md`, `STATUS.md`, `plan.md`).
- `534b9613c`: `git merge --no-ff origin/main` at `95f2fd3e7`, which main reached after the live drive (5 commits: the date
  and place at launch, `115c3aada` to `95f2fd3e7`: a boot reads the locale, zone and seed from its first settle, and on
  Apple `SessionTime.primeTime` tells the place before each boot; QUEUE). No conflict. A Mac boot change, so the bundle
  was rebuilt and the drive run again on it (after2, the one retry): every screen pixel-equal to the live drive's.
- `30c228b70`: the records name the second merge. The checks ran once on `534b9613c` (all exit 0, Checks); main moved
  while they ran.
- `37540190c`: `git merge --no-ff origin/main` at `2ba30e401` (3 commits: `1bcf7436f`, a wrapper removed by blur still
  hears its `focusout`, which closes X54's issue, with `FocusEvents.swift` on the Mac; `89a5b5e29`, the JS target's
  agent gate, web only; QUEUE). No conflict. Not driven: the clone writes no `focusin=` or `focusout=`, so the Mac change
  has nothing of the clone's to deliver, and the rest is web and QUEUE. The checks, the Bun suite, strict tsc, `contract
  build` and the bundle build ran again on this head.
- `a2fa638b5`: the records (the third merge, X54's rows, this record's results).
- `7f37bd614`: `git merge --no-ff origin/main` at `958523176` (one commit, `QUEUE.md` only), reached after the checks. No
  conflict; nothing to check again.

On the final head after a fresh fetch: `git merge-base --is-ancestor origin/main HEAD`, `git rev-list --count
HEAD..origin/feat(example)/t3-code` and `git diff origin/main HEAD -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
(Checks). The `Cargo.toml` / `Cargo.lock` difference is the `examples/t3-code/macos` member and the `t3-code-macos` package
only. The PR must land as a merge (or a fast-forward of the feature branch to its head), never a squash: a squash drops
main from the branch's ancestry.

## Each item of main's new commits

| Item | Main commit | Outcome | Clone change |
| --- | --- | --- | --- |
| X87: a scroll view pressed on its empty port leaves with its freed parent | `898ca3a09` (#523) | RT-1: round 12's trigger runs on | none (no workaround existed) |
| Intersection: sticky targets sampled where they stick; rotated roots | `c016978a6` | X22's `intersect=` (partly fixed, KW-12): the clone writes no `intersect=` (0 sites), and its one `position="sticky"` row (the pull request summary's section heading, `pages-pr-summary.contract:282`) is observed by nothing, so nothing changes; the `t3-turn` hooks stay (a rework) | none |
| Ibex's consumer adoption: the native executor's transports from upstream Ibex (Apple's native transport stays the default, the async switch stays off), the bounded portable worker pool, `fs.compressImage`'s job ownership, document leases with typed `doc:/` codes, a 16 MiB storage result limit | `f31c6e2ba`, `d26e929aa`, `f3de34746`, `313f51d4c`, `e9a6ef95f`, `2cd37da16`, merges `a7af46999`, `f9eaf7fd6`, `6dd91519c` | No `EXACT2-GAPS.md` row and no clone workaround names any of them: the clone's TypeScript calls no global `fetch`, no `fs.compressImage` and no document picker, and its server traffic is its own native WebSocket module. The drive ran its data module and storage on it (sessions 1 and 2 exit 0, every screen pixel-equal to the before's) | none |
| The date and place at launch (locale, zone, seed before each boot) | `115c3aada`, `5ccb7d797`, `6233c4cc2`, `95f2fd3e7` (second merge) | The clone's first settle reads the real locale and zone; the drive (agent facts `--time-zone Asia/Seoul`, a fixed epoch) is pixel-equal to the first merge's on all 15 screens, the relative ages ("3d", "8d") included | none |
| A wrapper removed by blur still hears its `focusout` (X54's issue closed) | `1bcf7436f` (third merge) | X54 (KW-12): the clone writes no `focusin=` or `focusout=` (0 sites), so nothing it runs changes. Its stand-ins ("Show full comment" as a Tab stop before a long comment's held body, the remark pencil shown while itself focused, `KmDoor` in `menu-keys.contract`) need `focusin` on their wrappers to retire: a rework of the pull request threads and of `menu-keys.contract`'s door, kept (`EXACT2-GAPS.md` X54) | none |
| The JS target's agent gate in lazily copied production web modules | `89a5b5e29` (third merge) | web only | none |
| QUEUE, LLP and review records | `22cf1cd6b`, `2ba30e401`, `958523176` (fourth merge) and the rest | nothing on the clone's path | none |

## Cause and fix

- **RT-1, X87 (#523, adopted).** #523's issue gives the mechanism, the one round 12 bisected: main `3998e650e` (X65) gave
  `ChainingScrollView` a `removeFromSuperview` that refused every removal while its mouse was held, AppKit's own when it
  frees the parent (`-[NSView _finalize]`) included, so the scroll view reached its dealloc still a subview and AppKit
  raised. #523 keeps the view under a held press only while it is in the window (#519's rule for `NodeView`). Round 12's
  agent trigger, the same ops on both builds: the fixture thread, `tap transcript down` (contact 1267 at (768, 446), the
  scroller's empty area below the reply's table), ⌘1, ⌘2, the up, the sidebar's Settings, Back. The T3 tip's bundle
  (main `0b956d63d`) exits at ⌘1's batch (SIGABRT; the crash report's last exception backtrace `-[NSView dealloc]` ←
  `-[NSScrollView dealloc]` ← `AutoreleasePoolPage::releaseUntil`, X87's frames). Round 13's runs every op: ⌘1 opens Sweep
  five and ⌘2 Sweep four under the held press (state: `…:sweep-4`), the up lands, Settings opens, Back returns. The
  reference does the same (CDP: the press held below the reply, ⌘1 `sweep-5`, ⌘2 `sweep-4`, the up, Settings ›
  General, Escape back to Sweep four). No clone source changed.
- **RT-2 (nothing to retire).** Main's 20 commits since `0b956d63d`, each read against `EXACT2-GAPS.md` and the 13 rows
  of [r12-kept-workarounds](20261013-r12-kept-workarounds.md) ("Each item" above): #523 closes X87, which had no
  workaround; `1bcf7436f` closes X54's issue, whose stand-ins need a rework to move to `focusin` (KW-12), not a
  one-attribute retirement; the intersection fix extends X22's `intersect=`, which the clone does not write (KW-12's hooks stay a
  rework); Ibex's adoption and the launch facts touch nothing a workaround stands in for. KW-1 to KW-13 stand as that
  record says, each with its reason (a parity change across many sites, a rework, or a drive the lanes lack).
- **AD-1.** Nothing the clone writes is refused or compiles differently (the same plan, byte for byte in size, and the
  same warnings); the Mac host changes (`ChainingScrollView`, the launch place) ran in both drives.

## Acceptance results

Drive: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/c1086cf3fd9943711c71f3ae872290b536c16f19/adopt-main-fixes-r13/drive.sh.txt), [lane.sh](https://raw.githubusercontent.com/ccheever/exact2/d5e7845ef8b8d4614599ce34d8ab1f8d9fae56ba/adopt-main-fixes-r13/lane.sh.txt),
[ref-rt1.mjs](https://raw.githubusercontent.com/ccheever/exact2/a6f7630fb0240ebdcbb63d4b23dae5ed1f7a2616/adopt-main-fixes-r13/ref-rt1.mjs.txt), [crashes.py](https://raw.githubusercontent.com/ccheever/exact2/ac93f2074bcd3d57102e8747c2935c09828f3dca/adopt-main-fixes-r13/crashes.py.txt); [readouts](https://raw.githubusercontent.com/ccheever/exact2/7099db73d3c2cb67e1ae32017c222e65e5c206bf/adopt-main-fixes-r13/readouts.txt) (exits, crash frames,
states, pixels, the contract build, RT-2 commit by commit).

| Row | Result | Proof |
| --- | --- | --- |
| AD-1: the branch contains main | pass at the final merge (main `958523176`) | Adopted main; Checks |
| AD-1: the framework tree is main's | pass: empty after `62a7c3c1d`, `534b9613c`, `37540190c`, `7f37bd614` and on the final head | Adopted main; Checks |
| AD-1: the checks pass | see Checks | Checks |
| AD-1: the clone works | pass: session 1 (the shell, Settings General, Appearance, Providers, Usage and back, the fixture thread, the right panel's file viewer, Settings from it, Back) exits 0 on both builds; all 11 screens pixel-equal before and after; the final bundle (after2) pixel-equal to the after on all 15 screens of both sessions | [smoke](https://raw.githubusercontent.com/ccheever/exact2/d21e799f5ea9141efddbeb0d8fb846198def98cb/adopt-main-fixes-r13/ad1-smoke.png), readouts |
| RT-1 (X87) | pass: session 2, before SIGABRT at ⌘1's batch (op 12 of 26, X87's frames); after and after2 every op runs (exit 0): Sweep five, Sweep four under the held press, the up, Settings, Back; the reference the same | [image](https://raw.githubusercontent.com/ccheever/exact2/7dd69a98a9c5f5041bf1440cd42178394fa77386/adopt-main-fixes-r13/rt1-x87.png), readouts |
| RT-1 records | pass: `EXACT2-GAPS.md` (X87's row, "Closed on main since round 12", the releaseHold section), `STATUS.md` (round 12 merged, round 13's entry, X87's "Found, not in scope" row), `plan.md` | the diff |
| RT-2 | pass: nothing a merged main fix ends trivially; each commit and KW-1 to KW-13 listed with why (X87 had no workaround; X54's stand-ins need a rework) | Each item; readouts |
| Real input | none needed (agent mode reaches X87's trigger); one optional step below | — |

Drives (agent mode, 1280×840, `--epoch 2026-10-14T22:00:00+09:00 --time-zone Asia/Seoul`, base port 16850, the clone's
server on 16852; each session on a lane reset to round 12's seed). Before: this worktree at the T3 tip `9149492d4` (main
`0b956d63d`), built before the merge (lane `adopt-main-fixes-r13-before`). After: the bundle of `62a7c3c1d` (main
`898ca3a09`; lane `adopt-main-fixes-r13`). After2: the bundle of `534b9613c` (main `95f2fd3e7`), the same lane. The third merge (`37540190c`, main `2ba30e401`)
was built, not driven (above). Reference:
T3 Code `1e2ecbd975` on lane `adopt-main-fixes-r13-ref` (16850/16851) over CDP.

## Real-input batch steps

Optional, for the coordinator's next session (agent mode already reaches the trigger). Launch this branch's bundle as a
lane copy seeded as `lane.sh` seeds it, window 1280×840. Never send a message.

1. **RT-1 X87 (round 12's step 7).** In the Timeline verification thread, press and hold below the last message (the
   timeline's empty area); with the button held press ⌘1, then ⌘2; release; click the sidebar's Settings, then Back.
   Expected: the threads change, Settings opens and Back returns; the app stays (round 12's bundle aborted at ⌘1).

## Tests

- No clone source changed (no `.contract`, `.ts`, Swift or Rust under `examples/t3-code`), so no Bun guard is added: the
  fix is main's, tested there (`ScrollRoutingTests.testAnEmptyScrollPortFreedUnderAPressGoesWithItsNode`), and this
  round's proof is the drive. No AppKit binary was rebuilt (`host/apple/modules/` and the clone's Swift unchanged).

## Checks

Once, on `37540190c` (the head after the third merge, main `2ba30e401`; `target/r13/checks.sh`). The first pass ran on
`534b9613c` (main `95f2fd3e7`), every check exit 0 (`cargo test` 4,351 passed); main moved while it ran, so the third merge
followed and every check ran again. The fourth merge (`7f37bd614`, main `958523176`) changes only `QUEUE.md`.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,856 pass, 1 skip, 0 fail (328 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,470 slots, 120,810 nodes, 48 resources, 33,309,129 bytes (round 12's plan); 233 `lower-colour-one-scheme` and 20 `lower-dialog-role` shown (the cap), nothing new. `app.contract` 1,453 lines |
| `cargo test -p t3-code-macos --lib` | 0 | 25 passed |
| AppKit binaries | — | none touched (no clone Swift changed) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed, 0 failed, 46 ignored (105 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `cargo check --workspace --all-targets --keep-going` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | four times: the before (`9149492d4`), the live drive's (`62a7c3c1d`), the retry's (`534b9613c`) and this head's |

After a fresh fetch on the final head (`7f37bd614`): `git merge-base --is-ancestor origin/main HEAD` yes (main
`958523176`); `git rev-list --count HEAD..origin/feat(example)/t3-code` 0; `git diff origin/main HEAD -- ':!examples/t3-code'
':!Cargo.lock' ':!Cargo.toml'` empty.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference, runs 1–2 | Electron `1e2ecbd975`, lane `adopt-main-fixes-r13-ref` | stopped on the script's own selectors (the thread row's `sr-only` text, then a press point on the composer); not used | — |
| Reference, run 3 | the same | the press held below the reply, ⌘1, ⌘2, the up, Settings, Escape: no crash | the reference column |
| Before drive | this worktree at `9149492d4` | session 1 exit 0; session 2 SIGABRT at ⌘1's batch (X87) | the before column |
| After drive (the live drive) | the bundle of `62a7c3c1d` | sessions 1 and 2 exit 0 | the after column |
| After2 (the one retry) | the bundle of `534b9613c` (main moved: a Mac boot change) | sessions 1 and 2 exit 0, every screen pixel-equal to the after's | readouts |
| Checks, first run | `534b9613c` | every check exit 0 (Checks); main moved while they ran, so the third merge and the checks again | Checks |

## Not done / not verified

None.

## Decision needed

None: X87 is fixed on main and every other item follows the reference or main's platform rules.

## Delivery

Draft PR [#524](https://github.com/ccheever/exact2/pull/524) against `feat(example)/t3-code`. It must land as a merge or a fast-forward of the feature branch to
its head, never a squash (a squash drops main from the branch's ancestry).

## Next action

The coordinator: review, then merge (merge or fast-forward); the optional real-input step with the next session.
