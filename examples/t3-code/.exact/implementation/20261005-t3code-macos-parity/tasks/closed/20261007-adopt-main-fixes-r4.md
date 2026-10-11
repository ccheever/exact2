---
name: 20261007-adopt-main-fixes-r4
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-r4
pr_url: https://github.com/ccheever/exact2/pull/218
verified_commit: null
---

# The clone adopts main's fixes, round 4 (#105, #118, #125, #128, #138, #115, #104; #103 and #119 recorded; #192 checked)

## Outcome

The task branch merges main `463acda68` (origin/main at task start; the embedded-server-runtime branch
merges the same SHA). Where a fix main merged covers what the clone worked around, the workaround is gone
and the clone behaves as T3 Code does. Where it does not, the issue files say exactly what is still missing
on main. `EXACT2-GAPS.md`, `STATUS.md`, the issues README, the issue records and the task records that held
rows blocked by these issues are current.

## Scope and exclusions

| Issue | Plan id | main PR | Clone sites |
| --- | --- | --- | --- |
| [#128](https://github.com/ccheever/exact2/issues/128) | X10 | #208 | none (no workaround); every `pre-wrap` + `overflow-wrap` row and Markdown paragraph |
| [#125](https://github.com/ccheever/exact2/issues/125) | X20 | #209 | `T3Composer.swift` paste routing, `composer-editor-files.ts` (checked) |
| [#138](https://github.com/ccheever/exact2/issues/138) | X23 | #210 | none for X23d; `pr-handoffs-and-quick-actions` fold row |
| [#115](https://github.com/ccheever/exact2/issues/115) | X29 | #205 | `R6MediaPreview.swift` PDF body, `r6-media.contract` (checked) |
| [#118](https://github.com/ccheever/exact2/issues/118) | X36 | #204 | `timestamp-format.ts`, `desktop-shell-details.test.ts` |
| [#104](https://github.com/ccheever/exact2/issues/104) | X5 | #201 | none (no scheme workaround; app activation not built, U10 pending) |
| [#105](https://github.com/ccheever/exact2/issues/105) and the local draft `20261006-native-module-termination` | X6 | #200 | `T3Ssh.swift` termination observer, `macos/tests/ssh/auth.swift` |
| [#103](https://github.com/ccheever/exact2/issues/103), [#119](https://github.com/ccheever/exact2/issues/119) | X4, X37 | #215, #199 | record only |
| [#192](https://github.com/ccheever/exact2/pull/192) | X19 | — (docs, already in the base) | `app.contract` mount polls |
| [#202](https://github.com/ccheever/exact2/pull/202) | X21 (web side) | — | none: web JS target only; the clone is macOS |

Excluded: the embedded server's use of #200, #215 and #199 (embedded-server-runtime's task, worked on its own
branch, so its task record is not edited here); portable-app-download (planned; its X4 and X37 rows are
updated); app activation (U10); framework changes.

## Context and guidance

Brief: coordinator's round-4 adoption task. Each fix was confirmed in `463acda68` by its merge commit
(`05c767ba8` #208, `78e53a813` #209, `b84fb5974` #210, `e3b0be7ba` #205, `bcec7d4d4`/`4132f02c5` #204,
`20017b7fc` #201, `6cd178efc` #200, `8d8fb0db9` #215, `13fcbfa6f`/`33aaa0b43` #199) and by reading the
issues and PR bodies (none of the closed issues has a comment saying more landed). Main's vocabulary was
checked for the parts not merged: `kernel/tables/schema.json` has no `text-wrap`, placeholder colour,
`-webkit-font-smoothing`, `scroll-padding`, `scroll-margin` or `overflow-anchor` row;
`docs/contract-grammar.md` has no field `selectionchange`, `beforeinput` or `setRangeText`. Earlier rounds:
`20261007-adopt-main-fixes-input` (#180), `-shell` (#181), `-r3` (#207).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged main | origin/main into the task branch | `463acda68` | merged as a merge commit (another task merges the same SHA) | `e9d095002`; only `QUEUE.md` conflicted (both sides kept); the merge changed no file under `examples/t3-code` |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| #105 / X6 + native-module-termination | **adopted** (the draft moved to `issues/closed/`; X6 stays open for the bounded hold) | #200 destroys every session from `applicationWillTerminate`, so `T3Module.destroy()` reaches `T3Ssh.destroy()` at ⌘Q, an Apple Event quit and last-window close. T3Ssh's own `willTerminateNotification` observer is removed; the AppKit case now checks that the notification alone closes nothing and that `destroy()` fails a waiting prompt and every later one. Live: both SSH tunnels end at ⌘W on the last window, an Apple Event quit and ⌘Q pressed twice, as they did on the base with the observer; a SIGKILL control leaves them orphaned (below). |
| #118 / X36 | **adopted** (issue moved to `issues/closed/`) | Hermes has `Intl.Locale` and `getWeekInfo()` as Chrome answers them. `resolveWeekStartsOn` (the reference's code) now answers on macOS; the test expects the weekday for every tag instead of accepting `undefined`. `RUNTIME_LOCALE` and `T3Locale.swift` stay (Hermes's default follows the Mac's region; Electron's `getSystemLocale()` is not `exactTime().locale`). |
| #128 / X10 | **nothing to remove** (`closed-upstream`) | #208 gives macOS paragraphs Chrome's break opportunities. No clone workaround existed. Balance, placeholder colour and font smoothing are not on main. |
| #125 / X20 | **not adoptable** (`closed-upstream`) | #209 adds cancelable `paste`/`copy`/`cut` to Contract fields only. The composer is a native `NSTextView` because chips (atomic ranges) and range replacement in one undo step are still missing, so its paste handling stays in Swift. No Contract field in the clone needs a paste handler. |
| #138 / X23 | **nothing to remove**; fold row unblocked (`closed-upstream`) | #210 anchors a plain `scroll` box on macOS (X23d). The clone had no X23d workaround; X23a–c are not on main, so `R9Input.swift` and the turn hold stay. `pr-handoffs-and-quick-actions` no longer holds its header fold row. |
| #115 / X29 | **not adoptable** (`closed-upstream`) | #205 fixes a bundled PDF `iframe`. The clone's PDF is an http URL that an `iframe` could load before; WebKit's PDF view ignores `#toolbar=0&view=FitH` and draws a white surround, where T3 Code shows the page alone on Chromium's #282828 surface. The PDF element #205 left open is what would replace `PDFView`. |
| #104 / X5 | **nothing to adopt** (`closed-upstream`) | #201 only journals a launch URL no navigation root hears. |
| #103 / X4, #119 / X37 | **recorded** (`closed-upstream`) | `host.macos.resources` (#215) and nested signing (#199) are available for embedded-server-runtime and portable-app-download; entitlements and a pre-seal hook are not on main. |
| #192 (X19) | **three polls converted** | No `pause`-mutation loop existed. The Pull Requests search poll (`every(250)`, applied 250–500 ms after the last keystroke) is now the reference's `SEARCH_DEBOUNCE_MS` 250 debounce as a keyed gated task; the terminal close confirm and the module-opened thread (both read on a 500 ms poll) are gated tasks that run at once. The other mount clocks advance shown times or retry reads on a fixed cadence and stay. |
| #202 | **nothing to adopt** | web JS target only. |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Clone builds on the merged base | — | `bun test examples/t3-code`, strict tsc (ES2023), `contract build`, `cargo test -p t3-code-macos --lib`, caps | green | macOS | numbers below |
| SSH teardown without the observer | `T3Ssh(agent:)` in the AppKit binary | `macos/tests/ssh` | the notification closes nothing; `destroy()` fails waiting and later prompts | macOS | ssh 15 tests, 1 live skip, 0 failures |
| SSH tunnels end at quit | lane copies `com.exact.t3code.laner4.before/.after`, each with two saved SSH targets; `fake-ssh.sh` as the ssh command, tunnels to a lane HTTP server (16643) | launch (two tunnels reopen); post ⌘W, ⌘Q ⌘Q or send the Apple Event quit to the lane pid; poll the tunnel pids and listeners every 50 ms; control: SIGKILL | no tunnel survives an orderly exit in either build; the control leaves them orphaned | macOS | records below |
| Week start on macOS | — | `cargo test -p exact-js --test it pure_utilities_match` (Hermes against Chrome); `desktop-shell-details.test.ts` | pass | macOS | 1/1; 32/32 |
| PR search debounce | lane server 16641, agent clock in 10 ms steps | type in `pull-requests-search`, step until `slots.prApplied` equals the query | 250 ms after the keystroke at any phase | macOS 1280×840 | records below |
| Terminal close confirm | thread "Wrap demo", right panel, Terminal surface | `tap close-tab-terminal:term-1`, step 10 ms until `app-confirm` | at once | macOS 1280×840 | records below, After image |
| Long paths in a user message (X10) | lane message with three long paths; provider banner and toasts dismissed | screenshots at 1280×840 and 840×620 | Chrome's breaks | macOS | images below (no change in either) |

## Progress

Implemented 2026-10-07. Verification: unverified (task PR review pending). The live SSH row and the X10 pair at 840×620 ran in attempt 2.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | merge of `463acda68` + this task's commit | merged base before changes: `bun test examples/t3-code` 2297 pass / 1 skip / 0 fail, strict tsc clean, contract build 2543 slots / 45 resources, `cargo test -p t3-code-macos --lib` 11/0 (main's commits broke nothing in the clone). After: `bun test examples/t3-code` 2300 pass / 1 skip / 0 fail (3 week-start cases added); strict tsc clean; contract build 2542 slots (`prTypedAt` gone), 45 resources; `cargo test -p t3-code-macos --lib` 11/0; AppKit `ssh` 15 tests, 1 live skip, 0 failures; `cargo test -p exact-js --test it pure_utilities_match` 1/0; caps within budget (`app.contract` at 1,500 lines); macOS bundles of base and branch build | records and image below | live SSH row (screen locked) |
| 2 (2026-10-07) | merge of `origin/feat(example)/t3-code` `20980ae10` (#217; `QUEUE.md` only: #217 removed the terminal-drag line, both sides otherwise kept) | `bun test examples/t3-code` 2300 pass / 1 skip / 0 fail; strict tsc clean; contract build 2542 slots; caps within budget; live SSH-at-quit drive on both builds (below) | records below | none |

Lane (not committed, `target/lane-r4`): reference server `1e2ecbd975` (`apps/server/dist/bin.mjs serve`) on
127.0.0.1:16641 with isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME, telemetry off, and
`codex`/`claude` stubs first on PATH that exit 1 (no provider account, no real Codex); project Alpha
(`projects/alpha`, a two-file git repository) with thread "Wrap demo" holding one user message of three long
paths (`message.dispatch`, `defer_start`). Pairing links were minted with `bin.mjs pair` and typed by the
driver, never printed. The SSH fixture: a second lane server on 16642 as the "remote" (its `t3` shim refuses
`serve`, so nothing can bind 3773), `macos/tests/ssh/fake-ssh.sh` as `T3_SSH_COMMAND`, `T3_SSH_HOME` with
one `devbox` host.

Drives, under `.t3-live-drive-lock` (15:23–15:30), agent sessions (credentials in memory, no storage),
dry-run first (`--dry` printed every call):

```
BEFORE (t3-code-evidence-base @ 887b2491b)
  attempt 1: failed at its first `clock +1500 real`: "the clock cannot go backwards (6100.0 → 6099.999999999998)"
             (scripts/agent.mjs; QUEUE line added). The script then waits on the wall clock and settles.
  retry:     X10 1280×840 screenshot; transcript content 1160×495.75
             pr search "fix": applied after 250 ms; "fixes": 250 ms (typed on the poll's 250 ms grid)
             terminal close: not reached (the script asked for `panel-toggle-right`; the toggle is `toggle-right-panel`)
AFTER (this branch)
  session:   X10 1280×840 screenshot; transcript content 1160×495.75; the bubble crop is pixel-identical to Before
             pr search "fix": 250 ms; "fixes": 250 ms; "fixed" typed 130 ms off the grid: 250 ms
             terminal close: not reached (same id); 840×620 capture covered by the lane's provider toasts
  retry (terminal only, id fixed):
             surface-chooser → surface-terminal → close-tab-terminal:term-1 → app-confirm
             "Close terminal "Terminal 1"?" shown after 10 ms
```

The Before session budget (one session plus its retry) was spent, so the off-grid search and the terminal
close were not taken on the base. The same constructs were measured in a scratch probe on the web target
(`task … every(500)` / `every(250)` polls as the base has them against this branch's gated tasks; five
phases, 10 ms steps; not committed):

```
offset   0: terminal poll 500 ms, gated 1 ms; search poll 250 ms, gated 250 ms
offset 130: terminal poll 170 ms, gated 1 ms; search poll 420 ms, gated 250 ms
offset 260: terminal poll 210 ms, gated 1 ms; search poll 460 ms, gated 250 ms
offset 370: terminal poll 140 ms, gated 1 ms; search poll 390 ms, gated 250 ms
offset 490: terminal poll 450 ms, gated 1 ms; search poll 450 ms, gated 250 ms
```

Live SSH row, attempt 1: not run (at 15:30 the screen was locked, `CGSSessionScreenIsLocked` true; the lock was
released at once).

Live SSH row, attempt 2 (16:08–16:15, under `.realinput-lock`). Lane bundle copies of the base build
(`887b2491b`: T3Ssh's `willTerminateNotification` observer, framework before #200) and the branch build, each
with its own bundle id (`com.exact.t3code.laner4.before` / `.after`, ad hoc signed) and two saved SSH targets
(`devbox`, `devbox2`) written to that domain, so each launch reopens two tunnels. Each copy was started by
exec'ing its executable with `T3_SSH_COMMAND=macos/tests/ssh/fake-ssh.sh`, `T3_SSH_HOME` (a lane `~/.ssh`) and
the fake's remote home and `t3` shim, so the real `/usr/bin/ssh` and `~/.ssh` were never used. A tunnel is the
fake's `node` forwarder, a child of the app; the "remote" is a lane HTTP server on 16643 (the reference server
without its web bundle answers `/` with 503, which the module's readiness check refuses, so a first launch's
tunnels were torn down at their deadline; that run is kept as `live/after-attempt1-503`). Keys were posted to
the lane pid only (`CGEventPostToPid`, a scratch tool that refuses any pid whose bundle id is not a lane copy);
`orca computer hotkey` refused (AX reads blocked), and that run's app exited without a recorded cause, so it is
not counted (`live/after-attempt2-orca`). The app and tunnel pids and the tunnels' listeners were polled every
50 ms; each run idled 8 s first (one After run idled 20 s and stayed up).

```
BEFORE (observer)  ⌘W on the last window → app gone 348 ms, tunnels :46055 :44159 gone 348 ms, no listener
                   Apple Event quit      → app gone 103 ms, tunnels :44159 :46055 gone 103 ms, no listener
                   ⌘Q ⌘Q (double press)  → app gone 557 ms, tunnels :44160 :46056 gone 557 ms, no listener
AFTER (#200)       ⌘W on the last window → app gone 359 ms, tunnels :44161 :46057 gone 359 ms, no listener
                   Apple Event quit      → app gone 224 ms, tunnels :44159 :46055 gone 224 ms, no listener
                   ⌘Q ⌘Q (double press)  → app gone 588 ms, tunnels :46056 :44160 gone 588 ms, no listener
AFTER control      SIGKILL (no teardown) → tunnels alive, ppid 1, still listening on :44161 :46057 (then killed by recorded pid)
```

The clone quits when its last window closes, so ⌘W is a quit too. The branch ends its SSH children through
`destroy()` on every orderly exit, as the base did through its observer; the control shows an exit without
teardown leaves them running. The lane copies, their preference domains, data roots and plists were deleted
after; the user's `com.exact.t3code.macos` domain hashed `acc150dd…` before and after; no Keychain item was
written (no environment was paired).

## Evidence

| Scenario | Before (`887b2491b`) | After |
| --- | --- | --- |
| PR search debounce (agent clock) | 250 ms when typed on the poll's grid; the poll gives 250–460 ms off it (probe) | 250 ms at every phase |
| Terminal close confirm | probe: 140–500 ms after the request (500 ms poll) | 10 ms in the clone (agent step), 1 ms in the probe |
| User message with long paths at 1280×840 (X10) | paths fit or break at a space | identical (no visible change at this width) |
| User message with long paths at 840×620 (X10) | the two long paths do not break; they run past the bubble and the window | pixel-identical |
| SSH tunnels at ⌘W / Apple Event quit / ⌘Q ⌘Q (X6) | both tunnels gone with the app (348 / 103 / 557 ms), through the observer | both tunnels gone with the app (359 / 224 / 588 ms), through `destroy()`; SIGKILL control: orphaned |
| Week start in the macOS data runtime (X36) | `undefined` (`Intl.Locale` throws there; #204's before column) | Chrome's (Hermes test against Chrome) |

Record: `https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r4/02-ssh-quit-record.txt` (the SSH runs).

Image: `https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r4/01-x10-wrap-1280-before-after.png`
(the user message at 1280×840; Before and After draw the same lines).

X10 pair at 840×620 (16:21:40–16:22:27, under `.realinput-lock`; one agent session per build, both builds
checked fresh by the driver's own receipt check, the branch rebuilt at `fabb7b19b` first): `dismiss-provider-warning`,
`toast-dismiss-2`, then `toast-dismiss-1` closed the banner and both toasts in each build. The two screenshots are
pixel-identical. Image: `https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r4/03-x10-wrap-840-before-after.png`.

Finding for a follow-up (not this task): at 840×620 the two long paths run past the bubble's right edge and off the
window in both builds. The clone's `UserMarkdown` paragraphs set no `overflow-wrap`; compare with a reference
capture of the same message before changing it.

## Next action

None. Merged into `feat(example)/t3-code` by #218 (`80eccbe63`, squash, 2026-10-07). The squash dropped main `463acda68` from the branch's ancestry (its content is on the branch); round 5 records it with `git merge -s ours` before merging main again. The overflow finding above still needs its own task.
