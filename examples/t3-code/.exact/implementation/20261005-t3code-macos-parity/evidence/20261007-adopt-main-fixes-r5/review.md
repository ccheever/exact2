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
