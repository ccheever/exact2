# Final independent terminal review

Review date: 2026-10-07. Scope: native terminal key/focus ownership, semantic-only event forwarding, selection actions, serial provider-auth input and deferred Tab traversal, retained drawer geometry and visibility, composer `NSTextInputClient` ownership, session attach and resize lifetime fencing. This report records reviewed sources and bounded evidence; it does not certify the entire parity matrix.

## Findings and verification

- Caught a reused-terminal-ID resize defect: an old in-flight resize completion could send its queued dimensions to the replacement terminal. The session lane reproduced the extra RPC, then fenced resize entry and completion by exact session ownership. Reviewed the repair; the session lane reported **54 passing transport tests**, including the regression that failed before the fix.
- Final current native Swift test binary compiled successfully. Executed **32 tests, one optional scale skip, zero failures**, in 6.745 seconds. The suite includes semantic-action isolation, native focus/clipboard behavior, auth queue lifecycle and forward/reverse Tab traversal without duplicate advance or PTY bytes. Logs: `target/terminal-verification/native-parity/build-final-current.log` and `tests-final-current.log`.
- Independently inspected the normal app's fresh-view physical Korean IME evidence: **50/50 trusted DOM events**, one composition commit for `한`, one for `글 `, no focus/blur during the sequence, and exact PTY bytes `ed959ceab88020`. Evidence: `target/terminal-parity/ime-normal/phase1-manual-ime-dom.json` and `phase1-manual-ime-pty.ndjson`. Root verified normal process 42138 was active, foreground, and key before the unprimed user input.
- Earlier split-Jamo results came from an accessory agent window while a different normal process owned foreground activation. Those observations remain harness/foreground-confounded; the precise IME mechanism was not proved. They do not justify changing the vendor implementation after the controlled normal launch passed.
- No remaining blocking source findings in the reviewed scope. This is a source-review conclusion, not a blanket runtime parity pass.

## Remaining acceptance boundaries

- Runtime **C02 Add to chat** acceptance and retained drawer **motion** evidence are still pending the root/sign-in lanes at this report's creation.
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
| `examples/t3-code/terminal-integrations.test.ts` | `6ff747c65f99e41ca2f55c79df2c2ea37a34a650f15d9ec564e0b6be5f8056e3` |

## Add-to-chat runtime follow-up

Root's actual selection action exposed forbidden ambient `new Date()` in the shared drawer/panel handler. Replaced it with `new Date(composerNow(client))`, using the existing injected snapshot clock. A real handler-to-editor insertion regression first failed against the original source while no-argument `Date` and `Date.now` were disabled, then passed with the fix and exact injected instant. Focused integration/drawer/panel tests: **45 pass, zero failures, 165 assertions**. Logs: `target/terminal-verification/native-parity/add-to-chat-clock-before.log` and `add-to-chat-clock-tests.log`. Root owns rebuilt C02 runtime confirmation. Fingerprints above include this follow-up; Swift sources and the final native test result are unchanged.
