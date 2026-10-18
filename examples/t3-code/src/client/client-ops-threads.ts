// The threads' client.command() ops (client-ops.ts): what the main view
// shows (a project's draft, a home hero's carried prompt, a thread here or in
// another environment, older history), Copy message, Fork and Un-settle.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { sidebarSelecting } from '../sidebar/sidebar-commands';
import { fleetThreadId, parseFleetThreadId, focusFleetThread, focusFleetProject } from '../connections/settings-b-fleet';
import { dismissThreadError } from '../composer/requests';
import { clearRowDrops } from '../sidebar/legacy-sidebar-drop';
import { composerTakesFocus } from '../sidebar/legacy-sidebar-links';
import { heroCarry, heroLand, ensureScratchProject } from '../pages/pages-home';
import { homeTarget } from '../pages/home-landing';
import { obj, str, messages } from './domain';
import { ClientError, type Native, type Files } from './protocol';
import { dismissPromptLink } from '../prompt-links/prompt-link-handler';

/** Selection: a project, a new thread or a hero's carry, a thread, older history; Copy message; dismissing the thread error. */
export async function threadOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  if (op === 'select-project' || op === 'select-thread' || op === 'new-thread' || op.startsWith('pages:hero-')) dismissPromptLink(this);
  let resultMessage = '';
  try {
    if (op === 'copy-message') {
      const message = messages(this.thread).find(message => message.id === id && ['user', 'assistant', 'plan'].includes(message.kind));
      if (!message) throw new ClientError('That message is no longer available.');
      await this.call(native, { op: 'copyText', text: message.body });
      resultMessage = 'Copied message';
    } else if (op === 'dismiss-error') { dismissThreadError(this); this.error = ''; }
    else if (op === 'select-project' || op === 'new-thread' || op.startsWith('pages:hero-')) {
      this.homePending = false; this.homeMove = null; // a choice made before the launch's landing picked keeps it
      // Try again lands as the index route does: the most recent project of every environment (home-landing.ts).
      const home = op === 'pages:hero-retry' ? homeTarget(this) : null;
      if (home && home.environmentId !== this.environmentId) {
        const focused = await focusFleetProject(this, native, home.environmentId, home.projectId);
        this.adoptStatus(focused.value, focused.generation);
        return true;
      }
      // A project named with its environment (a clone toast's Open project, project-clones.ts): the focused one's draft
      // opens here, another environment's opens there (settings-b-fleet.ts focusFleetProject).
      const named = op === 'new-thread' ? parseFleetThreadId(id) : null;
      if (named && named.environmentId !== this.environmentId) {
        const focused = await focusFleetProject(this, native, named.environmentId, named.threadId);
        this.adoptStatus(focused.value, focused.generation);
        return true;
      }
      if (named) id = named.threadId;
      const carry = op.startsWith('pages:hero-') ? await heroCarry(this, op.slice(11), id, () => ensureScratchProject(this, native!)) : null;
      if (carry) id = carry.projectId;
      if (id && this.shell.projects.some(project => project.id === id)) this.projectId = id;
      this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
      delete this.subscriptions.thread;
      this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
      if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
      if (carry) heroLand(this, carry);
    } else if (op === 'select-thread') {
      this.homePending = false; this.homeMove = null;
      // A notification names its thread with its environment (shell-notify.ts): the focused one's opens here.
      const pressed = id, before = `${this.environmentId}\n${this.threadId}`;
      const qualified = parseFleetThreadId(id);
      const dropKey = qualified ? pressed : fleetThreadId(this.environmentId, id);
      if (qualified && qualified.environmentId === this.environmentId) id = qualified.threadId;
      try {
        if (await sidebarSelecting(this, native, id, value)) return true;
        if (parseFleetThreadId(id)) { const focused = await focusFleetThread(this, native, id); this.adoptStatus(focused.value, focused.generation); } // settings-b: another environment
        else await this.openSelected(native, id);
      } catch (error) { await clearRowDrops(this, native, [dropKey]); throw error; } // handleThreadFileDrop: a drop whose thread did not open is let go
      // ChatView focuses the composer on every thread change (activeThread.id, a frame later), unless the terminal drawer is
      // open; a thread the shell does not hold shows the empty state, with no thread and no composer.
      const opened = this.threadId && `${this.environmentId}\n${this.threadId}` !== before && this.shell.threads.some(thread => thread.id === this.threadId);
      const target = parseFleetThreadId(id) ?? (opened ? { environmentId: this.environmentId, threadId: this.threadId } : null);
      if (target && composerTakesFocus(this, target)) resultMessage = 'focus:composer';
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
