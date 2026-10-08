---
name: 20261005-x25-keyboard-keyup-code-capture
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-browser-surface, 20261005-desktop-shell-details, 20261005-diff-review-engine, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-writing-and-metadata, 20261005-right-panel-tab-menu, 20261005-sign-in-terminals, 20261005-terminal-drawer, 20261005-terminal-layout, 20261005-terminal-surface, 20261005-thread-commands-and-keys]
upstream_url: https://github.com/ccheever/exact2/issues/140
reproduced_on: null
---

# X25: Keyboard facts for Contract: keyup, modifiers held, `code`, `repeat`, capture phase, composition end on a chord

## Summary

T3 Code listens to the keyboard in ways the web allows and exact2 does not yet: it tracks which modifiers are held (keydown and keyup on the window, capture phase), reads `event.code` and `event.repeat`,
and ends text composition when a chord arrives. The clone does each of these in Swift key monitors in five app modules. Contract key handlers on `main` carry modifiers and can claim a key, but they have no keyup, `code`, `repeat`, capture phase
or composition-end fact. Adding them in the web form would let the app drop the monitors.

## Why this issue arose

### The T3 Code behavior
- **Modifier-held state.** `useShortcutModifierState` listens on `window` for `keydown` and `keyup` in the capture phase, plus `paste`, `blur` and `focusin`, and tracks Meta, Ctrl, Alt and Shift
  (`apps/web/src/shortcutModifierState.ts:30-76`; the listeners at `:61-65`). It resets on a synthetic paste because dictation tools send a ⌘V whose Meta keyup never reaches the page ("the thread jump hints stick on screen").
  Users: sidebar thread jump hints while ⌘ is held (`components/Sidebar.tsx:4770`, `LegacySidebar.tsx:3188`), the Send button's alternate label and action while ⌘ is held (`components/chat/ComposerPrimaryActions.tsx:109`),
  the pull-request list's quick actions while Shift is held alone (`routes/_chat.pull-requests.tsx:347`, `ignoreEditable = true`).
- **Quit by holding ⌘Q.** The desktop main process intercepts the accelerator before the menu. Hold 1,200 ms to quit, double press within 500 ms, and a 600 ms release grace; "still held" is proven by auto-repeat
  keydowns because macOS hides the letter keyup while ⌘ is down (`apps/desktop/src/window/QuitHold.ts:7-20`). Quitting from the menu always quits at once.
- **`event.repeat`.** Held keys must not repeat commands: copy thread reference, close the right-panel surface (⌘W), toggle the model picker, open a composer control (`components/ChatView.tsx:7623,7698,7766,7778`),
  the command palette (`CommandPalette.tsx:530,547`), the legacy sidebar (`LegacySidebar.tsx:3570`), open-favorite-editor (`keybindings.ts:417`), and the composer's submit intent, which returns nothing while composing, on `keyCode === 229` or on repeat (`composer-logic.ts:50`).
  The terminal sends press, repeat and release to Ghostty (`terminal/ghostty/core.ts:481-483`).
- **`event.code`.** A shortcut matches the typed key, and falls back to the physical key for layouts that type non-Latin letters (`keybindings.ts:85-112`, `resolveEventKeys`). The terminal keeps a set of suppressed codes so a key whose
  keydown was consumed also swallows its keyup (`terminal/ghostty/surface.ts:1061-1158`); the device stream sends key up as well as down (`components/device/DeviceStreamView.tsx:405-408`).
- **Capture phase.** At least 12 window or document `keydown` listeners use the capture phase, for example the right-panel launcher letters (`RightPanelTabs.tsx:419`), the composer (`chat/ChatComposer.tsx:2483,5624`) and the chat shortcuts
  (`ChatView.tsx:7848,7924`), so a shortcut fires before a focused element swallows it.
- **Key recorder.** Settings › Keybindings and the project script editor capture the next chord into a binding string; Escape cancels; modifier-only presses are ignored
  (`components/settings/KeybindingsSettings.tsx:770`, `components/projectScriptEditor.tsx:211`).
- Reference tests: `keybindings.test.ts` (71 cases) and `shortcutModifierState.test.ts` (5 cases), counted by `it(` on 2026-10-05.

