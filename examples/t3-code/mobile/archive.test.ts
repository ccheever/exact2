import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mobileArchiveCommand, mobileArchiveRead, mobileArchiveScope, mobileArchiveView, projectMobileArchive, type ArchiveSnapshot } from './archive';
import { answer } from './app';
import { mobileClient } from './client';
import { initialShell, obj, type Obj } from './shared/domain';
import { fleet, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileCacheClear, mobileCacheClearKind } from './mobile-client-cache';

const now = Date.parse('2026-10-07T12:00:00Z');
const at = (minutes: number) => new Date(now + minutes * 60_000).toISOString();
const thread = (id: string, patch: Obj = {}): Obj => ({ id, projectId: 'p', title: id, archivedAt: at(-10), updatedAt: at(-1), branch: 'feature', ...patch });
const source = (environmentId: string, threads: Obj[]): ArchiveSnapshot => ({ environmentId, label: `Machine ${environmentId}`, machine: 'laptop', canOperate: true,
  shell: { ...initialShell(), projects: [{ id: 'p', title: 'Project', workspaceRoot: '/workspace/repo' }], threads } });
const threads = (items: ReturnType<typeof projectMobileArchive>) => items.filter(row => row.kind === 'thread');

// Test orchestration only: production owns the reader and projection separately.
async function readArchiveView(now: number, query = '', environment = '', sort = 'newest', input?: Native | null) {
  const prepared = await mobileArchiveRead(input?.available ? '@test' : '', mobileArchiveScope().key, input);
  return mobileArchiveView(now, query, environment, sort, !!input?.available, prepared);
}

describe('pinned archive grouping', () => {
  test('environment/project identities remain scoped; newest and oldest sort groups by their first row', () => {
    const input = [source('one', [thread('same', { archivedAt: at(-5) }), thread('early', { archivedAt: at(-20) }), thread('live', { archivedAt: null })]), source('two', [thread('same', { archivedAt: at(-10) })])];
    const before = JSON.stringify(input);
    expect(threads(projectMobileArchive(input, now)).map(row => row.key)).toEqual(['one:same', 'one:early', 'two:same']);
    expect(threads(projectMobileArchive(input, now, '', '', 'oldest')).map(row => row.key)).toEqual(['one:early', 'one:same', 'two:same']);
    expect(JSON.stringify(input)).toBe(before);
    expect(projectMobileArchive(input, now)[0]?.initial).toBe(true);
    expect(projectMobileArchive(input, now).at(-1)?.final).toBe(true);
  });
  test('project/path/environment query keeps whole groups; title and branch query narrows threads', () => {
    const input = [source('one', [thread('Needle', { branch: 'main' }), thread('Other', { branch: 'branch-needle' })]), source('two', [thread('Other')])];
    for (const query of [' PROJECT ', '/WORKSPACE', 'machine one']) expect(threads(projectMobileArchive(input.slice(0, 1), now, query))).toHaveLength(2);
    expect(threads(projectMobileArchive(input, now, 'needle')).map(row => row.title)).toEqual(['Needle', 'Other']);
    expect(threads(projectMobileArchive(input, now, 'main')).map(row => row.title)).toEqual(['Needle']);
    expect(threads(projectMobileArchive(input, now, '', 'two')).map(row => row.environmentId)).toEqual(['two']);
  });
  test('invalid dates sort at zero; equal dates tie on title then id; cards and labels match source', () => {
    const input = source('one', [thread('b', { title: 'Same' }), thread('a', { title: 'Same' }), thread('bad', { archivedAt: 'invalid' })]);
    const rows = threads(projectMobileArchive([input], now));
    expect(rows.map(row => row.threadId)).toEqual(['a', 'b', 'bad']);
    expect(rows.map(row => [row.first, row.last])).toEqual([[true, false], [false, false], [false, true]]);
    expect(rows[0]?.subtitle).toBe('Machine one · feature');
  });
  test('subagent titles are presented before Archive matching and sorting without changing the shell', () => {
    const raw = '/root/framework_press_421_main_audit';
    const input = source('one', [thread('child', { title: raw, lineage: { relationshipToParent: 'subagent' } }),
      thread('ordinary', { title: 'Earlier title' })]);
    const before = JSON.stringify(input);
    const rows = threads(projectMobileArchive([input], now));
    expect(rows.map(row => [row.threadId, row.title])).toEqual([
      ['ordinary', 'Earlier title'], ['child', 'Framework Press 421 Main Audit'],
    ]);
    expect(threads(projectMobileArchive([input], now, 'Framework Press')).map(row => row.threadId)).toEqual(['child']);
    expect(threads(projectMobileArchive([input], now, '/root/'))).toEqual([]);
    expect(threads(projectMobileArchive([input], now, 'feature'))).toHaveLength(2);
    expect(threads(projectMobileArchive([input], now, 'Project'))).toHaveLength(2);
    expect(JSON.stringify(input)).toBe(before);
  });
  test('title presentation preserves ordinary and fork paths, and follows the exact subagent word rules', () => {
    const cases = [
      ['user', '/root/user_title', { relationshipToParent: null }, '/root/user_title'],
      ['fork', '/root/fork_title', { relationshipToParent: 'fork' }, '/root/fork_title'],
      ['missing', '/root/literal_  composed name', undefined, '/root/literal_  composed name'],
      ['nested', 'Subagent: /root/parent/élève_  task-name/', { relationshipToParent: 'subagent' }, 'Élève Task-name'],
      ['named', 'Subagent: Keep_this Name', { relationshipToParent: 'subagent' }, 'Keep_this Name'],
      ['other-path', '/tmp/keep_this', { relationshipToParent: 'subagent' }, '/tmp/keep_this'],
    ] as const;
    const input = source('one', cases.map(([id, title, lineage]) => thread(id, { title, lineage })));
    const titles = new Map(threads(projectMobileArchive([input], now)).map(row => [row.threadId, row.title]));
    for (const [id, , , expected] of cases) expect(titles.get(id)).toBe(expected);
  });
});

