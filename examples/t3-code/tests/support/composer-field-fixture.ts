import type { T3Client } from '../../src/client/client';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldCurrent, composerFieldInputSerial, composerFieldObserved } from '../../src/composer/composer-field-bridge';
import { composerFieldReplacement } from '../../src/composer/composer-field-edit';
import { composerSourceSession } from '../../src/composer/composer-rich-state';

/** Business adapters use accepted field facts instead of a native editor's
 *  private text. Queue/ACK ordering is exercised by composer-field-bridge tests. */
export function observeComposer(client: T3Client, value: string, input = false) {
  const owner = composerFieldOwner(client);
  const snapshot = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner, value, start: value.length, end: value.length,
    composing: false, focused: true, live: true, inputSerial: composerFieldInputSerial(client) + Number(input) };
  composerFieldObserved(client, [snapshot, input], owner);
  composerSourceSession(client).observe(owner, value, input ? 'insertReplacementText' : '');
  return snapshot;
}

export function acceptComposerEdit(client: T3Client, request: unknown): boolean {
  const owner = composerFieldOwner(client), snapshot = composerFieldCurrent(client, owner), session = composerSourceSession(client);
  if (!snapshot) return false;
  const replacement = composerFieldReplacement(request, { snapshot, draftOwner: client.draftKey, mode: session.mode, chips: session.chips });
  if (replacement.kind === 'refused') return false;
  const edit = replacement.edit, value = edit.value.slice(0, edit.start) + edit.text + edit.value.slice(edit.end);
  client.local.drafts[client.draftKey] = value;
  const next = { ...snapshot, value, start: edit.selectionStart, end: edit.selectionEnd, focused: replacement.focus, inputSerial: composerFieldInputSerial(client) + 1 };
  composerFieldObserved(client, [next, true], owner);
  session.observe(owner, value, 'insertReplacementText');
  return true;
}
