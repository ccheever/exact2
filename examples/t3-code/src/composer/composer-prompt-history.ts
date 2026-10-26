// composerPromptHistory.ts at T3 Code 74541bab17 (MIT; LICENSE-T3).
import type { T3Client } from '../client/client';
import { messages, obj, visibleTurnItems, type ThreadState } from '../client/domain';
import { historyEntries } from './composer-editor-menu';
import { userMessagePresentation } from '../timeline/user-message';

type Entry = { id: string; prompt: string };
type Position = { entryId: string; recalled: string };
type Step = { position: Position | null; prompt: string };
type Scope = { generation: number; snapshotOwner: string };
type Receipt = Scope & { owner: string; history: number };
const positions = new WeakMap<object, Scope & { owner: string; position: Position | null; pending: Map<number, Position | null> }>();
const sequences = new WeakMap<object, number>();

/** A history position identifies a sent prompt; the shared field owns edits and undo. */
export function promptHistoryStep(entries: readonly Entry[], position: Position | null, current: string, backward: boolean): Step | null {
  let active = -1;
  if (position?.recalled === current) {
    active = entries.findIndex(entry => entry.id === position.entryId);
    if (active < 0) active = entries.findLastIndex(entry => entry.prompt === position.recalled);
  }
  if (backward) {
    if (active < 0 && current !== '') return null;
    const entry = entries[active < 0 ? entries.length - 1 : active - 1];
    return entry ? { position: { entryId: entry.id, recalled: entry.prompt }, prompt: entry.prompt } : null;
  }
  if (active < 0) return null;
  const entry = entries[active + 1];
  return entry ? { position: { entryId: entry.id, recalled: entry.prompt }, prompt: entry.prompt } : { position: null, prompt: '' };
}

/** The thread's messages, each user message with who sent it when the user did not type it (an automation or an agent). */
export function attributedMessages(thread: ThreadState) {
  const rows = visibleTurnItems(thread);
  return messages(thread).map((message, index) => ({ ...message, attribution: userMessagePresentation(obj(rows[index]?.item)).attribution }));
}

function history(client: T3Client, owner: string, value: string) {
  let state = positions.get(client);
  if (!state || state.owner !== owner || state.generation !== client.generation || state.snapshotOwner !== client.snapshotOwner) {
    state = { owner, generation: client.generation, snapshotOwner: client.snapshotOwner, position: null, pending: new Map() };
    positions.set(client, state);
  }
  const entries = client.thread ? historyEntries(attributedMessages(client.thread)) : [];
  return { state, previous: promptHistoryStep(entries, state.position, value, true), next: promptHistoryStep(entries, state.position, value, false) };
}
export function composerHistoryAvailability(client: T3Client, owner: string, value: string): { historyPrevious: boolean; historyNext: boolean } {
  const steps = history(client, owner, value);
  return { historyPrevious: steps.previous !== null, historyNext: steps.next !== null };
}
/** A receipt travels through the existing field command; its independent ACK
 *  commits the position after accepted input, even if input replaced the caller. */
export function composerHistoryRecall(client: T3Client, owner: string, value: string, backward: boolean): { receipt: Receipt; prompt: string } | null {
  const steps = history(client, owner, value), step = backward ? steps.previous : steps.next;
  if (!step || steps.state.pending.size >= 16) return null;
  const receipt = (sequences.get(client) ?? 0) + 1;
  if (!Number.isSafeInteger(receipt)) return null;
  sequences.set(client, receipt);
  steps.state.pending.set(receipt, step.position);
  return { receipt: { generation: client.generation, snapshotOwner: client.snapshotOwner, owner, history: receipt }, prompt: step.prompt };
}
export function composerHistoryReset(client: object): void { positions.delete(client); }
export function composerHistoryComplete(client: Scope, receipt: Receipt, applied: boolean): void {
  const state = positions.get(client);
  if (!state || state.owner !== receipt.owner || state.generation !== client.generation || state.snapshotOwner !== client.snapshotOwner
    || receipt.generation !== state.generation || receipt.snapshotOwner !== state.snapshotOwner || !state.pending.has(receipt.history)) return;
  if (applied) state.position = state.pending.get(receipt.history) ?? null;
  state.pending.delete(receipt.history);
}
