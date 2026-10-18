import type { Native } from '../client/protocol';
import { ComposerFieldCommands, type ComposerFieldCommand, type ComposerFieldIdentity, type ComposerFieldResult, type ComposerFieldSnapshot } from './composer-field-commands';
import { composerFieldReplacement } from './composer-field-edit';
import { composerSourceSession } from './composer-rich-state';
import { composerChipCompleted, composerChipStore } from './composer-chip-store';
import { composerMarkdown } from './composer-markdown';
import { chipDecorations, composerChips } from './composer-source-chips';
import { composerHistoryComplete, composerHistoryReset } from './composer-prompt-history';

export type ComposerFieldHost = { generation: number; snapshotOwner: string; draftKey: string };
export type ComposerFieldObservation = ComposerFieldSnapshot & { focused: boolean; inputSerial: number };
export type ComposerFieldAcknowledgement = ComposerFieldResult & ComposerFieldIdentity & { seq: number; then: string; send: boolean };
type Field = { commands: ComposerFieldCommands; inputSerial: number; focused: boolean; active: ComposerFieldCommand | null };
const fields = new WeakMap<object, Field>();
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value);
function field(host: ComposerFieldHost, owner: string): Field {
  let result = fields.get(host);
  if (!result) { result = { commands: new ComposerFieldCommands(), inputSerial: -1, focused: false, active: null }; fields.set(host, result); }
  const current = result.commands.read();
  if (current && (current.generation !== host.generation || current.snapshotOwner !== host.snapshotOwner || current.owner !== owner)) {
    composerHistoryReset(host);
    composerChipStore(host).citations.reset();
    result.commands.observe({ ...current, generation: host.generation, snapshotOwner: host.snapshotOwner, owner, live: false }, false);
    result.active = null;
  }
  return result;
}
function observation(value: unknown): ComposerFieldObservation | null {
  if (!record(value) || !integer(value.generation) || typeof value.snapshotOwner !== 'string' || typeof value.owner !== 'string'
    || typeof value.value !== 'string' || !integer(value.start) || !integer(value.end) || value.start < 0 || value.end < value.start || value.end > value.value.length
    || typeof value.composing !== 'boolean' || typeof value.live !== 'boolean' || typeof value.focused !== 'boolean' || !integer(value.inputSerial) || value.inputSerial < 0) return null;
  return { generation: value.generation, snapshotOwner: value.snapshotOwner, owner: value.owner, value: value.value,
    start: value.start, end: value.end, composing: value.composing, live: value.live, focused: value.focused, inputSerial: value.inputSerial };
}
const identityMatches = (a: ComposerFieldIdentity, b: ComposerFieldIdentity) => a.generation === b.generation && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;
/** Read the same accepted source/selection used by field edits. */
export function composerFieldCurrent(host: ComposerFieldHost, owner: string): ComposerFieldSnapshot | null {
  const current = fields.get(host)?.commands.read();
  return current?.live && current.generation === host.generation && current.snapshotOwner === host.snapshotOwner && current.owner === owner ? current : null;
}
const result = (entry: Field, reason = ''): ComposerFieldResult => {
  const current = entry.commands.read();
  return { applied: false, reason, value: current?.value ?? '', start: current?.start ?? 0, end: current?.end ?? 0 };
};
function observe(host: ComposerFieldHost, owner: string, value: ComposerFieldObservation, input: boolean): Field | null {
  const entry = field(host, owner), previous = entry.commands.read();
  if (value.generation !== host.generation || value.snapshotOwner !== host.snapshotOwner || value.owner !== owner) return null;
  const sameOwner = previous !== null && identityMatches(previous, value);
  if (sameOwner && (input ? value.inputSerial <= entry.inputSerial : value.inputSerial < entry.inputSerial)) return entry;
  const acceptedInput = input && sameOwner && value.inputSerial > entry.inputSerial;
  if (!sameOwner) { entry.inputSerial = input ? value.inputSerial : -1; entry.active = null; }
  else if (input) entry.inputSerial = Math.max(entry.inputSerial, value.inputSerial);
  entry.focused = value.focused;
  entry.commands.observe(value, acceptedInput);
  composerChipStore(host).presses.observe(value, 'composer');
  return entry;
}

/** Root resources publish current owner/selection facts, then wait for one
 *  request. Only the separate typed input turn may confirm its source edit. */
