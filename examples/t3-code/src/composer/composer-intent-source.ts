import { composerSourceSession } from './composer-rich-state';
import { composerLanguageEdit } from './composer-block-actions';
import type { ComposerMode, ComposerSessionResult } from './composer-source-session';
import type { T3Client } from '../client/client';
import { adoptPastedChips } from './composer-copied-chips';
import { reservedAttachments } from './composer-editor-attach';
import { MAX_ATTACHMENTS } from './composer-controls-attach';

export type ComposerIntent = {
  snapshotOwner: string; owner: string; value: string; serial: number;
  start: number; end: number; composing: boolean; mode: ComposerMode;
} & (
  | { kind: 'key'; key: string; shift: boolean; command: boolean }
  | { kind: 'input' | 'paste'; text: string }
  | { kind: 'language'; language: string }
);
export type ComposerIntentReply = {
  owner: string; value: string; serial: number; kind: string; mode: ComposerMode;
  start: number; end: number; text: string;
  selectionStart: number; selectionEnd: number; preserveSelection: boolean;
};

export function composerIntentAnswer(client: { snapshotOwner: string }, args: readonly unknown[]): ComposerIntentReply {
  const [snapshotOwner, owner, value, serial, start, end, kind, text, key, shift, command, composing, mode] = args;
  const refused: ComposerIntentReply = { owner: '', value: '', serial: -1, kind: 'stale', mode: 'literal', start: 0, end: 0,
    text: '', selectionStart: 0, selectionEnd: 0, preserveSelection: false };
  if (typeof snapshotOwner !== 'string' || typeof owner !== 'string' || typeof value !== 'string'
    || mode !== 'rich' && mode !== 'plain' && mode !== 'literal'
    || typeof serial !== 'number' || typeof start !== 'number' || typeof end !== 'number' || typeof composing !== 'boolean') return refused;
  const common = { snapshotOwner, owner, value, serial, start, end, composing, mode } satisfies Pick<ComposerIntent,
    'snapshotOwner' | 'owner' | 'value' | 'serial' | 'start' | 'end' | 'composing' | 'mode'>;
  switch (kind) {
    case 'key': return typeof key === 'string' && typeof shift === 'boolean' && typeof command === 'boolean'
      ? composerIntentSource(client, { ...common, kind, key, shift, command }) : refused;
    case 'input': case 'paste': return typeof text === 'string' ? composerIntentSource(client, { ...common, kind, text }) : refused;
    case 'language': return typeof text === 'string' ? composerIntentSource(client, { ...common, kind, language: text }) : refused;
    default: return refused;
  }
}

/** Pure source work runs in the app module. Contract rechecks the echoed
 *  owner/value/serial before making one undoable field command. */
export function composerIntentSource(client: { snapshotOwner: string }, intent: ComposerIntent): ComposerIntentReply {
  const reply: ComposerIntentReply = { owner: intent.owner, value: intent.value, serial: intent.serial, kind: 'stale', mode: intent.mode,
    start: 0, end: 0, text: '', selectionStart: 0, selectionEnd: 0, preserveSelection: false };
  const session = composerSourceSession(client);
  if (intent.snapshotOwner !== client.snapshotOwner || intent.composing || !Number.isSafeInteger(intent.serial) || intent.serial < 0
    || !session.matches(intent.owner, intent.value) || intent.mode !== session.mode
    || !Number.isSafeInteger(intent.start) || !Number.isSafeInteger(intent.end)
    || intent.start < 0 || intent.end < intent.start || intent.end > intent.value.length) return reply;
  const range = { start: intent.start, end: intent.end };
  if (intent.kind === 'language') {
    if (intent.mode !== 'rich') return reply;
    const replacement = composerLanguageEdit({ value: intent.value, current: intent.value, header: range, language: intent.language, blockOrigins: session.blockOrigins });
    if (replacement) session.input(intent.owner, intent.value, replacement, replacement.text);
    return replacement ? { ...reply, ...replacement, kind: 'edit', preserveSelection: true } : reply;
  }
  const result: ComposerSessionResult = intent.kind === 'key'
    ? session.key(intent.owner, intent.value, range, { name: intent.key, shift: intent.shift, command: intent.command })
    : session.input(intent.owner, intent.value, range, intent.text, intent.kind === 'paste');
  if (result.kind !== 'edit') return { ...reply, kind: result.kind };
  // A pasted image or file chip from another draft joins this one (composer-copied-chips.ts).
  if (intent.kind === 'paste' && 'local' in client) {
    const app = client as T3Client;
    adoptPastedChips(app, intent.text, MAX_ATTACHMENTS - reservedAttachments(app));
  }
  return { ...reply, kind: 'edit', start: result.edit.start, end: result.edit.end, text: result.edit.text,
    selectionStart: result.edit.selection.start, selectionEnd: result.edit.selection.end };
}
