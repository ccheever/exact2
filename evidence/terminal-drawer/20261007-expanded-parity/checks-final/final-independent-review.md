# Final independent terminal review

Review date: 2026-10-07. Scope: native terminal key/focus ownership, semantic-only event forwarding, selection actions, serial provider-auth input and deferred Tab traversal, retained drawer geometry and visibility, composer `NSTextInputClient` ownership, session attach and resize lifetime fencing. This report records reviewed sources and bounded evidence; it does not certify the entire parity matrix.

## Findings and verification

- Caught a reused-terminal-ID resize defect: an old in-flight resize completion could send its queued dimensions to the replacement terminal. The session lane reproduced the extra RPC, then fenced resize entry and completion by exact session ownership. Reviewed the repair; the session lane reported **54 passing transport tests**, including the regression that failed before the fix.
- Final current native Swift test binary compiled successfully. Executed **32 tests, one optional scale skip, zero failures**, in 6.745 seconds. The suite includes semantic-action isolation, native focus/clipboard behavior, auth queue lifecycle and forward/reverse Tab traversal without duplicate advance or PTY bytes. Logs: `target/terminal-verification/native-parity/build-final-current.log` and `tests-final-current.log`.
- Independently inspected the normal app's fresh-view physical Korean IME evidence: **50/50 trusted DOM events**, one composition commit for `한`, one for `글 `, no focus/blur during the sequence, and exact PTY bytes `ed959ceab88020`. Evidence: `target/terminal-parity/ime-normal/phase1-manual-ime-dom.json` and `phase1-manual-ime-pty.ndjson`. Root verified normal process 42138 was active, foreground, and key before the unprimed user input.
- Earlier split-Jamo results came from an accessory agent window while a different normal process owned foreground activation. Those observations remain harness/foreground-confounded; the precise IME mechanism was not proved. They do not justify changing the vendor implementation after the controlled normal launch passed.
- No remaining blocking source findings in the reviewed scope. This is a source-review conclusion, not a blanket runtime parity pass.

## Remaining acceptance boundaries

- Root subsequently reported runtime **C02 Add to chat** and retained drawer **motion** acceptance passed. C02 includes exact 55-byte Copy, tooltip, normal restart context retention, send, and details. **C03 retained-caret insertion** passed the rebuilt physical repeat; final one-space separator and atomic removal also passed the root's normal-app physical drive.
- **Browser framework issue #100 remains a full-parity blocker.**
- Other partially verified matrix rows retain their existing status. This review does not upgrade untested or partial rows to pass.
- Final native tests exited and restored the clipboard. GUI ownership was returned to root; report creation used no GUI or build actions.

## Source fingerprints

SHA-256 of current on-disk app sources and relevant regression tests at report creation:

