import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { buildRows, commandOptions, commandLabel, conflictLabels, conflictText, parseWhen, printWhen, shortcutLabel, whenEditor, type KeybindingRowView } from './keybinding-view';
// Canonical shortcut/when serialization from KeybindingsSettings.logic.ts.
export function shortcutInput(shortcut: Obj): string {
  return [...['mod', 'meta', 'ctrl', 'alt', 'shift'].filter(mod => shortcut[`${mod}Key`] === true), shortcut.key === ' ' ? 'space' : shortcut.key === 'escape' ? 'esc' : str(shortcut.key)].join('+');
}
export function whenExpression(ast: unknown, depth = 0): string {
  const node = obj(ast); if (!node.type) return ''; if (depth > 64) throw new Error('Unsupported keybinding condition depth.');
  const wrap = (child: unknown) => { const value = whenExpression(child, depth + 1), kind = obj(child).type; return kind === 'identifier' || kind === 'not' ? value : `(${value})`; };
  if (node.type === 'identifier') return str(node.name);
  if (node.type === 'not') return `!${wrap(node.node)}`;
  if (node.type === 'and' || node.type === 'or') return `${wrap(node.left)} ${node.type === 'and' ? '&&' : '||'} ${wrap(node.right)}`;
  throw new Error('Unsupported keybinding condition.');
}
// Same shortcut token and expression grammar as pinned shared/keybindings.ts.
export function validShortcut(value: string): boolean {
  const tokens = value.toLowerCase().split('+').map(token => token.trim());
  let trailing = false;
  while (tokens[tokens.length - 1] === '') { trailing = true; tokens.pop(); }
  if (trailing) tokens.push('+');
  const modifiers = ['cmd', 'meta', 'ctrl', 'control', 'shift', 'alt', 'option', 'mod'];
  return tokens.every(Boolean) && tokens.filter(token => !modifiers.includes(token)).length === 1;
}
export function validWhen(value: string): boolean {
  if (!value.trim()) return true;
  const tokens: string[] = []; let offset = 0;
  while (offset < value.length) {
    const token = /^(\s+|&&|\|\||[!()]|[A-Za-z_][A-Za-z0-9_.-]*)/.exec(value.slice(offset));
    if (!token) return false;
    if (!/^\s+$/.test(token[0])) tokens.push(token[0]);
    offset += token[0].length;
  }
  let index = 0;
  const primary = (depth: number): boolean => {
    if (depth > 64) return false;
    if (tokens[index] === '!') { index++; return primary(depth + 1); }
    if (tokens[index] === '(') { index++; if (!expression(depth + 1) || tokens[index] !== ')') return false; index++; return true; }
    if (!/^[A-Za-z_]/.test(tokens[index] || '')) return false;
    index++; return true;
  };
  const expression = (depth: number): boolean => {
    if (!primary(depth)) return false;
    while (tokens[index] === '&&' || tokens[index] === '||') { index++; if (!primary(depth)) return false; }
    return true;
  };
  return expression(0) && index === tokens.length;
}
export function bindingId(binding: Obj): string {
  return encodeURIComponent(JSON.stringify([str(binding.command), shortcutInput(obj(binding.shortcut)), whenExpression(binding.whenAst)]));
}
type ShortcutContext = { terminalFocus?: boolean; terminalOpen?: boolean; composerFocus: boolean; editableFocus: boolean; turnRunning: boolean; modelPickerOpen: boolean; draftThreadRoute: boolean; modalOpen: boolean };
const whenMatches = (ast: unknown, context: ShortcutContext, depth = 0): boolean | null => {
  const node = obj(ast); if (!node.type) return true; if (depth > 64) return null;
  if (node.type === 'identifier') {
    const values: Record<string, boolean> = { true: true, false: false, isDesktop: true, isWeb: false, terminalFocus: false, terminalOpen: false, previewFocus: false, previewOpen: false, usagePageOpen: false, ...context };
    return Object.prototype.hasOwnProperty.call(values, str(node.name)) ? values[str(node.name)]! : null;
  }
  const left = whenMatches(node.type === 'not' ? node.node : node.left, context, depth + 1);
  if (node.type === 'not') return left === null ? null : !left;
  const right = whenMatches(node.right, context, depth + 1);
  if (left === null || right === null) return null;
  if (node.type === 'and') return left && right;
  if (node.type === 'or') return left || right;
  return null;
};
export function keyboardSettings(config: Obj, composerFocus: boolean, extra: Partial<ShortcutContext> = {}) {
  const context: ShortcutContext = { composerFocus, editableFocus: composerFocus, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, ...extra };
  const named: Record<string, string> = { ' ': 'Space', escape: 'Escape', enter: 'Enter', tab: 'Tab', arrowup: 'ArrowUp', arrowdown: 'ArrowDown', arrowleft: 'ArrowLeft', arrowright: 'ArrowRight', backspace: 'Backspace', delete: 'Delete', pageup: 'PageUp', pagedown: 'PageDown', home: 'Home', end: 'End' };
  const chord = (rule: Obj) => {
    const shortcut = obj(rule.shortcut), key = str(shortcut.key);
    return [...(shortcut.modKey || shortcut.metaKey ? ['Meta'] : []), ...(shortcut.ctrlKey ? ['Control'] : []), ...(shortcut.altKey ? ['Alt'] : []), ...(shortcut.shiftKey ? ['Shift'] : []), named[key] || key].join('+');
  };
  const winners = new Map<string, string>();
  for (const rule of [...arr(config.keybindings)].reverse()) {
    const match = whenMatches(rule.whenAst, context), shortcut = chord(rule);
    if (match === false || winners.has(shortcut)) continue;
    // An unknown context cannot activate its own binding or expose an older
    // conflicting command whose precedence we cannot determine safely.
    winners.set(shortcut, match === null ? '' : str(rule.command));
  }
  const value = (command: string, fallback: string) => context.modalOpen ? '' : !Array.isArray(config.keybindings) ? fallback : [...winners].filter(([, winner]) => winner === command).map(([shortcut]) => shortcut).join(' ');
  return { keySidebar: value('sidebar.toggle', 'Meta+B'), keyNewThread: value('chat.new', 'Meta+N'), keyDiff: value('diff.toggle', 'Meta+D'), keyModels: value('modelPicker.toggle', 'Meta+Shift+M') };
}
export async function keybindingSettings(client: T3Client, native: Native | null | undefined, environmentId: string, query: string, active: boolean, draftRow = '', draftKey = '', draftWhen = '', draftCommand = '') {
  const empty = { available: false, writable: false, error: '', path: '', count: '', rows: [] as KeybindingRowView[], commands: [] as { value: string; label: string; selected: boolean }[],
    draftConflict: '', draftValid: true, draftCommandLabel: '', draftKeyLabel: '', whenView: whenEditor(draftWhen) };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId)) throw new Error('Choose a connected environment to manage keybindings.');
    const config = await client.readSettings(native, 'server.getConfig');
    const bindings = arr(config.keybindings);
    const listed = buildRows(bindings, ''), all = listed.map(row => ({ ...row, when: row.condition })), rows = query.trim() ? buildRows(bindings, query) : listed;
    const when = whenEditor(draftWhen);
    const draftKeyValue = draftKey.trim(), draftWhenValue = when.error ? '' : printWhen(parseWhen(draftWhen.trim()) || undefined);
    const conflict = draftRow || draftCommand ? conflictText(conflictLabels(all, { id: draftRow || 'new', key: draftKeyValue, when: draftWhenValue })) : '';
    const count = rows.length + (draftCommand !== '' || draftRow === 'new' ? 1 : 0);
    return { ...empty, available: true, writable: client.writable, path: str(config.keybindingsConfigPath), count: `${count} ${count === 1 ? 'binding' : 'bindings'}`, rows,
      commands: commandOptions(bindings).map(option => ({ ...option, selected: option.value === draftCommand })), draftConflict: conflict, draftValid: !when.error,
      draftCommandLabel: draftCommand ? commandLabel(draftCommand) : 'new keybinding', draftKeyLabel: draftKeyValue ? shortcutLabel(draftKeyValue) : '', whenView: when };
  } catch (error) { return { ...empty, error: error instanceof Error ? error.message : 'Could not load keybindings.' }; }
}

