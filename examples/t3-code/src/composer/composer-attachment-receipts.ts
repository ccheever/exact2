import type { T3Client } from '../client/client';
import type { ComposerFieldCommand, ComposerFieldSnapshot } from './composer-field-commands';
import { settleFold } from './composer-editor-files';
import { commitAnnotationSend } from '../browser/browser-capture';

type Effect = { kind: 'fold'; file: string } | { kind: 'annotation-send'; draftKey: string };
type Pending = { client: T3Client; snapshot: ComposerFieldSnapshot; expected: string; effect: Effect };
type State = { seq: number; pending: Map<number, Pending> };
const stores = new WeakMap<object, State>();
const same = (a: ComposerFieldSnapshot, b: Pick<ComposerFieldSnapshot, 'generation' | 'snapshotOwner' | 'owner'>) => a.generation === b.generation && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;

/** Attachment effects complete in the independent field ACK, even when that
 *  edit's input canceled the initiating resource or command continuation. */
export function prepareAttachmentEffect(client: T3Client, snapshot: ComposerFieldSnapshot, expected: string, effect: Effect): number {
  let state = stores.get(client);
  if (!state) { state = { seq: 0, pending: new Map() }; stores.set(client, state); }
  if (!snapshot.live || snapshot.composing || expected === snapshot.value || state.pending.size >= 16 || !Number.isSafeInteger(state.seq + 1)) return 0;
  const id = ++state.seq;
  state.pending.set(id, { client, snapshot: { ...snapshot }, expected, effect });
  return id;
}

export function finishAttachmentEffect(owner: object, command: Pick<ComposerFieldCommand, 'completion' | 'generation' | 'snapshotOwner' | 'owner' | 'value' | 'start' | 'end' | 'text'>,
  applied: boolean, current: ComposerFieldSnapshot | null): void {
  const state = stores.get(owner), pending = state?.pending.get(command.completion);
  if (!state || !pending) return;
  state.pending.delete(command.completion);
  const exact = same(pending.snapshot, command) && command.value === pending.snapshot.value
    && command.value.slice(0, command.start) + command.text + command.value.slice(command.end) === pending.expected;
  const live = current?.live && !current.composing && same(pending.snapshot, current);
  if (pending.effect.kind === 'fold') settleFold(pending.client, pending.effect.file, exact && applied, live === true);
  else if (exact && applied && live && current.value === pending.expected && pending.client.draftKey === pending.effect.draftKey) commitAnnotationSend(pending.client);
}

export function resetAttachmentEffects(owner: object): void {
  const state = stores.get(owner);
  if (!state) return;
  for (const pending of state.pending.values()) if (pending.effect.kind === 'fold') settleFold(pending.client, pending.effect.file, false, false);
  state.pending.clear();
}
