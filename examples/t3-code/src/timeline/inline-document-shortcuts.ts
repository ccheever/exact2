// BrowserDocumentFrame's view-only shortcut boundary (T3 Code 9c161462c9, MIT).
import { arr, obj, str, type Obj } from '../client/domain';
import { ariaChord, chordWinners } from '../shell/keyboard-dispatch';
import { htmlRenderShortcutsMessage, type HtmlRenderShortcut } from './html-render';

const COMMANDS = new Set(['commandPalette.toggle', 'filePicker.toggle', 'projectSearch.toggle', 'chat.find', 'sidebar.toggle', 'rightPanel.toggle', 'rightPanel.toggleMaximized',
  'threadPanel.toggle', 'terminal.toggle', 'preview.toggle', 'navigation.back', 'navigation.forward', 'thread.previous', 'thread.next']);
export function documentShortcutAllowed(command: string): boolean {
  return COMMANDS.has(command) || /^thread\.jump\.[1-9]$/.test(command) || /^modelPicker\.jump\.[1-9]$/.test(command);
}
const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
function rules(config: Obj) {
  const bindings = arr(config.keybindings), winners = chordWinners(bindings, context);
  const blocked = new Set(bindings.filter(binding => !documentShortcutAllowed(str(binding.command))).map(binding => ariaChord(obj(binding.shortcut))).filter(Boolean));
  return { bindings, winners, blocked };
}
export function documentShortcutCommand(config: Obj, key: { key: string; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; repeat: boolean }): string {
  if (key.repeat) return '';
  const chord = ariaChord({ ...key, key: key.key.toLowerCase() }), { winners, blocked } = rules(config);
  const command = winners.get(chord) ?? '';
  return documentShortcutAllowed(command) && !blocked.has(chord) ? command : '';
}
export function documentShortcutsMessage(config: Obj) {
  const { bindings, winners, blocked } = rules(config);
  const forward: HtmlRenderShortcut[] = [], refuse: HtmlRenderShortcut[] = [];
  const seen = new Set<string>();
  for (const binding of bindings) {
    const shortcut = obj(binding.shortcut), chord = ariaChord(shortcut);
    if (!chord || seen.has(chord)) continue;
    seen.add(chord);
    const value: HtmlRenderShortcut = { key: str(shortcut.key), metaKey: shortcut.modKey === true || shortcut.metaKey === true, ctrlKey: shortcut.ctrlKey === true, shiftKey: shortcut.shiftKey === true, altKey: shortcut.altKey === true };
    if (blocked.has(chord)) refuse.push(value);
    else if (documentShortcutAllowed(winners.get(chord) ?? '')) forward.push(value);
  }
  return htmlRenderShortcutsMessage(forward, refuse);
}
