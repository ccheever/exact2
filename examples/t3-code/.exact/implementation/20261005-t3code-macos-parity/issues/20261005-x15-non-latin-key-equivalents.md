---
name: 20261005-x15-non-latin-key-equivalents
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-desktop-shell-details, 20261005-terminal-layout, 20261005-terminal-surface, 20261005-thread-commands-and-keys]
upstream_url: null
reproduced_on: null
---

# X15: Chords and menu equivalents match the physical key under a non-Latin input source

## Summary
T3 Code's keyboard shortcuts keep working when the active input source types non-Latin
letters: ⌘B toggles the sidebar and ⌘K opens the palette under Korean 2-Set or a Cyrillic layout,
because the reference falls back to the physical key (`KeyboardEvent.code`) when the layout key
is not a Latin letter. exact2 matches `charactersIgnoringModifiers`, which is Hangul (`ㅠ`) under
Korean 2-Set, so declared chords and menu key equivalents do not fire. The clone re-issues every
⌘/⌃ chord with its Latin character from an app-level event monitor. exact2 should match chords
and menu equivalents the way a browser and macOS's own apps do.

## Why this issue arose

### The T3 Code behavior
- **Chord resolution.** `apps/web/src/keybindings.ts:97-114` (`resolveEventKeys`): the layout
  key counts; when it is not `a`–`z`, the physical letter (`code` = `KeyB` → `b`) is added; digits
  and punctuation use a fixed `code` table (`EVENT_CODE_SHORTCUT_KEYS`, :60-81). When the layout
  already types a Latin letter the physical key is ignored, so a remapped layout does not fire two
  shortcuts. `shortcutKeyFromEvent` (:90-95) and `isRichTextBoldShortcut` (:425-437, ⌘B bolds in the
  composer) use the same rule.
- **Tests (reference, `keybindings.test.ts`).** "matches non-Latin layout letters using the physical
  key code" (:1025), "ignores the physical key code when the layout types a different Latin letter"
  (:1036), "matches the B key on non-Latin layouts, like the sidebar toggle does" (:1099), "follows
  the letter a Latin layout types, not the physical key" (:1108).
- **Menu.** The Electron application menu declares accelerators (`CmdOrCtrl+,`, zoom,
  `apps/desktop/src/window/DesktopApplicationMenu.ts:170-251`), matched natively. Whether Electron
  matches `⌘ㅠ` to `CmdOrCtrl+B` was not established by the planner; the clone's chords that need
  this are page shortcuts, not menu accelerators.
- Users affected: anyone typing Korean, Russian, Greek, Arabic or Hebrew with a ⌘ chord, with the
  composer focused or not.

### What exact2 does today
- `EXACT2-GAPS.md` summary row X15: "Key equivalents under a non-Latin input source | ⌘B, ⌘K and menu
  chords under Korean 2-Set | host | `R10Connect.swift` re-issues chords". Detail in its X20–X30
  section for X25: "Shortcuts match `charactersIgnoringModifiers` (`Mac/ShortcutsMac.swift:50-64`);
  `aria-keyshortcuts` buttons hear chords before a focused element's `key` handler
  (`docs/contract-grammar.md:775-777`)." (written from `c1522fdac`, checked against `main` `d2cb661eb`).
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**.
- Observed in the clone (clone docs, attended sessions): `AGENT-HANDOFF.md` item 16 recorded "⌘B
  under Korean 2-Set (framework: does not toggle)" in round 9. The AppKit test
  `macos/tests/r10-connect/main.swift` `testTheRemappedChordReachesAMenuEquivalent` asserts
  `menu.performKeyEquivalent(with: key("ㅠ", 11))` is false without the workaround. The
  real-input check R1 ("⌘B / ⌘K under Korean 2-Set with the composer not focused") has "never run;
  AppKit 5/5" (`AGENT-HANDOFF.md:127`).

