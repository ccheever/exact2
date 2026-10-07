# Independent review, 2026-10-07 (round-5 follow-up)

Reviewer: a separate agent. It was given the uncommitted fix (`let-go.ts` `letGoAware` refuses `watch()` once its
answer was let go; a `let-go.test.ts` test through `client.refresh`), the real-input record 06, the author's diagnosis
and the diffs between the builds (`38352ceaf`..`364f5baa6`: the clone's shell files, `app.contract`,
`js/src/prelude.js`). It was not told an expected verdict and changed no tracked file. It ran
`bun test examples/t3-code/let-go.test.ts` (9 pass) and `bun test examples/t3-code` (2318 pass, 1 skip, 0 fail).

Verdict: PASS, no blocking findings.

| # | Finding (non-blocking) | Resolution |
| --- | --- | --- |
| 1 | `letGo` reads only the kind, so the two Aborted failures that are not a let-go (`host/apple/src/executor_core.rs` "native work panicked", `runner/src/request.rs` "the owner ended without a reply") were already hidden for `later()`, and are now hidden for `watch()` too | Recorded as a follow-up finding in the task record. It is not this regression: the clone cannot tell those failures from a let-go without a framework field |
| 2 | Let-go paths this fix does not cover. `auto-balance.ts` `retargetDraft` toasts "Could not switch machine" with no `letGo` check. `settings-b-outdated.ts` `probeDescriptors` caches a let-go probe as no descriptor until the page closes. Prelude calls outside `native` (crypto in `composer-editor.ts` and `settings-a-hosts.ts`, `terminal-drawer-view.ts`, a plain `fetch` in `r5-composer-paging.ts`) still throw a plain "outside an answer" error after a let-go. The reviewer traced one and found it rejects only the discarded answer | Recorded as follow-up findings in the task record, not fixed here: none of them was observed, and the coordinator asked for this regression only |
| 3 | The test goes through the real `client.refresh` → `fleet.sync` → `readLocalBackend` path. Against `HEAD`'s unfixed `letGoAware`, it fails (`watchedAfter` 1), and `client.error` is "native.watch outside an answer". It simulates the let-go and the prelude's refusal. No test checked `watch()` directly | Added: the seam test now checks that `watch()` passes through before the let-go and is refused as `superseded` after it |
| 4 | The mechanism holds. The clone files on the path are identical in both builds. `forget_calls` (`js/src/turns.rs`) runs a let-go answer's continuation with no answer current. The prelude diff only adds database tracking. #219 (`6af680b0e`) acts only through the app's own use of `hasFocus`. No framework commit is a likelier cause | Recorded: nothing filed |
| 5 | Calling round 5 the reason only the after build showed the banner is weak. Focus re-asks shellView, the details card and SnapShot settings, not the `data` snapshot. The race exists in both builds, there was one session per build, and the after session did an uncontrolled extra step (Snooze before its quit). Chance explains the result as well | The task record and the PR call it a hypothesis and name the uncontrolled step |

Runner: attempt 1 (`attempt1-report.json`) passed every check that runs here, with `source_unchanged: true`. Its
two required live checks are blocked. Attempt 2 (`attempt2-report.json`) reran it after finding 3's test.

## Storm-fix reviews, 2026-10-08

Three reviews by separate agents. Each was given the commit, the reference (T3 Code 1e2ecbd975), the observed sessions
(records 07 and 08 after) and the menu-stall repro. None was told an expected verdict, and none changed tracked files.

**Review 1, `2525124ca`** (the stream for status, the 64-pending cap removed): PASS, no blocking findings.

| # | Finding | Resolution |
| --- | --- | --- |
| 1 | The strip hid before the snapshot and after a stream failure, a switch could show the stale branch, and the 3 s retry ran on a 60 s clock | 3373f7919 and 3e939ecaa: assume Git as ChatView does; after a switch, the switched name and a resubscribe |
| 2 | `vcs.listRefs` also went out on every tick when slow (card row, strip picker), the same let-go pattern | Shared reads (3e939ecaa) |
| 3 | The menu stall comes from opening a menu inside a main-queue block; the clone's own menus do that too | Repro variant confirmed it; `T3MenuTurn` (3373f7919); ExactKit's `MenusMac` reported |
| 4 | The test drove only the closed card | The open card added |

**Review 2, `3373f7919`** (`sendRead`, a 3 s resend): FAIL.

| # | Finding | Resolution |
| --- | --- | --- |
| 1 | Blocking: the strip got the boot-relative `shellClock`, `Math.max` with `composerNow` (wall time, 60 s steps) ignored it, and a frozen clock never resends. The picker could stay on Loading for up to a minute | `sendRead` removed; reads are shared in the transport (3e939ecaa), with no clock. The test "the picker leaves its loading state when the shared reply reaches the answer that joined it" fails on 3373f7919 |
| 2-6 | The card could stall once the tick stops. A failed read flips the strip. The switch blanked the card. `stripShortcuts` ignored assume-Git. `runModal` still stalls | 2, 4, 5 fixed by 3e939ecaa. 3 kept: a failure still hides the strip, as before this task. 6 recorded as a follow-up |

**Review 3, the shared reads** (3e939ecaa before commit): PASS, no blocking findings. The join chain answers every caller
on reply, timeout and retire. A probe whose fake behaves like T3Transport saw one send and a loaded picker. Eight ticks
across card and picker states sent nothing per tick. Follow-ups applied before the session:
- a read after a write could join a read sent before it, so a write naming a cwd ends the sharing for that cwd (e4087409d, XCTest);
- the joined-reply test was added.

Not applied: a shared reply over 512 KB hands one native transfer to every joined caller.

Runner: attempt 6 (`attempt6-report.json`) passed every check that runs here on `e4087409d`, with `source_unchanged:
true`. The two live checks were run by hand: records 07 (banner) and 08 (menu choice, cadence).
