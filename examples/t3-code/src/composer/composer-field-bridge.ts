import { completeFieldRequest, fieldCompletion } from './composer-field-request';
import type { Native } from '../client/protocol';
import { ComposerFieldCommands, type ComposerFieldCommand, type ComposerFieldIdentity, type ComposerFieldResult, type ComposerFieldSnapshot } from './composer-field-commands';
import { composerFieldReplacement } from './composer-field-edit';
import { composerSourceSession } from './composer-rich-state';
import { composerChipCompleted, composerChipStore } from './composer-chip-store';
import { composerMarkdown } from './composer-markdown';
import { composerChips, composerChipsInBlocks } from './composer-source-chips';
import { composerHistoryComplete, composerHistoryReset } from './composer-prompt-history';
import { finishAttachmentEffect, resetAttachmentEffects } from './composer-attachment-receipts';

export type ComposerFieldHost = { generation: number; snapshotOwner: string; draftKey: string; error?: string };
export type ComposerFieldObservation = ComposerFieldSnapshot & { focused: boolean; inputSerial: number };
export type ComposerFieldAcknowledgement = ComposerFieldResult & ComposerFieldIdentity & { seq: number; then: string; send: boolean; focus: string; removedImage: string; inputSerial: number };
type Field = { commands: ComposerFieldCommands; inputSerial: number; focused: boolean; active: ComposerFieldCommand | null; effects: Map<number, ReturnType<typeof fieldCompletion>> };
const fields = new WeakMap<object, Field>();
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value);
function field(host: ComposerFieldHost, owner: string): Field {
  let result = fields.get(host);
  if (!result) { result = { commands: new ComposerFieldCommands(), inputSerial: -1, focused: false, active: null, effects: new Map() }; fields.set(host, result); }
  const current = result.commands.read();
  if (current && (current.generation !== host.generation || current.snapshotOwner !== host.snapshotOwner || current.owner !== owner)) {
    composerHistoryReset(host);
    resetAttachmentEffects(host);
    composerChipStore(host).citations.reset();
    result.commands.observe({ ...current, generation: host.generation, snapshotOwner: host.snapshotOwner, owner, live: false }, false);
    drainRefused(host, result);
    result.active = null;
  }
  return result;
}
export function composerFieldObservation(value: unknown): ComposerFieldObservation | null {
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
export function composerFieldInputSerial(host: ComposerFieldHost): number { return Math.max(0, fields.get(host)?.inputSerial ?? 0); }
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
  entry.commands.observe(value, acceptedInput, entry.inputSerial);
  drainRefused(host, entry);
  composerChipStore(host).presses.observe(value, 'composer');
  return entry;
}

function drainRefused(host: ComposerFieldHost, entry: Field): void {
  for (const { command, result } of entry.commands.drain()) {
    composerHistoryComplete(host, command, false);
    finishAttachmentEffect(host, command, false, entry.commands.read());
    const current = entry.commands.read();
    if (current) composerChipStore(host).citations.complete(command, false, current);
    try { completeFieldRequest(entry.effects.get(command.seq), result); }
    catch { host.error = 'The edit was refused; please try again.'; }
    finally { entry.effects.delete(command.seq); }
    if (entry.active?.seq === command.seq) entry.active = null;
    if (command.generation === host.generation && command.snapshotOwner === host.snapshotOwner && command.owner === entry.commands.read()?.owner)
      host.error = 'The prompt changed before the edit could be applied. Please try again.';
  }
}

/** A completed resource watches the existing wake topic; it never waits on a future input turn. */
export function composerFieldResource(host: ComposerFieldHost, args: readonly unknown[], owner: string, native?: Native | null): ComposerFieldCommand {
  if (native?.available) native.watch('t3.notify');
  const value = composerFieldObservation(args[0]), after = integer(args[1]) && args[1] >= 0 ? args[1] : 0;
  const entry = value ? observe(host, owner, value, false) ?? field(host, owner) : field(host, owner);
  const identity = value ?? { generation: host.generation, snapshotOwner: host.snapshotOwner, owner: '' };
  const next = entry.commands.wait(after, identity);
  const current = entry.commands.read();
  if (next.ready && current?.live && identityMatches(next, current)) entry.active = next;
  return next;
}