### Where the clone hits it
`modules/apple/R10Connect.swift:105-123` (`latinChord`, `route`) installs a local `keyDown`
monitor. For a ⌘ or ⌃ chord whose `charactersIgnoringModifiers` is not ASCII it rewrites the event
with the Latin letter from `physicalKeys` (ANSI key codes → `a-z`, `0-9`, `` ` \ [ ] , = - . ' ; / ``,
`:96-101`). It leaves the event alone while text is composing (`hasMarkedText`), for Option-only
presses, and for ASCII keys. Differences from the reference: (1) the key-code table duplicates the
reference's letter rule plus its `EVENT_CODE_SHORTCUT_KEYS` punctuation (`keybindings.ts:60`); no
behavioral difference is known, but the app must maintain it and keys outside it (numpad, ISO- or
JIS-only keys) are never rewritten;
(2) it is an app-wide monitor that runs before the framework's own shortcut layer, so it must be
re-verified for every new chord (terminal ⌘D/⌘N/⌘W/⌃L, ⌘W repeat block, ⌃C) — the plan tickets
list these; (3) WKWebView-hosted surfaces (terminal) receive the rewritten event only if the
monitor runs before the web view, to confirm; (4) menu equivalents work only because the event
is rewritten before AppKit's menu matching.

## Why it must be resolved
Complete parity means a Korean or Russian user keeps every shortcut. Without the monitor the
framework would drop ⌘B, ⌘K, ⌘N, ⌘W and every menu chord for them; with it, each new chord
and each new native surface (terminal, floating player, file editor) needs its own attended check
under Korean 2-Set. Tickets that carry the check: `20261005-thread-commands-and-keys` (new
chords), `20261005-terminal-surface` and `20261005-terminal-layout` (⌃C, ⌘D/⌘N/⌘W/⌃L),
`20261005-desktop-shell-details` (⌘W repeat block), and `20261005-main-fix-adoption` (decides which
key hooks stay). The workaround also depends on a US key-code table. Impact stays nonblocking,
but a declared workaround is not an end state.

## Requested support
The web way: a key event carries both `key` (layout character) and `code` (physical key);
shortcut matching uses `key` when it is a Latin letter and falls back to `code` otherwise.
- **A (preferred, macOS host).** Match `aria-keyshortcuts`, `key=` chord handlers and menu key
  equivalents against the character the user's ASCII-capable layout produces for that key code
  (macOS does this for Command shortcuts in many apps; the usual route is the current
  ASCII-capable keyboard layout via Text Input Services plus `UCKeyTranslate` — to confirm at
  `issue-open`), not the active input source's character. This honors AZERTY/Dvorak/ISO/JIS and
  needs no table.
- **B.** Keep matching `charactersIgnoringModifiers` but expose `code` and `repeat` on
  `KeyboardEvent` (see X25) and let the app match. Smaller, but menu equivalents stay broken
  and every app repeats the table.
Other hosts: iOS hardware keyboards follow the same rule for `UIKeyCommand` input; web already
exposes `code`.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: one button with
`aria-keyshortcuts="Meta+B"` that toggles a label, plus an app menu item with equivalent ⌘K.
Select Korean 2-Set (System Settings › Keyboard › Input Sources), focus a non-text control, press
⌘B and ⌘K. Expected (Chrome page with `keydown` handler using `code`; Safari/AppKit menu): the
button fires and the menu item runs. Actual (`EXACT2-GAPS.md`): neither. Clone scenario:
`AGENT-HANDOFF.md` item 20 (Korean 2-Set, composer not focused: ⌘B toggles the sidebar, ⌘K opens
the palette). The agent can switch the source with `r9input` but cannot send real chords; this is
an attended session.

## Acceptance for the fix
- Minimal app above: under Korean 2-Set and under a Cyrillic layout both chords fire; under
  AZERTY (physical `A` types `q`) ⌘Q does not fire the shortcut bound to ⌘A (the reference test
  at `keybindings.test.ts:1036`).
- AppKit test: `performKeyEquivalent` with `charactersIgnoringModifiers = "ㅠ"` and key code 11
  runs the ⌘B item.
- Clone: R1 and the ⌘B-while-composing cases pass with `R10Connect.latinChord` disabled; the
  AppKit `r10-connect` test for the chord part is deleted, not skipped.

## App adoption after resolution
Remove `latinChord`/`route`/`physicalKeys` and the key monitor from `R10Connect.swift` (keep the
wake, select-on-open and rehover parts), the `testKoreanChordsTakeTheirPhysicalKey` and menu
equivalent tests, and README's note on re-issued chords. Re-run the attended Korean 2-Set rows of
the five tickets above and `20261005-main-fix-adoption`'s key-hook audit. `issue-close` verifies
the rows and that no chord rewriting remains.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's
approval; publication only after approval).