| File | SHA-256 |
| --- | --- |
| `examples/t3-code/modules/apple/T3TerminalView.swift` | `350b99293d3de7c47a43c9668d238cce29c2ccfbdbcb3fb81812e12ac38260dd` |
| `examples/t3-code/modules/apple/T3TerminalKeys.swift` | `d3da904deb1e931120f9c1726f6027f5e5499b66a424e4fe229e274c03022e21` |
| `examples/t3-code/modules/apple/T3TerminalActions.swift` | `9d2a9d5497cc0d496c50b2b082105f41616717a710405f86ba971dd5d664ac44` |
| `examples/t3-code/modules/apple/T3TerminalAuth.swift` | `f0dafb5594a779997eb93cbc43234958d6af3d9d719a426823c0531ec586cf9e` |
| `examples/t3-code/modules/apple/T3TerminalSessions.swift` | `3bfc509798ae6a0d9d9c2e354598e122fb4fff39581f80d3e1a84ba87cc27b05` |
| `examples/t3-code/modules/apple/T3Composer.swift` | `9967d121cdfcf70e0c8e8035a1b593b107a5d09e8ee2bfba383081c42da76573` |
| `examples/t3-code/terminal.contract` | `29409d7b0e4e90f19f7d04b67d81efa10a2786f9bcbc37039191a7e8479f2541` |
| `examples/t3-code/terminal-host/src/entry.ts` | `e40ed2c540fd8f2148e7cb5941a6cff40f36afe221acfb8b8937fcb0937b6784` |
| `examples/t3-code/terminal-drawer-view.ts` | `f2a25114ceae4743935a8e6b55ce241e8c134ff56d218ca085031c5790f50c22` |
| `examples/t3-code/terminal-panel.ts` | `75134faa7ae4a0e3a86a44c8bccfa64370ac801c547b6f370a07a694bdeed0d8` |
| `examples/t3-code/provider-auth-terminal.ts` | `d864bb951efe1edc867348b290036c5e49546e0e0c9f690e4632e60782c26574` |
| `examples/t3-code/macos/tests/terminal/drawer.swift` | `ff674939db9fc5ea9dda6a78f222affe5e8ea651f0aa088c672837662246cddf` |
| `examples/t3-code/macos/tests/terminal/auth.swift` | `758dad52983b3eed2db93a4040326c856e51367212e23d7d9e6537aae28ead51` |
| `examples/t3-code/macos/tests/transport/terminal-streams.swift` | `2345459d2b2645cb32e70f14b61182ba71c34180404179072ed6e0a2cfc093d0` |
| `examples/t3-code/terminal-integrations.test.ts` | `6fa755bf01349cfda4e1c08587afe458576bf600d6504d3f2882e03356ecccd5` |

## Add-to-chat runtime follow-up

Root's actual selection action exposed forbidden ambient `new Date()` in the shared drawer/panel handler. Replaced it with `new Date(composerNow(client))`, using the existing injected snapshot clock. A real handler-to-editor insertion regression first failed against the original source while no-argument `Date` and `Date.now` were disabled, then passed with the fix and exact injected instant. Focused integration/drawer/panel tests: **45 pass, zero failures, 165 assertions**. Logs: `target/terminal-verification/native-parity/add-to-chat-clock-before.log` and `add-to-chat-clock-tests.log`. Root subsequently confirmed rebuilt C02 runtime acceptance. Fingerprints above include this follow-up; Swift sources and the final native test result are unchanged.

## Retained-caret follow-up

Root's actual C03 drive retained composer caret 5 through blur, but Add to chat appended. Pinned original `ChatComposer.tsx` inserts at `readComposerSnapshot().expandedCursor`; its editor snapshot reads the retained ProseMirror selection without a focus requirement. Native `T3ComposerEditor.insert` instead substituted end-of-text when blurred. It now uses the retained native selection, clamped to current UTF-16 bounds, before the existing focus/insertion operation.

Regression reproduced two failures before the fix (blurred caret and blurred selection); focused cases already passed. Final full composer suite: **46 tests, zero failures**. Logs: `target/terminal-verification/native-parity/build-composer-caret.log`, `tests-composer-caret-before.log`, and `tests-composer-caret.log`. Root subsequently confirmed physical middle insertion at the retained caret. Earlier native terminal32 result predates this composer-only follow-up; the relevant composer suite verifies the new change.

| Follow-up file | SHA-256 |
| --- | --- |
| `examples/t3-code/modules/apple/T3ComposerEditor.swift` | `fb1f209d69b4b79ada99379a70362e95f074ab8fa2a9713de838c2de044070c6` |
| `examples/t3-code/macos/tests/composer/editor.swift` | `db2236573f13d3a9f04be4b4fb078b2a57ea13b2334c29c657e69b80584fb7b7` |

## Worktree setup registration follow-up

The D04 live-card observation led to an executable reproduction of early stream snapshots being discarded before the subscribe reply installed its ID. The inbox advances past those events. The entire observed 45-second absence has not been causally proved; it remains consistent with the registration race.