/**
 * Whether the field the editing engine does not serve has a command waiting after its parked cursor (args[1]), as its
 * seq (0: none). The engine serves the focused field; with neither focused, it serves the one with an edit to apply.
 */
export function composerFieldWaiting(host: ComposerFieldHost, args: readonly unknown[], owner: string, native?: Native | null): number {
  if (native?.available) native.watch('t3.notify');
  if (args[0] !== true) return 0;
  const entry = fields.get(host), current = entry?.commands.read();
  if (!entry || !current || current.generation !== host.generation || current.snapshotOwner !== host.snapshotOwner || current.owner !== owner) return 0;
  return entry.commands.waiting(integer(args[1]) && args[1] >= 0 ? args[1] : 0);
}

export function composerFieldObserved(host: ComposerFieldHost, args: readonly unknown[], owner: string): ComposerFieldResult {
  const value = composerFieldObservation(args[0]);
  if (!value || typeof args[1] !== 'boolean') return result(field(host, owner), 'field observation invalid');
  const entry = observe(host, owner, value, args[1]);
  return entry ? result(entry) : result(field(host, owner), 'field observation stale');
}

export async function composerFieldAcknowledged(host: ComposerFieldHost, args: readonly unknown[], owner: string, native?: Native | null): Promise<ComposerFieldAcknowledgement> {
  const value = composerFieldObservation(args[0]), seq = integer(args[1]) && args[1] > 0 ? args[1] : 0;
  const entry = field(host, owner), active = entry.active;
  const fallback = { ...result(entry, 'field acknowledgement stale'), generation: host.generation, snapshotOwner: host.snapshotOwner, owner: value?.owner ?? '', seq, then: '', send: false, focus: '', removedImage: '', inputSerial: -1 };
  if (!value || typeof args[2] !== 'boolean' || !active || active.seq !== seq || !identityMatches(active, value)
    || value.generation !== host.generation || value.snapshotOwner !== host.snapshotOwner || value.owner !== owner) return fallback;
  // The independent ACK is a current selection read, never an input event.
  observe(host, owner, value, false);
  // Applying the field effect and publishing its independent input can cross
  // turns. Yield through finite host work without treating an echo as input.
  if (args[2] && native?.available && entry.commands.awaitingInput(seq)) {
    const sameOwner = () => active.generation === host.generation && active.snapshotOwner === host.snapshotOwner && entry.commands.read()?.owner === active.owner;
    for (let wake = 0; wake < 16 && entry.commands.awaitingInput(seq); wake++) {
      try { await native.later({ op: 'wake', topic: 't3.notify' }); }
      catch { break; }
      if (!sameOwner()) break;
    }
    if (!sameOwner()) { entry.commands.cancel(seq, 'field owner changed'); drainRefused(host, entry); return fallback; }
    if (entry.commands.awaitingInput(seq)) {
      entry.commands.cancel(seq, 'field input acknowledgement unavailable'); drainRefused(host, entry);
      host.error = 'The editor did not acknowledge this edit. Please try again.';
      return { ...fallback, reason: 'field input acknowledgement unavailable' };
    }
    if (entry.active?.seq !== seq) return fallback;
  }
  const current = entry.commands.read();
  if (!current) return fallback;
  const inputSerial = entry.inputSerial, generatedInputSerial = entry.commands.generatedInputSerial(seq);
  const expected = active.value.slice(0, active.start) + active.text + active.value.slice(active.end);
  const generatedCurrent = inputSerial === generatedInputSerial && current.value === expected && current.start === active.selectionStart && current.end === active.selectionEnd;
  const completed = entry.commands.acknowledge(seq, value, current, args[2]);
  const completions = entry.commands.drain();
  if (!completed.applied) host.error = 'The prompt changed before the edit could be applied. Please try again.';
  const receipt = entry.effects.get(seq);
  entry.effects.delete(seq);
  const currentNative: Native | undefined = native ? { available: native.available, watch: topic => native.watch(topic), later: async request => {
    const valid = () => active.generation === host.generation && active.snapshotOwner === host.snapshotOwner && active.owner === entry.commands.read()?.owner
      && entry.inputSerial === inputSerial && entry.commands.read()?.value === current.value;
    if (!valid()) throw new Error('The composer changed before cleanup.');
    const response = await native.later(request);
    if (!valid()) throw new Error('The composer changed during cleanup.');
    return response;
  } } : undefined;

  composerHistoryComplete(host, active, completed.applied);
  finishAttachmentEffect(host, active, completed.applied, current);
  const effect = composerChipStore(host).citations.complete(active, completed.applied, current);
  const session = composerSourceSession(host);
  const chips = effect?.kind === 'open' && session.mode !== 'literal'
    ? session.matches(current.owner, current.value) ? session.chips : composerChips(current.value) : [];
  const blocks = effect?.kind === 'open' ? composerMarkdown(current.value, { styling: session.mode === 'rich',
    blockOrigins: session.matches(current.owner, current.value) ? session.blockOrigins : undefined }).blocks : [];
  const send = composerChipCompleted(host, effect, current, composerChipsInBlocks(current.value, blocks, chips));
  let cleanupSucceeded = true;
  if (completions.some(item => item.command.seq === seq)) {
    try { await completeFieldRequest(receipt, completed, currentNative); }
    catch (error) { cleanupSucceeded = false; if (active.generation === host.generation && active.snapshotOwner === host.snapshotOwner && active.owner === entry.commands.read()?.owner) host.error = error instanceof Error ? error.message : 'The edit was applied but its cleanup failed.'; }
  }
  const latest = entry.commands.read();
  const unchanged = generatedCurrent && cleanupSucceeded && entry.inputSerial === inputSerial && latest && identityMatches(active, latest) && active.generation === host.generation && active.snapshotOwner === host.snapshotOwner
    && latest.value === current.value && latest.start === current.start && latest.end === current.end && !latest.composing && latest.live;
  if (entry.active?.seq === seq) entry.active = null;
  return { ...completed, generation: value.generation, snapshotOwner: value.snapshotOwner, owner: value.owner, seq,
    then: completed.applied && unchanged ? active.then : '', send: send && !!unchanged, focus: completed.applied && unchanged ? receipt?.effects.focus ?? '' : '', removedImage: completed.applied && unchanged ? receipt?.effects.removedImage ?? '' : '', inputSerial: completed.applied && unchanged ? inputSerial : -1 };
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
    const inputSerial = entry.inputSerial;
    const session = composerSourceSession(host);
    const replacement = composerFieldReplacement(request, { snapshot: current, draftOwner: host.draftKey, mode: session.mode, chips: session.chips,
      blockOrigins: session.matches(current.owner, current.value) ? session.blockOrigins : undefined });
    if (replacement.kind === 'refused') return reply({ accepted: false, queued: false, applied: false, reason: replacement.reason });
    field(host, currentOwner());
    const latest = entry.commands.read();
    if (entry.inputSerial !== inputSerial || !latest || latest.start !== current.start || latest.end !== current.end || latest.value !== current.value || latest.composing)
      return reply({ accepted: false, queued: false, applied: false, reason: 'field source changed' });
    const admitted = entry.commands.request(replacement.edit, { focus: replacement.focus, then: replacement.then, history: replacement.history, citation: replacement.citation, completion: replacement.completion });
    if (!admitted.accepted) return reply(admitted);
    entry.effects.set(admitted.seq, fieldCompletion(request));
    try {
      // Existing app wake: the completed resource watches t3.notify (T3LatinChords.swift).
      await native.later({ op: 'wake', topic: 't3.notify' });
    } catch (error) {
      entry.commands.cancel(admitted.seq, 'field wake failed'); drainRefused(host, entry);
      throw error;
    }
    return reply(admitted);
  } };
}
