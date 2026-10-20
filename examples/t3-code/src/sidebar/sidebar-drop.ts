// Adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3.
// Sidebar drag and drop (Sidebar.tsx handleThreadDragEnd, Sidebar.logic.ts
// resolveSidebarDropTarget/planSidebarThreadDrop, chat/threadContextDrag.ts;
// MIT, see LICENSE-T3). The window resolves the destination shelf from the
// lifted row's centre against the list's boundaries; here the slot inside
// Pinned or Active is read off the painted rows, and the plan becomes the
// reference's commands: pin with a fresh key, unpin/un-settle/wake into
// Active, settle onto the shelf, and key writes for the arranged order.
// Past the list edge, a drop on the composer adds the threads as context.
// Every environment's rows share the shelves, and each write goes to its
// thread's own environment (handleThreadDragEnd's scopeThreadRef).
import { str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { capabilities, planReorder } from './sidebar-model';
import { partition, renderedRows } from './sidebar-view';
import { parkThread, targetOf } from './sidebar-commands';
import { rowConfig } from './sidebar-menu';
import { fleetThreads } from '../connections/settings-b-fleet';
import { wall, sidebarSession } from './sidebar-state';
import { insertContext } from '../composer/composer-editor';
import { letGo } from '../client/let-go';

/** A card is 78pt plus 2pt above and below; rows sit 1pt apart; drag labels are 24pt. */
export const CARD = 83, LABEL = 24;
export interface DropGeometry { target: string; centerY: number; topY: number; dividerY: number; overComposer: boolean }
export function parseDrop(value: string): DropGeometry {
  const [target = '', centerY = '', topY = '', dividerY = '', over = ''] = value.split('|');
  return { target, centerY: Number(centerY), topY: Number(topY), dividerY: Number(dividerY), overComposer: over === 'true' };
}

/** The dragged row's slot among the section's other rows, from their painted middles while the labels are open. */
export function dropIndex(ids: string[], movedId: string, centerY: number, firstMid: number): string[] {
  const others = ids.filter(id => id !== movedId);
  let index = 0;
  ids.forEach((id, slot) => { if (id !== movedId && firstMid + slot * CARD < centerY) index++; });
  others.splice(Math.min(index, others.length), 0, movedId);
  return others;
}

export interface DropPlan { verb: string; steps: { type: string; threadId: string; orderKey?: string; reason?: string }[]; failure: string[] }
/**
 * planSidebarThreadDrop: what a drop writes, in order, with each step's failure toast. `pinReorder` and `activeReorder`
 * are the lifted thread's environment's; `pinReorderable` and `activeReorderable` name the rows whose environments
 * take order keys (reorderableKeys). Rows outside them are never written: keyless ones sort outside the keyed run, so
 * they leave the arrangement, and keyed ones stay as bounds; a drop that would still write one writes nothing.
 */
export function planDrop(input: { id: string; from: string; target: string; pinnedIds: string[]; activeIds: string[]; pinnedKeys: Map<string, string | null>;
  activeKeys: Map<string, string | null>; geometry: DropGeometry; supportsSettlement: boolean; pinReorder: boolean; activeReorder: boolean;
  pinReorderable?: Set<string>; activeReorderable?: Set<string> }): DropPlan {
  const { id, from, target, geometry } = input;
  const none: DropPlan = { verb: '', steps: [], failure: [] };
  if (target === 'working' || target === 'snoozed' || target === '' || target === 'context') return none;
  if (!input.supportsSettlement && (target === 'settled' || from === 'settled')) return none;
  const arrange = (order: string[], keys: Map<string, string | null>, writable: Set<string> | undefined) => {
    if (!writable) return planReorder(order, keys, id);
    if (!writable.has(id)) return null;
    const assignments = planReorder(order.filter(key => writable.has(key) || keys.get(key) != null), keys, id);
    return assignments.every(assignment => writable.has(assignment.id)) ? assignments : null;
  };
  if (target === 'settled') return from === 'settled' ? none : { verb: 'settle', steps: [{ type: 'thread.settle', threadId: id }], failure: ['Failed to settle thread'] };
  if (target === 'pinned') {
    const order = dropIndex(input.pinnedIds, id, geometry.centerY, geometry.topY + LABEL + CARD / 2);
    if (from === 'pinned' && order.join('\n') === input.pinnedIds.join('\n')) return none;
    const assignments = input.pinReorder ? arrange(order, input.pinnedKeys, input.pinReorderable) : [];
    if (assignments === null) return none;
    const steps: DropPlan['steps'] = [], failure: string[] = [];
    if (from !== 'pinned') {
      const key = assignments.find(assignment => assignment.id === id)?.orderKey;
      steps.push({ type: 'thread.pin', threadId: id, ...(key ? { orderKey: key } : {}) }); failure.push('Failed to pin thread');
    }
    for (const assignment of assignments) {
      if (from !== 'pinned' && assignment.id === id) continue;
      steps.push({ type: 'thread.pin.reorder', threadId: assignment.id, orderKey: assignment.orderKey }); failure.push('Failed to reorder pinned threads');
    }
    return { verb: from === 'pinned' ? '' : 'pin', steps, failure };
  }
  // Active: unpin, un-settle or wake first, then the arranged order's keys.
  const order = dropIndex(input.activeIds, id, geometry.centerY, geometry.dividerY + 2 * LABEL + CARD / 2);
  const moved = from !== 'active';
  if (!moved && order.join('\n') === input.activeIds.join('\n')) return none;
  const steps: DropPlan['steps'] = [], failure: string[] = [];
  if (from === 'pinned') { steps.push({ type: 'thread.unpin', threadId: id }); failure.push('Failed to unpin thread'); }
  if (from === 'settled') { steps.push({ type: 'thread.unsettle', threadId: id, reason: 'user' }); failure.push('Failed to un-settle thread'); }
  if (from === 'snoozed') { steps.push({ type: 'thread.unsnooze', threadId: id, reason: 'user' }); failure.push('Failed to wake thread'); }
  if (input.activeReorder) {
    const assignments = arrange(order, input.activeKeys, input.activeReorderable);
    if (assignments === null) return none;
    for (const assignment of assignments) {
      steps.push({ type: 'thread.active.reorder', threadId: assignment.id, orderKey: assignment.orderKey }); failure.push('Failed to reorder active threads');
    }
  }
  return { verb: from === 'pinned' ? 'unpin' : from === 'settled' ? 'unsettle' : from === 'snoozed' ? 'wake' : '', steps, failure };
}

/** The `drop` op: run the plan, stopping at the first failure (each written key stays a valid placement). */
export async function sidebarDrop(client: T3Client, native: Native, id: string, value: string): Promise<string> {
  const geometry = parseDrop(value), session = sidebarSession(client);
  if (geometry.target === 'context') {
    if (!geometry.overComposer || !targetOf(client, id)) return '';
    // The dragged threads: the multi-selection when the lifted row is part of it.
    const ids = session.selection.includes(id) ? [...session.selection] : [id];
    try { await insertContext(client, native, 'threads', JSON.stringify(ids)); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: error instanceof Error ? error.message : 'Unable to add to chat' }); }
    return '';
  }
  const lifted = targetOf(client, id);
  if (!lifted) return '';
  const parts = partition(client, wall(client));
  const rows = renderedRows(client, parts);
  const from = rows.find(entry => entry.thread.id === id)?.section ?? '';
  if (from === 'working') return '';
  const ids = (section: string) => rows.filter(entry => entry.section === section).map(entry => str(entry.thread.id));
  const keys = (list: Obj[], name: string) => new Map(list.map(entry => [str(entry.id), entry[name] == null ? null : str(entry[name])] as [string, string | null]));
  const capsOf = (thread: Obj) => capabilities(rowConfig(client, thread)), caps = capabilities(lifted.config);
  const reorderable = (test: (thread: Obj) => boolean) => new Set(rows.filter(entry => test(entry.thread)).map(entry => str(entry.thread.id)));
  const plan = planDrop({ id, from, target: geometry.target, pinnedIds: ids('pinned'), activeIds: ids('active'),
    pinnedKeys: keys([...client.shell.threads, ...fleetThreads()].filter(entry => entry.pinnedAt != null), 'pinOrderKey'),
    activeKeys: keys(parts.active, 'activeOrderKey'), geometry, supportsSettlement: caps.settlement, pinReorder: caps.pinReorder, activeReorder: caps.activeReorder,
    pinReorderable: reorderable(thread => capsOf(thread).pinning && capsOf(thread).pinReorder), activeReorderable: reorderable(thread => capsOf(thread).activeReorder) });
  if (plan.verb === 'settle') { await parkThread(client, native, id, 'settle', ''); return ''; }
  for (let index = 0; index < plan.steps.length; index++) {
    const { threadId, ...step } = plan.steps[index]!;
    try {
      const target = targetOf(client, threadId);
      if (!target) throw new Error('That thread is no longer available.');
      await target.send(native, step);
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: plan.failure[index]!, description: error instanceof Error ? error.message : 'An error occurred.' });
      break;
    }
  }
  return '';
}
