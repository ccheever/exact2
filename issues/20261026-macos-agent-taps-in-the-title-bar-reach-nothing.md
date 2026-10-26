# macOS: an agent tap on a node under the title bar reaches nothing

**Problem.** In a `window-controls-overlay` app (`display_override`, LLP 1039 D8), the page draws its own title
row under the title bar. `scripts/agent.mjs macos` taps on that row's buttons reply `delivery: "platform"`, but
the page gets no `pointerdown`, `pointerup` or `press`: the journal logs nothing for them and their state does not
change. A tap on a node below the title bar, in the same drive, logs all three. Reproduced with the T3 Code clone
(`examples/t3-code` on `daehyeon/t3-code-main` 6ead721537, window 1280 × 840): `tap toggle-right-panel` and
`tap toggle-terminal` in the chat header change nothing (`state rightPanel` stays `false`), and `tap new-thread`
in the sidebar logs `pointerdown`, `pointerup` and `press view 290`. The row's commands still run by key.

**Cause.** The platform tap (`host/apple/Sources/ExactKit/Mac/AgentMac.swift`, the `NSApp.sendEvent(down)` path)
sends a `leftMouseDown` at the node's window point. With the overlay, the window has an empty unified `NSToolbar`
(`host/apple/Sources/ExactMac/main.swift` `overlayToolbar`) and a full-size content view, so the title bar's
own views lie above the page in the title bar band. The window's hit test finds the title bar there, not the
page's view, and the agent's window is never key, so AppKit treats the click as one on the title bar. Whether a
person's click on a key window reaches the button was not checked.

**Fix.** The agent's tap on a node under the title bar should reach that node, as one below the title bar does:
deliver to the node's view when the title bar's hit view is not a control, or hit-test the page first in the
overlay's band. Check a person's click on the key window as well; if it is lost too, the overlay needs the same
fix for real events. A host test: a `window-controls-overlay` app whose title-row button counts presses, tapped by
the agent.