let requests: Obj[], saved: Obj[], permission: string[], hook: ((request: Obj) => unknown) | null;
const snapshot = { snapshotSequence: 4, projects: [{ id: 'p', title: 'Project' }], threads: [thread('archived')] };
const native: Native = { available: true, watch() {}, async later(input) {
  const request = obj(input); requests.push(request);
  const generation = request.fleet ? 8 : mobileClient.generation;
  const custom = hook?.(request); if (custom !== undefined) return await custom;
  if (request.op === 'mobileClientCache') return { ok: true, generation: 0, value: { removed: 0 } };
  if (request.op === 'environments') return { ok: true, generation, value: { saved } };
  if (request.op === 'http') return { ok: true, generation, value: { authenticated: true, permissions: permission } };
  if (request.op === 'ids') return { ok: true, generation, value: ['command-from-native'] };
  if (request.method === 'orchestration.getArchivedShellSnapshot') return { ok: true, generation, value: snapshot };
  if (request.method === 'orchestration.dispatchCommand') return { ok: true, generation, value: {} };
  throw new Error(`Unexpected operation ${JSON.stringify(request)}`);
} };
let previousClient: Partial<typeof mobileClient>, previousEntries: typeof fleet.entries, previousSaved: typeof fleet.saved, previousFleetRevision: number;
beforeEach(async () => {
  previousClient = { ...mobileClient };
  previousEntries = fleet.entries; previousSaved = fleet.saved; previousFleetRevision = fleet.revision;
  requests = []; saved = []; hook = null; permission = ['orchestration:operate'];
  fleet.entries = new Map(); fleet.saved = [];
  await readArchiveView(now, '', '', 'newest', native);
  mobileClient.environmentId = 'one'; mobileClient.origin = 'https://one.example'; mobileClient.generation = 3;
  mobileClient.connection = 'connected'; mobileClient.configLive = true; mobileClient.shellLive = true; mobileClient.threadId = '';
  const entry: FleetEntry = { key: 'https://two.example\ntwo', origin: 'https://two.example', environmentId: 'two', phase: 'connected', message: '', traceId: '', generation: 8,
    synchronized: 8, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true };
  fleet.entries.set(entry.key, entry);
  saved = [{ environmentId: 'one', label: 'Zed', origin: mobileClient.origin, enabled: true }, { environmentId: 'two', label: 'Alpha', origin: entry.origin, enabled: true }];
  fleet.saved = saved; requests = [];
});

