# Live functional verification, 2026-10-06

Implementation: `8498fdc8a`, branch `daehyeon/t3code-parallel-features`; no production changes during verification. Bun1.4.2, current Mac, native AppKit build from the prior successful bundle. Exact `verify`, `prove`, and Orca `computer-use` skills used. No pixel-perfect comparison or repair loop.

## Reset and fixture

Isolated fresh agent stores under `target/t3-verify/gui/scratch`; native1280×840, default light appearance. The reference server was built from `1e2ecbd975` in an isolated export. Telemetry disabled; server16843 and transparent HTTP/WebSocket proxy16844. The Effect patch was updated to the exact pin and524 server dependency snapshots audited. See runtime-proof.json and dependency-audit.json. No real provider invocation: project/thread created with reference RPC and synthetic turn rows seeded into isolated SQLite. The source repository and user app data were untouched.

## Actions and observations

1. `clock settle` before inspecting startup reveals the onboarding overlay. Earlier landing-button refusals were targeting an inert background layer. A real `tap welcome-pairing-help` exposes `npx t3 pair`.
2. Through native controls, paste isolated pairing URL, Pair, Continue, Agents Continue, Skip Import. Select thread `verify-timeline`. Dismiss provider warning before expanding work: the banner otherwise overlaps the first work row. No production adjustment made.
3. Open command, failed-command and read rows. Actual `orchestration.getTurnItem` responses match native source state; screenshot shows command text, full output, nonzero exit2, descriptive prose plus file chips. Skill row was not successfully opened in this capture; positive catalog chips, failure/delay fixture and real-wheel tooltip cases remain unverified.
4. Real RPC activity trace shows successful server responses, stable client ID,25s cadence, focusfalse→true, recentlyInteractedfalse→true, and VCS scope removal when the panel changes. Native reporter/workspace suites separately cover cadence/dedup/reset policy. Server provider refresh effects, two-server integrated lifecycle and root10s retry were not established by this capture.
5. Open fixture file via Markdown chip. Native file tab and content load. Orca physical right-click at tab coordinate(792,26) opens menu with Copy path/Close/Close others/Close to the right/Close all; only inapplicable bulk actions disabled. Screenshot inspected.
6. AXPress Copy path leaves localChanged pending; a later unrelated action cancels its parked request350. A second physical click at Copy path(840,38) also exceeds `clock settle`20s bound with localChanged pending. The clipboard eventually contains correct workspace-relative `fixture.txt`. Initial absolute-path comparison was erroneous and is excluded from the verdict. The observed failure is delayed/unanswered action completion at the check boundary; clipboard correctness passes late. Cause remains unproven (native menu reply/continuation or driver run-loop interaction).

## Verdict and limits

Timeline fails required Tab access in the separate real ExactKit/AppKit key-loop test. Tabs fail timely native-menu completion; remaining mounted device rename/bulk/relaunch cases need a clean passing workflow after diagnosis. Activity remains blocked on unexecuted integrated acceptance, despite component and wire checks passing. No task or framework issue is eligible for closure from these results.

`checks/report.json` preserves the first capture assertions. `checks-final/report.json` corrects the clipboard expectation and separates late clipboard correctness from the pending-menu failure. These reports assert captured observations; they do not pretend to replay the GUI. Complete component recipes and the independent review are in sibling evidence/reviews. Screenshots and sanitized JSON are local, tracked evidence; no artifacts were published externally.
