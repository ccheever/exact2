import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { fleet } from './shared/settings-b-fleet';
import { mobileComposerProvider as projectProvider, mobileComposerQueryDemand as demand, mobileComposerQueryPrepare as prepare,
  mobileComposerQuerySnapshot as snapshot, type ComposerQueryInput, type ComposerQueryLane } from './composer-command-query';
import { mobileComposerCommandRows, resolveProviderSkillsForCwd, resolveProviderSlashCommandsForCwd,
  hasCompleteProviderWorkspaceSnapshot } from './composer-command-model';
let sequence = 0;
const provider = { instanceId: 'codex1', driver: 'codex', skills: [], slashCommands: [], workspaceSnapshots: [{ cwd: '/repo', skills: [], slashCommands: [] }] };
function fixture(kind: 'path' | 'pull-request' | 'slash-command' = 'path', query = 'src') {
  const client = new T3Client(), id = `composer-query-${++sequence}`;
  client.origin = 'https://owned.test'; client.environmentId = id; client.connection = 'connected'; client.projectId = 'p';
  fleet.saved.push({ environmentId: id, origin: client.origin });
  const input: ComposerQueryInput = { owner: 'draft1', editorId: 'composer', routeVisit: 'visit1', renderEpoch: 'epoch1', mountId: 'mount1',
    documentRevision: 1, prompt: query, active: true, environmentId: id, cwd: '/repo', projectId: 'p', repository: 'o/r',
    provider: structuredClone(provider), trigger: { kind, query, rangeStart: 0, rangeEnd: query.length }, permissionRevision: 's1' };
  const calls: Obj[] = []; let now = 1000;
  const control: { reply?: (request: Obj) => Obj | Promise<Obj> } = {};
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(request);
    const value = control.reply ? await control.reply(request) : request.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] }
      : request.method === 'projects.searchEntries' ? { entries: [{ path: `${obj(request.payload).query}.ts`, kind: 'file' }] }
      : request.method === 'pullRequests.list' ? { entries: [], errors: [] } : { providers: [provider] };
    return { ok: true, generation: client.generation, value };
  } };
  const view = () => demand(client, input, now);
  const run = async (lane: ComposerQueryLane, captured = view()) => prepare(client, captured.admission, lane, captured[lane].key, native, () => now);
  const read = () => snapshot(client, view().admission, now);
  const change = (query: string, kind = input.trigger!.kind) => { input.prompt = query; input.documentRevision++; input.trigger = { kind, query, rangeStart: 0, rangeEnd: query.length }; return view(); };
  return { client, input, calls, native, control, view, run, read, change, time: (value: number) => { now = value; }, now: () => now };
}
const tick = () => new Promise<void>(resolve => queueMicrotask(resolve));
async function until(fn: () => boolean) { for (let i = 0; i < 30 && !fn(); i++) await tick(); expect(fn()).toBe(true); }
function held<T>() { let resolve!: (value: T) => void, reject!: (value: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
const pr = (number: number, extra: Obj = {}) => ({ projectId: 'p', repository: 'o/r', number, title: `PR ${number}`, headBranch: 'feature', baseBranch: 'main',
  state: 'open', isDraft: false, url: `https://owned.test/${number}`, updatedAt: '2026-10-09T12:00:00Z', ...extra });

test('initial path mounts immediately; later whole target waits200ms and preserves old rows while debouncing', async () => {
  const f = fixture(); expect(f.view().path.delayMs).toBe(0); await f.run('path');
  expect(f.calls.map(c => c.method || c.path)).toEqual(['/api/auth/session', 'projects.searchEntries']);
  expect(f.calls[1]!.payload).toEqual({ cwd: '/repo', query: 'src', limit: 20 }); expect(f.read().pathEntries[0]!.path).toBe('src.ts');
  const next = f.change('  lib  '); expect(next.path.delayMs).toBe(200); await f.run('path', next); expect(f.calls.length).toBe(2);
  expect(f.read().pathEntries[0]!.path).toBe('src.ts'); f.time(1199); await f.run('path'); expect(f.calls.length).toBe(2);
  f.time(1200); await f.run('path'); expect(f.read().pathEntries[0]!.path).toBe('lib.ts'); expect(f.read().pathPending).toBe(false);
});
test('explicit empty permission denies legacy grants, clears retained path rows, and never searches', async () => {
  const f = fixture(); await f.run('path'); f.input.permissionRevision = 's2';
  f.input.session = { authenticated: true, permissions: [], scopes: ['orchestration:read'] };
  expect(f.read().pathEntries).toEqual([]); await f.run('path'); expect(f.calls.length).toBe(2);
  expect(f.read().pathPending).toBe(false); expect(f.read().pullRequestsError).toBe('');
});
test('session refusal clears query data and prevents file RPC; legacy read permission remains compatible', async () => {
  const f = fixture(); f.control.reply = () => ({ authenticated: true, permissions: [], scopes: ['filesystem:read'] });
  await f.run('path'); expect(f.calls.length).toBe(1); expect(f.read().pathEntries).toEqual([]);
  const g = fixture(); g.control.reply = r => r.op === 'http' ? { authenticated: true, scopes: ['orchestration:read'] } : { entries: [{ path: 'a', kind: 'directory' }] };
  await g.run('path'); expect(g.read().pathEntries).toEqual([{ path: 'a', kind: 'directory' }]);
});
test('path SWR15s revalidates only on remount, then evicts after5min idle', async () => {
  const f = fixture(); await f.run('path'); f.time(16000); await f.run('path'); expect(f.calls.length).toBe(2);
  f.change(''); f.time(16200); f.view(); f.change('src'); f.time(16400); expect(f.read().pathEntries[0]!.path).toBe('src.ts');
  await f.run('path'); expect(f.calls.length).toBe(4);
  f.change(''); f.time(16600); f.view(); f.time(316601); f.change('src'); f.time(316801);
  expect(f.read().pathEntries).toEqual([]); await f.run('path'); expect(f.calls.length).toBe(6);
});
test('numeric PR starts list and exact detail concurrently; source request schemas and exact-first filters hold', async () => {
  const f = fixture('pull-request', '12'), list = held<Obj>(), detail = held<Obj>();
  f.control.reply = r => r.method === 'pullRequests.list' ? list.promise : detail.promise;
  const running = f.run('pullRequests'); await until(() => f.calls.length === 2);
  expect(f.calls.map(c => c.payload)).toEqual([{ projectId: 'p', state: 'all', limit: 200 }, { projectId: 'p', repository: 'o/r', number: 12 }]);
  detail.resolve(pr(12, { updatedAt: '2020', title: 'exact' })); list.resolve({ entries: [pr(112), pr(12, { title: 'duplicate' }), pr(12, { repository: 'wrong' }), pr(9)] });
  await running; expect(f.read().pullRequests.map(p => p.number)).toEqual([12, 112]);
  expect(f.read().pullRequestsPending).toBe(false);
});
test('PR transitions empty while180ms debounce; textual words, repository/project, dedupe and20limit', async () => {
  const f = fixture('pull-request', 'feature main');
  f.control.reply = () => ({ entries: [...Array.from({ length: 25 }, (_, i) => pr(i + 1, { updatedAt: String(i).padStart(2, '0') })),
    pr(99, { projectId: 'other' }), pr(88, { repository: 'wrong' }), pr(77, { title: 'x', headBranch: 'x' })] });
  await f.run('pullRequests'); expect(f.calls[0]!.payload).toEqual({ projectId: 'p', state: 'all', limit: 200, query: 'feature main' });
  expect(f.read().pullRequests.length).toBe(20); expect(f.read().pullRequests[0]!.number).toBe(25);
  const target = f.change('2'); expect(target.pullRequests.delayMs).toBe(180); expect(f.read().pullRequests).toEqual([]);
  f.time(1179); await f.run('pullRequests'); expect(f.calls.length).toBe(1);
  f.time(1180); await f.run('pullRequests'); expect(f.calls.length).toBe(3);
});
test('PR cached exact list entry suppresses detail;30s list versus60s detail SWR and no periodic requests', async () => {
  const f = fixture('pull-request', '7'); f.control.reply = r => r.method === 'pullRequests.list' ? { entries: [] } : pr(7);
  await f.run('pullRequests'); expect(f.calls.length).toBe(2); f.time(31000); await f.run('pullRequests'); expect(f.calls.length).toBe(2);
  f.change(''); f.time(31180); f.view(); f.change('7'); f.time(31360); await f.run('pullRequests');
  expect(f.calls.map(c => c.method)).toEqual(['pullRequests.list', 'pullRequests.detail', 'pullRequests.list']);
  const g = fixture('pull-request', ''); g.control.reply = () => ({ entries: [pr(7)] }); await g.run('pullRequests');
  g.change('7'); g.time(1180); await g.run('pullRequests'); expect(g.calls.length).toBe(1);
});
test('PR list error precedes returned errors and detail error; unavailable project stays explicit', async () => {
  const f = fixture('pull-request', '4'); f.control.reply = r => { if (r.method === 'pullRequests.list') throw new Error('list failure'); throw new Error('detail failure'); };
  await f.run('pullRequests'); expect(f.read().pullRequestsError).toBe('list failure');
  const g = fixture('pull-request', '4'); g.control.reply = r => { if (r.method === 'pullRequests.list') return { entries: [], errors: [{ message: 'provider error' }] }; throw new Error('detail error'); };
  await g.run('pullRequests'); expect(g.read().pullRequestsError).toBe('provider error');
  g.input.projectId = ''; expect(g.read().pullRequestsError).toBe('Pull requests are unavailable for this project.');
});
test('late path reply cannot cross document/route/provider/cwd/session/connection/catalog fences', async () => {
  for (const mutation of [
    (f: ReturnType<typeof fixture>) => { f.input.documentRevision++; f.view(); },
    (f: ReturnType<typeof fixture>) => { f.input.routeVisit = 'next'; f.view(); },
    (f: ReturnType<typeof fixture>) => { f.input.provider!.instanceId = 'other'; f.view(); },
    (f: ReturnType<typeof fixture>) => { f.input.cwd = '/other'; f.view(); },
    (f: ReturnType<typeof fixture>) => { f.input.permissionRevision = 'revoked'; f.input.session = { authenticated: true, permissions: [] }; f.view(); },
    (f: ReturnType<typeof fixture>) => { f.client.generation++; },
    (f: ReturnType<typeof fixture>) => { fleet.saved.find(e => e.environmentId === f.input.environmentId)!.origin = 'https://replacement.test'; },
  ]) {
    const f = fixture(), wait = held<Obj>(); f.control.reply = r => r.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] } : wait.promise;
    const old = f.view(), running = f.run('path', old); await until(() => f.calls.length === 2); mutation(f); wait.resolve({ entries: [{ path: 'stale', kind: 'file' }] });
    await expect(running).rejects.toBeInstanceOf(ClientError); expect(snapshot(f.client, old.admission, f.now()).pathEntries).toEqual([]);
  }
});
test('letGo preserves cancellation identity, makes no followup RPC and permits next owned retry', async () => {
  const f = fixture(), cancelled = { name: 'FetchError', kind: 'Aborted' }; f.control.reply = () => { throw cancelled; };
  await expect(f.run('path')).rejects.toBe(cancelled); expect(f.calls.length).toBe(1);
  f.control.reply = r => r.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] } : { entries: [{ path: 'retry', kind: 'file' }] };
  await f.run('path'); expect(f.read().pathEntries[0]!.path).toBe('retry'); expect(f.calls.length).toBe(3);
});
test('pending discovery cooldown starts at settle, uses exact cwd and schedules only pending snapshots', async () => {
  const f = fixture('slash-command', '/'); f.input.provider!.workspaceSnapshots = [];
  const wait = held<Obj>(); f.control.reply = () => wait.promise;
  const running = f.run('discovery'); await until(() => f.calls.length === 1); expect(f.calls[0]!.payload).toEqual({ instanceId: 'codex1', cwd: '/repo' });
  f.time(2000); wait.resolve({ providers: [{ ...provider, workspaceSnapshots: [{ cwd: '/repo', slashCommandsPending: true, skills: [], slashCommands: [] }] }] }); await running;
  expect(f.view().discovery).toMatchObject({ dueAt: 12000, delayMs: 10000 }); f.time(11999); await f.run('discovery'); expect(f.calls.length).toBe(1);
  f.control.reply = () => ({ providers: [provider] }); f.time(12000); await f.run('discovery'); expect(f.calls.length).toBe(2); expect(f.view().discovery.key).toBe('');
  expect(f.read().provider?.workspaceSnapshots?.[0]?.cwd).toBe('/repo');
});
test('ordinary discovery failure has no timer, retries only later wake aftercooldown; exact cwd overrides globals', async () => {
  const f = fixture('slash-command', '/'); f.input.provider!.workspaceSnapshots = [];
  f.control.reply = () => { throw new Error('discovery failed'); }; await f.run('discovery'); expect(f.view().discovery.key).toBe('');
  f.time(11000); expect(f.view().discovery.key).toBe(''); f.change('/m'); expect(f.view().discovery.key).not.toBe('');
  f.control.reply = () => ({ providers: [{ ...provider, skills: [{ name: 'global' }], workspaceSnapshots: [{ cwd: '/repo', skills: [], slashCommands: [] }] }] });
  await f.run('discovery'); expect(f.read().provider?.workspaceSnapshots?.[0]?.skills).toEqual([]); expect(f.calls.length).toBe(2);
});


