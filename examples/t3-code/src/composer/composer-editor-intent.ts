// The send gesture resolved through the server's keybindings, the way T3's
// composerSubmissionIntentForKey (composer-logic.ts) does: ⌘↩ while a turn
// runs is the follow-up alternate, ⌘↩ or ⌥⌘↩ in a new-thread draft starts it
// in the background (composer.sendBackground), and ⌥⌘↩ on an existing thread
// sends and then opens a fresh new-thread composer (composer.sendAndNewThread).
// The gesture rides with the press (`pressGesture`): a pointer press's own modifiers (its MouseEvent), or a send
// chord's, which the focused composer's key action reads from its keydown (app.contract composerKeyEvent).
import { arr, obj, str, type Obj } from '../client/domain';

export type SendIntent = 'foreground' | 'alternate' | 'background';

const id = (name: string): Obj => ({ type: 'identifier', name });
const and = (left: Obj, right: Obj): Obj => ({ type: 'and', left, right });
const enter = (altKey: boolean): Obj => ({ key: 'enter', modKey: true, metaKey: false, ctrlKey: false, altKey, shiftKey: false });
/** packages/shared/src/keybindings.ts DEFAULT_KEYBINDINGS (f90b77d), the composer send rules, for a server that sends none. */
export const DEFAULT_SEND_RULES: Obj[] = [
  { command: 'composer.sendAlternate', shortcut: enter(false), whenAst: and(id('composerFocus'), id('turnRunning')) },
  { command: 'composer.sendBackground', shortcut: enter(false), whenAst: and(id('composerFocus'), id('draftThreadRoute')) },
  { command: 'composer.sendBackground', shortcut: enter(true), whenAst: and(id('composerFocus'), id('draftThreadRoute')) },
  { command: 'composer.sendAndNewThread', shortcut: enter(true), whenAst: and(id('composerFocus'), { type: 'not', node: id('draftThreadRoute') }) },
];

const NAMED: Record<string, string> = { ' ': 'Space', escape: 'Escape', enter: 'Enter', tab: 'Tab', arrowup: 'ArrowUp', arrowdown: 'ArrowDown' };
function whenMatches(ast: unknown, context: Record<string, boolean>, depth = 0): boolean | null {
  const node = obj(ast); if (!node.type) return true; if (depth > 64) return null;
  if (node.type === 'identifier') return Object.prototype.hasOwnProperty.call(context, str(node.name)) ? context[str(node.name)]! : null;
  const left = whenMatches(node.type === 'not' ? node.node : node.left, context, depth + 1);
  if (node.type === 'not') return left === null ? null : !left;
  const right = whenMatches(node.right, context, depth + 1);
  if (left === null || right === null) return null;
  return node.type === 'and' ? left && right : node.type === 'or' ? left || right : null;
}
/** composerRequiresModifier: the Send shortcut setting makes plain Return a newline for this prompt. */
export function composerRequiresModifier(sendShortcut: string, prompt: string): boolean {
  return sendShortcut === 'mod-enter' || sendShortcut === 'mod-enter-multiline' && /[\r\n]/.test(prompt);
}
/** shortcutForContext (keybindings.ts, T3 Code 74541bab17): when Return needs ⌘ to send, a saved ⌘↩ alternate
 *  send takes ⇧ too, so ⌘↩ sends with the default follow-up behaviour and ⌘⇧↩ with the other. */
function shortcutForContext(rule: Obj, context: Record<string, boolean>): Obj {
  const shortcut = obj(rule.shortcut);
  if (context.composerRequiresModifier && rule.command === 'composer.sendAlternate' && shortcut.key === 'enter' && shortcut.modKey
    && !shortcut.metaKey && !shortcut.ctrlKey && !shortcut.altKey && !shortcut.shiftKey) return { ...shortcut, shiftKey: true };
  return shortcut;
}
function chordOf(shortcut: Obj): string {
  const key = str(shortcut.key);
  return [...(shortcut.modKey || shortcut.metaKey ? ['Meta'] : []), ...(shortcut.ctrlKey ? ['Control'] : []),
    ...(shortcut.altKey ? ['Alt'] : []), ...(shortcut.shiftKey ? ['Shift'] : []), NAMED[key] || key].join('+');
}
/** resolveShortcutCommand: the last rule whose chord and `when` match wins; an unknown context blocks its chord. */
export function chordCommand(config: Obj, chord: string, extra: Record<string, boolean>): string {
  const rules = Array.isArray(config.keybindings) ? arr(config.keybindings) : DEFAULT_SEND_RULES;
  const context: Record<string, boolean> = { true: true, false: false, isDesktop: true, isWeb: false, terminalFocus: false, terminalOpen: false,
    previewFocus: false, previewOpen: false, usagePageOpen: false, composerFocus: true, editableFocus: true, turnRunning: false,
    modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, composerRequiresModifier: false, ...extra };
  for (let index = rules.length - 1; index >= 0; index--) {
    const rule = rules[index]!;
    if (chordOf(shortcutForContext(rule, context)) !== chord) continue;
    const match = whenMatches(rule.whenAst, context);
    if (match === false) continue;
    return match === null ? '' : str(rule.command);
  }
  return '';
}
/** Every chord that resolves to `command` in this context, as `aria-keyshortcuts`. */
export function chordsFor(config: Obj, command: string, extra: Record<string, boolean>): string[] {
  const rules = Array.isArray(config.keybindings) ? arr(config.keybindings) : DEFAULT_SEND_RULES;
  const chords = [...new Set(rules.filter(rule => rule.command === command).map(rule => chordOf(shortcutForContext(rule, extra))))];
  return chords.filter(chord => chordCommand(config, chord, extra) === command);
}