afterEach(() => {
  Object.assign(mobileClient, previousClient);
  fleet.entries = previousEntries; fleet.saved = previousSaved; fleet.revision = previousFleetRevision;
});

describe('archive transport ownership', () => {
  test('native Archive snapshots use subagent presentation for dispatcher search and row titles', async () => {
    const previousThreads = snapshot.threads;
    const raw = '/root/framework_press_421_main_audit';
    snapshot.threads = [thread('child', { title: raw, lineage: { relationshipToParent: 'subagent' } })];
    try {
      const prepared = await mobileArchiveRead('subagent-title', mobileArchiveScope().key, native);
      const view = answer('archiveView', [now, 'Framework Press', 'one', 'newest', true, prepared]);
      expect(threads(view.items)).toMatchObject([{ threadId: 'child', title: 'Framework Press 421 Main Audit', canOperate: true }]);
      expect(snapshot.threads[0]!.title).toBe(raw);
    } finally { snapshot.threads = previousThreads; }
  });
  test('fetches all saved environments even with selector, and uses each transport', async () => {
    const view = await readArchiveView(now, '', 'two', 'newest', native);
    expect(view.error).toBe(''); expect(view.environments.map(row => row.id)).toEqual(['two', 'one']);
    expect(threads(view.items).map(row => row.environmentId)).toEqual(['two']);
    const reads = requests.filter(row => row.method === 'orchestration.getArchivedShellSnapshot');
    expect(reads.map(row => row.fleet ?? '').sort()).toEqual(['', 'https://two.example\ntwo']);
    expect(requests.some(row => ['connect', 'fleetConnect'].includes(String(row.op)))).toBe(false);
    expect(mobileClient.environmentId).toBe('one');
  });
  test('partial failure preserves last successful rows read-only while the other archive refreshes', async () => {
    await readArchiveView(now, '', '', 'newest', native);
    hook = request => request.fleet && request.method === 'orchestration.getArchivedShellSnapshot' ? Promise.reject(new Error('offline')) : undefined;
    const view = await readArchiveView(now, '', '', 'newest', native);
    expect(view.error).toBe('Failed to load archived threads.');
    expect(threads(view.items).map(row => [row.environmentId, row.canOperate]).sort()).toEqual([['one', true], ['two', false]]);
  });
  test('background unarchive dispatches once with native command id and never changes focus', async () => {
    const answer = await mobileArchiveCommand('unarchive', 'two', 'p', 'archived', native);
    expect(answer.message).toBe('');
    const writes = requests.filter(row => row.method === 'orchestration.dispatchCommand');
    expect(writes).toHaveLength(1);
    expect(writes[0]?.fleet).toBe('https://two.example\ntwo'); expect(writes[0]?.generation).toBe(8);
    expect(writes[0]?.payload).toEqual({ type: 'thread.unarchive', commandId: 'command-from-native', threadId: 'archived' });
    expect(mobileClient.environmentId).toBe('one'); expect(mobileClient.origin).toBe('https://one.example');
  });
  test('explicit empty permissions, disabled saved entries and wrong project prevent dispatch', async () => {
    permission = [];
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('permission');
    permission = ['orchestration:operate']; saved[1]!.enabled = false;
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('Connect');
    saved[1]!.enabled = true;
    expect((await mobileArchiveCommand('delete', 'two', 'wrong', 'archived', native)).message).toContain('no longer available');
    expect(requests.some(row => row.method === 'orchestration.dispatchCommand')).toBe(false);
  });
  test('a generation change while authorizing cannot dispatch on the replacement connection', async () => {
    hook = request => {
      if (request.op !== 'http') return undefined;
      fleet.entries.get('https://two.example\ntwo')!.generation = 9;
      return { ok: true, generation: 8, value: { authenticated: true, permissions: permission } };
    };
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('connection changed');
    expect(requests.some(row => row.op === 'ids' || row.method === 'orchestration.dispatchCommand')).toBe(false);
  });
  test('uncertain background write is reported and never replayed', async () => {
    hook = request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('socket closed')) : undefined;
    const answer = await mobileArchiveCommand('delete', 'two', 'p', 'archived', native);
    expect(answer.message).toContain('may have reached');
    expect(requests.filter(row => row.method === 'orchestration.dispatchCommand')).toHaveLength(1);
  });
  test('concurrent duplicate row actions allocate and dispatch only once', async () => {
    let release!: () => void; const wait = new Promise<void>(resolve => { release = resolve; });
    hook = request => request.op === 'http' ? wait.then(() => ({ ok: true, generation: request.fleet ? 8 : 3, value: { authenticated: true, permissions: permission } })) : undefined;
    const first = mobileArchiveCommand('delete', 'one', 'p', 'archived', native);
    const second = await mobileArchiveCommand('delete', 'one', 'p', 'archived', native);
    expect(second.message).toContain('already running'); release();
    expect((await first).message).toBe('');
    expect(requests.filter(row => row.method === 'orchestration.dispatchCommand')).toHaveLength(1);
  });
});


