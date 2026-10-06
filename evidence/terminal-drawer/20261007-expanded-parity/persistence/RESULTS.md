# Normal macOS persistence verification

Isolated bundle `com.exact.t3code.parity.persistence.16334`, disposable backend port16334. Normal `open -n` launch with all `EXACT_*` variables removed; no agent injection. Current source assembled explicitly using `host/apple/build.mjs --bundle`.

Verified on current bundle before the final auth-Tab-only repair:

- Physical click Terminal and individual p key retained native Terminal input focus; composer text did not change.
- Native clipboard paste followed by Return executed harmless `printf` in the real backend PTY; visible marker `PERSIST_A08_FINAL_16334`.
- Created Terminal2 as side-by-side split of Terminal1; created separate Terminal3 group; selected Terminal1.
- Physical drag changed drawer height to388pt (two visible native terminal panes363pt plus24pt title and1pt border).
- Normal quit PID27633, unchanged-bundle open-n relaunch PID42138, no re-pair and no keychain prompt during this final cycle.
- Relaunched app retained visible marker output, split/group structure, Terminal1 selection, exact363pt native pane frames, and the prior unsent composer text. It reconnected to the saved environment automatically.

Evidence: before-quit-final.png, after-relaunch-final.png, before-quit-final-ax.json, after-relaunch-final-ax.json, reopened-final.json. Final fixture bundle/hash and launch environment are in app-native-auth-copy.json. Original isolation roots are in isolation.json.

Limits: terminal-selection composer chip was not seeded, so chip persistence is not covered. During setup, changing the copied app ad-hoc signature triggered a macOS keychain password dialog; it disappeared before the attempted Deny action and saved-environment reconnection subsequently occurred. Its resolution is unknown. This report claims only the final unchanged-bundle quit/relaunch cycle, not automatic access across changed signing identities.

Build pitfall: default Apple build updates standalone products only. Refresh a test .app with --bundle before copying it; otherwise a previous bundle can silently remain stale.