export function composerFieldResource(host: ComposerFieldHost, args: readonly unknown[], owner: string): ComposerFieldCommand | Promise<ComposerFieldCommand> {
  const value = observation(args[0]), after = integer(args[1]) && args[1] >= 0 ? args[1] : 0;
  const entry = value ? observe(host, owner, value, false) ?? field(host, owner) : field(host, owner);
  const identity = value ?? { generation: host.generation, snapshotOwner: host.snapshotOwner, owner: '' };
  const command = entry.commands.wait(after, identity);
  const delivered = (next: ComposerFieldCommand) => {
    const current = entry.commands.read();
    if (next.ready && current?.live && identityMatches(next, current)) entry.active = next;
    return next;
  };
  return command instanceof Promise ? command.then(delivered) : delivered(command);
}

export function composerFieldObserved(host: ComposerFieldHost, args: readonly unknown[], owner: string): ComposerFieldResult {
  const value = observation(args[0]);
  if (!value || typeof args[1] !== 'boolean') return result(field(host, owner), 'field observation invalid');
  const entry = observe(host, owner, value, args[1]);
  return entry ? result(entry) : result(field(host, owner), 'field observation stale');
}

export function composerFieldAcknowledged(host: ComposerFieldHost, args: readonly unknown[], owner: string): ComposerFieldAcknowledgement {
  const value = observation(args[0]), seq = integer(args[1]) && args[1] > 0 ? args[1] : 0;
  const entry = field(host, owner), active = entry.active;
  const fallback = { ...result(entry, 'field acknowledgement stale'), generation: host.generation, snapshotOwner: host.snapshotOwner, owner: value?.owner ?? '', seq, then: '', send: false };
  if (!value || typeof args[2] !== 'boolean' || !active || active.seq !== seq || !identityMatches(active, value)
    || value.generation !== host.generation || value.snapshotOwner !== host.snapshotOwner || value.owner !== owner) return fallback;
  // The independent ACK is a current selection read, never an input event.
  observe(host, owner, value, false);
  const current = entry.commands.read();
  if (!current) return fallback;
  const completed = entry.commands.acknowledge(seq, value, current, args[2]);
  composerHistoryComplete(host, active, completed.applied);
  const effect = composerChipStore(host).citations.complete(active, completed.applied, current);
  const session = composerSourceSession(host);
  const chips = effect?.kind === 'open' && session.mode !== 'literal'
    ? session.matches(current.owner, current.value) ? session.chips : composerChips(current.value) : [];
  const atoms = effect?.kind === 'open' ? composerMarkdown(current.value, { styling: session.mode === 'rich', atoms: chipDecorations(chips) }).decorations.atoms : [];
  const send = composerChipCompleted(host, effect, current, chips.filter(chip => atoms.some(atom => atom.start === chip.start && atom.end === chip.end)));
  if (entry.active?.seq === seq) entry.active = null;
  return { ...completed, generation: value.generation, snapshotOwner: value.snapshotOwner, owner: value.owner, seq,
    then: completed.applied ? active.then : '', send };
}

/** Retained OS/paste calls pass through. These three app operations route
 *  through the one shared field owner instead of editing NSTextStorage. */
export function composerFieldNative(host: ComposerFieldHost, native: Native, currentOwner: () => string): Native {
  const owner = currentOwner();
  const scoped = { generation: host.generation, snapshotOwner: host.snapshotOwner, draftKey: host.draftKey, owner: field(host, owner).commands.read()?.owner ?? '' };
  const reply = (value: unknown) => ({ ok: true, generation: scoped.generation, value });
  return { available: native.available, watch: topic => native.watch(topic), later: async request => {
    if (!record(request) || !['editorEdit', 'editorInsert', 'editorState'].includes(String(request.op))) return native.later(request);
    const entry = field(host, currentOwner()), current = entry.commands.read();
    if (!current || !current.live || scoped.generation !== host.generation || scoped.snapshotOwner !== host.snapshotOwner
      || scoped.draftKey !== host.draftKey || scoped.owner !== current.owner) return reply({ applied: false, reason: 'no composer' });
    if (request.op === 'editorState') return reply({ text: current.value, caret: current.start, selectionLength: current.end - current.start, focused: entry.focused, composing: current.composing });
    const session = composerSourceSession(host);
    const replacement = composerFieldReplacement(request, { snapshot: current, draftOwner: host.draftKey, mode: session.mode, chips: session.chips });
    if (replacement.kind === 'refused') return reply({ applied: false, reason: replacement.reason });
    if (replacement.fold !== null) await native.later({ op: 'editorForgetFold', id: replacement.fold });
    field(host, currentOwner());
    const completed = await entry.commands.request(replacement.edit, { focus: replacement.focus, then: replacement.then, history: replacement.history, citation: replacement.citation });
    return reply({ applied: completed.applied, reason: completed.reason, caret: completed.start });
  } };
}
