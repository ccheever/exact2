// Lane r4-polish: the shell's header shortcut labels as shortcutLabelForCommand
// finds them (keybindings.ts findEffectiveShortcutForCommand, MIT reference, see
// LICENSE-T3). Newest binding first, a binding whose `when` fails the default
// context (no terminal, nothing focused) is skipped and a later binding on the
// same chord shadows an earlier one; the first binding left for the command
// names the shortcut. Reading the when AST as text dropped every
// `!terminalFocus` rule, so New thread (⇧⌘O) and its Shift+click hint (⇧⌘N)
// lost their labels.
import { arr, obj, str, type Obj } from './domain';
import { ariaChord, chordWinners, type DispatchContext } from './keyboard-dispatch';

const DEFAULT_CONTEXT: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };

/** The shortcut the command answers to in the default context, or null. */
export function effectiveShortcut(config: Obj, command: string, context: Partial<DispatchContext> = {}): Obj | null {
  const bindings = arr(config.keybindings);
  const winners = chordWinners(bindings, { ...DEFAULT_CONTEXT, ...context });
  for (const binding of [...bindings].reverse()) {
    if (str(binding.command) !== command) continue;
    const chord = ariaChord(obj(binding.shortcut));
    if (chord && winners.get(chord) === command) return obj(binding.shortcut);
  }
  return null;
}
