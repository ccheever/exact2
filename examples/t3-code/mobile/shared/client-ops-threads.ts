// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/client-ops-threads.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The threads' client.command() ops (client-ops.ts): what the main view
// shows (a project's draft, a home hero's carried prompt, a thread here or in
// another environment, older history), Copy message, Fork and Un-settle.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { sidebarSelecting } from './sidebar-commands';
import { parseFleetThreadId, focusFleetThread } from './settings-b-fleet';
import { dismissThreadError } from './requests';
import { heroCarry, heroLand, ensureScratchProject } from './pages-home';
import { obj, str, messages } from './domain';
import { ClientError, type Native, type Files } from './protocol';

/** Selection: a project, a new thread or a hero's carry, a thread, older history; Copy message; dismissing the thread error. */
export async function threadOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'copy-message') {
      const message = messages(this.thread).find(message => message.id === id && ['user', 'assistant', 'plan'].includes(message.kind));
      if (!message) throw new ClientError('That message is no longer available.');
      await this.call(native, { op: 'copyText', text: message.body });
      resultMessage = 'Copied message';
    } else if (op === 'dismiss-error') { dismissThreadError(this); this.error = ''; }
    else if (op === 'select-project' || op === 'new-thread' || op.startsWith('pages:hero-')) {
      const carry = op.startsWith('pages:hero-') ? await heroCarry(this, op.slice(11), id, () => ensureScratchProject(this, native!)) : null;
      if (carry) id = carry.projectId;
      if (id && this.shell.projects.some(project => project.id === id)) this.projectId = id;
      this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
      delete this.subscriptions.thread;
      this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
      if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
      if (carry) heroLand(this, carry);
    } else if (op === 'select-thread') {
      if (parseFleetThreadId(id)) { const focused = await focusFleetThread(this, native, id); this.adoptStatus(focused.value, focused.generation); } // settings-b: another environment
      else if (!(await sidebarSelecting(this, native, id, value))) await this.openSelected(native, id);
    } else if (op === 'history') await this.history(native);
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** Fork a completed response; Un-settle the open thread. */
export async function threadWrites(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'fork-message') {
      const source = messages(this.thread).find(message => message.id === id && message.kind === 'assistant');
      if (!source?.runId || !source.completed) {
        throw new ClientError('Only a completed response can be forked.');
      }
      const [commandId, targetThreadId] = await this.ids(native, 2);
      await this.dispatch(native, storage, { type: 'thread.fork', commandId, createdBy: 'user', creationSource: 'web',
        sourceThreadId: source.sourceThreadId || this.threadId, targetThreadId,
        sourcePoint: { type: 'run', runId: source.runId }, title: `${str(obj(this.projection.thread).title, 'Thread')} fork` }, 'Fork response');
      this.threadId = targetThreadId;
      await this.openThread(native, targetThreadId);
    } else if (op === 'unsettle') {
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'thread.unsettle', commandId, threadId: this.threadId, reason: 'user' }, 'Un-settle thread');
    }
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
