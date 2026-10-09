import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mobileArchive, mobileArchiveCommand, projectMobileArchive, type ArchiveSnapshot } from './archive';
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
let previousClient: Partial<typeof mobileClient>, previousEntries: typeof fleet.entries, previousSaved: typeof fleet.saved;
beforeEach(async () => {
  previousClient = { ...mobileClient };
  previousEntries = fleet.entries; previousSaved = fleet.saved;
  requests = []; saved = []; hook = null; permission = ['orchestration:operate'];
  fleet.entries = new Map(); fleet.saved = [];
  await mobileArchive(now, '', '', 'newest', native);
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
  fleet.entries = previousEntries; fleet.saved = previousSaved;
});

describe('archive transport ownership', () => {
  test('fetches all saved environments even with selector, and uses each transport', async () => {
    const view = await mobileArchive(now, '', 'two', 'newest', native);
    expect(view.error).toBe(''); expect(view.environments.map(row => row.id)).toEqual(['two', 'one']);
    expect(threads(view.items).map(row => row.environmentId)).toEqual(['two']);
    const reads = requests.filter(row => row.method === 'orchestration.getArchivedShellSnapshot');
    expect(reads.map(row => row.fleet ?? '').sort()).toEqual(['', 'https://two.example\ntwo']);
    expect(requests.some(row => ['connect', 'fleetConnect'].includes(String(row.op)))).toBe(false);
    expect(mobileClient.environmentId).toBe('one');
  });
  test('partial failure preserves last successful rows read-only while the other archive refreshes', async () => {
    await mobileArchive(now, '', '', 'newest', native);
    hook = request => request.fleet && request.method === 'orchestration.getArchivedShellSnapshot' ? Promise.reject(new Error('offline')) : undefined;
    const view = await mobileArchive(now, '', '', 'newest', native);
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
  const data=await mobileArchive(now,'','two','newest',native);
  expect(data.items[0]!.faviconTarget.cwd).toBe('/archived-only');
  expect(requests.filter(r=>r.method==='orchestration.getArchivedShellSnapshot')).toHaveLength(2);
  expect(requests.filter(r=>r.path==='/api/auth/session')).toHaveLength(2);
  expect(requests.some(r=>r.method==='assets.createUrl')).toBe(false);
 } finally {snapshot.projects=oldProjects;}
});


describe('warm Archive ownership', () => {
  const offline = () => { mobileClient.connection='error';for(const entry of fleet.entries.values())entry.phase='disconnected'; };
  test.each(['forgotten','replaced','ambiguous'])('%s saved identity retires retained project/thread rows', async change => {
    await mobileArchive(now,'','','newest',native);offline();
    if(change==='forgotten')saved=saved.filter(row=>row.environmentId!=='one');
    if(change==='replaced')saved=saved.map(row=>row.environmentId==='one'?{...row,origin:'https://new-home.example'}:row);
    if(change==='ambiguous')saved=[...saved,{...saved[0]!,origin:'https://duplicate.example'}];
    fleet.saved=saved;
    const data=await mobileArchive(now,'','','newest',native);
    expect(data.items.some(row=>row.environmentId==='one')).toBe(false);
    expect(threads(data.items).map(row=>[row.environmentId,row.canOperate])).toEqual([['two',false]]);
    expect(requests.some(row=>row.op==='mobileClientCache')).toBe(false);
  });
  test('disabled/offline fallback retains no grant and issues no read or command', async () => {
    expect(threads((await mobileArchive(now,'','','newest',native)).items).every(row=>row.canOperate)).toBe(true);
    saved=saved.map(row=>({...row,enabled:false}));fleet.saved=saved;requests=[];
    const data=await mobileArchive(now,'','','newest',native);
    expect(threads(data.items)).toHaveLength(2);expect(threads(data.items).every(row=>!row.canOperate)).toBe(true);
    expect(requests.some(row=>row.method==='orchestration.getArchivedShellSnapshot'||row.path==='/api/auth/session')).toBe(false);
    expect((await mobileArchiveCommand('delete','one','p','archived',native)).message).toContain('Connect');
    expect(requests.some(row=>row.method==='orchestration.dispatchCommand'||row.op==='ids')).toBe(false);
  });
  test.each(['environment','global','shell kind','other environment','favicon kind','internal VCS'])('%s clear observes the correct retained display boundary', async scope => {
    await mobileArchive(now,'','','newest',native);offline();
    if(scope==='environment')await mobileCacheClear(native,{environmentId:'one'});
    if(scope==='global')await mobileCacheClear(native);
    if(scope==='shell kind')await mobileCacheClearKind(native,'shell');
    if(scope==='other environment')await mobileCacheClear(native,{environmentId:'unrelated'});
    if(scope==='favicon kind')await mobileCacheClearKind(native,'project-favicon');
    if(scope==='internal VCS')await mobileCacheClear(native,{environmentId:'one',kind:'vcs-refs'},{retainDisplay:true});
    const data=await mobileArchive(now,'','','newest',native);
    expect(threads(data.items).map(row=>row.environmentId).sort()).toEqual(scope==='global'||scope==='shell kind'?[]:scope==='environment'?['two']:['one','two']);
    expect(threads(data.items).every(row=>!row.canOperate)).toBe(true);
  });
  test.each(['catalog','forget','ambiguity','clear'])('held archive reply cannot restore rows after %s changes', async change => {
    saved=saved.slice(0,1);fleet.saved=saved;await mobileArchive(now,'','','newest',native);
    const entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();
    hook=request=>request.method==='orchestration.getArchivedShellSnapshot'?(async()=>{entered.resolve();await release.promise;return{ok:true,generation:3,value:snapshot};})():undefined;
    const pending=mobileArchive(now,'','','newest',native);await entered.promise;
    if(change==='catalog')fleet.saved=saved.map(row=>({...row,origin:'https://replacement.example'}));
    if(change==='forget')fleet.saved=[];
    if(change==='ambiguity')fleet.saved=[...saved,{...saved[0]!}];
    if(change==='clear')await mobileCacheClear(native,{environmentId:'one'});
    release.resolve();const data=await pending;
    expect(data.items).toEqual([]);expect(data.error).not.toBe('');
    hook=null;offline();saved=fleet.saved;
    expect((await mobileArchive(now,'','','newest',native)).items).toEqual([]);
  });
  test('peer completion rechecks successful rows after the focused connection disconnects', async () => {
    const entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>(),authorized=Promise.withResolvers<void>();
    hook=request=>request.fleet&&request.method==='orchestration.getArchivedShellSnapshot'?(async()=>{entered.resolve();await release.promise;return{ok:true,generation:8,value:snapshot};})()
      :!request.fleet&&request.path==='/api/auth/session'?(()=>{authorized.resolve();return{ok:true,generation:3,value:{authenticated:true,permissions:permission}};})():undefined;
    const pending=mobileArchive(now,'','','newest',native);await entered.promise;await authorized.promise;
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
  const older=mobileArchive(now,'','','newest',native);await entered.promise;held=false;
  expect(threads((await mobileArchive(now,'','','newest',native)).items).map(row=>row.threadId)).toEqual(['new']);
  release.resolve();await expect(older).rejects.toMatchObject({kind:'superseded'});
  mobileClient.connection='error';hook=null;
  const retained=threads((await mobileArchive(now,'','','newest',native)).items);
  expect(retained.map(row=>row.threadId)).toEqual(['new']);expect(retained.every(row=>!row.canOperate)).toBe(true);
});
