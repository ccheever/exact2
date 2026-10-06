# Expanded terminal parity verification, 2026-10-07

Status: **core runtime checks passed; full parity has not passed**. Implementation: [`dc9de3504`](https://github.com/ccheever/exact2/commit/dc9de3504e8f5beda8b244eff8b9730539d57d85). [Source identity](implementation.json) and [artifact hashes](SHA256SUMS) accompany this report. The original is T3 Code `1e2ecbd975` (0.0.45), built and run in Electron 44.4.2. The clone is the actual macOS app on macOS 26.6.2. All projects, servers and provider fixtures used here are disposable and isolated; no real provider login, install or paid prompt was performed.

## Implemented behavior

This change expands the prior single-drawer repair to terminal groups and four-way splits, independent right-panel terminals, terminal-owned shortcuts, busy-process indicators, draft-to-thread session handoff, selection/context integration, script/fence execution, and provider/onboarding terminals. It also repairs stale callbacks crossing reused session IDs, provider instance propagation, native paste freezing WebKit, hidden-drawer focus handling, and native input routing. Physical normal-app Korean input, native paste, auth input, Tab traversal and drawer motion have been driven in the real app. Selection-context insertion, exact copy, retained caret, spacing, deletion, normal restart, fake-provider send and previews now pass the recorded runtime checks.

## Evidence boundaries

- [Original coverage](oracle/COVERAGE.md) records the reference's 39 acceptance rows. Most are partial, with explicit remaining subcases. It is not a full acceptance certificate.
- [Session transport](sessions/RESULTS.md), [44-session capacity and deletion](sessions/capacity/RESULTS.md), and [reused session IDs](sessions/reuse/RESULTS.md) use production Swift transport/session code against the real pinned server, with a headless view sink. They do not establish WKWebView memory usage or GUI interaction.
- `native-initial/` screenshots and raw keys precede the final paste/auth/focus repairs. They establish the recorded UI/core operations, not a fresh check of every final-source behavior.
- `native-paste-draft/` captures the repaired native Cmd+V path against a real raw PTY, subsequent responsive output, resize/theme bounds, and draft session identity across first send. Unicode paste is not an IME test.
- `selection/` preserves the original pre-fix failure. [Final selection proof](selection-final/RESULTS.md) records the repaired menu-to-chip path, exact copy, persistence, previews, send, retained cursor, spacing and atomic removal.
- [Final drawer observations](motion/final-drawer-observations.json) now show actual native drawer close 280 → 174.12 → 0 and open 0 → 105.88 → 280 at 0/100/400 ms with the 400 ms setting. Reversal retains 174.12 at the toggle, then reaches 262.98 after 100 ms and 280 after 400 ms. Reduced motion reaches 0/280 without advancing the clock. Inner height stays 280 and native height 279, with node IDs retained. Hide focuses the composer TextArea; completed open focuses the terminal WebView. Earlier jumping-height observations remain historical; the supported CSS size-interpolation opt-in resolves this tested path.
- `checks-before-final-input-fixes/` records root gates and 4,299 app tests before the last native auth/composer/selection changes. `checks-final/` records the later native suite (32 tests, one optional skip, zero failures), 54 passing transport tests, and final bundle, strict TypeScript, caps and app tests (4,299 pass, two skips, zero failures). The clock regression failed before the fix and passed afterward.

Raw app state, pairing/access tokens, private pasteboard snapshots, server configuration and unrelated user data are excluded. The oracle contributes only its explicit publish allowlist.

## Confirmed core runtime checks

- Real PTY raw bytes match Option navigation, Command navigation/deletion/clear, arrows, Tab, Ctrl+C/D. `native-initial/raw-keys.json`.
- Native Cmd+V preserves Korean/emoji and LF bytes inside bracketed-paste markers, exactly once; Ctrl+D and subsequent output remain responsive. `native-paste-draft/paste-result.json`.
- Horizontal/vertical splits stop at four; disabled fifth split gives the limit tooltip. New groups, lowest available ID reuse, close confirmation/cancellation, independent panel sessions and terminal-context shortcuts were exercised.
- Drawer 280→400 resize refits the PTY; 840×620 clamps at 465. Fixed inner sizing prevents terminal-grid collapse during hide; closing reaches height 0 and restores composer focus.
- First fake-provider send preserves the draft terminal's thread identity, session key, cwd and open state. `native-paste-draft/draft-handoff.json`.
- Real user and assistant shell fences execute their harmless markers. Invalid language, multiline and unclosed fences do not expose the same runnable action. Expired-only terminal context warns without sending.
- UI-created script runs reuse the idle active shell. A verified busy shell causes allocation of a new group; the visible sidebar reports the running process.
- Real-server transport checks verify cwd/worktree/env, 37×111→offline42×123 reconnect resize, output replay, 65,537-character write refusal, bounded buffers and ACK backpressure.
- 44 real terminal streams receive concurrent 600 kB floods beside 16 ordinary streams; the 45th terminal and 17th ordinary stream refuse without corrupting existing sessions. These are headless capacity results.
- Production thread deletion closes four PTYs and child processes with history deletion before deleting the thread; modern sidebar, legacy sidebar and palette clear their counts. This is a production-command harness, not a physical menu gesture.

## Remaining acceptance

- In-app browser links cannot match the original Browser surface while framework issue #100 remains unresolved. The app reports the unavailable path instead of claiming external-browser fallback is parity.
- Actual drawer open/close, mid-flight reversal, reduced motion and focus observations pass for the recorded 280 pt drawer with a 400 ms setting. This does not establish every duration, resize-during-motion or split-layout permutation.
- Normal-app physical Korean first-input commit passes against the original: `한글 ` once, with trusted composition events and no focus interruption. Accessory-agent observations are activation-confounded and preserved separately. Marked-text cancellation remains unverified. [IME comparison](ime/README.md), [normal control](ime/normal/RESULTS.md).
- Physical auth a/b/c, 10,000-character Cmd+V,Escape,Cancel and retry pass in the rebuilt native app. Normal unchanged-bundle restart also passes: output, split/group/selection,388pt drawer and composer text restore without re-pair. Physical Tab/ShiftTab leave the auth terminal with no PTY bytes after the final repair. Final Add-to-chat, retained-caret insertion and atomic removal pass. See [auth proof](auth/sanitized-runtime-proof.json) and [persistence report](persistence/RESULTS.md).
- [Onboarding proof](onboarding/RESULTS.md) covers both install buttons, exact pending command bytes with no Enter, and cleanup. [Setup and links](setup-and-links/RESULTS.md) covers automatic worktree completion and existing-terminal activation. File-link routing reaches successful editor process spawn/unref, but external editor display and caret placement were not observed.
- Final checks also include 46 passing native composer tests and the worktree registration regressions. The [independent review](checks-final/final-independent-review.md) names its scope. Fine-grained subcases remain as listed in the [matrix](MATRIX.md) and original coverage. No task is closed on build or unit tests alone.