test('archive emits targets from filtered archived project headers without signed URL requests', async () => {
 const input=[source('one',[thread('one')]),source('two',[thread('two')])];
 input[1]!.shell.projects[0]!.workspaceRoot='/archived-only';input[1]!.shell.projects[0]!.faviconPath='icon path.svg';
 const rows=projectMobileArchive(input,now,'','two');
 expect(rows[0]).toMatchObject({kind:'project',favicon:'',faviconTarget:{environmentId:'two',cwd:'/archived-only',faviconPath:'icon path.svg',key:'["two","/archived-only","icon path.svg"]'}});
 expect(rows[1]!.faviconTarget.key).toBe('');
 input[1]!.shell.projects[0]!.projectIcon={kind:'monogram',text:'A',color:'blue'};
 expect(projectMobileArchive(input,now,'','two')[0]).toMatchObject({iconKind:'monogram',faviconTarget:{key:''}});
 const oldProjects=snapshot.projects;
 snapshot.projects=[{id:'p',title:'Project',workspaceRoot:'/archived-only'}] as typeof snapshot.projects;
 try {
  const data=await readArchiveView(now,'','two','newest',native);
  expect(data.items[0]!.faviconTarget.cwd).toBe('/archived-only');
  expect(requests.filter(r=>r.method==='orchestration.getArchivedShellSnapshot')).toHaveLength(2);
  expect(requests.filter(r=>r.path==='/api/auth/session')).toHaveLength(2);
  expect(requests.some(r=>r.method==='assets.createUrl')).toBe(false);
 } finally {snapshot.projects=oldProjects;}
});