// T3 f90b77d809 shared/keybindings DEFAULT_KEYBINDINGS and contracts STATIC_KEYBINDING_COMMANDS (+ composer.cycleHost, 1e2ecbd975).
export const keybindingDefaults: { key: string; command: string; when?: string }[] = [{"key":"mod+b","command":"sidebar.toggle"},{"key":"mod+[","command":"navigation.back","when":"!terminalFocus"},{"key":"mod+]","command":"navigation.forward","when":"!terminalFocus"},{"key":"mod+j","command":"terminal.toggle"},{"key":"mod+alt+b","command":"rightPanel.toggle"},{"key":"mod+d","command":"terminal.split","when":"terminalFocus"},{"key":"mod+shift+d","command":"terminal.splitVertical","when":"terminalFocus"},{"key":"mod+n","command":"terminal.new","when":"terminalFocus"},{"key":"mod+w","command":"terminal.close","when":"terminalFocus"},{"key":"mod+w","command":"rightPanel.close","when":"!terminalFocus"},{"key":"mod+d","command":"diff.toggle","when":"!terminalFocus"},{"key":"mod+shift+j","command":"preview.toggle"},{"key":"mod+r","command":"preview.refresh","when":"previewFocus"},{"key":"mod+l","command":"preview.focusUrl","when":"previewFocus"},{"key":"mod+=","command":"preview.zoomIn","when":"previewFocus"},{"key":"mod++","command":"preview.zoomIn","when":"previewFocus"},{"key":"mod+-","command":"preview.zoomOut","when":"previewFocus"},{"key":"mod+0","command":"preview.resetZoom","when":"previewFocus"},{"key":"mod+k","command":"commandPalette.toggle","when":"!terminalFocus"},{"key":"mod+p","command":"filePicker.toggle","when":"!terminalFocus"},{"key":"mod+shift+f","command":"projectSearch.toggle","when":"!terminalFocus"},{"key":"mod+u","command":"usage.open","when":"!terminalFocus"},{"key":"mod+alt+a","command":"theme.select","when":"!terminalFocus"},{"key":"mod+alt+shift+a","command":"appearance.cycle","when":"!terminalFocus"},{"key":"mod+alt+shift+t","command":"themeEditor.toggle"},{"key":"mod+s","command":"composer.stash","when":"!terminalFocus"},{"key":"mod+shift+enter","command":"thread.steerQueuedMessage","when":"!terminalFocus"},{"key":"alt+arrowup","command":"thread.editQueuedMessage","when":"composerFocus"},{"key":"mod+enter","command":"composer.sendAlternate","when":"composerFocus && turnRunning"},{"key":"mod+enter","command":"composer.sendBackground","when":"composerFocus && draftThreadRoute"},{"key":"mod+alt+enter","command":"composer.sendBackground","when":"composerFocus && draftThreadRoute"},{"key":"mod+alt+enter","command":"composer.sendAndNewThread","when":"composerFocus && !draftThreadRoute"},{"key":"mod+n","command":"chat.new","when":"!terminalFocus"},{"key":"mod+shift+o","command":"chat.new","when":"!terminalFocus"},{"key":"mod+shift+n","command":"chat.newLocal","when":"!terminalFocus"},{"key":"mod+alt+n","command":"chat.newWithoutProject","when":"!terminalFocus"},{"key":"mod+shift+m","command":"modelPicker.toggle","when":"!terminalFocus"},{"key":"mod+shift+h","command":"composer.host","when":"!terminalFocus"},{"key":"mod+shift+e","command":"composer.effort","when":"!terminalFocus"},{"key":"mod+shift+a","command":"composer.mode","when":"!terminalFocus"},{"key":"mod+shift+x","command":"composer.workspace","when":"!terminalFocus"},{"key":"mod+shift+g","command":"composer.branch","when":"!terminalFocus"},{"key":"mod+shift+l","command":"composer.previousWorktree","when":"!terminalFocus"},{"key":"mod+shift+k","command":"pullRequest.copyNumber","when":"!terminalFocus"},{"key":"mod+shift+arrowup","command":"modelPicker.previousProvider","when":"modelPickerOpen"},{"key":"mod+shift+arrowdown","command":"modelPicker.nextProvider","when":"modelPickerOpen"},{"key":"mod+o","command":"editor.openFavorite"},{"key":"mod+shift+[","command":"thread.previous"},{"key":"mod+shift+]","command":"thread.next"},{"key":"mod+shift+c","command":"thread.copyReference","when":"!terminalFocus"},{"key":"mod+shift+s","command":"thread.settle","when":"!terminalFocus"},{"key":"mod+shift+p","command":"thread.pin","when":"!terminalFocus"},{"key":"mod+z","command":"thread.undo","when":"!terminalFocus && !editableFocus"},{"key":"mod+1","command":"thread.jump.1","when":"isDesktop"},{"key":"mod+2","command":"thread.jump.2","when":"isDesktop"},{"key":"mod+3","command":"thread.jump.3","when":"isDesktop"},{"key":"mod+4","command":"thread.jump.4","when":"isDesktop"},{"key":"mod+5","command":"thread.jump.5","when":"isDesktop"},{"key":"mod+6","command":"thread.jump.6","when":"isDesktop"},{"key":"mod+7","command":"thread.jump.7","when":"isDesktop"},{"key":"mod+8","command":"thread.jump.8","when":"isDesktop"},{"key":"mod+9","command":"thread.jump.9","when":"isDesktop"},{"key":"mod+1","command":"modelPicker.jump.1","when":"modelPickerOpen && isDesktop"},{"key":"mod+2","command":"modelPicker.jump.2","when":"modelPickerOpen && isDesktop"},{"key":"mod+3","command":"modelPicker.jump.3","when":"modelPickerOpen && isDesktop"},{"key":"mod+4","command":"modelPicker.jump.4","when":"modelPickerOpen && isDesktop"},{"key":"mod+5","command":"modelPicker.jump.5","when":"modelPickerOpen && isDesktop"},{"key":"mod+6","command":"modelPicker.jump.6","when":"modelPickerOpen && isDesktop"},{"key":"mod+7","command":"modelPicker.jump.7","when":"modelPickerOpen && isDesktop"},{"key":"mod+8","command":"modelPicker.jump.8","when":"modelPickerOpen && isDesktop"},{"key":"mod+9","command":"modelPicker.jump.9","when":"modelPickerOpen && isDesktop"},{"key":"c","command":"usage.cost","when":"usagePageOpen"},{"key":"t","command":"usage.tokens","when":"usagePageOpen"},{"key":"l","command":"usage.limits","when":"usagePageOpen"},{"key":"mod+shift+1","command":"usage.period.day","when":"usagePageOpen"},{"key":"mod+shift+2","command":"usage.period.week","when":"usagePageOpen"},{"key":"mod+shift+3","command":"usage.period.month","when":"usagePageOpen"},{"key":"mod+shift+4","command":"usage.period.quarter","when":"usagePageOpen"}];
export const keybindingCommands = ["sidebar.toggle","navigation.back","navigation.forward","terminal.toggle","terminal.split","terminal.splitVertical","terminal.new","terminal.close","rightPanel.toggle","threadPanel.toggle","rightPanel.toggleMaximized","rightPanel.close","pullRequest.copyNumber","diff.toggle","preview.toggle","preview.refresh","preview.focusUrl","preview.zoomIn","preview.zoomOut","preview.resetZoom","commandPalette.toggle","filePicker.toggle","projectSearch.toggle","usage.open","theme.select","appearance.cycle","themeEditor.toggle","composer.stash","composer.sendAlternate","composer.sendBackground","composer.sendAndNewThread","composer.host","composer.cycleHost","composer.effort","composer.mode","composer.workspace","composer.previousWorktree","composer.branch","chat.new","chat.newLocal","chat.newWithoutProject","editor.openFavorite","usage.cost","usage.tokens","usage.limits","usage.period.day","usage.period.week","usage.period.month","usage.period.quarter","modelPicker.toggle","modelPicker.previousProvider","modelPicker.nextProvider","modelPicker.jump.1","modelPicker.jump.2","modelPicker.jump.3","modelPicker.jump.4","modelPicker.jump.5","modelPicker.jump.6","modelPicker.jump.7","modelPicker.jump.8","modelPicker.jump.9","thread.stop","thread.steerQueuedMessage","thread.editQueuedMessage","thread.previous","thread.next","thread.copyReference","thread.settle","thread.pin","thread.undo","thread.jump.1","thread.jump.2","thread.jump.3","thread.jump.4","thread.jump.5","thread.jump.6","thread.jump.7","thread.jump.8","thread.jump.9"];
