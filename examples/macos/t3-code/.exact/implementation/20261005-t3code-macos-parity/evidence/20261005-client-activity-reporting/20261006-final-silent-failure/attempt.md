# Final app silent backend failure

2026-10-06 KST. Named `T3 Code (Exact).app`, final-agent4 process61311, macOS1280×840. The bundle receipt check reports no source changes. This is the final app including timeline node-id corrections. Its activity sources match the recorded hashes; no activity production edit was necessary.

1. `silenceBefore(s)` captured the real app connected to isolated reference server16843 through transparent proxy16844, with existingtoastIds[2,1], at17:59:54.354UTC.
2. The coordinator sent SIGTERM to its recorded server PID6207. The proxy remained running until the after-capture completed.
3. `silenceAfter(s)` waited27 real seconds (independent of the Exact clock), captured current state, logs and tree, and asserted disconnection, zero newly added toasts and no activity-report refusal/failure log. Elapsed time from the before-capture was71,746ms. Existing toasts were not misclassified as new failures.
4. The supported driver closed process61311. The proxy owner was then notified to stop its recorded proxy PID19073. Earlier native component servers had already been stopped by their owner.

This is main-app failure of its selected backend. The earlier production-fleet two-server capture separately covers B's off/on lifecycle and B shutdown while A continues. It is not claimed that both environments were mounted in the final GUI simultaneously. The combined evidence establishes the transport/reporting behavior while retaining this scope distinction.

Portable files contain actual selected state/logs, the captured tree and executed helper. Full state remains local at `target/t3-repair/activity-final3/silent-after-state.json`; queue responses are under `target/t3-repair/final-agent4`. No pairing token is included. Run `python3 <this-directory>/assert-capture.py` from repository root to validate the captured assertions and source identity.

Task closure remains blocked: existing records lack a paired high-resolution input/focus-event timestamp to establish the specified0.3s latency, and the actual Electron lifecycle trace scenario has not run. Executed reference payload-function parity is useful separate evidence and is not relabeled as Electron proof. No framework issue is resolved by this application check.
