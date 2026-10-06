// The thread details panel's Automations section (MIT reference, see LICENSE-T3; T3 Code
// 1e2ecbd975: components/chat/ThreadAutomationsPanel.tsx, mounted by ThreadDetailsPanel.tsx
// between Version Control and the relationships section). Fed by the thread's environment's
// live scheduled-task list (live-streams.ts), so run status and next-run times move as the
// scheduler works. The section renders only for a thread with bound tasks or a load error:
// a load error must not look like "no automations". `now` is the window's wall time.
import type { T3Client } from './client';
import { str, type Obj } from './domain';
import type { Native } from './protocol';
import { automationLine, runStatus } from './scheduled-tasks';
import { liveEnvironment, watchLive } from './live-streams';
import { busyAutomation } from './scheduled-tasks-commands';

export type AutomationRow = { id: string; title: string; line: string; status: string; enabled: boolean; runDisabled: boolean; switchDisabled: boolean;
  editLabel: string; runLabel: string; switchLabel: string };
export type AutomationsView = { show: boolean; environmentId: string; error: string; rows: AutomationRow[] };
export const NO_AUTOMATIONS: AutomationsView = { show: false, environmentId: '', error: '', rows: [] };

/** The section for one thread from its environment's list (null until the first value). */
export function automationsView(tasks: Obj[] | null, error: string, threadId: string, environmentId: string, busyId: string, now: number, writable = true): AutomationsView {
  if (!threadId) return NO_AUTOMATIONS;
  const bound = (tasks ?? []).filter(task => str(task.threadId) === threadId);
  if (!error && bound.length === 0) return NO_AUTOMATIONS;
  const busy = busyId !== '' || !writable;
  return { show: true, environmentId, error: error ? `Could not load automations: ${error}` : '', rows: bound.map(task => {
    const title = str(task.title), enabled = task.enabled === true, status = runStatus(task);
    return { id: str(task.id), title, line: automationLine(task, now), status, enabled, runDisabled: busy || status === 'running', switchDisabled: busy,
      editLabel: `Edit ${title}`, runLabel: `Run ${title} now`, switchLabel: enabled ? `Pause ${title}` : `Resume ${title}` };
  }) };
}

/** shell-details.ts: the focused thread's section (a draft has none). */
export async function threadAutomations(client: T3Client, native: Native | null | undefined, threadId: string, now: number): Promise<AutomationsView> {
  if (!threadId || !client.shell.threads.some(thread => thread.id === threadId)) return NO_AUTOMATIONS;
  await watchLive(client, native);
  const environment = liveEnvironment(client, native, client.environmentId);
  if (!environment) return NO_AUTOMATIONS;
  return automationsView(environment.tasks.value, environment.tasks.error, threadId, environment.environmentId, busyAutomation(client), now, client.writable);
}