### What exact2 does today
- `EXACT2-GAPS.md` X25 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): the need is "Keyboard: keyup / modifiers-held fact, `KeyboardEvent.code` and `repeat`, capture-phase handler,
  compositionend on a chord". Detail: "Shortcuts match `charactersIgnoringModifiers` (`Mac/ShortcutsMac.swift:50-64`); `aria-keyshortcuts` buttons hear chords before a focused element's `key` handler
  (`docs/contract-grammar.md:775-777`)."
- Already fixed on `main` (same file, "Already fixed on exact2 main"): "Key events carry modifiers; `preventDefault()` claims a key (`f35b3eafc`, `8a0afbeab`)." "Key events bubble to ancestors with a key handler, `tabindex` exists, so rows can take keys
  (`f35b3eafc`, `d672f9372`)." These need the rebase (`20261005-main-fix-adoption`); this issue lists only what remains.
- Related: X15 (key equivalents under a non-Latin input source) and X20 (rich-text editing) are separate issues that touch the same area.
- Library (`20261005-platforms-v3`): keyboard access and visible focus are guidance (accessibility.md:38); key event handlers and facts are not covered: unknown in this library.

### Where the clone hits it
- Held modifiers: `modules/apple/T3Menus.swift:44,267-277,334` (flagsChanged, keyDown and keyUp monitor for the ⌘ jump hints) and `T3Sidebar.swift:40`; the Send intent in `T3ComposerIntent.swift:21-24,53` (74 lines), whose header says
  "Exact's press action carries no event, so the client asks for the newest Return key-down or pointer press when it sends".
- Repeat and capture phase: `R8KeysLauncher.swift` (86 lines; `!event.isARepeat` at `:71`, a window-level handler that runs before type-to-focus), the Send path `T3Composer.swift:144` (`.consumeRepeat`), and the quit-hold path in the native shell.
- `code` and composition: `R9Input.swift` (412 lines) ends composition when a ⌘ chord comes ("as a browser's compositionend does, so ⌘B bolds under Korean 2-Set") and `R10Connect.swift:113,122` re-issues chords with the Latin
  letter (X15). `T3KeyRecorder.swift` (107 lines) captures chords as binding strings.
- Differences from the reference: none known for the covered cases; all cases need a person at a real keyboard to prove, which is why the plan has `(attended session)` keyboard rows, including held-key and Korean 2-Set checks.

## Why it must be resolved

The goal is a complete clone, and keyboard behavior is where Mac users notice small differences first: jump hints that stick or never show, a held ⌘W that closes five tabs, a Send button that does the wrong thing under ⌘.
Eleven tickets list this issue; each one carries a native monitor or an attended row for it (`blocks`). The workaround is working, but it is five Swift modules (838 lines in total; part of that code does other work) that must track every new shortcut
the reference adds, and each reaches the key path below the Contract, where tests cannot see it. Web-standard facts would let the app declare these behaviors in the Contract, testable by the agent.

## Requested support

The web way, on the macOS host first:

| Web form | Request |
| --- | --- |
| `keyup` event | A key handler that fires on release, with the same fields as keydown |
| `KeyboardEvent.code` | The physical key name (`KeyB`, `Digit1`), independent of layout and input source |
| `KeyboardEvent.repeat` | True for auto-repeat keydowns |
| `event.getModifierState()` / modifiers-held fact | A live fact (`shift`, `meta`, `ctrl`, `alt` held), reset on window blur, usable in a `derive` |
| Capture phase | A key handler option so an ancestor sees the key before the focused element (`addEventListener(..., true)`) |
| `compositionend` on a chord | When a chord arrives during composition, composition ends first, as in a browser, then the chord is delivered |
| Press with modifiers | The `press` action (a button click) can read the modifiers held at the click (the Send button case) |

Alternative: a single `keys` source that streams key events (down, up, repeat, code, modifiers) to a data module. The element-level forms are preferred because they match the web.

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a column with a `key` handler that logs `key`, `code`, `repeat`, `modifiers`; a derive that shows whether ⌘ is held.
2. Agent: `press Meta+b` and a held-key sequence. Expected (web): a keyup event, `code: "KeyB"`, `repeat: true` on the held repeats, and the ⌘-held derive true while the key is down. Actual (expected on `main`, to confirm): modifiers on keydown only, no keyup,
   no `code`, no `repeat`, no live held fact.
