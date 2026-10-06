# Terminal repair verification, 2026-10-06

Implementation commit: `4e65300c1b715befeba8c42db9488b16864bbcdd`. The committed checkout matches the passing repair source fingerprint.

Two reproduced defects were repaired. The repair checks pass; this is not a claim of complete Electron parity.
The original comparison remains at [PR #175, prior evidence](https://github.com/ccheever/exact2/pull/175#issuecomment-6017408751).

## Changes and proof

| Defect | Before | After |
| --- | --- | --- |
| Late terminal readiness steals composer focus | New actual WKWebView regression compiled with original `7caade7a` view fails at `drawer.swift:62`, first responder no longer composer. [Log](baseline-focus.log) | Focus requested at mount; ready completes only while terminal owns responder. Initial open, composer retention and later explicit request pass in terminal XCTest. Corrected live capture confirms drawer open before typing and TextArea remains after 1.5 seconds. [State](native/result-4.json), [image](native/05-open-composer-focus.png) |
| Disconnected resize remains stale after reattach | Executable with original production session class: reported size120×40 while server stayed100×30 after two attaches. [Result](resize-repro/result.json) | Track requested and successful sizes separately; reapply requested grid on attach snapshot. Server becomes120×40 without another fit. [Result](resize-repro/result-fixed.json). Loopback WebSocket regressions cover failure retry, duplicate success suppression, and latest pending size. |

The standalone resize reproduction substitutes its transport and view, so it is not a real-server reconnect capture. The transport XCTest uses the production WebSocket transport with a loopback fixture and delivers the attach snapshot through the production reducer. See [reproduction instructions](resize-repro/README.md).

## Actual macOS app

Built using `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle` with Bun1.4.2. [Build log](native-build.log).
Launched the rebuilt app with the supported agent,1280×840, isolated named storage, pinned server1e2ecbd975 at127.0.0.1:16321, isolated HOME/Codex/Claude/XDG/T3 directories, `/bin/sh`.
Reused the prior disposable project and three threads. Authentication credentials and full state dumps are excluded.

- Opening focuses WebView. [State](native/result-1.json).
- Opening, advancing100ms, asserting drawer open, then typing composer preserves TextArea focus after1500ms. This timing check proves the observed live sequence; the actual WKWebView unit regression isolates loading deterministically.
- Shell `printf`, `stty size`, and `touch repair-proof` execute; the file exists in the fixture project.
- Drag−120 changes drawer280→400. Immediately submitted `stty` still returned14×129; after settling, it returned21×129. [Settled image](native/06-settled-grid.png).
- Terminal-focused ⌘J closes and focuses composer. [State](native/result-3.json).
- Close dialog text matches; Cancel retains drawer; Confirm closes and focuses composer. [State](native/result-6.json), [image](native/07-close-confirmed.png).

Evidence correction: the previous live focus-race sequence and its immediate repetition here typed before the asynchronous open had completed. Those sequences alone do not isolate late-ready focus theft. [Retained confounded sequence](native/result-2.json). The baseline WKWebView failure above supplies the direct before-fix proof. The corrected live sequence checks that the drawer is already open.

## Checks

- Bun app suite:2115 passed,1 skipped,0 failed.
- Cargo root suite:2928 passed,18 ignored,0 failed.
- Root build,Clippy,format,boot,caps pass.
- Terminal XCTest:16 executed,1 expected scale-test skip,0 failures.
- Transport XCTest:51 executed,2 expected live-server skips,0 failures.
- [Independent review](independent-review.md): no blocking findings in the two repairs.

The first root test run picked Bun1.3.14 for child processes and failed two version checks. Preserved under `checks/`. Setting PATH to the pinned Bun1.4.2 yielded `checks-pinned-bun/`. No product change was made for this environment failure.
`repair-recipe.json` contains explicit Swift compile/run and live-state assertions. `recipe.json` contains root/app checks. Both reports fingerprint the scoped app and Apple-host sources. Evidence integrity is indexed under `manifest/`.

## Limits

The prior comparison covers more behavior on the unchanged Electron oracle, including flood output, three-thread switching, bounds, themes and font settings. This repair capture is macOS agent mode only. Normal-launch credential/storage persistence, physical Korean IME, old three-scope authorization, live thread-deletion cleanup, worktree environment parity, huge paste, full motion/reduced-motion and complete RPC/Ack equivalence remain unverified. Normal-launch persistence cannot be inferred from the agent's per-process native store. Tabs/splits remain another ticket. Full terminal acceptance therefore stays open.