The setup subscriber now uses the existing subscription serial/newest-event pattern, fences late replies by thread/environment/generation and request lifetime, and handles retry notifications. Four regressions cover early delivery/late replies, thread changes, connection identity changes, and retry retirement. **14 focused tests passed; full application Bun run: 2,184 pass, one skip, zero failures (2,185 tests across 182 files, 12,600 assertions).** Strict application TypeScript and diff checks passed. Logs: `target/terminal-parity/worktree-registration-{tests,full-tests,tsc}.log`. The rebuilt fresh D04 run subsequently showed the failed completion card automatically after 45 seconds, without navigation (integration-lane report).

| Follow-up file | SHA-256 |
| --- | --- |
| `examples/t3-code/timeline-worktree.ts` | `23088e3e1594ddc37a3aee4476007728ec981b5947c6b138e3fef58aac38af78` |
| `examples/t3-code/timeline-worktree.test.ts` | `b46e3d5bca04f970639c48f49ff96f15a394498d717d60e7ff463a18b9256087` |

## Terminal insertion separator follow-up

Root's rebuilt C03 physical drive confirmed middle insertion, then exposed two separator spaces. The terminal TS caller supplied a trailing space and native insertion supplied another. Removed only the terminal caller's suffix; native insertion continues to own spacing. The actual TS request now asserts the exact bare reference, complemented by the passing native insertion test's exact one-separator result. **29 relevant TS tests passed, 108 assertions, zero failures** (`target/terminal-verification/native-parity/terminal-insertion-padding-tests.log`). Native code is unchanged from the 46-test composer result. Root confirmed final consumer one-space/backspace behavior in the normal app. Unrelated review-comment behavior was not changed.

| Follow-up file | SHA-256 |
| --- | --- |
| `examples/t3-code/composer-editor.ts` | `e29edad60995bbaa47f4fbc2a08da6daf642b9fb94a41232a8cef62a86679844` |

## Final D04 interpretation and publication boundary

A second observation of no running setup card was compared against the pinned original rather than treated as another defect. Executed production `deriveRows` and the literal placement predicate extracted from pinned `MessagesTimeline.logic.ts:1683`: with the agent stage done and its turn started, both hide a still-running async setup card and both show its failed completion. `worktree-handoff-probe.log` records `false/false` while running and `true/true` when done. The integration lane's fresh run showed completion automatically without navigation, consistent with this behavior. No additional source fix was needed.

Root reports the final wider test run: **4,299 pass, two skips, zero failures; 24,951 assertions across 360 files**, plus strict TypeScript and the final padding app build passing. These wider checks are root-reported; this review directly ran the focused checks described above.

Final source audit: no outstanding blocking source finding within this reviewed terminal scope. Final runtime spacing/removal acceptance passed the root's normal-app physical drive. Browser issue #100 still blocks a full-parity claim, and other partial matrix entries remain partial. The historical early-registration defect is reproduced and fixed; a complete causal explanation for the first 45-second observation was not established.

## Final C03 consumer acceptance

Root's final normal app (PID 55412, latest source bundle) retained caret `(5, 0)` before and after terminal focus, inserted the terminal reference between `LEFT ` and `RIGHT` with exactly one separator space, and returned composer focus at caret 100 in a 105-character prompt. One physical Backspace removed the separator (length 104, caret 99); the next removed the chip atomically and restored exact `LEFT RIGHT` (length 10, caret 5).

Independently read `target/terminal-parity/selection-complete/caret-proof.json`: `exactMiddleSpacing: true`, `afterTwoBackspaces: "LEFT RIGHT"`, `atomicRemovalRestoresOriginal: true`. Root's before/blurred/after/delete-space/delete-chip captures and after/removed screenshots supply the physical-event evidence. C03 is complete for these observed insertion and removal cases; this does not upgrade unrelated partial matrix rows or remove Browser #100's full-parity blocker.
