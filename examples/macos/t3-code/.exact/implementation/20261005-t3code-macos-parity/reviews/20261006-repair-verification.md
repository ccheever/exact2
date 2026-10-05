# Independent repair review, 2026-10-06

Final independent review, including mounted boundary and silent-failure supplements below. This record does not authorize task closure. The reviewer made no implementation changes and read the three task records, parent specification's pixel-loop waiver, Exact verify/workflow instructions, previous independent review and the repair diff over `7444d5999`. The final verdict at the end supersedes intermediate checkpoints.

## Findings resolved or reclassified

- The original native-menu observation used an invalid timing procedure. The unchanged production `T3ContextMenu.swift` returns the correct selected item and cancellation promptly in the [causal reproduction](../evidence/tab-menu/20261006-menu-run-loop-repair/attempt.md). Adding a common-mode agent-style nested settle delays return until the wait unwinds. Source/fixture inspection supports this cause; no production menu change is justified. This reclassifies the earlier observation, but does not prove mounted Copy path, rename, bulk-close or relaunch acceptance.
- The output scroll now has focus/blur handlers, a visible focus indicator and keyboard scrolling in an app-owned component. Review caught a repeated-target problem in an intermediate implementation: after wheel movement, assigning the same previously requested offset could omit a new scroll prop. The worker changed it to observed offset state. Boundary behavior after clamped requests still requires runtime confirmation.
- Timestamp focus now reveals the timestamp and preserves its tooltip on focused-trigger scrolling. The initial attempted `tabIndex` exclusion compiled in a manually constructed host fixture but was rejected by the actual Contract compiler; it was removed. Host support is not compiler support. The corrected native fixture preserves the real hidden hook and explicitly reports that it is an extra Tab stop. The previous generic tooltip already used this hidden press hook; this repair does not claim to resolve that limitation.

## Evidence examined

The supported-source [checks](../evidence/parallel/20261006-repair-verification/checks-supported/report.json) pass all seven commands, including 1,197 Bun tests, strict TypeScript, Contract compile, Rust formatting, root build, clippy and boot. A fresh independent runner comparison returned `matches: true`, `source_matches: true`, `recipe_matches: true`. Root tests, ten app Rust tests, complete native bundle and staged caps also pass in preserved logs. Independently validated the [regression manifest](../evidence/parallel/20261006-repair-verification/regression-proof/manifest.json): integrity valid, no mismatches. Mounted acceptance and final source comparison are still required for final verification; a passing component recipe is not a whole-task verdict.

The new native keyboard fixture tests real ExactKit nodes, Tab/Shift-Tab and delivery of focus, blur and key events. It does not execute the Contract action or prove output scrolling in the mounted app. Its output explicitly reports the hidden dismissal hook's extra Tab stop. Prior failed evidence remains preserved.

Independently ran and inspected the activity [capture assertions](../evidence/20261005-client-activity-reporting/20261006-server-effects-and-reference/assert-capture.py). Both real reference servers accepted production native transport/reporter/fleet requests. Baseline provider timestamps stay fixed, foreground timestamps advance over successive intervals, and timestamps stay fixed after 48 seconds hidden. Accepted diagnostics scope appears and disappears; B disconnect/reconnect changes its accepted lease as expected. I also checked A and B lease client IDs match. The assertions pass against current production hashes. Reference comparison executes the original payload function with controlled DOM facts; it is function parity, not Electron-process parity. The native harness has no toast surface and does not run the root Contract retry.

Reviewed source SHA-256 at this checkpoint:

| Path relative to app | SHA-256 |
| --- | --- |
| `timeline-work.contract` | `c5823e3d46688422d7b789c46a808492861eaa9b759c49a9a07591292665935e` |
| `shell-tip.contract` | `f99ba9a75d2191754ef60f2d9e196077f2e69307bd03920159be15a6726c2206` |
| `apple/tests/timeline-keyboard/main.swift` | `005bbb2b5a5dd40d986c02498431b41740a4c415ff089c2f7ce4b013f43f6283` |
| `modules/apple/T3ContextMenu.swift` | `a868b4534cdfac00c96b97c626a2c3472ea8033244ec268d7d2293fdfb2d6bb8` |