3. Clone scenario: lane build, hold ⌘ in the sidebar; jump hints appear (from the Swift monitor). Hold ⌘W on a right-panel tab with the Korean 2-Set source (attended): one close only.

## Acceptance for the fix

- A conformance case against Chrome: the same key sequence (down, repeat, up, with modifiers) gives equal `key`, `code`, `repeat` and modifier values in Chrome and on the macOS host, including under a non-Latin input source and during composition.
- Agent `press` with a hold form (to be defined) delivers repeats and release; `state` shows the live modifier fact.
- An AppKit test: composition ends before a chord; a capture-phase handler sees a key before the focused text field.

## App adoption after resolution

Delete or shrink the monitors: `T3Menus.swift` jump-hint monitor and `T3Sidebar.swift` modifier tracking, `T3ComposerIntent.swift`, `R8KeysLauncher.swift`, the re-issue in `R10Connect.swift` (with X15), the composition part of `R9Input.swift`,
and keep `T3KeyRecorder.swift` only if the Contract handler cannot capture a chord. Rows that must pass after the move: jump hints, Send alternates, ⌘W repeat, launcher letters, the key recorder, held-key checks in the terminal tickets, Korean 2-Set rows.
`issue-close` checks the held-key and Korean rows by agent where the new `press` form allows it.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Merged upstream in part (2026-10-07, adopt-main-fixes-r5)

Filed as [#140](https://github.com/ccheever/exact2/issues/140), still open for a capture-phase handler and a
held-modifier fact. Main #220 (`5a20af1cf`) adds DOM's `keyup`, `KeyboardEvent.code` and `.repeat` on every host
(macOS: AppKit's keyUp and a released modifier, `code` from the key code, `repeat` from `isARepeat`); a ⌘ chord
during a composition commits it first (main's QUEUE). `#140` also notes that `press` already takes a `MouseEvent`
with modifiers. In the feature branch since main `261dd4e10` ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)).

Nothing to remove. Every clone monitor needs what is still missing, or lives where Contract key handlers do not reach:
- held ⌘ for the sidebar's jump hints (`T3Sidebar.swift`) and the Send button's alternate label
  (`T3ComposerIntent.swift`): the reference listens on `window` in the capture phase, so the hints show with nothing
  focused; a Contract `key`/`keyup` hears keys only inside the focused element (#140, held-modifier fact);
- the surface launcher's letters before type-to-focus (`R8KeysLauncher.swift`), the key recorder that must see ⌘ chords
  before the menu (`T3KeyRecorder.swift`) and the tab rename field's Escape and chords (`RightPanelTabsInput.swift`): a
  window capture-phase handler (#140);
- the held ⌘W drop and the ⌘Q hold (`T3Menus.swift`): the reference does both in its main process
  (`DesktopWindow.ts`, `QuitHold.ts`), before the menu, so they stay native;
- the terminal's suppressed key-ups, the device stream's key up/down, the native composer's ⌥↑ and Return: native views,
  not Contract fields.

Found, not this round: the Send gesture's and the sidebar row's click modifiers could read `press`'s `MouseEvent`
(`T3ComposerIntent.take`, `T3Sidebar.pressModifiers`), which main already had before #220. The issue stays open for #140.

## Decided upstream (2026-10-08): waits for main fix of #140; no held-modifier fact

[Charlie on #140](https://github.com/ccheever/exact2/issues/140#issuecomment-6055585110): "Choose capture-phase key handling; decline a new keyboard fact. … existing keyup plus page focus
covers held modifiers."
- Waits for main fix of [#140](https://github.com/ccheever/exact2/issues/140): a capture handler covers `R8KeysLauncher.swift`, `T3KeyRecorder.swift` and the
  Escape and chords of `RightPanelTabsInput.swift`.
- **Different design:** held ⌘ (the jump hints in `T3Sidebar.swift`, the Send label in `T3ComposerIntent.swift`) is
  rebuilt as a root capture `key`/`keyup` handler plus `exactPage().hasFocus`. At adoption, check that a root
  capture handler hears keys while nothing is focused.
- The ⌘W repeat drop and the ⌘Q hold stay native (the reference does both in its main process).
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved; the capture event's syntax is still to design.
