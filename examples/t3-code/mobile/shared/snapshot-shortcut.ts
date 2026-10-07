// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/snapshot-shortcut.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
import { arr, obj, str, type Obj } from './domain';
import { shortcutInput, validShortcut } from './keybinding-settings';
export function snapshotShortcut(value: string): string {
  const aliases: Record<string, string> = { cmd: 'meta', command: 'meta', control: 'ctrl', option: 'alt', escape: 'esc', arrowleft: 'left', arrowright: 'right', arrowup: 'up', arrowdown: 'down' };
  const input = value.trim().toLowerCase(), tokens = (input.endsWith('++') ? input.slice(0, -1) + 'plus' : input).split('+').map(token => aliases[token] || token);
  if (tokens.length === 2 && tokens[0] === tokens[1] && ['shift', 'meta', 'ctrl', 'alt'].includes(tokens[0])) return tokens.join('+');
  const key = tokens.pop() || '', modifiers = tokens.map(token => token === 'mod' ? 'meta' : token);
  if (!key || !modifiers.length || new Set(modifiers).size !== modifiers.length || !modifiers.every(token => ['meta', 'ctrl', 'alt', 'shift'].includes(token)) || !validShortcut([...modifiers, key === 'plus' ? '+' : key].join('+'))) return '';
  return [...['meta', 'ctrl', 'alt', 'shift'].filter(token => modifiers.includes(token)), key].join('+');
}
export function snapshotConflict(config: Obj, value: string): string {
  const canonical = snapshotShortcut(value);
  if (!canonical || ['shift+shift', 'meta+meta', 'ctrl+ctrl', 'alt+alt'].includes(canonical)) return '';
  const found = arr(config.keybindings).find(binding => snapshotShortcut(shortcutInput(obj(binding.shortcut))) === canonical);
  return found ? str(found.command) : '';
}

const pairs: Record<string, string> = { 'shift+shift': 'Shift', 'meta+meta': 'Command', 'ctrl+ctrl': 'Control', 'alt+alt': 'Option' };
const symbols: Record<string, string> = { shift: '⇧', meta: '⌘', ctrl: '⌃', alt: '⌥' };
/** Reference sameSnapShotShortcut: pairs by modifier, chords by conflict key. */
export function sameSnapshotShortcut(left: string, right: string): boolean {
  const a = snapshotShortcut(left), b = snapshotShortcut(right);
  return a !== '' && a === b;
}
/** Reference formatShortcutKeyLabel over this client's canonical key names. */
export function snapshotKeyLabel(key: string): string {
  const named: Record<string, string> = { plus: '+', space: 'Space', esc: 'Esc', up: 'Up', down: 'Down', left: 'Left', right: 'Right' };
  if (named[key]) return named[key];
  return key.length === 1 ? key.toUpperCase() : key.slice(0, 1).toUpperCase() + key.slice(1);
}
/** Reference snapShotShortcutKeyLabels on macOS: ⌃ ⌥ ⇧ ⌘ then the key, or a pair twice. */
export function snapshotKeyLabels(value: string): string[] {
  const canonical = snapshotShortcut(value);
  if (!canonical) return [];
  if (pairs[canonical]) { const symbol = symbols[canonical.split('+')[0]]; return [symbol, symbol]; }
  const tokens = canonical.split('+'), key = tokens.pop() || '';
  return [...['ctrl', 'alt', 'shift', 'meta'].filter(token => tokens.includes(token)).map(token => symbols[token]), snapshotKeyLabel(key)];
}
/** Reference formatSnapShotShortcutLabel on macOS ("Shift + Shift", "⌃⌥K"). */
export function snapshotShortcutAria(value: string): string {
  const canonical = snapshotShortcut(value);
  if (pairs[canonical]) return `${pairs[canonical]} + ${pairs[canonical]}`;
  return snapshotKeyLabels(canonical).join('');
}
/** Desktop snapShotShortcutSystemConflict: single-modifier chords the system or typing owns. */
export function snapshotSystemConflict(value: string): string {
  const canonical = snapshotShortcut(value);
  if (!canonical || pairs[canonical]) return '';
  const tokens = canonical.split('+'), key = tokens.pop() || '';
  if (tokens.length !== 1) return '';
  const [modifier] = tokens;
  if (modifier === 'shift') return 'Shift combinations are used for typing and text selection. Add another modifier.';
  const actions: Record<string, string> = { a: 'Select All', c: 'Copy', f: 'Find', n: 'New', o: 'Open', p: 'Print', q: 'Quit', s: 'Save', t: 'New Tab', v: 'Paste', w: 'Close Window', x: 'Cut', z: 'Undo' };
  // On macOS a saved "mod" is Command, so Command chords take the mod rule first.
  if (modifier === 'meta' && actions[key]) return `This shortcut is ${actions[key]} in most apps.`;
  if (modifier === 'ctrl' && ['c', 'd', 'z'].includes(key)) return 'This shortcut controls running commands in terminals.';
  if (modifier === 'alt' && key === 'tab') return 'The system uses Alt+Tab to switch apps.';
  return '';
}

const specialCommands: Record<string, string> = {
  'composer.sendAlternate': 'Composer: Opposite Queue or Steer Action', 'composer.sendBackground': 'Composer: Start in Background',
  'composer.sendAndNewThread': 'Composer: Send and Start New Thread',
  'thread.steerQueuedMessage': 'Queue: Send First Queued Message as Steer', 'thread.editQueuedMessage': 'Queue: Edit Last Queued Message',
  'thread.copyReference': 'Pull Request: Copy Link or Thread ID', 'usage.cost': 'Usage: Cost', 'usage.tokens': 'Usage: Tokens', 'usage.limits': 'Usage: Limits',
  'usage.period.day': 'Usage: Period: Past 24h', 'usage.period.week': 'Usage: Period: 7 days', 'usage.period.month': 'Usage: Period: 30 days', 'usage.period.quarter': 'Usage: Period: 90 days',
};
const titleCase = (segment: string) => segment.replace(/([a-z0-9])([A-Z])/g, '$1 $2').split(/[-_\s]+/).filter(Boolean).map(part => part.slice(0, 1).toUpperCase() + part.slice(1)).join(' ');
/** Reference KeybindingsSettings.logic commandLabel. */
export function commandLabel(command: string): string {
  if (specialCommands[command]) return specialCommands[command];
  if (command.startsWith('script.') && command.endsWith('.run')) return `Run Script: ${titleCase(command.slice('script.'.length, -'.run'.length))}`;
  return command.split('.').map(titleCase).join(': ');
}
