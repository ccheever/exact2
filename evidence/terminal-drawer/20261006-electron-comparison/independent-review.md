# Independent evidence review — PR #175

Reviewed 2026-10-06 against HEAD 7caade7a8 and pinned Electron reference 1e2ecbd975. This review made no implementation changes and did not drive either application.

**Verdict: core drawer behavior has fresh execution evidence, but full Electron parity is not established. One focus regression is confirmed.**

## Confirmed finding: delayed terminal startup steals composer focus

Severity: P2. After opening the drawer, returning to the composer before the terminal finishes loading must preserve the user's chosen focus. The native implementation instead focuses the terminal when its page becomes ready.

Evidence: `native/result-17.json`, zero-based entries 7–10, records `type composer "focus probe"`, then `state.focus = {responder: "TextArea", editor: 149, logical: 149}`. The only intervening operation is `clock +1500 real`. The next state reports `{responder: "WebView", editor: null, logical: 6605}`. `native/24-focus-typed.png` accompanies the final state. Entry 3 independently establishes that typing targets the actual composer responder. Earlier tap-only attempts did not establish composer focus and are not evidence for this finding.

The Electron comparison, `electron/focus-race.json`, contains two trials in which the active element stays the contenteditable DIV labelled Message with class ProseMirror-focused immediately and after two seconds.

Code cause: `examples/t3-code/modules/apple/T3TerminalView.swift:299` calls `focusTerminal()` at ready whenever the historical focus-request value is positive. The reference `apps/web/src/components/ThreadTerminalDrawer.tsx:546–549` checks both visibility and whether the mount still contains `document.activeElement` before focusing. The native ready callback needs to respect focus that moved after the request.

## Supported progress and limits

- `native/result-1.json` records an open 280-point drawer and the expected project cwd/env.
- `native/result-3.json` records 400-point resizing and focused Meta+J closing the drawer with TextArea/composer focus.
- `native/result-4.json` records three mounted thread drawers and restoration of the first thread's 400-point height during thread switching.
- `native/result-6.json` records flood execution and Cancel/Escape keeping the drawer open. This review did not independently count wire Acks or retained bytes from these records; those require the transport evidence.
- `native/result-8.json` records shell exit closing the drawer and returning focus to TextArea. `native/result-9.json` records height clamps at 180 and 630, a 465-point clamp for the smaller window, and confirmed close returning focus to TextArea.
- `native/result-14.json` records changing terminal size to 18 and light/dark settings, followed by terminal input and screenshots. This review confirms execution coverage, not an independent pixel-diff result.

Normal relaunch persistence is **unverified**, not a demonstrated product failure. `host/apple/Sources/ExactKit/NativeModule.swift:461–465` creates process-specific native-module storage under agent mode, and `examples/t3-code/modules/apple/T3Module.swift:59–62` disables persistent credentials and saved environments in that mode. A named agent store does not by itself make those module paths persistent. The observed return to onboarding therefore cannot adjudicate normal application relaunch/history restoration.

Korean text passed through programmatic typing is not proof of real IME composition. Tabs, splits and additional terminal integrations remain declared follow-up scope, so this ticket cannot establish complete original-terminal equivalence.

## Unconfirmed code-review follow-ups

- `T3TerminalSessions.swift:259–269` stores lastSize before resize completion and ignores the RPC error; reconnect attach does not invalidate or resend that grid. Exercise disconnect → resize → reconnect and compare `stty size` without an extra resize before classifying a defect.
- `terminal.contract` passes the fixed Meta+J chord to the terminal while app commands support configured shortcuts. Customized shortcut behavior with terminal focus remains unverified; the outer key monitor may handle it.

No additional runtime defects are asserted from these static concerns.
