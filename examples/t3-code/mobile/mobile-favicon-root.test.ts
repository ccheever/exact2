import { afterEach, beforeEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { fleet } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { projectMobileHome } from './home';
import { mobileNewTaskChooser } from './new-task';
import { mobileRootFaviconAdmission, mobileRootFavicons, mobileRootFaviconImages, mobileRootFaviconEvent } from './mobile-favicon-root';

let original: Partial<typeof mobileClient>, saved: typeof fleet.saved, entries: typeof fleet.entries;
beforeEach(() => { original = { ...mobileClient }; saved = fleet.saved; entries = fleet.entries; fleet.entries = new Map(); });
afterEach(async () => {
  mobileRootFaviconAdmission(['none', 'cleanup', false, false, [], [], [], [], {}, Date.now()]);
  Object.assign(mobileClient, original); fleet.saved = saved; fleet.entries = entries;
});
let sequence = 0;
function fixture() {
  const environmentId = `favicon-root-${++sequence}`, origin = `https://${environmentId}.invalid`;
  Object.assign(mobileClient, { environmentId, origin, generation: 9, connection: 'connected', shellLoaded: true,
    shellLive: true, configLive: true, config: {}, shell: { ...initialShell(),
      projects: [{ id: 'p', title: 'Repo', workspaceRoot: '/work' }],
      threads: [{ id: 't', title: 'Thread', projectId: 'p', archivedAt: null }] } });
  fleet.saved = [{ environmentId, origin }];
  const calls: Obj[] = [], state = { response: async () => ({ relativeUrl: '/icon.png' }) };
  const native: Native = { available: true, watch() {}, async later(input) {
    const r = obj(input); calls.push(r);
    if (r.op === 'request') return { ok: true, generation: 9, value: await state.response() };
    if (r.op === 'mobileFaviconImage') return { ok: true, generation: 0, value: { dataUrl: 'data:image/png;base64,QQ==' } };
    if (r.op === 'mobileClientCache') return { ok: true, generation: 0, value: r.action === 'list' ? { rows: [] }
      : r.action === 'read' ? { record: null } : r.action === 'ticket' ? { ticket: '1' }
      : r.action === 'write' ? { written: true, stale: false } : { removed: 0 } };
    throw Error(`Unexpected ${r.op}`);
  } };
  const home = projectMobileHome([{ environmentId, focused: true, label: 'Env', machine: 'laptop',
    config: {}, shell: mobileClient.shell }], Date.now()).items;
  const projects = mobileNewTaskChooser('', 'repository', mobileClient, fleet).projects;
  const args = (route = 'home', routeId = 'visit-a', sidebar = true) => [route, routeId, route === 'home', sidebar,
    home, home, projects, [], {}, Date.now()];
  const admit = (values = args()) => mobileRootFaviconAdmission(values);
  const run = (values = args(), bridge: Native | null = null) => { const admission = admit(values); return mobileRootFavicons(admission.revision, Number(values[9]), bridge); };
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
  expect(mobileRootFaviconImages().items.every(item => item.url === inline && item.loaded)).toBe(true);
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  f.state.response = async () => { entered.resolve(); await release.promise; return { relativeUrl: '/refreshed.png' }; };
  const next = f.args('home', 'later-visit', false); next[9] = Date.now() + 300_001;
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