describe('warm Archive ownership', () => {
  const offline = () => { mobileClient.connection='error';for(const entry of fleet.entries.values())entry.phase='disconnected'; };
  test.each(['forgotten','replaced','ambiguous'])('%s saved identity retires retained project/thread rows', async change => {
    await readArchiveView(now,'','','newest',native);offline();
    if(change==='forgotten')saved=saved.filter(row=>row.environmentId!=='one');
    if(change==='replaced')saved=saved.map(row=>row.environmentId==='one'?{...row,origin:'https://new-home.example'}:row);
    if(change==='ambiguous')saved=[...saved,{...saved[0]!,origin:'https://duplicate.example'}];
    fleet.saved=saved;
    const data=await readArchiveView(now,'','','newest',native);
    expect(data.items.some(row=>row.environmentId==='one')).toBe(false);
    expect(threads(data.items).map(row=>[row.environmentId,row.canOperate])).toEqual([['two',false]]);
    expect(requests.some(row=>row.op==='mobileClientCache')).toBe(false);
  });
  test('disabled/offline fallback retains no grant and issues no read or command', async () => {
    expect(threads((await readArchiveView(now,'','','newest',native)).items).every(row=>row.canOperate)).toBe(true);
    saved=saved.map(row=>({...row,enabled:false}));fleet.saved=saved;requests=[];
    const data=await readArchiveView(now,'','','newest',native);
    expect(threads(data.items)).toHaveLength(2);expect(threads(data.items).every(row=>!row.canOperate)).toBe(true);
    expect(requests.some(row=>row.method==='orchestration.getArchivedShellSnapshot'||row.path==='/api/auth/session')).toBe(false);
    expect((await mobileArchiveCommand('delete','one','p','archived',native)).message).toContain('Connect');
    expect(requests.some(row=>row.method==='orchestration.dispatchCommand'||row.op==='ids')).toBe(false);
  });
  test.each(['environment','global','shell kind','other environment','favicon kind','internal VCS'])('%s clear observes the correct retained display boundary', async scope => {
    await readArchiveView(now,'','','newest',native);offline();
    if(scope==='environment')await mobileCacheClear(native,{environmentId:'one'});
    if(scope==='global')await mobileCacheClear(native);
    if(scope==='shell kind')await mobileCacheClearKind(native,'shell');
    if(scope==='other environment')await mobileCacheClear(native,{environmentId:'unrelated'});
    if(scope==='favicon kind')await mobileCacheClearKind(native,'project-favicon');
    if(scope==='internal VCS')await mobileCacheClear(native,{environmentId:'one',kind:'vcs-refs'},{retainDisplay:true});
    const data=await readArchiveView(now,'','','newest',native);
    expect(threads(data.items).map(row=>row.environmentId).sort()).toEqual(scope==='global'||scope==='shell kind'?[]:scope==='environment'?['two']:['one','two']);
    expect(threads(data.items).every(row=>!row.canOperate)).toBe(true);
  });
  test.each(['catalog','forget','ambiguity','clear'])('held archive reply cannot restore rows after %s changes', async change => {
    saved=saved.slice(0,1);fleet.saved=saved;await readArchiveView(now,'','','newest',native);
    const entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();
    hook=request=>request.method==='orchestration.getArchivedShellSnapshot'?(async()=>{entered.resolve();await release.promise;return{ok:true,generation:3,value:snapshot};})():undefined;
    const pending=readArchiveView(now,'','','newest',native);await entered.promise;
    if(change==='catalog')fleet.saved=saved.map(row=>({...row,origin:'https://replacement.example'}));
    if(change==='forget')fleet.saved=[];
    if(change==='ambiguity')fleet.saved=[...saved,{...saved[0]!}];
    if(change==='clear')await mobileCacheClear(native,{environmentId:'one'});
    release.resolve();const data=await pending;
    expect(data.items).toEqual([]);expect(data.error).not.toBe('');
    hook=null;offline();saved=fleet.saved;
    expect((await readArchiveView(now,'','','newest',native)).items).toEqual([]);
  });
  test('peer completion rechecks successful rows after the focused connection disconnects', async () => {
    const entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>(),authorized=Promise.withResolvers<void>();
    hook=request=>request.fleet&&request.method==='orchestration.getArchivedShellSnapshot'?(async()=>{entered.resolve();await release.promise;return{ok:true,generation:8,value:snapshot};})()
      :!request.fleet&&request.path==='/api/auth/session'?(()=>{authorized.resolve();return{ok:true,generation:3,value:{authenticated:true,permissions:permission}};})():undefined;
    const pending=readArchiveView(now,'','','newest',native);await entered.promise;await authorized.promise;
    mobileClient.connection='error';release.resolve();const data=await pending;
    expect(threads(data.items).filter(row=>row.environmentId==='one').every(row=>!row.canOperate)).toBe(true);
    expect(threads(data.items).find(row=>row.environmentId==='two')?.canOperate).toBe(true);
  });
  test('catalog replacement during authorization prevents ids and archive mutation dispatch', async () => {
    hook=request=>request.path==='/api/auth/session'?(()=>{fleet.saved=fleet.saved.map(row=>row.environmentId==='one'?{...row,origin:'https://replacement.example'}:row);return{ok:true,generation:3,value:{authenticated:true,permissions:permission}};})():undefined;
    expect((await mobileArchiveCommand('delete','one','p','archived',native)).message).toContain('connection changed');
    expect(requests.some(row=>row.op==='ids'||row.method==='orchestration.dispatchCommand')).toBe(false);
  });
});


