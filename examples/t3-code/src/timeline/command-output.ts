// T3 Code 9c161462c9 commandOutput.ts and CommandOutputPanel.tsx (MIT, LICENSE-T3).
// Exact keeps streams with expanded rows and consumes their frames in the client's event batch.
import { arr, num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { subscriptionSerial } from '../details/shell-vcs';
import { composerNow } from '../composer/composer-controls';
import { turnItemIsOpen } from './timeline-item-fetch';
import { letGo } from '../client/let-go';
import { appendTerminalOutput, EMPTY_TERMINAL_OUTPUT, terminalOutputSpans, TERMINAL_PALETTES, type TerminalOutputState } from './terminal-output';

export type CommandOutputFrame = { kind: 'replace' | 'append'; text: string; truncated: boolean; running: boolean };
export type CommandOutputView = { output: TerminalOutputState; running: boolean };
export const EMPTY_COMMAND_OUTPUT: CommandOutputView = { output: EMPTY_TERMINAL_OUTPUT, running: true };
export function commandOutputFrame(value: unknown): CommandOutputFrame | null {
  const v = obj(value);
  return (v.kind === 'replace' || v.kind === 'append') && typeof v.text === 'string' && typeof v.truncated === 'boolean' && typeof v.running === 'boolean'
    ? { kind: v.kind, text: v.text, truncated: v.truncated, running: v.running } : null;
}
export function applyCommandOutputFrame(view: CommandOutputView, frame: CommandOutputFrame): CommandOutputView {
  const base = frame.kind === 'replace' ? EMPTY_TERMINAL_OUTPUT : view.output;
  return { output: appendTerminalOutput({ ...base, truncated: base.truncated || frame.truncated }, frame.text), running: frame.running };
}
const PREFIX = 'command-output:';
interface Stream { row: string; thread: string; item: string; subscription: string; floor: number; maxSeen: number;
  value: CommandOutputView | null; error: string; pending: boolean; retryAt: number }
interface State { owner: string; streams: Map<string, Stream> }
const states = new WeakMap<T3Client, State>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.origin, client.threadId]);
export function commandOutputStreaming(client: T3Client): boolean {
  return obj(obj(client.config.environment).capabilities).commandOutputStreaming === true;
}
function stateOf(client: T3Client): State {
  const key = owner(client), prior = states.get(client);
  if (prior?.owner === key) return prior;
  const state = { owner: key, streams: new Map<string, Stream>() }; states.set(client, state); return state;
}
function desired(client: T3Client): Map<string, { row: string; thread: string; item: string }> {
  const wanted = new Map<string, { row: string; thread: string; item: string }>();
  if (!client.ready || !commandOutputStreaming(client)) return wanted;
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item), rowId = JSON.stringify([row.sourceThreadId, row.sourceItemId]);
    if (item.type !== 'command_execution' || (!turnItemIsOpen(client, rowId) && !turnItemIsOpen(client, str(item.id)))) continue;
    const identity = { row: rowId, thread: str(row.sourceThreadId) || client.threadId, item: str(row.sourceItemId) || str(item.id) };
    wanted.set(PREFIX + rowId, identity);
  }
  return wanted;
}
export async function syncCommandOutputs(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available || client.connection !== 'connected') return;
  const previous = states.get(client), state = stateOf(client), wanted = desired(client), now = composerNow(client);
  if (previous && previous !== state) for (const key of previous.streams.keys()) await client.restAccess(native).call({ op: 'unsubscribe', key }).catch(() => undefined);
  if (state.owner !== owner(client)) return;
  for (const [key] of state.streams) if (!wanted.has(key)) {
    state.streams.delete(key);
    await client.restAccess(native).call({ op: 'unsubscribe', key }).catch(() => undefined);
  }
  for (const [key, identity] of wanted) {
    let stream = state.streams.get(key);
    if (!stream) { stream = { ...identity, subscription: '', floor: 0, maxSeen: 0, value: null, error: '', pending: false, retryAt: 0 }; state.streams.set(key, stream); }
    if (stream.subscription || stream.pending || stream.retryAt > now || stream.value?.running === false) continue;
    stream.pending = true; stream.floor = stream.maxSeen;
    try {
      const reply = await client.restAccess(native).call({ op: 'subscribe', key, method: 'orchestration.subscribeCommandOutput', payload: { threadId: stream.thread, itemId: stream.item } });
      if (states.get(client) !== state || state.owner !== owner(client) || state.streams.get(key) !== stream) continue;
      const id = str(reply.id), serial = subscriptionSerial(id);
      stream.maxSeen = Math.max(stream.maxSeen, serial);
      if (serial > stream.floor && (!stream.subscription || serial > subscriptionSerial(stream.subscription))) stream.subscription = id;
    } catch (error) {
      if (letGo(error)) throw error;
      stream.error = "Couldn't load output: Reconnect to try again."; stream.retryAt = now + 1000;
    } finally { stream.pending = false; }
  }
}
export function commandOutputEvent(client: T3Client, entry: Obj): boolean {
  const key = str(entry.key);
  if (!key.startsWith(PREFIX)) return false;
  const state = states.get(client), stream = state?.streams.get(key);
  if (!state || state.owner !== owner(client) || !stream || num(entry.generation, -1) !== client.generation) return true;
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  stream.maxSeen = Math.max(stream.maxSeen, serial);
  if (serial <= stream.floor || stream.subscription && serial < subscriptionSerial(stream.subscription)) return true;
  stream.subscription = id;
  const value = obj(entry.value);
  if (value._retryDue) { stream.subscription = ''; stream.floor = stream.maxSeen; return true; }
  if (value._transportError || value._streamEnded) {
    if (stream.value?.running !== false) stream.error = `Couldn't load output: ${str(obj(value._transportError).message, 'The output stream ended.')}`;
    return true;
  }
  const frame = commandOutputFrame(value);
  if (frame) { stream.value = applyCommandOutputFrame(stream.value ?? EMPTY_COMMAND_OUTPUT, frame); stream.error = ''; }
  return true;
}
export function commandOutputView(client: T3Client, row: string, item: Obj): { value: CommandOutputView | null; error: string } | null {
  if (item.type !== 'command_execution' || !commandOutputStreaming(client)) return null;
  const state = states.get(client);
  if (state?.owner !== owner(client)) return { value: null, error: '' };
  const stream = [...state.streams.values()].find(stream => stream.row === row || stream.item === row);
  return stream ? { value: stream.value, error: stream.error } : { value: null, error: '' };
}

export function commandSpans(text: string) {
  const light = terminalOutputSpans(text, TERMINAL_PALETTES.light), dark = terminalOutputSpans(text, TERMINAL_PALETTES.dark);
  return light.map((span, index) => ({ id: String(index), text: span.text, light: span.style?.color ?? '#71717b', dark: dark[index]?.style?.color ?? '#a1a1a1',
    bgLight: span.style?.backgroundColor ?? '#00000000', bgDark: dark[index]?.style?.backgroundColor ?? '#00000000',
    bold: span.style?.fontWeight === '600', dim: span.style?.opacity !== undefined, italic: span.style?.fontStyle === 'italic',
    underline: span.style?.textDecorationLine?.includes('underline') === true, strike: span.style?.textDecorationLine?.includes('line-through') === true }));
}