## Outstanding acceptance

The actual app [workspace retry capture](../evidence/20261005-client-activity-reporting/20261006-root-clock-retry/attempt.md) now establishes completion at clock 1000, no retry after another 9000, exactly one same-key/provider/cwd retry at 11000 and next deadline 21000. Independently inspected and executed its source-checking assertion: passed. It uses an existing thread instead of a draft, correctly disclosed, with the same root scheduler. Partial snapshot preservation is separate unit coverage.

The [mounted tab attempt](../evidence/tab-menu/20261006-mounted-acceptance/acceptance.md) establishes five distinct surfaces, expected device menu order, inactive close preserving selection, and active close selecting the surviving neighbor. Rename input was inconclusive, not a proved app defect; the agent corrected earlier wording that attributed it definitively to input delivery. Clipboard, rename, bulk and relaunch acceptance remain outstanding in that attempt.

Mounted timeline capture, remaining tab input, main-app silent failure and final source/evidence checks are still in progress. Activity also lacks the specified Electron trace scenario and measured 0.3-second focus/input latency; source-function payload parity does not prove lifecycle timing. No task is currently approved for closure by this review. No framework issue is resolved by application-only repairs.

## Subsequent functional repair

The [mounted timeline attempt](../evidence/20261005-upstream-timeline-and-markdown/20261006-repair-verification/mounted-acceptance.md) establishes known-skill rendering with negative exclusions, original-source clipboard copying, forced RPC error and missing output. It also found a real defect: two quick disclosures left the second collapsed and did not show loading while the first RPC was delayed. Global command gating and awaiting network work inside disclosure caused this behavior.

Reviewed the subsequent changes to `app.contract`, `timeline-presentation.ts`, `timeline-item-fetch.ts` and its test. Disclosure is now a local command that marks keyed open state synchronously. The existing root read mutation admits later rows through a changed signal and retains ownership of all pending native promises, including reads for rows closed while pending. No detached native work was introduced. Independently ran the focused suite: 12 tests, 55 assertions, zero failures, including B completing before A and closing A before its late reply. No blocking source finding in this change; actual mounted rerun remains required. The earlier supported-source fingerprint is now historical and cannot verify these changed paths.

The new [detail-flow checks](../evidence/parallel/20261006-repair-verification/checks-detail-flow/report.json) pass with unchanged source, including 1,198 app tests. An independent comparison against this later report returns all three identity flags true. This later report supersedes the earlier fingerprint for the changed source. The known-skill screenshot was independently viewed: positive Verify chip, plain price/code/link cases and expected file-label rendering are visible; the unrelated notification overlay does not obscure those assertions.

## Native icon runtime correction

The next mounted run exposed `Cannot read property 'set' of undefined` after a native icon URL response. The worker reduced it to pinned Hermes capturing an async IIFE's per-iteration values incorrectly when the `for...of` loop advances. Independently executed the original compiled reduction: the exact TypeError is reproduced at `destination.set`. Independently compiled the tracked [corrected reduction](../evidence/20261005-upstream-timeline-and-markdown/20261006-hermes-icon-capture/after.js) with the app's provisioned Hermes compiler and ran its lean engine: `PASS 2`, preserving two distinct keys with skipped iterations.

Reviewed `loadNativeToolIcon`, the app-only correction: explicit helper arguments retain each request's key, origin, prior asset and destination maps across await. Cache behavior, request cleanup and root invocation ownership are otherwise preserved. No blocking source finding; mounted verification and a new source report remain necessary. This diagnoses the runtime failure without attributing it to the new disclosure drain, and does not claim an upstream framework fix.

## Final mounted findings

