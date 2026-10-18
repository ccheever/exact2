import { afterEach, beforeEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { fleet } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import { composerNow, noteNow } from './shared/composer-controls';
import type { Native } from './shared/protocol';
import { projectMobileHome } from './home';
import { mobileNewTaskChooser } from './new-task';
import { mobileRootFaviconAdmission, mobileRootFavicons, mobileRootFaviconImages, mobileRootFaviconEvent } from './mobile-favicon-root';
import { mobileCacheClear } from './mobile-client-cache';
import { liveEvent } from './shared/live-streams';

let original: Partial<typeof mobileClient>, saved: typeof fleet.saved, entries: typeof fleet.entries;
beforeEach(() => { original = { ...mobileClient }; saved = fleet.saved; entries = fleet.entries; fleet.entries = new Map(); });
afterEach(async () => {
  const now = Math.max(Date.now(), composerNow(mobileClient));
  mobileRootFaviconAdmission(['none', 'cleanup', false, false, [], [], [], [], {}, now]);
  mobileRootFaviconAdmission(['none', 'expired', false, false, [], [], [], [], {}, now + 3_600_001]);
  Object.assign(mobileClient, original); fleet.saved = saved; fleet.entries = entries;
});
let sequence = 0;
function fixture() {
  const environmentId = `favicon-root-${++sequence}`, origin = `https://${environmentId}.invalid`;
  const now = Math.max(Date.now(), composerNow(mobileClient));
  noteNow(mobileClient, now);
  Object.assign(mobileClient, { environmentId, origin, generation: 9, connection: 'connected', shellLoaded: true,
    shellLive: true, configLive: true, config: {}, shell: { ...initialShell(),
      projects: [{ id: 'p', title: 'Repo', workspaceRoot: '/work' }],
      threads: [{ id: 't', title: 'Thread', projectId: 'p', archivedAt: null }] } });
  fleet.saved = [{ environmentId, origin }];
  const calls: Obj[] = [], state = { response: async () => ({ relativeUrl: '/icon.png' }), image: 'data:image/png;base64,QQ==' };
  const native: Native = { available: true, watch() {}, async later(input) {
    const r = obj(input); calls.push(r);
    if (r.op === 'request') return { ok: true, generation: 9, value: await state.response() };
    if (r.op === 'mobileFaviconImage') return { ok: true, generation: 0, value: { dataUrl: state.image } };
    if (r.op === 'mobileClientCache') return { ok: true, generation: 0, value: r.action === 'list' ? { rows: [] }
      : r.action === 'read' ? { record: null } : r.action === 'ticket' ? { ticket: '1' }
      : r.action === 'write' ? { written: true, stale: false } : { removed: 0 } };
    throw Error(`Unexpected ${r.op}`);
  } };
  const home = projectMobileHome([{ environmentId, focused: true, label: 'Env', machine: 'laptop',
    config: {}, shell: mobileClient.shell }], Date.now()).items;
  const projects = mobileNewTaskChooser('', 'repository', mobileClient, fleet).projects;
  const args = (route = 'home', routeId = 'visit-a', sidebar = true) => [route, routeId, route === 'home', sidebar,
    home, home, projects, [], {}, now];
  const admit = (values = args()) => mobileRootFaviconAdmission(values);
  const run = (values = args(), bridge: Native | null = null) => { const admission = admit(values); return mobileRootFavicons(admission.owner, bridge); };
  const seed = () => mobileClient.rpc(native, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd: '/work' } });
  return { args, admit, run, seed, calls, native, state, environmentId, origin };
}