test('fresh cache mount stays settled after stale deadline without periodic revalidation', async () => {
  for (const kind of ['path', 'pull-request'] as const) {
    const f = fixture(kind, kind === 'path' ? 'src' : ''); await f.run(kind === 'path' ? 'path' : 'pullRequests');
    f.input.active = false; f.view(); f.time(2000); f.input.active = true; f.input.renderEpoch = 'next';
    await f.run(kind === 'path' ? 'path' : 'pullRequests'); const calls = f.calls.length;
    f.time(100000); expect(f.read().pathPending).toBe(false); expect(f.read().pullRequestsPending).toBe(false);
    await f.run(kind === 'path' ? 'path' : 'pullRequests'); expect(f.calls.length).toBe(calls);
  }
});
test('route departure preserves warm cache until5min idle and reentry uses initial no-debounce', async () => {
  const f = fixture(); await f.run('path'); f.input.active = false; f.view(); expect(f.read().pathEntries).toEqual([]);
  f.time(2000); f.input.active = true; f.input.renderEpoch = 'return'; expect(f.view().path.delayMs).toBe(0);
  expect(f.read().pathEntries[0]!.path).toBe('src.ts'); await f.run('path'); expect(f.calls.length).toBe(2);
  f.input.active = false; f.view(); f.time(302001); f.input.active = true; f.input.renderEpoch = 'expired';
  expect(f.read().pathEntries).toEqual([]); await f.run('path'); expect(f.calls.length).toBe(4);
});
test('discovery cooldown timer key does not restart on prompt edits; ordinary elapsed wake is immediate', async () => {
  const f = fixture('slash-command', '/'); f.input.provider!.workspaceSnapshots = [];
  f.control.reply = () => ({ providers: [{ ...provider, workspaceSnapshots: [{ cwd: '/repo', slashCommandsPending: true }] }] });
  await f.run('discovery'); const key = f.view().discovery.key; f.time(3000); f.change('/m');
  expect(f.view().discovery.key).toBe(key); expect(f.view().discovery.dueAt).toBe(11000);
  const g = fixture('slash-command', '/'); g.input.provider!.workspaceSnapshots = []; g.control.reply = () => { throw new Error('failed'); };
  await g.run('discovery'); g.time(11000); g.change('/m'); expect(g.view().discovery.delayMs).toBe(0);
});


