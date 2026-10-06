// Lane r11-upstream: sweep a row's Settle, Un-settle or Wake button across its
// section (upstream 1826fb55cc; T3 Code, MIT, see LICENSE-T3: Sidebar.tsx
// startActionSweep / settleThreads, Sidebar.logic.ts resolveSidebarSweepKeys).
// The section's view (sidebar.contract ThreadSection) arms the rows between the
// pressed button and the pointer once the press has travelled 6 pt; the release
// reaches here as a `drop` whose id is `sweep|verb|section|origin|keys`. A release
// that armed nothing is the button's own click: the pan took the press from it.
import { str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Files, Native } from './protocol';
import { capabilities } from './sidebar-model';
import { partition } from './sidebar-view';
import { wall, sidebarSession } from './sidebar-state';
import { park, sidebarCommand } from './sidebar-commands';

export type SweepAction = 'settle' | 'unsettle' | 'unsnooze';

/** resolveSidebarSweepKeys: eligible rows between the pressed action and the pointer, in sidebar order. */
export function resolveSidebarSweepKeys(orderedKeys: readonly string[], originKey: string, targetKey: string, canApply: (key: string) => boolean): string[] {
  const origin = orderedKeys.indexOf(originKey), target = orderedKeys.indexOf(targetKey);
  if (origin === -1 || target === -1) return [];
  return orderedKeys.slice(Math.min(origin, target), Math.max(origin, target) + 1).filter(canApply);
}

export interface SweepRelease { action: SweepAction; section: string; origin: string; keys: string[] }
export function parseSweep(id: string): SweepRelease | null {
  const [tag, action, section = '', origin = '', ...rest] = id.split('|');
  if (tag !== 'sweep' || (action !== 'settle' && action !== 'unsettle' && action !== 'unsnooze') || !origin) return null;
  return { action, section, origin, keys: rest.filter(Boolean) };
}

/** The release: settle (with co-settling navigation), un-settle or wake each armed row still in the section. */
export async function sweepRelease(client: T3Client, native: Native, storage: Files, id: string): Promise<string> {
  const session = sidebarSession(client);
  session.sweepEpoch++;
  const sweep = parseSweep(id);
  if (!sweep) return '';
  // r12-sidebar: Escape cancelled the sweep (sidebar.contract ThreadSection): the release applies nothing, not even the click.
  if (sweep.keys.includes('cancel')) return '';
  // A pan that never armed a row took the press from the button: apply it as its click would.
  if (!sweep.keys.length) {
    if (sweep.action === 'settle') await park(client, native, sweep.origin, 'settle', '', new Set());
    else await sidebarCommand(client, native, storage, sweep.action, sweep.origin, '');
    return '';
  }
  // Rows that changed section mid-gesture (pinned from another device, say) are skipped.
  const caps = capabilities(client.config);
  const sections = new Map(Object.entries(partition(client, wall(client))).flatMap(([section, threads]) => (threads as Obj[]).map(thread => [str(thread.id), section] as [string, string])));
  const supported = sweep.action === 'unsnooze' ? caps.snooze : caps.settlement;
  const keys = sweep.keys.filter(key => supported && sections.get(key) === sweep.section);
  if (sweep.action === 'settle') {
    // settleThreads: navigation skips the rows settling in this batch; already settled rows have nothing to do.
    const batch = new Set(keys);
    for (const key of keys) {
      const thread = client.shell.threads.find(entry => entry.id === key);
      if (!thread || thread.settledOverride === 'settled') continue;
      await park(client, native, key, 'settle', '', batch);
    }
    return '';
  }
  for (const key of keys) await sidebarCommand(client, native, storage, sweep.action, key, '');
  return '';
}
