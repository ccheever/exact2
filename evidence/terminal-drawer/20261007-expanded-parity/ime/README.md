# Physical Korean IME comparison

The user typed with a physical Korean keyboard into the original Electron terminal and the actual macOS clone. No paste or synthetic Unicode input was used for these captures. Both used a raw PTY recorder that logs bytes and never executes them.

- Original: `한글` + Space emitted `ed959ceab88020` exactly once. Trusted composition events are recorded in [the original report](../oracle/MANUAL-IME.md).
- Clone first capture: `e3858ee3858fe384b4eab88020` (`ㅎㅏㄴ글 `). There was no DOM trace during this capture, so its cause remains unresolved.
- The first diagnostic repeat went into a different fixture app, according to the user's correction. It provides no evidence about this recorder. The other fixture was subsequently hidden and the target window identified as `IME CHECK - Verify two - 16331`.
- Correctly targeted diagnostic repeat: a preparatory `ㅁ` and Backspace emitted `e385817f`; the following `한글` + Space emitted `ed959ceab88020` exactly once. The trusted DOM trace records `ㅎ→하→한` and `ㄱ→그→글`, with no intervening focus/blur. The preparatory input is not removed from the raw evidence.
- Fresh accessory-view repeat also emitted split Jamo. Its trusted trace shows the first `ㅎ` arriving as non-composing `insertText`, before any `compositionstart`. Although its window was key, another PID remained the foreground app; this is not a valid normal-app IME acceptance result.
- **Normal foreground first-input control passes.** A fresh view in the unchanged normal bundle received `한글 ` once, with 50 trusted DOM events and no focus changes. App activation, foreground PID and key-window state were verified before input; the recorder was started by backend RPC without local typing. See [normal report](normal/RESULTS.md).

The passive diagnostic observer was injected into the existing WKWebView with LLDB after the first capture. It records textarea events and does not change input or vendor behavior. The capture reads its event array through `evaluateJavaScript`; app source was unchanged. The original mixed Escape attempt does not establish marked-text cancellation.

The accessory and normal launches differ in activation policy. No input/vendor change was made from the confounded accessory observation. Normal first-input commit matches the original; composition cancellation and other IMEs remain unverified.
