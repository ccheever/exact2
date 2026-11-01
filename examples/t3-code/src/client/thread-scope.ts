// A second thread shown beside the window's (T3 Code 2359643cff, MIT, see LICENSE-T3: ChatView takes its thread as a
// prop, so SideChatPane embeds a whole ChatView for the side chat). The clone's presentation and commands read the
// window's one thread from the client (`client.threadId`, `client.projection`, the composer's selection); a scope is the
// client seen through another thread: those fields are the scope's own, everything else is the client's. The thread's
// detail is its kept stream (keep-alive.ts), which the pane pins while it shows (lineage-parent.ts pinThreads).
//
// A scope is a Proxy over the client, so getters and methods run with `this` the scope: `draftKey`, `answers`,
// `projection` and `ready` follow its thread, and `command` dispatches to it. Writes to the client's other fields reach
// the client (`revision`, `local`). Module stores keyed by the client object (WeakMaps) see the scope as another key:
// state a thread's composer keeps there (its overlay, its feedback upload) is the scope's own, and the stores that are
// the window's (toasts, upload slots, layout frames) look up `windowClient(client)`. The module's anchors and the
// layout boxes the scope reads are the ones its ids report, prefixed "side-chat-" (side-chat.contract).
import type { T3Client } from './client';
import { keptThread } from './keep-alive';
import { obj, str, type Obj, type ThreadState } from './domain';
import { applyStaged, reconcileStagedModels } from '../composer/composer-controls';
import { SCOPE_IDS, SCOPE_ROOT, windowClient } from './scope-root';

/** The fields a scope keeps for itself; the rest are the client's. */
const OWN = new Set(['threadId', 'absentThreadId', 'projectId', 'providerId', 'modelId', 'modelOptions', 'runtimeMode', 'interactionMode',
  'error', 'busy', 'historyLoading', 'threadEpoch', 'diffOpen', 'diffLoading', 'diffError', 'homePending', 'homeMove']);

type Own = Record<string, unknown> & { selectionFrom: Obj | null };

const presentations = new WeakMap<Obj, Obj>();
/** The window's presentation with the anchors the scope's ids report, under their plain names. */
function scopedPresentation(presentation: Obj): Obj {
  let scoped = presentations.get(presentation);
  if (!scoped) {
    // Text probes are the same text at the same size wherever they are drawn, so the window's stand until the scope's own.
    const anchors: Obj = { ...obj(presentation.anchors) };
    for (const [key, value] of Object.entries(obj(presentation.anchors))) if (key.startsWith(SCOPE_IDS)) anchors[key.slice(SCOPE_IDS.length)] = value;
    presentations.set(presentation, scoped = { ...presentation, anchors });
  }
  return scoped;
}
const scopes = new WeakMap<T3Client, Map<string, T3Client>>();

export { windowClient, isThreadScope } from './scope-root';

/** updateSelectionFromThread over the scope's thread, once per projection: its model, options and modes, then a staged choice. */
function selectFrom(scope: T3Client, own: Own, thread: ThreadState | null): void {
  const projection = thread?.projection ?? null;
  if (!projection || own.selectionFrom === projection) return;
  own.selectionFrom = projection;
  const shell = obj(projection.thread), selection = obj(shell.modelSelection);
  own.providerId = str(selection.instanceId); own.modelId = str(selection.model);
  own.modelOptions = Array.isArray(selection.options) ? selection.options : [];
  own.runtimeMode = str(shell.runtimeMode, 'approval-required'); own.interactionMode = str(shell.interactionMode, 'default');
  reconcileStagedModels(scope); applyStaged(scope);
}

/**
 * The client seen through `threadId`, one per thread for the client's life, so stores keyed by it last. The window's own
 * thread is the client itself.
 */
export function threadScope(client: T3Client, threadId: string): T3Client {
  const root = windowClient(client);
  if (!threadId || threadId === root.threadId) return root;
  let byThread = scopes.get(root);
  if (!byThread) scopes.set(root, byThread = new Map());
  const existing = byThread.get(threadId);
  if (existing) return existing;
  const own: Own = { threadId, absentThreadId: '', projectId: '', providerId: '', modelId: '', modelOptions: [], runtimeMode: 'approval-required',
    interactionMode: 'default', error: '', busy: false, historyLoading: false, threadEpoch: 0, diffOpen: false, diffLoading: false, diffError: '',
    homePending: false, homeMove: null, selectionFrom: null };
  const thread = () => keptThread(root, threadId);
  const scope: T3Client = new Proxy(root, {
    get(target, prop, receiver) {
      if (prop === SCOPE_ROOT) return target;
      // The module's anchors under the scope's ids are its own (composer-label-probes.contract LabelProbe).
      if (prop === 'presentation') return scopedPresentation(target.presentation);
      if (prop === 'thread') return thread();
      // The kept stream is in sync once its detail is live; its subscription is the scope's own.
      if (prop === 'threadLive') return thread() !== null;
      if (prop === 'threadSubscription') return threadId;
      if (typeof prop === 'string' && OWN.has(prop)) {
        if (prop === 'projectId') return str(target.shell.threads.find(entry => entry.id === threadId)?.projectId, str(own.projectId));
        if (prop === 'providerId' || prop === 'modelId' || prop === 'modelOptions' || prop === 'runtimeMode' || prop === 'interactionMode') selectFrom(receiver as T3Client, own, thread());
        return own[prop];
      }
      return Reflect.get(target, prop, receiver);
    },
    set(target, prop, value) {
      if (prop === 'thread' || prop === 'threadLive' || prop === 'threadSubscription') return true; // the kept stream owns them
      if (typeof prop === 'string' && OWN.has(prop)) { own[prop] = value; return true; }
      return Reflect.set(target, prop, value);
    },
  });
  byThread.set(threadId, scope);
  return scope;
}