test('PR project change keeps settled query immediate; path cwd change stilldebounces whole target', async () => {
  const f = fixture('pull-request', 'feature'); await f.run('pullRequests');
  f.input.projectId = 'p2'; f.client.projectId = 'p2'; expect(f.view().pullRequests.delayMs).toBe(0);
  await f.run('pullRequests'); expect(f.calls.length).toBe(2); expect(obj(f.calls[1]!.payload).projectId).toBe('p2');
  const g = fixture(); await g.run('path'); g.input.cwd = '/other'; expect(g.view().path.delayMs).toBe(200);
  await g.run('path'); expect(g.calls.length).toBe(2);
});
test('session check failure on stale remount removes cached rows rather than retaining former authority', async () => {
  const f = fixture(); await f.run('path'); f.input.active = false; f.view(); f.time(16001); f.input.active = true;
  f.control.reply = () => { throw new Error('session unavailable'); }; await f.run('path');
  expect(f.read().pathEntries).toEqual([]); expect(f.read().pathPending).toBe(false); expect(f.calls.length).toBe(3);
});
test('late concurrent PR list/detail replies cannot populate replaced route', async () => {
  const f = fixture('pull-request', '7'), list = held<Obj>(), detail = held<Obj>();
  f.control.reply = r => r.method === 'pullRequests.list' ? list.promise : detail.promise;
  const old = f.view(), running = f.run('pullRequests', old); await until(() => f.calls.length === 2);
  f.input.routeVisit = 'new'; f.input.renderEpoch = 'new'; f.view(); list.resolve({ entries: [pr(7)] }); detail.resolve(pr(7));
  await expect(running).rejects.toBeInstanceOf(ClientError); expect(f.read().pullRequests).toEqual([]);
  expect(snapshot(f.client, old.admission, f.now()).pullRequests).toEqual([]);
});
test('PR detail becomes stale at60s on remount and idle expiry removes both records', async () => {
  const f = fixture('pull-request', '7'); f.control.reply = r => r.method === 'pullRequests.list' ? { entries: [] } : pr(7);
  await f.run('pullRequests'); f.input.active = false; f.view(); f.time(61000); f.input.active = true; f.input.renderEpoch = 'return';
  await f.run('pullRequests'); expect(f.calls.length).toBe(4); expect(f.read().pullRequests[0]!.number).toBe(7);
  f.input.active = false; f.view(); f.time(361001); f.input.active = true; f.input.renderEpoch = 'expired';
  expect(f.read().pullRequests).toEqual([]); await f.run('pullRequests'); expect(f.calls.length).toBe(6);
});
test('discovery cancellation propagates unchanged and retries without a failure cooldown', async () => {
  const f = fixture('slash-command', '/'); f.input.provider!.workspaceSnapshots = []; const cancelled = { name: 'FetchError', kind: 'Aborted' };
  f.control.reply = () => { throw cancelled; }; await expect(f.run('discovery')).rejects.toBe(cancelled);
  expect(f.view().discovery.delayMs).toBe(0); f.control.reply = () => ({ providers: [provider] }); await f.run('discovery');
  expect(f.calls.length).toBe(2); expect(f.view().discovery.key).toBe('');
});

