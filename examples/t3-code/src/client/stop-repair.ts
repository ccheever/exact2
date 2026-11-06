// Stop settles background rows the server already finished (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// packages/client-runtime/src/state/threadCommands.ts interruptTurn, state/threads.ts reconcilePendingWork).
// A row can stay running here after the server finished it, with no event left to name it (it finished while
// the socket was down, outside the loaded window). After each Stop action (the reference's interruptTurn wraps
// interruptThreadTurn, however many commands that sends, or none), whether or not the server accepted it, its
// thread's active background items are read with `getTurnItem`, four at a time with 10 s each, and those the
// server settled replace their local copies (domain.ts settleTurnItems). Only the open thread's state is held.
// The lookups do not hold the Stop: as the reference forks them off, each is sent with a `deliver` key, so its
// reply joins the inbox and the snapshot's drain takes it (stopRepairEvent), sending the next. The reference also
// repairs the snapshot it cached for the thread; this client keeps no thread cache.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { obj, str, pendingBackgroundTurnItems, settleTurnItems, turnItemReply, workActive, type Obj } from './domain';

/** The Stop actions (client.ts `command`): the composer's Stop, Stop background work, a subagent's Stop. */
export const STOP_ACTIONS = ['stop', 'cc:stop-background', 'shell:stop-subagent'];
const KEY = 'stop-repair:';
const CONCURRENCY = 4;
const TIMEOUT_SECONDS = 10;

type Repair = { id: number; generation: number; threadId: string; queue: string[]; inFlight: Set<string>; settled: Map<string, Obj> };
const repairs = new WeakMap<object, Repair>();
let serial = 0;

const keyOf = (repair: Repair, itemId: string) => `${KEY}${repair.id}:${itemId}`;
const current = (client: T3Client, repair: Repair) => repairs.get(client) === repair && client.generation === repair.generation
  && client.threadId === repair.threadId && !!client.thread;

/** Starts the repair of `threadId` while it is the open thread; a newer Stop's repair replaces one still running. */
export async function repairAfterStop(client: T3Client, native: Native, threadId: string): Promise<void> {
  if (!threadId || threadId !== client.threadId || !client.thread || client.connection !== 'connected') return;
  const pending = pendingBackgroundTurnItems(client.thread.projection);
  if (!pending.length) { repairs.delete(client); return; }
  const repair: Repair = { id: ++serial, generation: client.generation, threadId, queue: pending.map(item => str(item.id)), inFlight: new Set(), settled: new Map() };
  repairs.set(client, repair);
  await pumpStopRepair(client, native);
}

/** Sends the lookups the repair has room for; once every reply is in, settles the rows. */
export async function pumpStopRepair(client: T3Client, native: Native): Promise<void> {
  const repair = repairs.get(client);
  if (!repair) return;
  if (!current(client, repair)) { repairs.delete(client); return; }
  while (repair.inFlight.size < CONCURRENCY && repair.queue.length) {
    const itemId = repair.queue.shift()!;
    repair.inFlight.add(itemId);
    try {
      await client.call(native, { op: 'request', method: 'orchestration.getTurnItem', payload: { threadId: repair.threadId, itemId },
        deliver: keyOf(repair, itemId), timeout: TIMEOUT_SECONDS }, repair.generation);
    } catch (error) {
      repair.inFlight.delete(itemId);
      // A let-go answer sent nothing: the next pump sends the lookup again (let-go.ts).
      if (letGo(error)) { repair.queue.unshift(itemId); throw error; }
      // A lookup that could not be sent counts as unanswered, as the reference's failed one does.
      if (!current(client, repair)) { repairs.delete(client); return; }
    }
  }
  if (repair.inFlight.size || repair.queue.length) return;
  repairs.delete(client);
  const settled = settleTurnItems(client.thread!, repair.settled);
  if (settled !== client.thread) { client.thread = settled; client.revision++; }
}

/** Whether the repair can send another lookup or has every reply in, so the drain pumps it. */
export function stopRepairDue(client: T3Client): boolean {
  const repair = repairs.get(client);
  return !!repair && (repair.inFlight.size === 0 || (repair.queue.length > 0 && repair.inFlight.size < CONCURRENCY));
}

/** One `stop-repair:` inbox entry (client.ts drain): a settled item is kept, anything else (a failure, a timeout) ignored. */
export function stopRepairEvent(client: T3Client, entry: Obj): boolean {
  const key = str(entry.key);
  if (!key.startsWith(KEY)) return false;
  const repair = repairs.get(client);
  const prefix = repair ? `${KEY}${repair.id}:` : '';
  if (!repair || !key.startsWith(prefix)) return true;
  const itemId = key.slice(prefix.length);
  if (!repair.inFlight.delete(itemId)) return true;
  const item = turnItemReply(obj(entry.value)._reply, repair.threadId, itemId);
  if (item && !workActive(item.status)) repair.settled.set(itemId, item);
  return true;
}
