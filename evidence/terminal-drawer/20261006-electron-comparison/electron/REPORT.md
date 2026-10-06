# Original Electron terminal verification — 2026-10-06

Original reference: T3 Code pinned source `1e2ecbd975`, copied from `../t3-code/target/t3-ref/src-1e2ecbd975` into this attempt's `source/`; application version 0.0.45. Real Electron 44.4.2 arm64 runtime from its official GitHub release. Playwright-core 1.60.0 drives the original Electron main/preload/renderer at `t3code://app/`, not a browser replacement. macOS 26.6.2 (25G83). Build commands used installed Node v26.7.0 (reference engine declares ^24.13.1; this mismatch is a reproducibility limitation). Existing dependency installs reused read-only through node_modules symlinks.

## Build and isolation

- Copy source excluding node_modules/.git, link existing dependency installs.
- `cd source/apps/desktop; node scripts/build-browser-secret.mjs && node scripts/build-preview-annotation-css.mjs && ../../node_modules/.bin/vp pack` (pass; build.log).
- `cd source/apps/web; ../../node_modules/.bin/vp build` (pass; web-build.log); copy renderer dist into source/apps/server/dist/client.
- Server bundle was reused from the pinned reference, copied locally. Desktop/renderer freshly built.
- `node drive.mjs` launches the downloaded Electron executable through Playwright `_electron` with `--use-mock-keychain`, isolated profile and child environment HOME/T3CODE_HOME/CODEX_HOME/CLAUDE_CONFIG_DIR/XDG paths under this attempt. Embedded backend port 16322; local driver port 16324.
- `source/apps/desktop/isolation-bootstrap.cjs` blocks `app.setAsDefaultProtocolClient` before requiring original boot bundle. This is an explicit test isolation substitution for OS protocol registration; terminal, transport, UI and application source are unchanged.
- Isolated new Git project `t3-home/projects/terminal-oracle`; shell `/bin/sh`, PS1 `$ `. Three persisted threads seeded through normal orchestration RPC, using parent's seed.mjs. No model request sent. Provider-unavailable banner is expected in the isolated fixture.
- Real ~/.t3 directory mtime unchanged; login keychain search list unchanged; pre-existing T3 bundle/scheme registration entries unchanged. Full before/after local dumps and identity-isolation.json retained. Mock keychain flag used; existing keychain item contents were never queried.
- Electron closed through app.close after testing; ports 16322 and 16324 no longer listening. No reference files or implementation files edited.

## Observed results

| Scenario | Observation | Evidence |
|---|---|---|
| Open/type/cwd/effect | Drawer opens at 280 CSS px. `printf ORACLE_READY; pwd; touch drawer-proof; stty size` prints isolated project cwd; file exists; initial stty 15 rows × 129 cols. | 03-output.png, terminal-rpc-only.ndjson |
| Direct Hangul text | `printf '한글 테스트\n'` received through keyboard.insertText and output renders correctly. Does not prove IME composition. | 04-resize-400-hangul.png |
| Height drag | Real pointer drag separator -120 px gives 400; dragging toward extremes gives min 180/max 630 at 1280×840. | 04-resize-400-hangul.png, 05-resize-max.png |
| Cmd+J / focus | With terminal input focused, Meta+j closes drawer, activeElement becomes contenteditable DIV role textbox. Meta+j reopens saved 400 drawer, focus returns to terminal textarea. | Live DOM assertions; terminal RPC subset |
| Close dialog | Exact text `Close terminal "Terminal 1"?` / `This stops the running process and clears its history.` Cancel and Escape each retain terminal; Confirm removes it; RPC close includes deleteHistory:true. | 06-close-dialog.png, terminal-rpc-only.ndjson |
| Shell exit | `exit` + Enter automatically removes drawer; activeElement contenteditable=true composer. Transient Process exited text not separately captured. | Live DOM assertions; terminal RPC subset |
| Three threads | One/two/three each open own terminal. Return to one retains 400 height and previous command output; 3 terminal inputs mounted. | 07-three-thread-return.png |
| Full relaunch | app.close and new Electron launch also restart embedded backend. Selecting Verify one automatically opens drawer with saved 400 height and visible THREAD_ONE_REPLAY history, without re-pairing. | 08-relaunch-history.png |
| Settings | Appearance UI Dark, Typography Advanced, Terminal font size 12→18; larger text/dark terminal visible when returning to thread. Grid refits from 129×21 to 93×16. | 09-dark-font18.png, 10-font18-stty.png, terminal-rpc-only.ndjson |
| Flood | `seq 1 200000` ends visibly with 200000 and prompt. App responds to subsequent window resize/settings interactions. Retained buffer bytes/Ack counts not measured. | 11-flood.png |
| Window resize | Real BrowserWindow.setContentSize 1280×840→840×620 refits grid 93×16→57×16 at font18. Dark/light UI captures. | 12-small-dark.png, 13-small-light.png, terminal-rpc-only.ndjson |
| Delayed focus race | Twice: hide, reopen via toolbar, immediately click composer; activeElement is Message contenteditable DIV immediately and after 2 seconds. Original does not steal focus in either attempt. | focus-race.json, 14-composer-retains-focus.png |

## Limits

No claim of full acceptance or complete parity. Screenshots capture the original Electron renderer, not native title/menu chrome. Original tabs/splits visible but not tested (outside drawer scope). Worktree env, three-scope authorization, deletion cleanup, very large paste, animation/reduced-motion and actual Korean IME composition were not exercised. RPC capture is a sent-frame subset, not full request/response/Ack trace and not a normalized clone/reference trace diff. Snapshot hashes establish artifact integrity only. Native computer-use accessibility returned permission_denied; its failed attempt retained as 06-close-dialog.json, but no native OS interaction was needed because the close confirmation is HTML and was subsequently tested through real renderer clicks.

Keep pairing.private, home/, t3-home/ and raw local system dumps private; only selected PNGs, this report, artifact hashes and filtered terminal-rpc-only.ndjson are suitable evidence candidates.
