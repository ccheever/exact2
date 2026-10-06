# Independent review — terminal focus and reconnect resize repairs

2026-10-06. Reviewer: separate Electron-oracle agent; did not implement either repair. Scope: four-file working-tree diff against `7caade7a8781779e81a3610f3df1fa188a7cbabd`. No tracked edits or UI launch performed during review.

**Verdict: no blocking findings in these two scoped repairs. This is not a full terminal-parity approval.**

## Correctness

- **Focus:** initial focus request is fulfilled when the native web view first enters a window. WebKit ready now forwards focus to the page only if the current native responder still belongs to that terminal and the view is active. This removes the stale positive-request counter's ability to reclaim focus from the composer. Explicit later focus requests remain handled by setProps. One-shot mount callback clears itself and captures the instance weakly.
- **Reference match:** in my original Electron run, immediate composer clicks after reopening retained Message/contenteditable focus at both immediate and two-second checkpoints, twice (`electron/focus-race.json`). The new guard addresses the parent agent's observed native divergence.
- **Resize:** desired grid is kept separately from last successfully applied grid. A failed RPC no longer poisons deduplication; reattach invalidates applied-size knowledge, and its snapshot reissues the desired grid without waiting for another fit event. Existing single in-flight request/pending latest-grid behavior remains intact. No unconditional failure retry loop added. Positive size validation and 1000×500 clamps remain.
- **Ordering:** reviewed transport terminalCall, terminalAttach and retire. Transport retirement completes outstanding RPC callbacks before notifying terminal stream retirement; callbacks reach the main queue, consistent with session state ownership and reconnect reapplication.

## Evidence checked

- `focus-fix-tests.log`: 16 terminal tests, one skipped, zero failures. New tests exercise actual WKWebView loading with composer focus moved before ready, preserve typed composer text, check a later explicit focus request, and retain expected focus when opening normally.
- `resize-repro/result.json`: pre-fix reported lastSize 120×40 while server stayed 100×30, pass=false, two attaches.
- `resize-repro/result-fixed.json`: post-fix sends 120×40 after reconnect and serverGrid equals expectedGrid, pass=true, two attaches.
- `resize-repro/transport-tests.log`: 51 transport tests, zero failures. New checks cover disconnected resize then attach snapshot without another fit, success deduplication, failure retry without looping, and latest pending grid replacing intermediate fits.
- `git diff --check`: clean during review.

Tests were inspected, not independently rerun by this reviewer. Parent owns final rebuilt-app recapture/source consistency checks.

## Remaining limits / risks

- New transport XCTest injects the production snapshot reducer rather than driving a full server reconnect itself; the separate before/after reconnect reproduction supplies complementary evidence.
- No new test specifically covers a terminal becoming inactive or moving between windows during initial load. The ready-time active/responder guard limits stale-focus risk; this omission does not block the observed composer-focus repair.
- This review does not certify IME composition, normal-launch credential persistence/reconnect, worktree environment parity, authorization refusal, deletion cleanup, or complete RPC/Ack trace equivalence. Previously reported parity limitations remain.

## Reviewed SHA-256

| Path under examples/t3-code | SHA-256 |
|---|---|
| modules/apple/T3TerminalView.swift | 1b5cbe36504b3d294e70184b172063ed72431a504b97d828997765dd87d8e1a9 |
| modules/apple/T3TerminalSessions.swift | 770c4a74230c697d91e7266bd23784bdad4d4a9c28f1839d0f80154498b538ac |
| macos/tests/terminal/drawer.swift | 50806dd48da67f62583fcb6ba9654e8a1b83ecb5b059bd907ed39ec156db60b5 |
| macos/tests/transport/terminal-streams.swift | e22a8b67189bcc49c59a663b360af549e0c23e463ba6cfa27bac7fcb49d19083 |

## Evidence correction and baseline confirmation

The initial native live focus-race recipe was confounded by asynchronous drawer opening: its immediate state still had the drawer closed, so later terminal focus could legitimately come from the pending open command. That recipe is **not isolated proof of the WebKit-ready focus defect**, and the earlier reference-match paragraph must not be read as establishing causality from that live comparison alone.

The defect is instead established by the actual WKWebView regression run against the original HEAD implementation: `fixes/baseline-focus.log` fails `testLoadingTerminalDoesNotStealComposerFocus` at drawer.swift:62 (`window.firstResponder === composer`), one test/one expected failure. The same test passes with the reviewed repair in focus-fix-tests.log. This before/after regression is the causal evidence.

Reviewed corrected live recapture `native-fix/result-4.json`: the open drawer is established before composer input; state entries 4 and 6 both report responder TextArea/editor 149 (the latter after the delay). This supports fixed app behavior without claiming the live action isolated WebKit's readiness interval. Original Electron's twice-stable composer observations remain valid as observed behavior.

Also reviewed native-fix/06-settled-grid.png: settled shell output is `21 129` after 400-height resize. The earlier `14 129` read taken immediately after dragging preceded asynchronous grid fitting and is not evidence of a persistent resize defect. No source changes occurred for these evidence qualifications. Scoped no-blocking-findings verdict remains unchanged.
