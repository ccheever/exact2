# Actual Korean IME recheck after Mac unlock

Later user-operated testing verified basic composition/commit; see MANUAL-IME.md. This report preserves the earlier failed automation controls.

The normal-textarea control still did not engage Korean composition. **Actual IME remains unverified; no terminal defect or pass is established.** The terminal comparison was deliberately not run after the control failed. This distinguishes valid Unicode/paste tests from actual IME behavior.

The parent allocated an exclusive GUI slot after the user unlocked the Mac. Only isolated Electron PID40550 was activated; a temporary normal HTML textarea was inserted in that app. AXIsProcessTrusted was true. Each attempt checked the frontmost PID and current input-source ID before every character, used explicit empty modifiers for text keys, and restored the original ABC input source. Physical virtual-key sequence5,40,1,15,46,3,49 corresponds to g/k/s/r/m/f/Space, which should compose `한글 ` under Korean2-Set. Captured DOM events were genuine browser events (`isTrusted`), but trusted keyboard events alone do not prove that an IME participated.

| Fix round | Mechanism | Normal-textarea result |
|---|---|---|
|1|TIS selects Korean2-Set; private CGEvent source; flags cleared; run loop between down/up; source and foreground rechecked per key|`gksrmf `, zero composition events, despite Korean source ID at each check|
|2|Same with hidSystemState event source and explicitly cleared Unicode payload|`gksrmf `, zero composition events|
|3|Physical Ctrl+Space attempts source switch before any text key|Input source stayedABC; guard aborted before text keys; empty textarea|

Stopped after three rounds as requested. Temporary textarea and capture listeners were removed, Electron hidden, GUI slot returned to the sign-in agent, and ABC restoration logged. No clipboard access, personal-app interaction, shell command execution, or native fixture change occurred during these attempts.

Evidence: ime-unlocked-control*.json records only the temporary control's own fixture events; matching .log files record source/frontmost checks and restoration. ime-physical-round1.swift, ime-physical-round2.swift and ime-physical.swift preserve the three helper variants; ime-recorder.js defines bounded event capture. A valid manual or otherwise verified input-method path is still needed for composition/backspace/cancel and Korean-layout shortcut acceptance.
