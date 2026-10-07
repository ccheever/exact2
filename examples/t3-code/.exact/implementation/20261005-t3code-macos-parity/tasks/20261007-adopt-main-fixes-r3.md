---
name: 20261007-adopt-main-fixes-r3
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-r3
pr_url: null
verified_commit: null
---

# The clone adopts main's fixes, round 3 (#109, #107, #135; X27, X33, X34 re-checked)

## Outcome

The task branch merges main `cff90b364` (origin/main at task start). Where a fix main merged covers
what the clone worked around, the workaround is gone and the clone behaves as T3 Code does. Where
it does not, the docs say exactly what is still missing. `EXACT2-GAPS.md`, the issues README,
the issue records and the task documents that cite these issues are current.

## Scope and exclusions

| Issue | Plan id | main PR | Clone sites |
| --- | --- | --- | --- |
| [#109](https://github.com/ccheever/exact2/issues/109) | X14 | #183 | `T3ReadGate.swift`, its wiring in `T3Module.swift` and `T3Module+Connection.swift`, `r3-protocol-reader.ts` (`beginRead`, `readerSession`), `client.ts` `refresh()`, `macos/tests/transport/r3.swift` gate cases; `r6-pr-actions.ts` `readDetail` (checked) |
| [#107](https://github.com/ccheever/exact2/issues/107) | X8 | #186 | `RightPanelTabsInput.swift` (middle click, an attended row); monitor comments in `T3Sidebar.swift`, `T3Timeline.swift`, `T3PanelsNative.swift` |
| [#135](https://github.com/ccheever/exact2/issues/135) | side of X7 | #184 | none: no clone page is served from `assets/` |
| [#113](https://github.com/ccheever/exact2/issues/113), [#132](https://github.com/ccheever/exact2/issues/132), [#133](https://github.com/ccheever/exact2/issues/133) | X27, X33, X34 | — | re-check only |

Excluded: #185 (`setRootFontSize`, X3; the interface-font-size task adopts it on its own branch);
#188 (X18 SVG `d`) and #189 (X47 focus ring), which arrive with the merge but are not adopted or
checked here; framework changes; GitHub issue comments.

## Context and guidance

Brief: coordinator's round-3 adoption task. Each fix was confirmed in `cff90b364` (`git log` of
the merge commits `8a4a503d7` #183, `af69be639` #186, `282e99a5b` #184). Sites were found with
`grep -rn` of the issue numbers, X ids, `T3ReadGate`, `readEnd`, `beginRead` and the
`addLocalMonitorForEvents` sites in `examples/t3-code`. Earlier rounds: `20261007-adopt-main-fixes-input`
(PR #180) and `20261007-adopt-main-fixes-shell` (PR #181).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged main | origin/main into the task branch | `cff90b364` | merged as a merge commit (another task merges the same SHA) | `9a9cd61d7`; the merge changed no file under `examples/t3-code` |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| #109 / X14 | **adopted** (issue moved to `issues/closed/`) | #183: a watched topic announced while the resource's request is in flight lets that reply land, then asks once more (LLP 1016.002 D4). That is what `T3ReadGate.swift` emulated (hold `t3.status`, `t3.events`, `t3.fleet` until `readEnd`, then replay once). Removed: the gate, its wiring, the reader tags and `readEnd`, and the gate's 5 AppKit cases. Kept: the RPC trace ids and `readDetail`'s per-answer reads, because a re-ask for new arguments or a `refresh` still forgets the old request (LLP 1016 D5); `managed-codex-chatgpt`'s start/take stays (on hold, optional). |
| #107 / X8 | **partly adopted** (issue stays open in the plan, `closed-upstream`) | #186 adds `auxclick`, `clicks 1-3`, wheel `at`, held `modifiers`, and routes agent mouse and wheel events through `NSApplication.sendEvent`, so local monitors see them. The right-panel tab's middle click became an agent row. Its first drive closed the **wrong tab**: `RightPanelTabsInput` tested `view.visibleRect`, which an unclipped NSView reports beyond its bounds, so every tab matched and the first in the dictionary closed. Fixed (`bounds ∩ visibleRect`) with a two-tab AppKit case that fails on the old code. The other attended pointer rows convert when their tasks are re-driven (listed in the X8 file). |
| #135 | **nothing to adopt** | #184 serves a bundled page at `http://exact.localhost`; no clone page comes from `assets/` (the terminal page is a module scheme, previews come from the T3 server). |
| #113 / X27 | still missing | No title-row height, traffic-light inset or window full-screen fact on `cff90b364` (`bc6bc35f4`'s `fullscreenchange` is a `video` element's). `T3WindowChrome.swift`, `T3FullScreen.swift` stay. |
| #132 / X33 | still missing | No selection end rectangle, no `clearSelection()`. |
| #133 / X34 | still missing | No `frame()` of an inline run. |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Clone builds on the merged base | — | `bun test examples/t3-code`, strict tsc (ES2023), `contract build`, `cargo test -p t3-code-macos --lib`, caps | green | macOS | numbers below |
| Topic changes while the snapshot read is in flight | lane server 16450 (isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME, telemetry off), project Alpha, 2 threads | pair; create 5 threads at the server while the app is idle; sample tree and `state.pending`; read `logs` | all 5 rows shown, nothing pending, no "Some requests are slow"; runner journal says "lands first, then it is asked again" instead of "forget request" | macOS 1280×840 | records below |
| No gate requests | — | `r3-protocol.test.ts` | the snapshot read sends no `reader`, `readerSession` or `readEnd` | — | test (fails on the base) |
| Middle click closes the tab under the pointer | thread Alpha 1, right panel with Files and Diff | `tap panel-tab-files auxclick` | Files closes, Diff stays | macOS 1280×840 | before/after image, AppKit `contextmenu` |
| Kept and touched code | — | AppKit `transport`, `contextmenu`, `r10-connect`, `sidebar`, `r5-panels` | pass | macOS | counts below |

## Progress

Implemented 2026-10-07. Verification: unverified (task PR review pending).

Not run: the 15-turn reply-ownership scenario (no authenticated provider in the lane); the other
attended pointer rows of X8; a real hand's middle click (the AppKit two-tab case covers the hit
test); 840×620 and dark.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | merge `9a9cd61d7` + this task's commit | merged base before changes: `bun test` 2255 pass / 1 skip / 0 fail, strict tsc clean, contract build 2543 slots / 45 resources, `cargo test -p t3-code-macos --lib` 11/0 (main's 150 commits broke nothing in the clone). After: `bun test examples/t3-code` 2254 pass / 1 skip / 0 fail (2 gate tests removed, 1 added); strict tsc clean; contract build 2543 slots, 45 resources; `cargo test -p t3-code-macos --lib` 11/0; caps pass; AppKit `transport` (all suites 0 failures), `contextmenu` 15/0 (new two-tab case fails on the old `RightPanelTabsInput`), `r10-connect` 4/0, `sidebar` 5/0, `r5-panels` 6/0; five checks pass (cargo build; cargo test 3339 pass / 0 fail / 33 ignored; clippy; fmt; caps; boot) | drive records below; PR image | none for the adopted rows; X27/X33/X34 upstream |

Drives (lane fixtures under `target/lane-r3`, not committed): BEFORE is
`t3-code-evidence-base` at `4f523ef5c` (its own driver, which has no `auxclick`), AFTER this
branch; the same script, one `open()` session each at 1280×840, under the drive lock. The
first BEFORE drive hung in the script (a missed child-exit event) and the second had a stale
fixture prefix and no tabs (the empty panel shows a surface list, not `panel-add-surface`); the
first AFTER drive found the wrong-tab bug. The records below are the last BEFORE and AFTER
drives.

```
burst (5 thread.create at the server, app idle)
  BEFORE +1812 ms none, pending [212]; +2047 ms 1,2,3,4,5, pending []; … +8103 ms 1,2,3,4,5
  AFTER  +609 ms  none, pending [213]; +792 ms  1,2,3,4,5, pending []; … +6709 ms 1,2,3,4,5
runner journal over the whole drive
  BEFORE forget request ×17 (data 8, editor 3, branches 2, details 2, canvas 2); "lands first" ×0
  AFTER  forget request ×8 (data 3, branches 2, details 2, canvas 1);
         "changed t3.status: request N (data) lands first, then it is asked again" and the like ×14 (data 6, canvas 5, editor 3)
"Some requests are slow" toast: BEFORE false, AFTER false
tap panel-tab-files auxclick (tabs Files, Diff)
  BEFORE reply {"delivery":"platform","at":[778.01,26]} (a plain press); tabs after: Files, Diff
  AFTER  reply {"auxclick":true,"delivery":"platform","at":[778.01,26]}; tabs after: Diff
  AFTER, first drive (old hit test): tabs after: Files  (the Diff tab closed)
```

Evidence image: `https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r3/01-tab-middle-click-before-after.png`
(the right panel after the middle click: Before keeps Files and Diff, After keeps Diff).

## Next action

Review the PR. Later tasks re-drive their attended pointer rows with #186's forms (X8 file).
