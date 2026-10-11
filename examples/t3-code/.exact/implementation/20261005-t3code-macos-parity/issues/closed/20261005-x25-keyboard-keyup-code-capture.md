---
name: 20261005-x25-keyboard-keyup-code-capture
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-browser-surface, 20261005-desktop-shell-details, 20261005-diff-review-engine, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-writing-and-metadata, 20261005-right-panel-tab-menu, 20261005-sign-in-terminals, 20261005-terminal-drawer, 20261005-terminal-layout, 20261005-terminal-surface, 20261005-thread-commands-and-keys]
upstream_url: https://github.com/ccheever/exact2/issues/140
reproduced_on: null
---

# X25: Keyboard facts for Contract: keyup, modifiers held, `code`, `repeat`, capture phase, composition end on a chord

Moved to main `issues/20261009-capture-phase-key-events.md` (2026-10-09); tracked there.

## Summary

T3 Code listens to the keyboard in ways the web allows and exact2 did not: it tracks which modifiers are held (keydown and keyup on the window, capture phase), reads `event.code` and `event.repeat`,
and ends text composition when a chord arrives. The clone does each of these in Swift key monitors in its app modules. Main #220 has since added `keyup`, `code` and `repeat`; the capture phase is what main still tracks.

## Why it arose

### The T3 Code behavior
- **Modifier-held state.** `useShortcutModifierState` listens on `window` for `keydown` and `keyup` in the capture phase, plus `paste`, `blur` and `focusin`, and tracks Meta, Ctrl, Alt and Shift
  (`apps/web/src/shortcutModifierState.ts:30-76`). Users: sidebar thread jump hints while ⌘ is held (`components/Sidebar.tsx:4770`, `LegacySidebar.tsx:3188`), the Send button's alternate label and action while ⌘ is held
  (`components/chat/ComposerPrimaryActions.tsx:109`), the pull-request list's quick actions while Shift is held alone (`routes/_chat.pull-requests.tsx:347`).
- **Quit by holding ⌘Q.** The desktop main process intercepts the accelerator before the menu: hold 1,200 ms to quit, double press within 500 ms, a 600 ms release grace (`apps/desktop/src/window/QuitHold.ts:7-20`).
- **`event.repeat`.** Held keys must not repeat commands: copy thread reference, close the right-panel surface (⌘W), toggle the model picker, open a composer control (`components/ChatView.tsx:7623,7698,7766,7778`),
  the command palette (`CommandPalette.tsx:530,547`), the legacy sidebar (`LegacySidebar.tsx:3570`), open-favorite-editor (`keybindings.ts:417`), and the composer's submit intent (`composer-logic.ts:50`).
  The terminal sends press, repeat and release to Ghostty (`terminal/ghostty/core.ts:481-483`).
- **`event.code`.** A shortcut matches the typed key, and falls back to the physical key for layouts that type non-Latin letters (`keybindings.ts:85-112`). The terminal swallows the keyup of a consumed keydown
  (`terminal/ghostty/surface.ts:1061-1158`); the device stream sends key up as well as down (`components/device/DeviceStreamView.tsx:405-408`).
- **Capture phase.** At least 12 window or document `keydown` listeners use the capture phase, for example the right-panel launcher letters (`RightPanelTabs.tsx:419`), the composer (`chat/ChatComposer.tsx:2483,5624`) and the chat shortcuts
  (`ChatView.tsx:7848,7924`), so a shortcut fires before a focused element swallows it.
- **Key recorder.** Settings › Keybindings and the project script editor capture the next chord into a binding string (`components/settings/KeybindingsSettings.tsx:770`, `components/projectScriptEditor.tsx:211`).

### Where the clone hits it
Twelve tickets list this issue (`blocks`); each carries a native monitor or an attended keyboard row for it, including held-key and Korean 2-Set checks.

## Clone workaround

Swift key monitors in the app modules:
- Held modifiers: `modules/apple/T3Menus.swift` (flagsChanged, keyDown and keyUp monitor for the ⌘ jump hints) and `T3Sidebar.swift`; the Send intent in `T3ComposerIntent.swift`.
- Repeat and capture phase: `R8KeysLauncher.swift` (`!event.isARepeat`, a window-level handler that runs before type-to-focus), the Send path `T3Composer.swift` (`.consumeRepeat`), and the quit-hold path in the native shell.
- `code` and composition: `R9Input.swift` ends composition when a ⌘ chord comes ("as a browser's compositionend does, so ⌘B bolds under Korean 2-Set") and `R10Connect.swift` re-issues chords with the Latin
  letter (X15). `T3KeyRecorder.swift` captures chords as binding strings; `RightPanelTabsInput.swift` takes the tab rename field's Escape and chords.
- The held ⌘W drop and the ⌘Q hold (`T3Menus.swift`) stay native for good: the reference does both in its main process (`DesktopWindow.ts`, `QuitHold.ts`), before the menu. The terminal's suppressed key-ups,
  the device stream's key up/down and the native composer's ⌥↑ and Return are native views, not Contract fields.
- #140's decision (2026-10-08) declined a held-modifier fact: at adoption, held ⌘ (the jump hints in `T3Sidebar.swift`, the Send label in `T3ComposerIntent.swift`) is rebuilt as a root capture `key`/`keyup`
  handler plus `exactPage().hasFocus`, and a capture handler retires `R8KeysLauncher.swift`, `T3KeyRecorder.swift` and the Escape and chords of `RightPanelTabsInput.swift`.

## Evidence and history

- Filed 2026-10-06 as [#140](https://github.com/ccheever/exact2/issues/140).
- 2026-10-07 ([adopt-main-fixes-r5](../../tasks/closed/20261007-adopt-main-fixes-r5.md), main `261dd4e10`): main #220 (`5a20af1cf`) adds DOM's `keyup`, `KeyboardEvent.code` and `.repeat` on every host
  (macOS: AppKit's keyUp and a released modifier, `code` from the key code, `repeat` from `isARepeat`); a ⌘ chord during a composition commits it first. Nothing was removed: every clone monitor needs a window
  capture-phase handler or held-modifier state, or lives where Contract key handlers do not reach.
- Found then, not done: the Send gesture's and the sidebar row's click modifiers could read `press`'s `MouseEvent` (`T3ComposerIntent.take`, `T3Sidebar.pressModifiers`), which main already had before #220.