The [icon-fixed mounted packet](../evidence/20261005-upstream-timeline-and-markdown/20261006-repair-verification/icon-fixed-agent/observations.md) runs the later source whose `checks-icon-capture` report independently compares with all identity flags true. Native icon URL minting no longer raises the Hermes error. Actual Tab reaches output after the two hidden hooks and timestamp, Space/Enter closes/opens the disclosure while retaining focus, and native output offset changes from 0 to 40 on ArrowDown. These resolve the original skipped-output defect and verify useful parts of the repair.

Two runtime findings block timeline verification:

1. **Keyboard scrolling sticks at the bottom after overshoot.** Independently inspected native layout records: output scroll node 1623 reaches 334.88, remains there after three additional Down presses and still remains there after Up. Expected immediate movement upward by 40. This confirms the earlier source-review risk: controlled `top` grows beyond native bounds without movement feedback to correct it.
2. **A newly opened row does not begin its read during the active read mutation.** The corrected concurrent AX experiment opens B more than thirteen seconds before delayed A returns. Both rows visibly expand and show loading, but the trace has only A's request and B remains loading at mutation completion. This is stronger than the earlier invalid sequential-clock experiment. Prompt disclosure is repaired; the claimed admission of B into A's in-flight read is not established by the actual runtime and the observed late scheduling remains. Bun's concurrent-drain assertion does not supersede this result.

The app's newly documented actual-host keyboard compilation recipe was also reviewed: separating it from the module-only XCTest loop correctly matches its dependencies. No additional source issue found in that documentation.

## Per-task verdict at this checkpoint

| Task | Verdict | Reason |
| --- | --- | --- |
| Right-panel tab menu | **Blocked** | Earlier menu delay was a driver artifact; mounted identities and single-close policy pass. Rename, bulk, clipboard responsiveness and renamed relaunch remain unverified. The normal-launch fallback stopped in `T3Credentials.save` → `SecItemAdd` authorization before the connected scene. No app-menu defect is inferred from that prerequisite. |
| Timeline and Markdown | **Failed** | Original focus and native-icon exception are repaired, but the two concrete runtime findings above remain. Focused-tooltip/real-wheel and complete icon-kind acceptance also remain unproved. |
| Client activity | **Blocked** | Server policy, fleet/scopes, payload-function parity and actual root retry have substantial proof. Required Electron lifecycle trace and measured 0.3-second reaction latency remain unproved; final-source retry/silent-failure evidence is being finalized. |

No task or framework issue is approved for closure by this review. Passing build and logic checks are retained as regression evidence, with the failed and blocked runtime outcomes kept distinct.

## Targeted second repair under verification

The coordinator authorized a second targeted correction for each of the two runtime findings. Reviewed the replacement: a dedicated fast disclosure mutation refreshes data, then dispatches two independent root read slots. Each source invocation atomically claims one eligible, non-pending row before awaiting it. The previous cross-invocation wake/drain is removed; no native promise outlives its source invocation. More than two requests queue for the existing root tick. Keyboard Down/Space now clamp state to measured content height minus viewport height, preventing the reproduced overshoot; ArrowUp subtracts from that bounded state.

Independently ran the focused item/icon tests: 21 pass, 82 assertions. The [two-slot report](../evidence/parallel/20261006-repair-verification/checks-two-read-slots/report.json) passes with unchanged source; a fresh independent comparison returns `matches`, `source_matches` and `recipe_matches` all true. No blocking source finding, but the two failed mounted assertions must be rerun before changing the timeline verdict above.

## Two-slot capture interpretation corrected

Read the runner and host scheduling implementation after the two-slot replay. Its late B request **does not establish a production concurrency failure**. `runner/src/runner/commit.rs::arm_then` records `then_due = now` and explicitly waits for the next clock advance. `Agent.swift::advanceStepped` waits for pending replies before advancing when a timer is due; that wait pumps I/O without advancing the clock. `ExactSession.scheduleClock` suppresses automatic clock progression in any agent mode, including platform timing. Thus AXPress can complete the disclosure while A is pending, but B's `then dispatchTimelineReads` remains armed until A returns. This is consistent with the exact observed request timestamps and differs from a normal continuously advancing host.

