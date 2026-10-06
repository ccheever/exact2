# PR #175: Electron terminal comparison, 2026-10-06

**Verdict: verification failed on one confirmed focus regression. Core drawer operations work in the exercised cases. Full Electron parity is not certified.** No implementation code changed during this verification.

Implementation: `7caade7a8781779e81a3610f3df1fa188a7cbabd`, PR #175. Original: T3 Code `1e2ecbd975`, real Electron 44.4.2 with its original main/preload/renderer and embedded server. macOS 26.6.2, build 25G83; Bun 1.4.2. Native app freshly built from this branch. See [independent review](independent-review.md), [source/check report](checks/report.json), and [original build/isolation report](electron/REPORT.md).

## Confirmed difference: delayed terminal startup steals composer focus

1. Open a project thread and its terminal; hide the drawer, then reopen it.
2. Before the terminal finishes loading, return to the composer and type `focus probe` without sending it.
3. Read focus immediately; wait 1.5 seconds without further input; read focus again.

Native [result-17.json](native/result-17.json), entries 7–10, records:

```text
After typing in composer: responder=TextArea, editor=149, logical=149
After clock +1500 real:   responder=WebView, editor=null, logical=6605
```

The screenshot shows the draft still in the composer and the cursor now in the terminal: [native capture](native/24-focus-typed.png). Electron retained the Message composer immediately and after two seconds in both trials: [focus observations](electron/focus-race.json), [Electron capture](electron/14-composer-retains-focus.png). The original comparator used real renderer clicks; the native reproducer used the app's input driver and verified the established native TextArea responder before waiting.

Code: `examples/t3-code/modules/apple/T3TerminalView.swift:299` focuses at page-ready whenever `focus-request > 0`. Original `ThreadTerminalDrawer.tsx:546–549` also checks that focus still belongs to the terminal mount. Independent review classifies this as P2. Earlier native click-only attempts did not establish composer focus and are preserved as inconclusive, not counted as reproductions.

## Live behavior comparison

| Scenario | Native Exact app | Original Electron | Evidence |
|---|---|---|---|
| Open and execute shell commands | Opens at 280; prints fixture cwd; creates `parity-native-proof` | Opens at 280; prints its fixture cwd; creates `drawer-proof` | [native output](native/02-command-output.png), [file effect](native/file-effect.json), [Electron output](electron/03-output.png) |
| Direct Unicode text | `한글 테스트` renders correctly | Same | [native](native/02-command-output.png), [Electron](electron/04-resize-400-hangul.png); this is not IME composition |
| Drawer drag | 280→400; min 180, max 630 at 1280×840 | Same | [native 400](native/03-resized.png), [min](native/16-min-height.png), [max](native/17-max-height.png), [Electron 400](electron/04-resize-400-hangul.png), [max](electron/05-resize-max.png) |
| Focused Cmd+J | Platform key delivery closes drawer and returns focus to composer | Closes and returns focus to Message composer | [native result-3](native/result-3.json), original report |
| Close dialog | Correct title/body; Cancel and Escape keep terminal; confirming sole terminal closes and focuses composer | Same behavior; sent close request has `deleteHistory:true` | [native dialog](native/11-close-dialog.png), [Electron dialog](electron/06-close-dialog.png), [native results 6](native/result-6.json) and [9](native/result-9.json) |
| Shell exit | `exit` removes drawer; composer receives focus | Same | [native result-8](native/result-8.json), original report; transient exit text not independently captured |
| Three threads | Three mounted sessions; return to first restores its 400 height and output | Three sessions; first preserves 400 and output | [native result-4](native/result-4.json), [native capture](native/06-three-thread-return.png), [Electron capture](electron/07-three-thread-return.png) |
| Flood | `seq 1 200000` reaches `200000`, `FLOOD_DONE`, prompt; subsequent UI responds | Reaches `200000` and prompt; subsequent UI responds | [native](native/10-flood.png), [Electron](electron/11-flood.png) |
| Appearance/font | Explicit Light/Dark and advanced size 18 update terminal; grid refits | Same controls update rendering and grid | [native light](native/20-font18-light.png), [native dark](native/21-font18-dark.png), [Electron font/grid](electron/10-font18-stty.png) |
| Window resizing | 840×620 clamps drawer to 465 and refits shell grid | 840×620 refits grid; dark/light captures | [native](native/18-small-window.png), [Electron dark](electron/12-small-dark.png), [light](electron/13-small-light.png) |
| Application relaunch | **Unverified for normal launch**, see below | Full Electron/backend relaunch restores 400 height and history without re-pairing | [Electron restoration](electron/08-relaunch-history.png) |

