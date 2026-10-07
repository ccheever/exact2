// Lane composer-controls: the fake T3 backend its tests drive (not a test file).
import { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import type { Native, Files } from './protocol';

export const at = '2026-10-03T01:00:00.000Z';
export const effort = { id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] };
export const fast = { id: 'fastMode', label: 'Fast mode', type: 'boolean', currentValue: false };
export const provider = (instanceId: string, driver = 'codex', extra: Obj = {}): Obj => ({ instanceId, driver, displayName: instanceId, enabled: true, installed: true,
  auth: { status: 'authenticated' }, status: 'ready', models: [{ slug: 'model-a', name: 'Model A', isDefault: true, capabilities: { optionDescriptors: [effort, fast] } },
    { slug: 'model-b', name: 'Model B', capabilities: { optionDescriptors: [effort] } }], ...extra });
export const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, modelSelection: { instanceId: 'codex', model: 'model-a' },
  runtimeMode: 'full-access', interactionMode: 'default', ...extra });
export const projection = (id: string, extra: Obj = {}): Obj => ({ thread: thread(id), runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [], providerThreads: [],
  providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [], contextTransfers: [],
  visibleTurnItems: [], updatedAt: at, ...extra });

export class Fake implements Native {
  available = true; branch = 'main'; extraRefs: Obj[] = []; picked: Obj[] = []; menuPick: string | null = null; copied: string[] = []; generation = 1; serial = 0; seq = 0; events: Obj[] = []; subs: Record<string, string> = {}; committed: Obj[] = []; gesture: Obj = {};
  shell: Obj = { snapshotSequence: 1, projects: [{ id: 'p1', title: 'Example', workspaceRoot: '/repo' }], threads: [thread('t1'), thread('t2')] };
  details: Record<string, Obj> = { t1: { snapshotSequence: 1, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0, projection: projection('t1') },
    t2: { snapshotSequence: 1, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0, projection: projection('t2') } };
  config: Obj = { environment: { environmentId: 'env1', orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [provider('codex'), provider('codex-work'), provider('claude', 'claudeAgent')], settings: { defaultModelSelection: { instanceId: 'codex', model: 'model-a' }, defaultRuntimeMode: 'full-access' } };
  watch() {}
  ok(value: unknown) { return { ok: true, generation: this.generation, value }; }
  emit(key: string, value: Obj) { this.events.push({ seq: ++this.seq, generation: this.generation, key, subscriptionId: this.subs[key], value }); }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), op = request.op;
    if (op === 'readPreferences') return this.ok({ text: '' });
    if (op === 'writePreferences' || op === 'devicePresentation' || op === 'unsubscribe') return this.ok({});
    if (op === 'status') return this.ok({ state: 'connected', origin: 'http://127.0.0.1:3773', environmentId: 'env1', message: '', descriptor: this.config.environment });
    // The connected server is this Mac's embedded server: the primary environment (local-primary.ts).
    if (op === 'localBackendStatus') return this.ok({ state: 'ready', enabled: true, httpBaseUrl: 'http://127.0.0.1:3773', wsBaseUrl: 'ws://127.0.0.1:3773', bearerReady: true, environmentId: 'env1', label: 'This Mac' });
    if (op === 'ids') return this.ok(Array.from({ length: Number(request.count) }, () => `id-${++this.serial}`));
    if (op === 'composerAttachPick') return this.ok({ files: this.picked });
    if (op === 'contextMenu') return this.ok({ clicked: this.menuPick });
    if (op === 'copyText') { this.copied.push(str(request.text)); return this.ok({ copied: true }); }
    if (op === 'composerSendIntent') { const gesture = this.gesture; this.gesture = {}; return this.ok(gesture); }
    if (op === 'http') {
      if (request.path === '/api/auth/session') return this.ok({ authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'] });
      if (request.path === '/api/orchestration/shell') return this.ok(this.shell);
      const found = str(request.path).match(/\/threads\/([^/]+)\/bounded/);
      return found ? this.ok(this.details[found[1]!]) : { ok: false, generation: this.generation, error: { kind: 'transport', message: 'nf', uncertain: false } };
    }
    if (op === 'subscribe') {
      const key = str(request.key), id = `sub-${++this.serial}`; this.subs[key] = id;
      if (key === 'config') this.emit(key, { type: 'snapshot', config: this.config }); else this.emit(key, { kind: 'synchronized' });
      return this.ok({ id });
    }
    if (op === 'events') return this.ok({ events: this.events.filter(event => Number(event.seq) > Number(request.after)), latest: this.seq, reset: false });
    if (op === 'ack') { this.events = this.events.filter(event => Number(event.seq) > Number(request.through)); return this.ok({ latest: this.seq }); }
    if (op === 'request') {
      if (request.method === 'server.getConfig') return this.ok(this.config);
      if (request.method === 'vcs.refreshStatus') return this.ok({ isRepo: true, refName: this.branch });
      if (request.method === 'vcs.listRefs') return this.ok({ isRepo: true, hasPrimaryRemote: true, nextCursor: null, totalCount: 3,
        refs: [{ name: 'main', current: this.branch === 'main', isDefault: true, worktreePath: null }, { name: 'feature', current: this.branch === 'feature', isDefault: false, worktreePath: null },
          { name: 'origin/main', isRemote: true, current: false, isDefault: false, worktreePath: null }, ...this.extraRefs].filter(ref => !obj(request.payload).query || ref.name.includes(str(obj(request.payload).query))) });
      if (request.method === 'vcs.switchRef') { this.branch = str(obj(request.payload).refName); this.committed.push({ method: request.method, ...obj(request.payload) }); return this.ok({ refName: this.branch }); }
      const payload = obj(request.payload); this.committed.push({ method: request.method, ...payload });
      if (request.method === 'orchestration.launchThread') {
        const id = str(payload.threadId);
        this.details[id] = { snapshotSequence: 1, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0, projection: projection(id, { thread: thread(id, { modelSelection: payload.modelSelection }) }) };
        arr(this.shell.threads).push(thread(id));
        return this.ok({ threadId: id });
      }
      return this.ok({ sequence: 1 });
    }
    return { ok: false, generation: this.generation, error: { kind: 'transport', message: `unknown ${String(op)}`, uncertain: false } };
  }
}
export function storage(): Files {
  let saved = '';
  return { fs: { async mkdir() {}, async readFile() { if (!saved) throw new Error('missing'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { saved = new TextDecoder().decode(bytes); } } };
}
export async function connected() {
  const client = new T3Client(), native = new Fake(), disk = storage();
  await client.refresh(native, disk);
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, disk);
  return { client, native, disk, command };
}
export async function opened(id = 't1') {
  const context = await connected();
  await context.command('select-thread', id);
  await context.client.refresh(context.native, context.disk);
  return context;
}
export const running = (client: T3Client, extra: Obj = {}) => { client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'running', ...extra }]; };