The two-slot overlap criterion is therefore **blocked on a suitable real-time fixture**, not a confirmed production defect. The normal activated fixture is separately blocked at Keychain credential saving. Earlier source versions' observations remain historical; they are not a verdict on the final two-slot code. No generic serialization of distinct mutation targets was established: the runner explicitly keys equal-argument calls by distinct targets.

The frame-clamp replay did expose a separate real defect: it moved from bottom to zero. Independent source inspection found `frame()` resolves `id` using `kernel.find_by_id`; `testId` alone is not a geometry identity. Missing frame IDs yielded unavailable/zero heights. The minimal last correction adds matching `id` attributes to output and content. Its mounted boundary result is still pending; no formula or framework change is needed to explain this failure.

## Final source review and verification verdict

Reviewed the final matching `id` attributes; they name the same unique per-row strings used by `frame()`. Independently compared current source and recipe with [checks-frame-ids](../evidence/parallel/20261006-repair-verification/checks-frame-ids/report.json): `matches`, `source_matches` and `recipe_matches` are all true. The report passes with unchanged source. No unaddressed blocking source-review finding remains; that is distinct from completing acceptance.

| Task | Final verification verdict | Outstanding evidence |
| --- | --- | --- |
| Right-panel tab menu | **Blocked** | Required mounted rename, bulk actions, clipboard responsiveness and renamed relaunch. Normal connected fixture stopped in Keychain saving; no menu source defect proved. |
| Timeline and Markdown | **Blocked** | Corrected real-time overlapping requests cannot be judged with the frozen-clock fixture. Final geometry-ID boundary replay passes as recorded below. Full focused-tooltip/real-wheel and icon-kind acceptance remain unproved. Earlier source-version failures do not establish a current production failure. |
| Client activity | **Blocked** | Required Electron lifecycle scenario and measured 0.3-second reaction latency remain unproved. Final-source retry and silent-failure assertions pass as recorded below. Proven server policy and root-clock behavior are not discarded. |

No task or related framework issue is eligible for closure. Do not label the whole repair wave failed merely because acceptance is blocked; equally, the static passes do not establish complete feature verification. This review supersedes the intermediate failed timeline checkpoint only for the final source and corrected fixture interpretation above.

## Final boundary supplement

Independently inspected the final named-bundle [frame-ID replay](../evidence/20261005-upstream-timeline-and-markdown/20261006-repair-verification/frame-id-agent/observations.md), executable helper and native layout records. The helper requires actual long output before measuring. Independently extracted the output scroll ancestor offsets: bottom 334.88, three Down presses 334.88, then Up 294.88. This passes the exact previously failing assertion and clears the final known output-scroll defect. The two-slot overlap and other remaining acceptance limitations above are unchanged. Activity's final silent-failure evidence is reviewed below.

## Final activity supplement

Independently inspected and executed both final activity capture assertions. [Root deadline](../evidence/20261005-client-activity-reporting/20261006-final-root-deadline/attempt.md) passes with current activity source hashes: completion 21000, no new request at 30000, exactly one same-workspace request at 31000, next deadline 41000. The later timeline-only geometry IDs do not alter this scheduler.

[Silent backend failure](../evidence/20261005-client-activity-reporting/20261006-final-silent-failure/attempt.md) passes on the final named bundle: initially connected, then disconnected after its recorded isolated server was terminated; 71,746 ms elapsed including a 27-second explicit observation. Existing toast IDs remain unchanged, no new toast appears, and captured logs contain no activity-report refusal. Its bundle receipt and current activity hashes match. It exercises failure of the selected backend in the main app; fleet B lifecycle is separate earlier evidence, correctly disclosed. These passes clear the pending activity supplements, while the precise latency and actual Electron lifecycle requirements keep the task blocked.

Final outcome: **all three task records remain active with verification blocked**, for the distinct remaining acceptance prerequisites in the final verdict table. No known failure from an earlier implementation is asserted as a current production failure without matching current evidence. No task/issue closure is approved.