test('superseded archive answer cannot overwrite a newer successful warm snapshot', async () => {
  saved=saved.slice(0,1);fleet.saved=saved;
  const entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let held=true;
  hook=request=>request.method==='orchestration.getArchivedShellSnapshot'?(held?(async()=>{entered.resolve();await release.promise;return{ok:true,generation:3,value:{...snapshot,threads:[thread('old')]}};})():{ok:true,generation:3,value:{...snapshot,threads:[thread('new')]}}):undefined;
  const older=readArchiveView(now,'','','newest',native);await entered.promise;held=false;
  expect(threads((await readArchiveView(now,'','','newest',native)).items).map(row=>row.threadId)).toEqual(['new']);
  release.resolve();await expect(older).rejects.toMatchObject({kind:'superseded'});
  mobileClient.connection='error';hook=null;
  const retained=threads((await readArchiveView(now,'','','newest',native)).items);
  expect(retained.map(row=>row.threadId)).toEqual(['new']);expect(retained.every(row=>!row.canOperate)).toBe(true);
});

describe('stable Archive reader and current projection', () => {
  test('held source read survives unrelated revisions, labels, filters and clock without native watches or new IO', async () => {
    const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
    const watched: string[] = [], scoped: Native = { ...native, watch: topic => watched.push(topic) };
    hook = request => !request.fleet && request.method === 'orchestration.getArchivedShellSnapshot'
      ? (async () => { entered.resolve(); await release.promise; return { ok: true, generation: 3, value: snapshot }; })() : undefined;
    const scope = mobileArchiveScope(), pending = answer('archiveRead', ['visit-1', scope.key, 0], undefined, undefined, scoped);
    await entered.promise;
    const before = requests.length;
    for (let i = 1; i <= 20; i++) {
      mobileClient.revision++; fleet.revision++;
      saved = saved.map(row => row.environmentId === 'one' ? { ...row, mobileLabel: `Current ${i}` } : row); fleet.saved = saved;
      const admitted = answer('archiveScope', [mobileClient.revision, 'one', 3, true, 'connected', []]);
      expect(admitted.key).toBe(scope.key);
      const view = answer('archiveView', [now + i * 60_000, i % 2 ? 'missing' : '', i % 3 ? '' : 'one', i % 2 ? 'oldest' : 'newest', true, { serial: 0, receipts: [], error: '' }, admitted, true], undefined, undefined, scoped);
      expect(view.environments.find(row => row.id === 'one')?.label).toBe(`Current ${i}`);
    }
    expect(requests).toHaveLength(before); expect(watched).toEqual([]);
    release.resolve(); const prepared = await pending;
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, prepared).items).every(row => row.canOperate)).toBe(true);
    expect(requests.filter(row => row.method === 'orchestration.getArchivedShellSnapshot')).toHaveLength(2);
    expect(requests.filter(row => row.path === '/api/auth/session')).toHaveLength(2);
    expect(JSON.stringify(prepared)).not.toContain('archivedAt');
  });
  test('empty completed answer projects stably and filtering never rereads', async () => {
    hook = request => request.method === 'orchestration.getArchivedShellSnapshot' ? { ok: true, generation: request.fleet ? 8 : 3, value: { ...snapshot, threads: [] } } : undefined;
    const prepared = await mobileArchiveRead('empty', mobileArchiveScope().key, native), before = requests.length;
    for (const [query, environment, sort] of [['', '', 'newest'], ['missing', '', 'oldest'], ['', 'one', 'newest']]) {
      const view = mobileArchiveView(now, query, environment, sort, true, prepared);
      expect(view.items).toEqual([]); expect(view.loading).toBe(false); expect(view.error).toBe('');
      expect(view.emptyTitle).toBe(query || environment ? 'No matching threads' : 'No archived threads');
    }
    expect(requests).toHaveLength(before);
  });
  test.each(['generation', 'disconnect', 'disabled', 'producer', 'focus'])('%s changes admission and retires a captured grant without native IO', async kind => {
    const scope = mobileArchiveScope(), prepared = await mobileArchiveRead('grant', scope.key, native), before = requests.length;
    const entry = fleet.entries.get('https://two.example\ntwo')!;
    if (kind === 'generation') mobileClient.generation++;
    if (kind === 'disconnect') mobileClient.connection = 'disconnected';
    if (kind === 'disabled') { fleet.saved = saved.map(row => row.environmentId === 'one' ? { ...row, enabled: false } : row); saved = fleet.saved; }
    if (kind === 'producer') fleet.entries.set(entry.key, { ...entry });
    if (kind === 'focus') { mobileClient.environmentId = 'two'; mobileClient.origin = entry.origin; mobileClient.generation = 8; }
    expect(mobileArchiveScope().key).not.toBe(scope.key);
    const view = mobileArchiveView(now, '', '', 'newest', true, prepared);
    const affected = kind === 'producer' ? 'two' : 'one';
    expect(threads(view.items).filter(row => row.environmentId === affected).every(row => !row.canOperate)).toBe(true);
    expect(requests).toHaveLength(before);
    if (kind === 'disconnect') { mobileClient.connection = 'connected'; expect(threads(mobileArchiveView(now, '', 'one', 'newest', true, prepared).items).every(row => !row.canOperate)).toBe(true); }
  });
  test('catalog permission request cannot adopt a generation that changed while catalog read was held', async () => {
    const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
    hook = request => request.op === 'environments' ? (async () => { entered.resolve(); await release.promise; return { ok: true, generation: 3, value: { saved } }; })() : undefined;
    const pending = mobileArchiveRead('catalog', mobileArchiveScope().key, native); await entered.promise;
    mobileClient.generation++; release.resolve(); const prepared = await pending;
    expect(prepared.error).not.toBe('');
    expect(requests.some(row => !row.fleet && (row.method === 'orchestration.getArchivedShellSnapshot' || row.path === '/api/auth/session'))).toBe(false);
  });
  test('a departed visit cancels publication; fresh visit retries, with no promise or handle shared', async () => {
    const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(); let held = true;
    hook = request => !request.fleet && request.method === 'orchestration.getArchivedShellSnapshot' && held
      ? (async () => { entered.resolve(); await release.promise; throw { name: 'FetchError', kind: 'Aborted', message: 'departed' }; })() : undefined;
    const scope = mobileArchiveScope(), pending = mobileArchiveRead('old', scope.key, native); await entered.promise;
    await mobileArchiveRead('', scope.key, native); release.resolve(); await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    expect(mobileArchiveView(now, '', '', 'newest', false).items).toEqual([]);
    held = false; const prepared = await mobileArchiveRead('new', mobileArchiveScope().key, native);
    expect(prepared.error).toBe(''); expect(threads(mobileArchiveView(now, '', '', 'newest', true, prepared).items)).toHaveLength(2);
  });
  test('new explicit read never retains the preceding authorization on a failed session', async () => {
    const scope = mobileArchiveScope(), first = await mobileArchiveRead('live', scope.key, native);
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, first).items).every(row => row.canOperate)).toBe(true);
    permission = []; const second = await mobileArchiveRead('live', scope.key, native);
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, second).items).every(row => !row.canOperate)).toBe(true);
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, first).items).every(row => !row.canOperate)).toBe(true);
  });
  test('pure projection retires scoped cache-clear rows before another read', async () => {
    const prepared = await mobileArchiveRead('clear', mobileArchiveScope().key, native);
    const before = mobileArchiveScope(); await mobileCacheClear(native, { environmentId: 'one' }); const calls = requests.length;
    expect(mobileArchiveScope().key).not.toBe(before.key);
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, prepared).items).map(row => row.environmentId)).toEqual(['two']);
    expect(requests).toHaveLength(calls);
  });
  test.each(['rpc', 'permission'])('real Aborted at %s cannot publish an error or grant and fresh owner retries', async phase => {
    saved = saved.slice(0, 1); fleet.saved = saved;
    hook = request => (phase === 'rpc' ? request.method === 'orchestration.getArchivedShellSnapshot' : request.path === '/api/auth/session')
      ? Promise.reject({ name: 'FetchError', kind: 'Aborted', message: 'retired answer' }) : undefined;
    await expect(mobileArchiveRead('aborted', mobileArchiveScope().key, native)).rejects.toMatchObject({ kind: 'superseded' });
    expect(mobileArchiveView(now, '', '', 'newest', true).items).toEqual([]);
    hook = null; const prepared = await mobileArchiveRead('retry', mobileArchiveScope().key, native);
    expect(prepared.error).toBe(''); expect(threads(mobileArchiveView(now, '', '', 'newest', true, prepared).items)[0]?.canOperate).toBe(true);
  });
});


