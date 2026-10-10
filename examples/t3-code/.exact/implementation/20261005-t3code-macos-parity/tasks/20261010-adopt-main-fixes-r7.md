---
name: 20261010-adopt-main-fixes-r7
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r7'
pr_url: https://github.com/ccheever/exact2/pull/384
verified_commit: null
---

# Main adoption, round 7: merge current main into the T3 branch and adopt its fixes

## Outcome

`feat(example)/t3-code` takes current `main` (224 commits past the last adopted main `e200397ec` on 2026-10-10) and adopts
the fixes listed in `examples/t3-code/STATUS.md` "Next up" item 4 and `plan.md` "Status, 2026-10-08" (round 7). The round
was blocked by X67 (main's examples sweep aborts on the clone at a 2 MiB test stack); the user decided on 2026-10-10 to
flatten the clone instead of waiting ([view-depth-under-test-stack](20261009-view-depth-under-test-stack.md)), so this
round starts after that task merges.

## Steps

1. `#297` was squash-merged, so first record the last adopted main without its content:
   `git merge -s ours e200397ec` (its framework content is already on the branch), then `git merge origin/main`.
   Resolve conflicts keeping the T3 side for `examples/t3-code/` and main's side for framework files; the branch carries
   no framework changes of its own (check with `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
   after the merge: only the workspace registration of the example crates may remain).
2. Adopt (STATUS "Next up" 4): main #305 (closes #300, X59: drop the clone's line-clamp workaround), main #304 (closes
   #285: drop the branch's `QUEUE.md` `clock +N real` entry), the `now()` → `performanceNow()` rename (main `9731c8056`,
   the clone's call sites), remove the comment-only `panels.contract` and `settings-panels.contract` (main's examples
   test refuses them), main #309 (#101: the clone's `isInspectable` follows main's development-only rule), #313 (SIGTERM
   through the orderly quit), #314 (the ContextMenu key: retire the clone's own handling where main's covers it) and #325
   (closes #286). #327 is still open: its retirements wait for a later round.
3. Re-read main's binding rules that changed since round 6 (`rules/RULES.md`, `rules/DEFERRED.md`, `docs/issues.md` — open
   framework issues now live in main's `issues/` folder) and record any that change the clone.
4. Update the issue records whose main fix landed (each X file's status line, `issues/README.md` buckets) and
   `EXACT2-GAPS.md` rows that a landed fix retires.

## Acceptance

| Row | How to verify |
| --- | --- |
| The branch contains main | `git merge-base --is-ancestor origin/main HEAD` after the merge (at the merge time) |
| main's examples sweep passes with the clone | `cargo test -p contract --test it button_migration` exit 0 (needs the flatten) |
| The five checks pass on the merged tree | CLAUDE.md's five checks once on the final head |
| The clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries; one live agent drive of the main surfaces (shell, thread, composer, right panel, Settings) with before/after screenshots |
| Each adopted fix is removed or kept with its reason | the task record lists each item with its commit |

## Adopted main

**`bc357d03c951127d9dc999ffbaab64bcd5902d77`** (247 commits past round 6's `e200397ec`). This PR is squash-merged
like the others, so round 8 starts with `git merge -s ours bc357d03c`, then merges main. Main moved 11 commits after
the merge (`bc357d03c..abc1eadff`: "A tap that names a node presses that node", LLP 1012 §1, which changes how the
agent's `tap <target>` aims, and Canvas host passes): round 8 brings them, and its drives should re-check `tap` targets
whose middle holds another control.

## Each adopted fix

| Item | Main commit | Clone change | Commit |
| --- | --- | --- | --- |
| #304 (closes #285): `clock +N real` monotonic | `235164b3a` | the branch's `QUEUE.md` entry dropped (main's `QUEUE.md` taken) | `c1941d82b` |
| #305 (closes #300, X59): line-clamp ellipsis at first paint | `9314e7a81` | nothing to remove (the clone had no workaround) | — |
| #309 (#101, X2): development-only Safari inspection | `f2f0e7092` | kept: `T3WebInspection.swift` marks the module's own `WKWebView`s, which main's rule (Exact's iframes) does not reach | — |
| #313 (#269's SIGTERM half): SIGTERM through the orderly quit | `a3d61c023` | kept: the `atexit` stop still serves the agent's `exit(0)`; #269's bounded hold is open | — |
| #314 (#235's ContextMenu key) | `d236c36d5` | a thread row's ContextMenu is the host's: its `contextmenu` and context popover (ThreadMenu) at the row's centre, as Chromium's keyboard contextmenu; `rowKeyMenu` opens no thread menu and `isMenuKey` knows only `ContextMenu` (the host names the key so on every path now); a draft row's and a legacy row's handlers prevent the default and keep their module menus (bottom-left as SidebarDraftRow's handler, centre); Shift+F10 stays the draft row's own | `d2713f214` |
| #325 (closes #286): bounded ordered reads | `2e48efad1` | kept: the PR panel's stack and default-branch reads stay one at a time. #325 and LLP 1041 §8.4 (`b4a398002`) queue past the sixteen only plain GET/HEAD reads; the panel's reads are native calls (`executor_core.rs` `read`: `!request.is_native()`), which keep the 16-ticket limit. The list's refusal path: nothing in the drive was refused (logs clean); the Pull Requests page reads identically before and after | — |
| `now()` → `performanceNow()` (LLP 1109 D1) | `9731c8056` | 24 call sites in `app.contract`, `r6-device`, `settings-appearance-editor`, `theme-color-picker`; the tests and comments that quote them | `d2713f214` |
| `panels.contract`, `settings-panels.contract` | — | already gone (the root rewrite removed them) | — |
| `.exact/…/rem-probe.contract` (main refuses its `calc(rem + px)`) | LLP 1069.000 | kept verbatim as `rem-probe.contract.txt` (the folder's `drive.mjs.txt` convention), out of main's sweep; the closed interface-font-size record still names the old file name | `d2713f214` |
| X67 (#320): main's sweep on a 2 MiB thread | — | passes with #382's flatten; `view-depth.test.ts` re-measured on main's `contract-lower` (below) | `d2713f214` |
| #327 (bucket 2) | open | nothing: its retirements wait for a later round | — |
| PR #239 (X44) | open | nothing | — |

## What else main changed that the clone relied on

Main's binding rule changed (RULES.md, LLP 1115: "Write the web, ship the platform. … Author > platform > CSS default"):
what an app leaves unsaid is now the platform's. The clone copies a web app, so where the reference relied on the web's
default the clone now says it. Checked against every LLP 1115 wave 1 and 2 commit on the Mac:

- **Icon-only help tags** (`550aa871a`): a custom `button` with `aria-label`, no `title` and no text shows its label as
  AppKit's help tag. The reference shows no native tooltip for `aria-label`, and the clone's own tips would show beside
  one. The first count (an instrumented lowering) found 316 labelled icon-only buttons (5 with a title) and 311 got
  `title=""` (HTML's "no advisory text"). That count took any text node in a button's lowered subtree as its text; the
  host's rule is narrower (review round 1, below), and `help-tags.test.ts` now applies it: 342 sites on the base
  `b731bdb39`, 31 after the first pass, 0 now. README's "Source and checks" states the rule for new buttons.
- **Edit and Help menus** (`550aa871a`, LLP 1115 D8): the host's Edit is Xcode's template (Paste and Match Style ⌥⇧⌘V,
  Find, Spelling and Grammar, Substitutions, Transformations) and it adds Help ("T3 Code Help" ⌘? and AppKit's search
  field). DesktopApplicationMenu.ts has none of these: `R8KeysMenus.concealCommands` hides them (an app command the
  host stood in Find's place keeps its chord: ⇧⌘G, the branch picker); `T3Menus.augment` hides the help-book item and
  puts Check for Updates... in the host's Help (it added its own Help only when the host had none).
- **The Mac's root font size is 13** (`4dbfec2fe`): the clone's `rootFont` task already sets the stored size at launch;
  it now sets 16 when the data has not answered yet (`data.look.fontSize > 0 ? … : 16`), so no rem lays out at 13.
- **Heading styles** (`2d26c223f`, D3): no change. Every clone `text role="heading"` writes its size; the four that
  write no weight inherit 400, which is title2's, and the visually hidden welcome heading is 1×1.
- **Buttons native by default** (LLP 1104, `cbd76f7d7`): none of the clone's buttons lowers native (each has a
  background or a border), so main's sweep reports no migration and nothing draws differently.
- **Page background, link runs, `hr`, alertdialog `closedby`, `user-select`, search fields**: the drive's screenshots
  are unchanged, so no unset page background shows; all 9 inline `href` runs set their colour; the `hr`s write height, borders and background; the clone's
  alertdialogs are columns, not `dialog`s; no `input type="search"`. `user-select: auto` and a secondary click on
  selected text (AppKit's text menu) follow the web more closely now; they need a real pointer (real-input step 4).
- Agent driver ops used by the lanes (`tap`, `type … key`, `clock +N real`, `screenshot … window`) work unchanged on
  bc357d03c; `type … key ContextMenu` is new (#314).

rules/DEFERRED.md changed only the `now()` spelling and admits a terminal command-line run (LLP 1101.003): neither
changes the clone. Open framework issues live in main's `issues/` folder (73 files); no T3 bucket moves except bucket 1.

For the coordinator (the brief keeps these files out of a task PR): `issues/README.md` bucket 1 rows #285, #286, #300,
#101 and #314's #235 half are adopted (#286's sequential reads kept, reason above); X59's and X26's status lines;
`STATUS.md` "Next up" item 4 is done; X67's row: main's sweep passes on the branch.

## Review round 1 (2026-10-10)

The independent review of `70de0e1cf` found the PR conflicting with its base and five should-fix items:

- **Base drift (blocking)**: the base took #383 (`954f6bbff`) and two record commits. `4a7c53051` merges `b731bdb39`; the
  one conflict, `snapshot.contract`'s Play Whoosh and Play Click (#383 added their hover and focus handlers), keeps #383's
  lines with `title=""` appended. #383 adds no `now()` call. On the merged head: `contract build` exit 0, main's sweep
  exit 0, `view-depth.test.ts` and the Bun suite pass (Tests, below).
- **The help-tag opt-out was incomplete**: PresenterMac.propsChanged and settleTips give a custom button (`button`, and
  `link`, which also lowers to Pressable, the host's kind "button") its `aria-label` or `aria-labelledby` text as the
  help tag when it has no `title` and Accessibility.swift `accessibleText` is empty. That text is read from the
  mounted children that are not `aria-hidden`. `help-tags.test.ts` walks the view tree from T3Code with components
  inlined and `children` filled, as `view-depth.test.ts` does. It counts a button as having text only when some
  `text` is mounted on every `when` arm (an `each` may mount nothing, and an `aria-hidden` that is an expression may
  hide). It found 31 more: the reviewer's three (the sidebar's Filter threads by project, its folder icon alone by
  default and its project glyph `aria-hidden`; Mermaid's Expand diagram, its drawing `aria-hidden`; the model
  picker's provider rail, a mark with text only for a badge), 4 icon-only `link`s (Open activity on host ×2, About
  agent, View source for) and buttons whose text is `aria-hidden` (PR linked threads, PR emoji choices, usage
  segments) or only on some arms (composer controls when icon-only, PR actions when compact, …). The reference sets no
  `title` on any of them (PullRequestTimelineTab.tsx, ThemeSearchSection.tsx, MermaidDiagram.tsx, Sidebar.tsx,
  ChatComposer.tsx), so each says `title=""`. Of the 937 button and link sites the walk reaches, 557 are labelled:
  7 have a real title, 214 have text on every arm and 342 say `title=""`. The guard fails on the base (342) and on
  `70de0e1cf` (31), and catches #383's play buttons on the base: [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §2.
- **Tests for the changed behaviours**: `appearance-fonts.test.ts` reads rootFont's 16 fallback and that it is the
  root size's one writer; `r12-sidebar.test.ts` reads the draft row's and the legacy row's `preventDefault()` for
  ContextMenu (and the draft row's Shift+F10) and the thread row's handler that leaves ContextMenu to the host. All but
  the last fail on the base: [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §3.
- **Stale comments**: ThreadMenu's comment (`sidebar-row.contract`) and `T3Sidebar.swift`'s header now say a thread
  row's ContextMenu is the host's #314 default, at the row's centre, and the module's anchored menus serve the draft
  and legacy rows.
- **Frontmatter**: `verification: verified-with-unverified-rows`, as the closed audit-wave-followups-3 and -4 and
  realinput-1010-fixes records say for open real-input rows.
- **Housekeeping**: the detached worktree `t3-code-r7-before` (the before drive's `309e49344`) is removed; it was
  clean and nothing ran from it. No lane app copy was left.

`title=""` changes no pixel (a help tag shows only under a resting pointer), so this round has no before/after
images; the counts above are its before/after. Real-input step 1 now names the reviewer's three buttons. One agent
drive of the merged head (the bundle of `e585f13b1`, the previous drive's ops, lane and size, one try) against the
previous after drive (`d2713f214`): screenshots within 7/255 on every channel (Settings identical), logs with no
refusal or error, and every agent-tree difference is a guarded button that now carries `title: ""` (Filter threads by
project, the composer's model picker, effort and runtime mode, the chat header's thread title, Workspace, the add
project palette's rows, Settings' scope buttons) or one of #383's hidden `…-km` menu-item focus views:
[review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §4.

## Guard re-measured (view-depth.test.ts)

Main's `contract-lower` gained LLP 1115 D3's `heading_style` in `Lowerer::node`. The #382 scratch tool, rebuilt on
main's sources: lldb at a forced overflow gives `Lowerer::nodes` 19,392 + `node` 512 = 19,904 B per site (was 19,328)
and `expr::compile` 12,944 B per level (was 12,720). The instrumented peak is 1,611,616 B (1,573 KiB) at
`timeline-files.contract:39`, 67 sites and 17 levels, where the old constants predicted 1,536 KiB (37 KiB under).
New constants: 19,904, 12,944 and `BELOW_LOWERING` 58,000 (the peak exactly). On the twelve deepest leaves the model is
0 to 13 KiB over; 13 of the 200 listed leaves, whose nodes carry handlers the model does not count, are under by up to
40 KiB, all at or below 1,320 KiB (470 KiB under the 1,792 KiB budget). Ground truth: main's debug compiler at
`ulimit -s 2048` exit 0. Calibration: [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/b689128501ba2f1145a1d2c4ec067cd36bb84458/adopt-main-fixes-r7/evidence.txt) §4.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| The branch contains main | pass: `git merge-base --is-ancestor bc357d03c HEAD` (origin/main was bc357d03c at merge time); `git diff bc357d03c -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` is empty | [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/b689128501ba2f1145a1d2c4ec067cd36bb84458/adopt-main-fixes-r7/evidence.txt) §1 |
| main's examples sweep passes with the clone | pass: `cargo test -p contract --test it button_migration`: exit 0, "source sweep: 90 roots, 192 imported source files, 0 failures"; `(ulimit -s 2048; target/debug/contract build …)` exit 0. Again on the merged head (with #383): exit 0, 1 passed in 235 s ([review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §5) | [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/b689128501ba2f1145a1d2c4ec067cd36bb84458/adopt-main-fixes-r7/evidence.txt) §3 |
| The five checks pass on the merged tree | pass on `5e7597f38`, and again on `e585f13b1` after merging the base `b731bdb39` (below) | Tests; [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §5 |
| The clone works | pass: Bun, tsc, contract build, `cargo test -p t3-code-macos --lib`, the AppKit binaries (mermaid needs a running T3 server, as in earlier rounds); the live drive of shell, thread, composer, right panel, dialog, Settings and Pull Requests: agent tree identical on all 8 shared states, screenshots with no pixel over 24/255, no refusal in the logs | [shell](https://raw.githubusercontent.com/ccheever/exact2/fae26a8f58e185e82cff581f6850ce2877da3e3a/adopt-main-fixes-r7/1-shell.png), [thread](https://raw.githubusercontent.com/ccheever/exact2/9aecc3083b539c914b394ae86200c578894730b8/adopt-main-fixes-r7/2-thread.png), [composer](https://raw.githubusercontent.com/ccheever/exact2/29a18059be77060c3ee651dff6d88b946cd99871/adopt-main-fixes-r7/3-composer.png), [right panel](https://raw.githubusercontent.com/ccheever/exact2/3ed50b195b54ea860b0b096172d4ace4d90fe82c/adopt-main-fixes-r7/4-right-panel.png), [dialog](https://raw.githubusercontent.com/ccheever/exact2/e084f5db950115c6a3aca4f91b9515d42f82fdc3/adopt-main-fixes-r7/5-dialog.png), [Settings](https://raw.githubusercontent.com/ccheever/exact2/421b412f43e669c4c620674ef23f55282a533001/adopt-main-fixes-r7/6-settings.png), [Pull Requests](https://raw.githubusercontent.com/ccheever/exact2/1919eb4cec29a5f212d5a0703c40a93ce48541c6/adopt-main-fixes-r7/7-pull-requests.png); [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/b689128501ba2f1145a1d2c4ec067cd36bb84458/adopt-main-fixes-r7/evidence.txt) §5 |
| The sidebar's ContextMenu and Shift+F10 (#314) | pass: ContextMenu on the focused thread row opens its context popover (ThreadMenu; before: the old agent refused the key); Shift+F10 opens nothing on a thread row, as the reference's Chromium on macOS | [Shift+F10](https://raw.githubusercontent.com/ccheever/exact2/25bd90e267db326f6197fbcb2a7a710b8369ea73/adopt-main-fixes-r7/8-shift-f10.png), [ContextMenu](https://raw.githubusercontent.com/ccheever/exact2/c30efd3b711645d363d2ae65e02fababa40f3fed/adopt-main-fixes-r7/9-contextmenu.png); [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/b689128501ba2f1145a1d2c4ec067cd36bb84458/adopt-main-fixes-r7/evidence.txt) §5 |
| Each adopted fix is removed or kept with its reason | pass: the table above | this record |
| No AppKit help tag from `aria-label` (LLP 1115 wave 1; review round 1) | pass by the host's rule: `help-tags.test.ts` finds 342 sites on the base, 31 on `70de0e1cf`, 0 now; the drive's trees show `title: ""` on the titled buttons. The help tag itself needs a resting pointer: real-input step 1 (open) | [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §2, §4 |
| rootFont's 16 and the rows' ContextMenu prevention are tested | pass: the new tests fail on the base | [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §3 |

## Tests

- `r12-sidebar.test.ts`: a thread row's ContextMenu asks the module for no menu; a draft row's two keys still anchor at
  its bottom left; `isMenuKey` knows only `ContextMenu`.
- `macos/tests/menus`: `testTheHostTemplateEditAndHelpShowOnlyTheReferenceItems` (the host's LLP 1115 Edit and Help,
  the reference's items shown, ⇧⌘G still fires from the hidden Find); 46 tests.
- `view-depth.test.ts`: the constants and the calibration comment; `timeline-work-rows.test.ts` and
  `sidebar-palette-keys.test.ts` read `performanceNow()`.
- Review round 1: `help-tags.test.ts` (new: the host's help-tag rule over the view tree; the attribute reader; the
  walk resolves every use); `appearance-fonts.test.ts` "the root font size app.contract sets"; `r12-sidebar.test.ts`
  "the rows' key handlers and the host's ContextMenu default" (three tests).

Final checks on `e585f13b1` (review round 1; the base `b731bdb39` merged; the record follows): `bun test
examples/t3-code --timeout 60000` 4242 pass, 1 skip, 0 fail (289 files), exit 0; strict `tsc` exit 0; `bun
scripts/exact.mjs contract build examples/t3-code/app.contract` exit 0 (5958 slots, 110,024 nodes); main's sweep
`cargo test -p contract --test it button_migration` exit 0; `cargo build --all-targets --keep-going` exit 0;
`cargo test --lib --bins --tests --no-fail-fast` exit 0 (3,675 passed, 0 failed, 34 ignored, 95 suites); `cargo
clippy --all-targets --keep-going -- -D warnings` exit 0; `cargo fmt --all -- --check` exit 0; `git add -A && bun
scripts/caps.mjs` exit 0; `bun scripts/boot.mjs` exit 0; `cargo test -p t3-code-macos --lib` 17 pass; the AppKit
binaries for the changed `T3Sidebar.swift` comment: sidebar 6, r12-sidebar 3, 0 failures. `app.contract`: 1,329 lines.

Round 0's final checks on `5e7597f38` (records and the guard's comment follow): `bun test examples/t3-code --timeout 60000`
4192 pass, 1 skip, 0 fail (288 files), exit 0; strict `tsc` exit 0; `bun scripts/exact.mjs contract build
examples/t3-code/app.contract` exit 0 (6341 slots, 109,849 nodes); `cargo test -p t3-code-macos --lib` 17 pass;
`cargo build --all-targets --keep-going` exit 0; `cargo test --lib --bins --tests --no-fail-fast` exit 0 (3,675
passed, 0 failed, 34 ignored, 95 suites); `cargo clippy --all-targets --keep-going -- -D warnings` exit 0;
`cargo fmt --all -- --check` exit 0; `git add -A && bun scripts/caps.mjs` exit 0; `bun scripts/boot.mjs` exit 0.
AppKit binaries (README recipe): 35 of 36 pass (activity 9, app-control 27, attach 3, browser 29, browser-automation
25, browser-profiles 16, codex-auth 6, composer 4, composer-files 4, contextmenu 19, fleet 9, intent 4, local-backend 82,
media-actions 7, menus 46, notifications 4, r10-connect 4, r10-device 4, r11-device 3, r11-upstream 3, r12-sidebar 3,
r5-composer 3, r5-panels 6, r6-device 3, r6-media 10, r7-device 14, r8-keys 6, r8-pointer 2, r9-device 13, r9-input 13,
sidebar 6, snapshot (all checks), ssh 15 (1 skipped), terminal 38, transport 3) and `timeline-keyboard` passes; mermaid
needs `T3_SERVER`. The terminal binary failed one triple-click assertion on its first run and passed on the rerun (it
links only the clone's module and the unchanged facade). `app.contract`: 1,329 lines.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | the feature tip `309e49344`, built in the detached worktree `t3-code-r7-before` (`t3-code-evidence-base` is at `950e8e2e5`, 41 commits behind; the view-depth worktree's build was stale to the driver) | the first try stopped at `tap connection-settings` on the Pull Requests page (no such button there); reordered, the second completed; ContextMenu refused by the old agent, as expected | `target/r7/drive/before` |
| After drive (the live drive) | the bundle of `d2713f214`'s sources | completed in one try, both parts | the images above |
| Reference | `ref-app.sh` on lane `adopt-main-fixes-r7` (16480) | shots of the same states; its ContextMenu menu is native (Electron's), so the page shot shows none | third column of each image |
| Review round 1 drive | the bundle of `e585f13b1`'s sources (base `b731bdb39` merged) | completed in one try; no pixel change, `title: ""` on the guarded buttons in the trees; the before worktree `t3-code-r7-before` is removed | `target/r7/drive/after-r2`; [review-round-1.txt](https://raw.githubusercontent.com/ccheever/exact2/d08102b3002fd42f6f430e2467c7b5627e85b54c/adopt-main-fixes-r7/review-round-1.txt) §4 |

## Real-input batch steps

Normal launch of this branch's bundle (lane copy), real keyboard and pointer:

1. Rest the pointer 3 s on the sidebar's New thread (✎) and Settings (⚙) buttons and on the chat header's Toggle right
   panel: only the clone's own tip shows, no yellow AppKit help tag (the reference shows its own tip alone). Then rest
   it 3 s, with no AppKit help tag expected, on the sidebar's Filter threads by project (folder icon, shown once a
   project exists), on a rendered Mermaid diagram in a message (Expand diagram), on a provider in the model picker's
   left rail (open the picker with ⇧⌘M), and on the Play button beside Whoosh in Settings › SnapShots' sound menu.
2. Open the menu bar's Edit: Undo, Redo, —, Cut, Copy, Paste, Paste as Text, Delete, —, Select All, —, Speech (then
   AppKit's AutoFill, Start Dictation, Emoji & Symbols); no Find, Spelling and Grammar, Substitutions, Transformations
   or Paste and Match Style. Help: the search field and Check for Updates... (which shows "Automatic updates are not
   available right now."). In the composer, ⇧⌘G still opens the branch picker.
3. Focus the "Timeline verification" row with Tab and press ContextMenu (a keyboard with the key, or `orca computer`
   key ContextMenu): one native menu at the row's centre (Pin thread … Delete); Escape closes it; Shift+F10 opens
   nothing. On a draft row (start a new thread in a project and leave a draft) ContextMenu and Shift+F10 open the
   draft menu at the row's bottom left, once.
4. Drag across a timeline message's text: it selects; right-click the selection: AppKit's text menu (Look Up, Copy,
   …) appears as main's LLP 1115 wave 2 gives it; note whether that differs from the reference's (Electron shows the
   app's own menu or none) for the next audit.

## Next action

Review round 1 is fixed (above). The coordinator reviews the draft PR [#384](https://github.com/ccheever/exact2/pull/384), syncs `STATUS.md`, `plan.md` and `issues/README.md` from the lists above,
merges it, and runs the real-input steps in the next batch. Round 8 starts with `git merge -s ours bc357d03c`.