test('actual Home and persistent sidebar own separate callbacks; leaving Home preserves the sidebar request', async () => {
  const f = fixture(); await f.seed(); await f.run();
  const initial = mobileRootFaviconImages().items;
  expect(initial).toHaveLength(2); expect(initial.every(item => item.url === `${f.origin}/icon.png`)).toBe(true);
  const home = initial.find(item => item.mountId.includes(':home-list:'))!;
  const sidebar = initial.find(item => item.mountId.startsWith('sidebar-list:'))!;
  await f.run(f.args('thread', 'visit-b'));
  expect(mobileRootFaviconImages().items[0]!.requestKey).toBe(sidebar.requestKey);
  mobileRootFaviconEvent(home.mountId, home.requestKey, home.url, 'load');
  expect(mobileRootFaviconImages().items[0]!.loaded).toBe(false);
  mobileRootFaviconEvent(sidebar.mountId, sidebar.requestKey, sidebar.url, 'load');
  expect(mobileRootFaviconImages().items[0]!.loaded).toBe(true);
  expect(f.calls.filter(call => call.op === 'request')).toHaveLength(1);
});

test('a new chooser visit cannot accept the previous view callback for the same project and URL', async () => {
  const f = fixture(); await f.seed(); await f.run(f.args('newTask', 'first', false));
  const old = mobileRootFaviconImages().items[0]!;
  await f.run(f.args('newTask', 'replacement', false));
  const fresh = mobileRootFaviconImages().items[0]!;
  expect(fresh.requestKey).not.toBe(old.requestKey);
  mobileRootFaviconEvent(old.mountId, old.requestKey, old.url, 'error');
  expect(mobileRootFaviconImages().items[0]!.failed).toBe(false);
});

test('catalog replacement immediately removes stale image projection before the next awaited preparation', async () => {
  const f = fixture(); await f.seed(); await f.run();
  expect(mobileRootFaviconImages().items.some(item => item.url)).toBe(true);
  fleet.saved = [{ environmentId: f.environmentId, origin: 'https://replacement.invalid' }];
  f.admit(); // Mirrors the synchronous Contract demand resource, before async preparation.
  expect(mobileRootFaviconImages().items.every(item => !item.url)).toBe(true);
});

test('superseded root preparation cannot restore a departed surface after its actual request finishes', async () => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/late.png' }; };
  const preparing = f.run(f.args(), f.native); await entered.promise;
  mobileRootFaviconAdmission(['none', 'gone', false, false, [], [], [], [], {}, Date.now()]);
  release.resolve(); await expect(preparing).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileRootFaviconImages().items).toEqual([]);
  expect(f.calls.some(call => call.op === 'mobileFaviconImage')).toBe(false);
});


test('synchronous admission displays a cached image while the new visit live lookup is held', async () => {
  const f = fixture(); await f.run(f.args(), f.native);
  const inline = 'data:image/png;base64,QQ==';
  expect(mobileRootFaviconImages().items).toHaveLength(2);
  expect(mobileRootFaviconImages().items.every(item => item.url === inline && item.loaded)).toBe(true);
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/refreshed.png' }; };
  const next = f.args('home', 'later-visit', false); next[9] = Number(next[9]) + 300_001;
  f.admit(['none', 'between-visits', false, false, [], [], [], [], {}, next[9]]);
  expect(mobileRootFaviconImages().items).toEqual([]);
  const preparing = f.run(next, f.native); await entered.promise;
  expect(mobileRootFaviconImages().items).toHaveLength(1);
  expect(mobileRootFaviconImages().items[0]).toMatchObject({url:inline,loaded:true});
  expect(mobileRootFaviconImages().items[0]!.mountId.startsWith('later-visit:')).toBe(true);
  release.resolve(); await preparing;
});

test('offline and disabled saved environments show retained images without a new native URL request', async () => {
  const f=fixture();await f.run(f.args(),f.native);
  mobileClient.connection='error';fleet.saved=fleet.saved.map(row=>({...row,enabled:false}));
  f.calls.length=0;await f.run(f.args(),f.native);
  expect(mobileRootFaviconImages().items.every(item=>item.url.startsWith('data:'))).toBe(true);
  expect(f.calls.some(call=>call.op==='request'||call.op==='mobileFaviconImage')).toBe(false);
});

