# Terminal acceptance matrix

This maps the pinned reference inventory to clone evidence. “Runtime core” means the named path was exercised; it does not pass every unlisted permutation. Ported/unit tests and headless transport proof are distinguished from real visible-app proof. The final verdict remains **not full parity** until blockers and required unknowns are resolved.

| Row | Clone evidence level | Observed behavior / remaining boundary |
|---|---|---|
| A01 | Runtime core + transport | Real project/worktree cwd and env; toggle/open/attach. No-project and failure permutations use tests. |
| A02 | Runtime core | Draft terminal retains thread/session/cwd across first fake-provider send; draft cancellation variants are component-tested. |
| A03 | Runtime core + final drawer motion | 280/min180/small-window max465 and PTY refit. Actual 400 ms close/open samples, continuous sampled reversal, reduced-motion immediate endpoints and fixed inner280/native279 with retained IDs pass. [Sanitized observations](motion/final-drawer-observations.json). Other duration/resize-during-motion permutations remain partial. |
| A04 | Runtime core | Raw PTY keys, UTF-8 bracketed native paste and subsequent responsive input; original ANSI/alternate-screen comparison is separately recorded. Full mouse-reporting app not driven. |
| A05 | Normal runtime core | Physical first-input `한글` + Space reaches original and normal clone exactly once. Normal clone has 50 trusted DOM events with no focus change. Accessory-agent first-syllable failures are activation-confounded; cancellation and other IMEs remain unverified. |
| A06 | Runtime core + tests | Close text, Cancel/Confirm, repeated close guard, shell exit; close-RPC failure and loading races covered by component/transport tests. |
| A07 | Runtime core + native regression | Hide/last-close composer focus; late-ready retains composer; hidden final exit does not steal panel focus. Normal physical terminal typing fixed and verified. |
| A08 | Runtime core | Normal unchanged-bundle restart reconnects without re-pair and restores output, two-pane group, separate group, selected pane,388pt drawer and unsent composer draft. Separate final selection bundle also restores the context chip and exact three-line hover text across normal restart. |
| A09 | Real server, headless views |44 streams ×600kB flood plus16 ordinary streams; bounded buffers, ACK withholding/resume,45th/17th refusals. Whole-app44-WKWebView memory is not inferred. |
| A10 | Real server, headless views | Real three-scope credential receives one terminal:operate refusal without retry loop. Full permission-denial GUI matrix remains unverified. |
| A11 | Production command + real server | Four PTYs/children close and history deletes before thread deletion; metadata/list counts clear. Worktree deletion GUI variant remains unverified. |
| A12 | Runtime core + tests | Light/dark, small window, font/grid and theme projections. Full custom palette/cursor/selection/hydration visual matrix remains partial. |
| B01 | Runtime core + tests | New groups, sidebar labels, shared lowest-free IDs and active group; stale reused native session callback repaired against real server. |
| B02 | Runtime core | Horizontal/vertical splits, four limit, fifth muted control gives tooltip/no fifth pane. Both-axis resize edge permutations remain partial. |
| B03 | Runtime core + tests | Last-group Cancel/Confirm and ID reuse; fallback choices and process labels have component tests. |
| B04 | Runtime core + tests | Independent drawer and right-panel terminal surfaces, split/new/active owner behavior. |
| B05 | Runtime core + tests | Panel confirmation/cancel/close retains drawer. Bulk close and stale captured targets are component-tested; full genuine OS menu/middle-click matrix remains partial. |
| B06 | Runtime core + tests | Cmd+J/D/Shift+D/N/W take terminal context; custom-binding matching has native/component regressions. |
| B07 | Runtime core + tests | Repeated Cmd+W creates one confirmation; Cancel retains focused terminal. Physical Korean-layout shortcut variant remains unverified. |
| B08 | Runtime core | Raw key bytes match reference for Option/Command navigation/deletion/clear, arrows,Tab,Ctrl+C/D. Kitty key-release variant is not driven. |
| B09 | Runtime core + tests | Terminal-owned/unconditional app commands use native winning shortcuts; composer type-to-focus no longer intercepts native text clients. Remaining app commands not all physically driven. |
| B10 | Runtime core + tests | Actual busy shell gives visible process indicator and busy-script allocation; modern/legacy/palette count/environment/reduced-motion projections tested. |
| C01 | Runtime core | Actual native reverse drag selects three output lines. Multiclick/scroll/clamp permutations remain partial. |
| C02 | Runtime core | Real three-line CG drag opens AppKit selection menu; Add to chat inserts the chip and restores composer focus in the rebuilt normal app. Passive-event cancellation and forbidden ambient Date repairs are both exercised. |
| C03 | Runtime core + native regression | Exact clipboard is55 bytes; chip label retains lines1-3. Final actual insertion preserves caret5 across blur, inserts before RIGHT with exactly one separator, and restores composer focus. Focused/blurred selection replacement also has native tests. |
| C04 | Runtime core + tests | One actual fake-provider send receives the selected three lines exactly once with line numbers1-3; composer clears. Chip and captured text survive unchanged normal bundle restart. Two actual Backspaces remove the separator then whole chip and restore the original draft.64k GUI permutations remain unverified; expired-only send refusal is driven. |
| C05 | Runtime core | Draft hover preview displays the exact three selected lines, including after restart; clicking the sent terminal chip opens the actual Lines1-3 preview with the same text. |
| C06 | Runtime routing; editor outcome unverified | Physical Command-click routes real worktree file with :7:4 to preferred Cursor, then bounded VS Code fallback. Server launch reports success, but no fixture editor window/caret observed. [Evidence](setup-and-links/RESULTS.md). |
| C07 | Blocked by framework#100 | Original in-app Browser target observed. Clone reports unsupported in-app route. This is a functional mismatch, not parity via system-browser fallback. |
| C08 | Native/component-tested | Auth surface excludes app selection menu; URL external failure has native path. Full actual external-app outcome matrix remains partial. |
| D01 | Runtime core | UI-created script reuses idle shell; actual busy shell allocates a new group. Prefer-new and remembered script choices tested. |
| D02 | Component-tested | Open/write errors and preview routing tested; real failed-RPC injection through GUI not run. |
| D03 | Runtime core | Actual user/assistant valid closed single-line shell fences run markers; invalid language/multiline/unclosed examples do not expose Run. |
| D04 | Runtime core, final consumer | Fresh GUI worktree automatically shows failed setup card after script completion without navigation. Open terminal and hide/reopen reveal existing setup PTY; running card hidden after fast agent handoff matches reference. Fixture settings restored. [Evidence](setup-and-links/RESULTS.md). |
| E01 | Runtime core + native tests | Harmless ACP Account terminal is actual WKWebView/PTY; transcript offset/reset tested. No real account login. |
| E02 | Runtime core + tests | Separate physical a/b/c and exact10,000-character Cmd+V reach real PTY; native serial4096 queue, failure clearing/recovery,read-only and identity fencing tested. |
| E03 | Runtime core + native regression | Physical Escape reaches PTY and keeps Account; Cancel unmounts and retry works. Physical Tab/ShiftTab traverse outside the terminal without PTY bytes; actual WK tests also cover stale key-view links and no double advance. |
| E04 | Runtime core + tests | Actual Codex and Claude Install buttons open native setup terminals and prefill the exact commands. Destination PTY records52/46 bytes with zeroCR/LF; no Enter or installer execution. |
| E05 | Runtime core + real server harness | Both actual install terminals close, their wrapper/child shells exit, and Install is re-enabled. Separate harmless-command harness covers explicit Return, history deletion and provider env reattach; rapid replacement/stale paths remain regression-tested. |

Evidence entry points: [report](README.md), [reference inventory and coverage](oracle/COVERAGE.md), [session checks](sessions/RESULTS.md), [capacity/deletion](sessions/capacity/RESULTS.md), [reused lifetime regression](sessions/reuse/RESULTS.md). Pending rows must be updated only after their observed assertions pass.