describe('Archive idle retention', () => {
  test('the last observer starts five minutes, then actual retained data and grants are removed', async () => {
    const prepared = await mobileArchiveRead('idle-retention', mobileArchiveScope().key, native);
    expect(threads(mobileArchiveView(now, '', '', 'newest', true, prepared, true).items)).toHaveLength(2);
    mobileArchiveView(now + 17, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 300_016, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 300_017, '', '', 'newest', false, prepared, false);
    // Re-observation resets the idle marker; deleted snapshots cannot reappear.
    expect(mobileArchiveView(now + 300_018, '', '', 'newest', true, prepared, true).items).toEqual([]);
    const fresh = await mobileArchiveRead('idle-retention-return', mobileArchiveScope().key, native);
    expect(threads(mobileArchiveView(now + 300_019, '', '', 'newest', true, fresh, true).items))
      .toMatchObject([{ canOperate: true }, { canOperate: true }]);
  });
  test('covered and returning observers retain snapshots, then a new last removal receives a full lifetime', async () => {
    const prepared = await mobileArchiveRead('idle-retention-covered', mobileArchiveScope().key, native);
    mobileArchiveView(now, '', '', 'newest', true, prepared, true);
    mobileArchiveView(now + 700_000, '', '', 'newest', false, prepared, true);
    expect(threads(mobileArchiveView(now + 700_001, '', '', 'newest', true, prepared, true).items)).toHaveLength(2);
    mobileArchiveView(now + 700_010, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 999_999, '', '', 'newest', true, prepared, true);
    mobileArchiveView(now + 1_000_010, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 1_000_011, '', '', 'newest', false, prepared, false);
    expect(threads(mobileArchiveView(now + 1_000_012, '', '', 'newest', true, prepared, true).items)).toHaveLength(2);
    mobileArchiveView(now + 1_000_020, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 1_300_019, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 1_300_020, '', '', 'newest', false, prepared, false);
    expect(mobileArchiveView(now + 1_300_021, '', '', 'newest', true, prepared, true).items).toEqual([]);
  });
  test('a read issued before eviction cannot restore expired snapshots after its late response', async () => {
    const prepared = await mobileArchiveRead('idle-retention-old', mobileArchiveScope().key, native);
    mobileArchiveView(now, '', '', 'newest', true, prepared, true);
    let release!: () => void, reached!: () => void;
    const wait = new Promise<void>(resolve => { release = resolve; });
    const entered = new Promise<void>(resolve => { reached = resolve; });
    hook = request => {
      if (request.method !== 'orchestration.getArchivedShellSnapshot') return undefined;
      reached();
      return wait.then(() => ({ ok: true, generation: request.fleet ? 8 : 3, value: snapshot }));
    };
    const pending = mobileArchiveRead('idle-retention-held', mobileArchiveScope().key, native).then(() => null, error => error);
    await entered;
    mobileArchiveView(now + 10, '', '', 'newest', false, prepared, false);
    mobileArchiveView(now + 300_010, '', '', 'newest', false, prepared, false);
    release();
    expect((await pending)?.message).toContain('superseded');
    expect(mobileArchiveView(now + 300_011, '', '', 'newest', true, prepared, true).items).toEqual([]);
  });
  test('the generated source forwards mounted observation independently of foreground visibility', async () => {
    const prepared = await mobileArchiveRead('idle-retention-dispatcher', mobileArchiveScope().key, native);
    answer('archiveView', [now, '', '', 'newest', true, prepared, mobileArchiveScope(), true]);
    answer('archiveView', [now + 400_000, '', '', 'newest', false, prepared, mobileArchiveScope(), true]);
    expect(answer('archiveView', [now + 400_001, '', '', 'newest', true, prepared, mobileArchiveScope(), true]).items).not.toEqual([]);
    answer('archiveView', [now + 400_010, '', '', 'newest', false, prepared, mobileArchiveScope(), false]);
    answer('archiveView', [now + 700_010, '', '', 'newest', false, prepared, mobileArchiveScope(), false]);
    expect(answer('archiveView', [now + 700_011, '', '', 'newest', true, prepared, mobileArchiveScope(), true]).items).toEqual([]);
  });
});