test('provider projection preserves command behavior and missing versus empty workspace overrides', () => {
  const skill = { name: 'deploy', enabled: true, userInvocable: true, displayName: 'Ship It', shortDescription: 'Release',
    description: 'Build release', path: '/repo/.codex/skills/deploy', scope: 'repo' };
  const full = { ...provider, showInteractionModeToggle: false, skills: [skill], slashCommands: [{ name: 'compact', description: 'Summarize' }],
    workspaceSnapshots: [{ cwd: '/repo', slashCommandsPending: true }, { cwd: '/empty', skills: [], slashCommands: [], slashCommandsPending: false }],
    models: [{ slug: 'model', catalog: 'catalog'.repeat(300_000) }], auth: { status: 'ready' } };
  const projected = projectProvider(full)!;
  for (const cwd of ['/repo', '/empty', '/other']) {
    expect(resolveProviderSkillsForCwd(projected, cwd)).toEqual(resolveProviderSkillsForCwd(full, cwd));
    expect(resolveProviderSlashCommandsForCwd(projected, cwd)).toEqual(resolveProviderSlashCommandsForCwd(full, cwd));
    expect(hasCompleteProviderWorkspaceSnapshot(projected, cwd)).toBe(hasCompleteProviderWorkspaceSnapshot(full, cwd));
    for (const kind of ['slash-command', 'skill'] as const) {
      const input = { trigger: { kind, query: '', rangeStart: 0, rangeEnd: 1 }, projectCwd: cwd, hasThread: true,
        hasCompactableConversation: true, offersUsageLimits: false, allowInteractionMode: true, environmentId: 'env',
        currentThreadId: 't', threadShells: [], pathEntries: [], pullRequestEntries: [] };
      expect(mobileComposerCommandRows({ ...input, selectedProviderStatus: projected }))
        .toEqual(mobileComposerCommandRows({ ...input, selectedProviderStatus: full }));
    }
  }
  expect(Object.hasOwn(projected.workspaceSnapshots![0]!, 'skills')).toBe(false);
  expect(projected.workspaceSnapshots![1]!.skills).toEqual([]);
  expect(JSON.stringify(projected).length).toBeLessThan(JSON.stringify(full).length / 1000);
  projected.skills[0]!.description = 'Changed'; expect(full.skills[0]!.description).toBe('Build release');
  expect(full.models[0]!.catalog.length).toBe(2_100_000);
});
test('demand, fingerprints, discovery and returned snapshots never read the model catalog', async () => {
  const f = fixture('slash-command', '/'), source = { ...provider, skills: [{ name: 'global', enabled: true, path: '/skills/global' }],
    workspaceSnapshots: [] as Obj[], get models(): never { throw new Error('Command queries must not copy models'); } };
  f.client.config.providers = [source]; f.input.provider = source;
  const initial = f.view(); expect(initial.discovery.key).not.toBe('');
  f.control.reply = () => ({ providers: [{ ...provider, skills: source.skills,
    workspaceSnapshots: [{ cwd: '/repo', skills: source.skills, slashCommands: [] }],
    get models(): never { throw new Error('Discovery must not copy models'); } }] });
  await f.run('discovery', initial); expect(f.view().discovery.key).toBe('');
  const returned = f.read().provider!; expect(returned.workspaceSnapshots![0]!.skills![0]!.name).toBe('global');
  returned.workspaceSnapshots![0]!.skills![0]!.name = 'mutated';
  expect(f.read().provider!.workspaceSnapshots![0]!.skills![0]!.name).toBe('global');
  expect(Object.hasOwn(returned, 'models')).toBe(false); expect(f.calls).toHaveLength(1);
});
test('held command-query replies remain owned across model-only catalog refreshes', async () => {
  for (const lane of ['path', 'pullRequests', 'discovery'] as const) {
    const f = fixture(lane === 'path' ? 'path' : lane === 'pullRequests' ? 'pull-request' : 'slash-command', lane === 'path' ? 'src' : ''), wait = held<Obj>();
    const before = { ...provider, models: [{ slug: 'old' }], ...(lane === 'discovery' ? { workspaceSnapshots: [] } : {}) };
    f.client.config.providers = [before]; f.input.provider = before;
    f.control.reply = r => r.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] } : wait.promise;
    const old = f.view(), running = f.run(lane, old); await until(() => f.calls.length === (lane === 'path' ? 2 : 1));
    const after = { ...before, models: [{ slug: 'new', catalog: 'large'.repeat(100_000) }], auth: { status: 'changed' } };
    f.client.config.providers = [after]; f.input.provider = after; expect(f.view().admission).toBe(old.admission);
    wait.resolve(lane === 'path' ? { entries: [{ path: 'owned', kind: 'file' }] } : lane === 'pullRequests'
      ? { entries: [pr(7)] } : { providers: [provider] });
    await running; const view = snapshot(f.client, old.admission, f.now());
    if (lane === 'path') expect(view.pathEntries).toEqual([{ path: 'owned', kind: 'file' }]);
    else if (lane === 'pullRequests') expect(view.pullRequests.map(row => row.number)).toEqual([7]);
    else expect(view.provider?.workspaceSnapshots?.[0]?.cwd).toBe('/repo');
    expect(f.calls).toHaveLength(lane === 'path' ? 2 : 1);
  }
});
test('relevant provider changes still invalidate a held command query before publication', async () => {
  for (const patch of [{ driver: 'other' }, { instanceId: 'other' }, { showInteractionModeToggle: false },
    { skills: [{ name: 'new', enabled: true, path: '/skills/new', description: 'Changed' }] },
    { slashCommands: [{ name: 'new', description: 'Changed' }] },
    { workspaceSnapshots: [{ cwd: '/repo', skills: [], slashCommands: [], slashCommandsPending: true }] }]) {
    const f = fixture(), wait = held<Obj>(); f.client.config.providers = [structuredClone(provider)];
    f.control.reply = r => r.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] } : wait.promise;
    const old = f.view(), running = f.run('path', old); await until(() => f.calls.length === 2);
    f.client.config.providers = [{ ...provider, ...patch }]; wait.resolve({ entries: [{ path: 'stale', kind: 'file' }] });
    await expect(running).rejects.toBeInstanceOf(ClientError);
    expect(snapshot(f.client, old.admission, f.now()).pathEntries).toEqual([]); expect(f.calls).toHaveLength(2);
  }
});
