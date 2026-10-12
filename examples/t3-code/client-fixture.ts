// The T3Client test double (a fake native transport over the server's RPCs) and its fixtures,
// shared by client.test.ts and client-settings.test.ts.
import { T3Client } from './client';
import { type Native, type Files } from './protocol';
import { obj, arr, str, type Obj } from './domain';

export const timestamp = '2026-10-03T01:00:00.000Z';
export const thread = (id = 't1'): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, runtimeMode: 'approval-required', interactionMode: 'default' });
function detail(id = 't1'): Obj {
  return { snapshotSequence: 10, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0,
    projection: { thread: thread(id), runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [], providerThreads: [], providerTurns: [],
      runtimeRequests: [], messages: [], plans: [], turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [], contextTransfers: [], visibleTurnItems: [], updatedAt: timestamp } };
}
export function storage(initial?: Obj) {
  let saved = initial ? JSON.stringify(initial) : '', writes = 0;
  const files: Files = { fs: {
    async mkdir() {},
    async readFile() { if (!saved) throw new Error('not found'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { writes++; saved = new TextDecoder().decode(bytes); },
  } };
  return { files, data: () => obj(JSON.parse(saved || '{}')), writes: () => writes };
}
export class Backend implements Native {
  available = true;
  calls: Obj[] = [];
  watchers: string[] = [];
  generation = 1;
  sequence = 20;
  eventSequence = 0;
  serial = 0;
  autoMarkers = true;
  pageSize = 100;
  subscriptions: Record<string, string> = {};
  events: Obj[] = [];
  scopes = ['orchestration:read', 'orchestration:operate'];
  failure: '' | 'before' | 'after' = '';
  preferencesText = '';
  seen = new Set<string>();
  committed: Obj[] = [];
  shell: Obj = { snapshotSequence: 10, projects: [{ id: 'p1', title: 'Example', workspaceRoot: '/repo' }], threads: [thread(), thread('t2')], archivedThreads: [] };
  details: Record<string, Obj> = { t1: detail(), t2: detail('t2') };
  config: Obj = { environment: { environmentId: 'env1', orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'codex-personal', driver: 'codex', displayName: 'Codex', enabled: true, installed: true,
      auth: { status: 'authenticated' }, status: 'ready', supportedRuntimeModes: ['approval-required', 'full-access'],
      models: [{ slug: 'model-a', name: 'Model A', isDefault: true }, { slug: 'model-b', name: 'Model B' }] }],
    settings: { defaultModelSelection: { instanceId: 'codex-personal', model: 'model-a' }, defaultRuntimeMode: 'full-access' },
    shellResumeCompletionMarker: true, threadResumeCompletionMarker: true };
  watch(topic: string) { this.watchers.push(topic); }
  good(value: unknown) { return { ok: true, generation: this.generation, value }; }
  bad(message: string, uncertain = false) { return { ok: false, generation: this.generation, error: { kind: 'transport', message, uncertain } }; }
  status() { return { state: 'connected', origin: 'http://127.0.0.1:3773', environmentId: 'env1', message: '', descriptor: this.config.environment }; }
  emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) {
    this.events.push({ seq: ++this.eventSequence, generation: this.generation, key, subscriptionId, value });
  }
  emitDomain(type: string, payload: Obj, threadId = 't1') {
    this.emit('thread', { kind: 'event', sequence: ++this.sequence,
      event: { id: `e${this.sequence}`, type, threadId, occurredAt: timestamp, payload } });
  }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (typeof request.generation === 'number' && request.generation !== this.generation) return this.bad('stale');
    const op = request.op;
    if (op === 'readPreferences') return this.good({ text: this.preferencesText });
    if (op === 'writePreferences') { this.preferencesText = str(request.text); return this.good({}); }
    if (op === 'devicePresentation') return this.good({});
    if (op === 'copyText') return this.good({ copied: true });
    if (op === 'status') return this.good(this.status());
    if (op === 'connect') { this.generation++; this.subscriptions = {}; return this.good(this.status()); }
    if (op === 'disconnect') { this.generation++; return this.good({ ...this.status(), state: 'disconnected' }); }
    if (op === 'ids') return this.good(Array.from({ length: Number(request.count) }, () => `uuid-${++this.serial}`));
    if (op === 'http') {
      if (request.path === '/api/auth/session') return this.good({ authenticated: true, scopes: this.scopes });
      if (request.path === '/api/orchestration/shell') return this.good(this.shell);
      const found = str(request.path).match(/\/threads\/([^/]+)\/bounded/);
      if (found) return this.good(this.details[found[1]]);
      if (str(request.path).includes('/history?')) return this.good({ snapshotSequence: this.sequence, items: [], nextCursor: null, hasMoreHistory: false });
      return this.bad('not found');
    }
    if (op === 'subscribe') {
      const key = str(request.key), id = `sub-${++this.serial}`;
      this.subscriptions[key] = id;
      if (key === 'config') this.emit(key, { type: 'snapshot', config: this.config });
      if (key !== 'config' && this.autoMarkers) this.emit(key, { kind: 'synchronized' });
      return this.good({ id });
    }
    if (op === 'unsubscribe') { delete this.subscriptions[str(request.key)]; return this.good({}); }
    if (op === 'events') return this.good({ events: this.events.filter(event => Number(event.seq) > Number(request.after)).slice(0, this.pageSize), latest: this.eventSequence, reset: false });
    if (op === 'ack') { this.events = this.events.filter(event => Number(event.seq) > Number(request.through)); return this.good({ latest: this.eventSequence }); }
    if (op === 'request') {
      if (request.method === 'orchestration.getArchivedShellSnapshot') return this.good({ snapshotSequence: this.sequence, projects: this.shell.projects, threads: this.shell.archivedThreads || [] });
      if (request.method === 'server.getConfig') return this.good(this.config);
      if (request.method === 'server.getSettings') return this.good(this.config.settings);
      if (request.method === 'server.updateSettings') {
        const payload = obj(request.payload), patch = obj(payload.patch), old = obj(this.config.settings);
        const overrides = { ...obj(old.projectSettingsOverrides) };
        for (const [id, entry] of Object.entries(obj(patch.projectSettingsOverrides))) { if (entry === null) delete overrides[id]; else overrides[id] = entry; }
        this.config.settings = { ...old, ...patch, projectSettingsOverrides: overrides };
        const mutation = obj(payload.providerInstanceMutation);
        if (mutation.operation) {
          const providers = { ...obj(old.providerInstances) };
          if (mutation.operation === 'remove') delete providers[str(mutation.instanceId)];
          else providers[str(mutation.instanceId)] = mutation.instance;
          obj(this.config.settings).providerInstances = providers;
        }
        return this.good(this.config.settings);
      }
      if (request.method === 'orchestration.getFullThreadDiff' || request.method === 'orchestration.getTurnDiff') return this.good({ diff: 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new' });
      const payload = obj(request.payload);
      if (this.failure === 'before') { this.failure = ''; return this.bad('lost connection', true); }
      if (!this.seen.has(str(payload.commandId))) {
        this.seen.add(str(payload.commandId)); this.committed.push(payload);
        this.commit(str(request.method), payload);
      }
      if (this.failure === 'after') { this.failure = ''; return this.bad('lost acknowledgment', true); }
      if (request.method === 'orchestration.launchThread') return this.good({ threadId: payload.threadId, projection: obj(this.details[str(payload.threadId)]).projection, resumed: false });
      return this.good({ sequence: this.sequence });
    }
    return this.bad(`Unknown operation ${String(op)}`);
  }
  commit(method: string, payload: Obj) {
    if (method === 'orchestration.launchThread') {
      const id = str(payload.threadId), created = { ...thread(id), projectId: payload.projectId, title: payload.title, modelSelection: payload.modelSelection };
      this.details[id] = detail(id); obj(this.details[id].projection).thread = created;
      arr(this.shell.threads).push(created);
      this.emit('shell', { kind: 'thread.updated', sequence: ++this.sequence, location: 'active', thread: created });
    } else if (payload.type === 'thread.unarchive' || payload.type === 'thread.delete') {
      const id = str(payload.threadId);
      const archived = arr(this.shell.archivedThreads).find(thread => thread.id === id);
      this.shell.archivedThreads = arr(this.shell.archivedThreads).filter(thread => thread.id !== id);
      if (payload.type === 'thread.unarchive' && archived) this.shell.threads = [...arr(this.shell.threads), { ...archived, archivedAt: null }];
    } else if (method === 'projects.mutate') {
      const existing = arr(this.shell.projects).find(project => project.id === payload.projectId);
      if (payload.type === 'project.delete') {
        this.shell.projects = arr(this.shell.projects).filter(project => project.id !== payload.projectId);
        this.shell.threads = arr(this.shell.threads).filter(thread => thread.projectId !== payload.projectId);
        this.emit('shell', { kind: 'project.removed', sequence: ++this.sequence, projectId: payload.projectId });
      } else {
        const project = { ...existing, id: payload.projectId, title: payload.title, workspaceRoot: existing?.workspaceRoot || payload.workspaceRoot };
        this.shell.projects = [...arr(this.shell.projects).filter(entry => entry.id !== project.id), project];
        this.emit('shell', { kind: 'project.updated', sequence: ++this.sequence, project });
      }
    } else if (payload.type === 'message.dispatch') {
      const message = { id: payload.messageId, threadId: payload.threadId, role: 'user', text: payload.text };
      const projection = obj(this.details[str(payload.threadId)].projection);
      (projection.messages as Obj[]).push(message);
      this.emitDomain('message.updated', message, str(payload.threadId));
    }
  }
}
export async function connected() {
  const client = new T3Client(), native = new Backend(), disk = storage();
  await client.refresh(native, disk.files);
  return { client, native, disk, command: (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, disk.files) };
}
export async function opened() {
  const context = await connected();
  await context.command('select-thread', 't1');
  await context.client.refresh(context.native, context.disk.files);
  return context;
}
