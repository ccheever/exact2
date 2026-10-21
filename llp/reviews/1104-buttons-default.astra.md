# Review: LLP 1104 step 4 code (52eb9fccf), 2026-10-07 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, at `52eb9fccf` (`git diff 0365ad1a4..52eb9fccf`).
- **Method:** a code-review brief (sha256 `5100091f27eb94f30659353895e286cd4b0622cedccf39e19532f0651722c176`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 2 and 4 fixed in lane S4s (a12e305b1); 1 and 3 in lane F6 (ee26a299b). Of 1, the agent's cancel on a native macOS button still activates sometimes: a non-strict expected failure and a QUEUE entry.

---

**Verdict: DO NOT LAND.**

**Findings**

1. **Held contacts hang on newly native macOS buttons — blocking.** [AgentMac.swift:408](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/AgentMac.swift:408)  
   `down` synchronously dispatches into NSButton’s tracking loop before recording the contact or replying. The driver waits for that reply before sending `up`, so neither completes. F5 records the resulting 120-second precedence-smoke timeout in [QUEUE.md:1074](/Users/ccheever/projects/exact2-wt-1104/QUEUE.md:1074). Store and acknowledge the contact before entering tracking, support subsequent queued move/up/cancel events, and test held contacts through a native button.

2. **macOS buttons cannot shrink to their text’s min-content width — material.** [ButtonMeasureMac.swift:50](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/ButtonMeasureMac.swift:50)  
   MinContent and MaxContent requests both use unconstrained `fittingSize`. A newly native multiword button therefore acquires its entire title width as its automatic flex minimum, causing constrained rows to overflow. The definite-width tests do not exercise this. Implement distinct min-content measurement using the longest unbreakable text plus chrome, respecting whitespace rules, and test an actual constrained flex row.

3. **The default change loses `aria-labelledby` names on Apple — material.** [NativeButtonsMac.swift:163](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/NativeButtonsMac.swift:163), [NativeButtonsIOS.swift:158](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/NativeButtonsIOS.swift:158)  
   Native labels use only `aria-label` or the face title; referenced-name synchronization explicitly skips native buttons. An existing button displaying “Go” with `aria-labelledby` pointing to “Delete permanent copy” now exposes “Go”. The compiler accepts this case. Resolve the shared authored accessible name onto the native control and refresh it when referenced text changes.

4. **The repository fixture ships with an inaccessible header action — material.** [app.contract:306](/Users/ccheever/projects/exact2-wt-1104/apps/native-fixture/app.contract:306)  
   F5 records the native header segments growing from 69.56 to 121.33 points, pushing the 32-point compose button to x=396.97 on a 402-point viewport ([QUEUE.md:1073](/Users/ccheever/projects/exact2-wt-1104/QUEUE.md:1073)). This is separate from multiword shrinking: the labels are “All” and “Missed”. Repair the header’s sizing or reflow while preserving native controls, then verify compose remains visible and operable.

**What is right**

The lowering reuses native admission checks, correctly handles radius and conditional disabling rows, and supplies bare-button reasons. I found no definite D9 violation in the 54-button migration. The available compiler accepted all 34 app roots; all three agent metadata tests passed. Native findings above rely on source inspection and recorded F5 evidence; fresh native execution was unavailable.