test.each(['denied', 'missing'])('held %s lookup survives eight same-mount clock/visual/work admissions', async outcome => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => {
    entered.resolve(); await release.promise;
    if (outcome === 'denied') throw new Error('This finite synthetic peer does not implement the requested operation.');
    return { relativeUrl: '/api/assets/synthetic/project-favicon-missing' };
  };
  const values = f.args(), admission = f.admit(values);
  const pending = mobileRootFavicons(admission.owner, f.native);
  await entered.promise;
  for (let index = 1; index <= 8; index++) {
    const changed = [...values]; changed[9] = Number(values[9]) + index;
    expect(f.admit(changed).owner).toBe(admission.owner);
  }
  release.resolve(); await pending;
  for (let index = 0; index < 8; index++) await f.run(values, f.native);
  expect(f.calls.filter(call => call.op === 'request')).toHaveLength(1);
  expect(f.calls.some(call => call.op === 'mobileFaviconImage' || call.op === 'mobileClientCache' && call.action === 'write')).toBe(false);
  expect(mobileRootFaviconImages().items.every(item => item.url === '')).toBe(true);
});

test('a surviving sidebar keeps its held RPC and completion cannot resurrect departed Home', async () => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/sidebar.png' }; };
  const admission = f.admit(), pending = mobileRootFavicons(admission.owner, f.native);
  await entered.promise;
  expect(f.admit(f.args('thread', 'detail-visit')).owner).toBe(admission.owner);
  expect(mobileRootFaviconImages().items).toHaveLength(1);
  release.resolve(); await pending;
  expect(mobileRootFaviconImages().items).toHaveLength(1);
  expect(mobileRootFaviconImages().items[0]!.mountId.startsWith('sidebar-list:')).toBe(true);
  expect(mobileRootFaviconImages().items[0]!.url).toBe('data:image/png;base64,QQ==');
  expect(f.calls.filter(call => call.op === 'request')).toHaveLength(1);
});

test('replacing the only visit retires held work for the same resource', async () => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/departed.png' }; };
  const admission = f.admit(f.args('home', 'first', false));
  const pending = mobileRootFavicons(admission.owner, f.native).then(value => value, error => error);
  await entered.promise;
  expect(f.admit(f.args('home', 'replacement', false)).owner).not.toBe(admission.owner);
  release.resolve(); expect(await pending).toMatchObject({ kind: 'superseded' });
  expect(mobileRootFaviconImages().items.every(item => item.url === '')).toBe(true);
});

test.each(['clear', 'catalog', 'generation'])('%s invalidates a held refresh and hides stale visuals before its reply', async changed => {
  const f = fixture(); await f.run(f.args(), f.native);
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/late.png' }; };
  const values = f.args(); values[9] = Number(values[9]) + 1_800_000;
  const admission = f.admit(values), pending = mobileRootFavicons(admission.owner, f.native).then(value => value, error => error);
  await entered.promise;
  if (changed === 'clear') await mobileCacheClear(f.native, { environmentId: f.environmentId, kind: 'project-favicon' });
  if (changed === 'catalog') fleet.saved = [{ environmentId: f.environmentId, origin: 'https://replacement.invalid' }];
  if (changed === 'generation') mobileClient.generation++;
  expect(f.admit(values).owner).not.toBe(admission.owner);
  if (changed !== 'generation') expect(mobileRootFaviconImages().items.every(item => item.url === '')).toBe(true);
  release.resolve(); expect(await pending).toMatchObject({ kind: 'superseded' });
  expect(mobileRootFaviconImages().items.every(item => !item.url.includes('late'))).toBe(true);
});

