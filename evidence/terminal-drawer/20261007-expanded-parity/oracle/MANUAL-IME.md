# Original Electron: user-operated Korean IME

**Physical Korean composition and commit passed:** the user switched to Korean 2-Set and typed `한글` followed by Space in the original terminal. The real PTY received exactly `ed959ceab88020` (`한글 `), once. This supersedes the earlier automation-only unknown for the basic composition/commit path. Cancellation and Korean-layout shortcuts remain unverified.

Original Electron PID40550 displayed its real terminal in the isolated Verify three thread. A raw PTY recorder recorded and echoed committed input without executing it. No temporary textarea or synthesized text was used for this test. The user explicitly confirmed completion before each capture. Prior input source was ABC; Korean 2-Set was left selected for the parent's subsequent native-app comparison.

## Phase 1: physical `한글` and Space

- PTY chunks joined to seven bytes: `ed959c` + `eab88020`.
- Trusted compositionstart/update events showed `ㅎ → 하 → 한`, then `ㄱ → 그 → 글 → 글 `.
- Captured compositionend events had `isTrusted:false`; the reason is not established. Trusted start/update events, user-confirmed physical typing and exact real PTY bytes establish the successful commit independently.
- Separate snapshots: manual-ime-phase1-events.json, manual-ime-phase1-pty.ndjson and manual-ime-phase1-original.png. Boundary: three PTY rows including ready, 42 DOM events.

## Phase 2: mixed input; no cancellation conclusion

The requested test was a single `ㅎ` followed by Escape. The user clarified that mixed input and repeated Escape presses occurred. Therefore this is not a controlled single-character cancellation trial.

Observed delta after the preserved phase 1 boundary:

```text
PTY hex: e3858ee3858e207f7f7f7f1b1b
Meaning: ㅎㅎ + Space + four DEL bytes + two ESC bytes
```

Captured trusted keydowns were KeyG (`ㅎ`), KeyG (`ㅎ`), Space while composing, four Backspaces while not composing, and two Escapes while not composing. Composition commits contained `ㅎ` then `ㅎ `. No Escape DOM key event was observed during composition; the IME could consume a physical key before a DOM event, so this does not establish the complete physical-key sequence. The first-phase bytes remained unchanged.

Separate snapshots: manual-ime-phase2-events.json, manual-ime-phase2-pty.ndjson, manual-ime-phase2-summary.json and manual-ime-phase2-original.png. These snapshots exclude cleanup.

## Cleanup and remaining work

After capture, listeners were removed and the recorder received one separate Ctrl+D (`04`), then logged closed and restored termios. The original window was hidden and GUI ownership returned to the parent. Input source remained Korean 2-Set by explicit parent request; the parent must restore the prior ABC source after its native comparison. No clipboard access or personal-app input occurred.

Still required for full A05: controlled composition Backspace/cancellation, Korean-layout shortcuts, and the matching native-app physical-input result. The prepared manual capture scripts and recorder are retained for reproducibility; they must only run under an explicit GUI allocation and user agreement.
