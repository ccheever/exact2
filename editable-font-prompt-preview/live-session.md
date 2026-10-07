# editable-font-prompt-preview: live session record (2026-10-08)

macOS 26 (Darwin 25.6.0), Xcode 27, Bun 1.4.2; agent driver at 1280x840, window screenshots at 2x.

## Lane

- Port 16520 (the app's embedded server), in the task range 16500-16599. Isolated HOME, CODEX_HOME,
  CLAUDE_CONFIG_DIR, XDG_*, T3_LOCAL_HOME under the worktree's ignored `target/lane/`;
  `T3CODE_TELEMETRY_ENABLED=false`. Never 3773, `~/.t3` or the `t3code` scheme.
- Server: the pinned `t3 v0.0.46-nightly.20261005.2667`; fixture project `target/lane/project` added offline
  (`t3 project add --base-dir`). No provider was configured or called.
- The thread draft: the new-thread composer of that project, typed "Unrelated thread draft" first.

## Sessions

1. After, attempt 1 (agent driver, the bundle's binary). This found three gaps, which were then fixed:
   - Tab typed a tab character and kept focus. Fixed: the preview's editor moves focus with Tab and Shift+Tab.
   - Chip labels kept the system font under New York. Fixed: chips inherit the prompt family.
   - An unset Prompt font previewed in system-ui while its label said New York. Fixed: it follows the interface font.
   Typing, caret, selection, atomic chips, family change, Advanced remount and draft isolation already
   worked. Transcript: `ops-after-attempt1.txt`.
2. Before (base `da4f4512f`, never-edited evidence worktree): the sample is static. `type prompt-font-preview key a`
   changes nothing (`value: ""`); setting text is refused with "view 2423 is not an input" (as the audit saw).
   The composer draft was read back unchanged. Transcript: `ops-before.txt`.
3. After, retry (the final build except the later `appearance="none"`, which only removes the field's
   focus ring): every row below, transcript `ops-after.txt`.
   - Key-by-key " AUDIT_FONT_PROBE" after Cmd+Down lands at the end.
   - Shift+Right x3 + "Try" replaces "Use".
   - Five Right presses from the start step over the 16-character skill chip as one.
   - From the end, 34 Left presses reach the end of the SettingsPanels.tsx chip, and Backspace removes the whole chip.
   - Tab moves focus to "Monospace font family" with no tab typed. Shift+Tab returns to the sample,
     and Shift+Tab again reaches "Interface font size".
   - New York keeps the draft, with the text and chip labels in New York.
   - Advanced on remounts the Prompt font sample (the reference sample again, in the inherited New York);
     " OK" typed; size 18 keeps it.
   - Advanced off remounts the Interface sample fresh.
   - The thread draft reads "Unrelated thread draft" after all of this.
   - The lane `settings.json` was last written at 03:42:39, at server start (session 3 ran
     04:22–04:24), and no thread row exists.

## Not reachable in agent mode

Cmd+Z: the driver hands a key equivalent to the focused view, then the main menu. ExactKit's TextArea routes
the undo chord to its own undo manager only for markup editors (`TextAreaMac.swift` performKeyEquivalent),
and in agent mode the menu's `undo:` finds no key window, so the agent's Meta+z leaves the text unchanged
on any plain textarea (the composer's too). The undo history itself is tested in
`macos/tests/composer/promptpreview.swift` (typing then `undoManager.undo()` restores the sample).
The real Cmd+Z row is deferred to the real-input batch: the screen is locked while the user is away.