test('adopted query and first matching clone-phase work changes do not replace a live resource owner', async () => {
  const f = fixture(), before = f.admit(); await f.seed();
  const observed = f.admit(); expect(observed.owner).toBe(before.owner);
  expect(observed.workRevision).toBeGreaterThan(before.workRevision);
  liveEvent(mobileClient, { key: 'project-clones', subscriptionId: '9-101', value: [{ destinationPath: '/work', phase: 'running' }] });
  const cloned = f.admit(); expect(cloned.owner).toBe(before.owner);
  expect(cloned.workRevision).toBeGreaterThan(observed.workRevision);
  const unrelated = f.args(); unrelated[10] = 999;
  expect(f.admit(unrelated).workRevision).toBe(cloned.workRevision);
  const result = await mobileRootFavicons(cloned.owner, f.native);
  expect(result.revision).toBe(mobileRootFaviconImages().revision);
});

test('due completion publishes a changed image revision with an unchanged owner and re-arms from completion', async () => {
  const f = fixture(), first = f.admit();
  const initial = await mobileRootFavicons(first.owner, f.native);
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.image = 'data:image/png;base64,Qg==';
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/changed.png' }; };
  const due = f.args(); due[9] = initial.nextDeadline;
  expect(f.admit(due).owner).toBe(first.owner);
  const pending = mobileRootFavicons(first.owner, f.native); await entered.promise;
  const completed = [...due]; completed[9] = Number(due[9]) + 250;
  expect(f.admit(completed).owner).toBe(first.owner);
  release.resolve(); const result = await pending;
  expect(result.revision).toBeGreaterThan(initial.revision);
  expect(result.revision).toBe(mobileRootFaviconImages().revision);
  expect(result.nextDeadline).toBe(composerNow(mobileClient) + 1_800_000);
  expect(mobileRootFaviconImages().items.every(item => item.url === f.state.image)).toBe(true);
});


test('only the first exact clone controls refresh; later duplicates and missing phase stay inert', async () => {
  const f = fixture();
  const clones = (value: Obj[]) => liveEvent(mobileClient, { key: 'project-clones', subscriptionId: `9-${1000 + sequence}`, value });
  clones([{ destinationPath: '/work', phase: '' }, { destinationPath: '/work', phase: 'queued' }]);
  await f.run(f.args(), f.native); const initial = f.admit(); f.calls.length = 0;
  clones([{ destinationPath: '/work', phase: '' }, { destinationPath: '/work', phase: 'running' }]);
  expect(f.admit().workRevision).toBe(initial.workRevision);
  await f.run(f.args(), f.native);
  clones([{ destinationPath: '/work' }, { destinationPath: '/work', phase: 'complete' }]);
  expect(f.admit().workRevision).toBe(initial.workRevision);
  await f.run(f.args(), f.native);
  expect(f.calls.filter(call => call.op === 'request')).toHaveLength(0);
  clones([{ destinationPath: '/work', phase: 'running' }, { destinationPath: '/work', phase: 'complete' }]);
  const changed = f.admit(); expect(changed.owner).toBe(initial.owner);
  expect(changed.workRevision).toBeGreaterThan(initial.workRevision);
  await mobileRootFavicons(changed.owner, f.native);
  expect(f.calls.filter(call => call.op === 'request')).toHaveLength(1);
});

test('missing and unrelated adopted clones may signal work but add no RPC before a query is due', async () => {
  const f = fixture();
  const clones = (value: Obj[]) => liveEvent(mobileClient, { key: 'project-clones', subscriptionId: `9-${1000 + sequence}`, value });
  clones([]); await f.run(f.args(), f.native);
  let previous = f.admit(); f.calls.length = 0;
  for (const value of [[{ destinationPath: '/else', phase: 'running' }], [{ destinationPath: '/work ', phase: 'complete' }], []]) {
    clones(value); const admitted = f.admit();
    expect(admitted.owner).toBe(previous.owner); expect(admitted.workRevision).toBeGreaterThan(previous.workRevision);
    await mobileRootFavicons(admitted.owner, f.native); previous = f.admit();
    expect(f.calls.filter(call => call.op === 'request')).toHaveLength(0);
  }
});
