# Normal macOS first-input Korean IME control

PASS for the observed committed input. The user physically typed `한글` followed by Space once into a fresh native terminal view; no synthetic typing or local command entry primed the view.

The existing normal application stayed unchanged: PID42138, bundle com.exact.t3code.parity.persistence.16334, window175621. A new GUI-created term-4 received only a backend-RPC command launching the safe raw-byte recorder. Before the user typed, the recorder contained one ready event and no input events. Accessibility identified Terminal input as focused; System Events returned foreground PID42138. The passive LLDB capture confirmed NSApplication active=YES, NSWorkspace frontmost PID42138 and window key=YES. Other owned test windows were hidden.

The recorder received exactly UTF-8 `ed959ceab88020` (`한글 `), once. The passive DOM log contains50 events, all trusted, with two composition sequences and no focus/blur events: ㅎ→하→한 and ㄱ→그→글. This is a normal foreground application control, distinct from the earlier accessory agent-window observations. It does not test composition cancellation.

Evidence preserved before cleanup: phase1-manual-ime-dom.json, phase1-manual-ime-pty.ndjson and phase1-result.png. Ready state: ready.png, focused-ax.json and inject-ime.log.

Cleanup: backend RPC sent CtrlD to only this recorder; the log records byte04 then closed. The native close confirmation removed only term-4. Original term-1/term-2 split and term-3 remain, with drawer height388. Keyboard input source restored from Korean2Set to ABC (status0). No signing, GUI re-pairing or keychain changes occurred.

Source qualification: this existing normal bundle predates the final auth-Tab and resize-only refinements. The terminal input/composition code exercised here is unchanged by those refinements. Do not present this as a final-bundle rebuild test or as proof for all IMEs.