/**
 * What pressed Send (app.contract `submit`): a pointer press with its modifiers (`pointer:meta+alt`,
 * press-modifiers.contract heldModifiers), or a send chord typed in the composer (`key:meta+alt`, `key:` for none).
 * A plain press (a click, a key with no chord, an assistive or shortcut press) is a plain send: `{}`.
 */
export function pressGesture(pressed: string): Obj {
  const source = pressed.startsWith('pointer:') ? 'pointer' : pressed.startsWith('key:') ? 'key' : '';
  if (!source) return {};
  const modifiers = pressed.slice(source.length + 1).split('+').filter(name => ['meta', 'control', 'alt', 'shift'].includes(name)).join('+');
  return modifiers || source === 'key' ? { modifiers, source, ageMs: 0 } : {};
}

/** The gesture's chord: its modifiers are named meta, control, alt, shift (press-modifiers.contract). */
export function gestureChord(modifiers: string, key = 'Enter'): string {
  const has = (name: string) => new RegExp(`\\b${name}\\b`).test(modifiers);
  return [...(has('meta') ? ['Meta'] : []), ...(has('control') ? ['Control'] : []), ...(has('alt') ? ['Alt'] : []), ...(has('shift') ? ['Shift'] : []), key].join('+');
}

/**
 * composerSubmissionIntentForKey for the gesture that pressed Send. A key
 * gesture resolves through the keybindings; a pointer press with ⌘/Ctrl alone
 * while a turn runs is the alternate (the send button follows the held key).
 * A stale or missing gesture is a plain send.
 */
export function sendIntent(config: Obj, gesture: Obj, running: boolean, draft: boolean, terminalOpen = false, requiresModifier = false): SendIntent {
  const modifiers = str(gesture.modifiers), age = Number(gesture.ageMs);
  if (!Number.isFinite(age) || age < 0 || age > 2000) return 'foreground';
  if (gesture.source !== 'key') {
    const meta = /\b(meta|control)\b/.test(modifiers), alt = /\balt\b/.test(modifiers), shift = /\bshift\b/.test(modifiers);
    return meta && !alt && !shift && running ? 'alternate' : 'foreground';
  }
  const command = chordCommand(config, gestureChord(modifiers), { draftThreadRoute: draft, turnRunning: running, terminalOpen, composerRequiresModifier: requiresModifier });
  if (command === 'composer.sendAlternate' && running) return 'alternate';
  if (command === 'composer.sendBackground' && draft) return 'background';
  if (command === 'composer.sendAndNewThread' && !draft) return 'background';
  return 'foreground';
}

/**
 * The chords that resolve to a send command here, which composerSubmissionIntentForKey makes no plain send: the
 * alternate while running, start-in-background in a draft, and send-and-new-thread on an existing thread. An open
 * command menu leaves these to Send and takes the other Returns.
 */
export function sendCommandChords(config: Obj, running: boolean, draft: boolean, terminalOpen = false, requiresModifier = false): string {
  const context = { draftThreadRoute: draft, turnRunning: running, terminalOpen, composerRequiresModifier: requiresModifier };
  return [...new Set([...(running ? chordsFor(config, 'composer.sendAlternate', context) : []),
    ...(draft ? chordsFor(config, 'composer.sendBackground', context) : chordsFor(config, 'composer.sendAndNewThread', context))])].join(' ');
}

/**
 * Every send chord: ⌘↩ always (the mod-enter send shortcut) and sendCommandChords. The focused composer takes them
 * with their modifiers; the Send buttons declare them as `aria-keyshortcuts` while it is not focused.
 */
export function sendChords(config: Obj, running: boolean, draft: boolean, terminalOpen = false, requiresModifier = false): string {
  return [...new Set(['Meta+Enter', ...sendCommandChords(config, running, draft, terminalOpen, requiresModifier).split(' ').filter(Boolean)])].join(' ');
}
