# Final activity root-clock acceptance

Actual named `T3 Code (Exact).app`, final-agent3 process44625, macOS1280×840, reference server16843/proxy16844, transparent proxy rules. Selected isolated `verify-timeline` composer uses Codex/gpt-5.4 and an incomplete workspace snapshot. Native timing is platform timing; the Exact clock remains driven explicitly. No provider turn was submitted.

The supported queue invoked `rootRetry(s)` from `acceptance-exact-deadline.mjs`. Actual refresh completion at clock21000 set deadline31000. At30000 (+9000) the exact request list was unchanged. At31000 (+10000) one refresh request was added for the same provider/workspace, and next deadline41000 was recorded. All assertions passed. Current task-owned source hashes are captured and checked by `assert-capture.py`.

The first helper invocation sought one second beyond an already-existing deadline and incorrectly assumed the completion clock equaled the final seek target. The runner correctly processed the timer at the intermediate deadline. `capture-overshoot.json` and the old helper preserve this rejected verification assumption. The corrected helper seeks the exact existing deadline; this was a test-helper correction, not an application change or a relaxed deadline assertion.

Full original states and shared queue responses remain in `target/t3-repair/activity-final3` and `target/t3-repair/final-agent3`. Portable capture retains selected domain state, real sanitized RPC request lists and observation times. The app was closed by its supported driver after capture. The following peer-only timeline node-id change does not modify any captured activity source; final combined source identity and reviewer impact assessment remain the coordinator's responsibility.

Run `python3 <this-directory>/assert-capture.py` from repository root. This checks captured behavior and source identity, not a GUI replay. Actual Electron lifecycle comparison and measured0.3s focus/pointer latency remain outside this proof. Silent failure has a separate final-app capture.