These are behavior checks, not pixel-equality assertions. The default captures differ: the native agent's system theme yielded a dark terminal inside light app chrome, while Electron was light; explicit Light/Dark worked. Initial `stty size` was 14×129 native versus 15×129 Electron. Font/settings and view geometry were changed during the run; captures are individually labelled and must not be treated as identical layout states.

## Relaunch limitation

The native agent was genuinely closed and relaunched, but returned to onboarding and reset drawer UI state. This is not classified as a production persistence failure: `NativeModule.swift:462–465` gives native modules a per-process temporary data root; `T3Module.swift:59–62` disables persistent credentials under agent mode. The driver's named `--storage` does not change this native-module behavior. Re-pairing still found server sessions, but did not prove automatic restoration of saved drawer selection/height. A normal-launch isolated-profile run is still required.

## Automated checks and source identity

- Fresh macOS bundle build: pass, [log](native-build.log).
- `bun test examples/t3-code`: 2,115 pass, 1 skip, 0 failures, [log](bun-tests.log).
- Strict TypeScript check of `app.ts`: exit 0, [empty error log](typecheck.log).
- Fresh AppKit terminal build/run: 14 total, 13 passed, 1 scale/cost test skipped, 0 failures. Includes shared output vectors, status JSON, attach reducer and session view, [log](swift-terminal.log).
- Fresh AppKit transport build/run: 49 tests, 0 failures, [log](checks/001.stdout.log). Its deterministic terminal-stream fixture checks backpressure/Ack behavior and separation from the app-stream cap; this is test evidence, not measurement of the live flood.
- Focus acceptance assertion: fails, [failure](checks/002.stderr.log). [Runner report](checks/report.json) records `status: failed` and `source_unchanged: true` for its declared source set. [Recipe](recipe.json) records the commands and source scope. The check validates the preserved runtime observations; it does not rerun GUI capture.
- Independent evidence review confirms the focus finding and the relaunch limitation. No repairs were attempted. Repository-wide gates were not rerun for this evidence-only task.

## Reproduction and scope

Build native from the implementation revision:

```sh
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2020 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2020,DOM examples/t3-code/app.ts
```

Start the pinned server with isolated home/config/data directories, `/bin/sh`, and a disposable Git project. Native used loopback port 16321, Electron's embedded server 16322. Seed three ordinary server threads named Verify one/two/three via `orchestration.dispatchCommand` / `thread.create`; no model requests were sent. Pair the native app through its welcome UI. Native capture used exported `open()` from `scripts/agent.mjs`, at 1280×840, with `storage: terminal-review-20261006`; use the operation sequences in `native/result-*.json`. Their state records are intentionally reduced to terminal/focus/confirmation fields so pairing credentials and unrelated data are not published. Pairing inputs and raw runtime stores remain private. Build/run AppKit suites with the existing recipe in `examples/t3-code/README.md`.

The [original report](electron/REPORT.md) records Electron build/isolation and playback details. It ran the original desktop, not a browser substitute. Protocol-registration suppression and a mock keychain were isolation substitutions. Node 26.7.0 differed from the reference's declared ^24.13.1 engine; this is a reproducibility limitation. [Terminal RPC subset](electron/terminal-rpc-only.ndjson) contains sent terminal requests only, not a complete Ack/response trace or a native/original normalized trace diff.

Still unverified: actual Korean IME composition; normal native relaunch; three-scope authorization; worktree-specific environment; thread-deletion cleanup through UI; resize across network loss/reconnect; oversized paste; full reduced-motion/animation matrix; matched pixel comparison. Static review also flags failed-resize retry and customized-shortcut paths for future runtime checks. Tabs/splits/right-panel terminal and additional integrations remain outside this drawer PR's scope.

Recorded app/server processes were stopped after capture; fixture ports are closed. Evidence publication changes no app implementation, ticket acceptance criteria or PR merge state.
