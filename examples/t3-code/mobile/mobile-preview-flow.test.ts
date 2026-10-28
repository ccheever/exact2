import { contractFn } from '../tests/support/contract-fn-fixture';
import { mobileFileRead, mobileFilesRead } from './file-data';
import { expect, test } from 'bun:test';
import { mobileClient, mobileNative } from './client';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserRead, mobileBrowserSnapshot, BROWSER_EVENTS } from './browser-mobile-data';
import { mobilePreviewMenus, mobilePreviewMenuAction, mobilePreviewOwner, mobilePreviewPrepare, mobilePreviewAction } from './mobile-preview-flow';
import { mobileDevicesRead, mobileDevicesSnapshot } from './devices-mobile-data';
import { T3Client } from './shared/client';
import { deviceStateEvent, deviceStateOf, DEVICE_STATE_KEY } from './shared/r4-surfaces-device';
import { obj, type Obj } from './shared/domain';
import { mobileThreadHeaderSnapshot, mobileThreadHeaderPrepare, type ThreadHeaderOptions } from './thread-header';
import { terminalMetadataEvent } from './shared/terminal-drawer-view';
import { mobileWorkingControl } from './working-control';
import type { Native } from './shared/protocol';

test('native preview menus preserve immutable owner/target and explicit operation arguments', () => {
  const menus = mobilePreviewMenus('browser', { owner: 'owner', source: 'target', tabs: [{ id: 'b', title: 'Tab B', selected: true }], menu: [{ id: 'clear:cache', title: 'Clear cache', destructive: true, group: 'Site data' }], pipSupported: true });
  const choices = JSON.parse(menus.selection), options = JSON.parse(menus.options);
  expect(choices).toMatchObject({ owner: 'owner', target: 'target', items: [{ id: 'b', operation: 'select', value: 'b', selected: true }] });
  expect(options.items[0]).toMatchObject({ operation: 'clear:cache', destructive: true, group: 'Site data' });
  expect(options.items[1].operation).toBe('pip');
});

function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
let preparationGeneration = 50_000;
function preparationFixture() {
  const saved = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, projectId: mobileClient.projectId,
    threadId: mobileClient.threadId, generation: mobileClient.generation, threadEpoch: mobileClient.threadEpoch,
    connection: mobileClient.connection, configLive: mobileClient.configLive, shellLive: mobileClient.shellLive,
    threadLive: mobileClient.threadLive, shellLoaded: mobileClient.shellLoaded, config: mobileClient.config,
    shell: mobileClient.shell, thread: mobileClient.thread, local: mobileClient.local, revision: mobileClient.revision, error: mobileClient.error };
  const generation = ++preparationGeneration, pending: Promise<unknown>[] = [], calls: Obj[] = [], waiters = new Set<() => void>();
  const auth: { request: Obj; answer: ReturnType<typeof deferred<unknown>>; done: boolean }[] = [];
  const pulse = () => { for (const notify of [...waiters]) notify(); };
  async function reached(condition: () => boolean) {
    if (condition()) return;
    const event = deferred<void>(), notify = () => { if (condition()) event.resolve(); };
    waiters.add(notify); let timer: ReturnType<typeof setTimeout>;
    try { await Promise.race([event.promise, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Preview event did not arrive.')), 2000); })]); }
    finally { clearTimeout(timer!); waiters.delete(notify); }
  }
  Object.assign(mobileClient, { origin: 'https://admission.test', environmentId: `prepare-${generation}`, projectId: 'project',
    threadId: 'thread', generation, threadEpoch: generation, connection: 'connected', configLive: true, shellLive: true,
    threadLive: true, shellLoaded: true, local: new T3Client().local, error: '',
    config: { environment: { label: 'QA', capabilities: { serverResolvedCommandContext: true } } },
    shell: { ...new T3Client().shell, projects: [{ id: 'project', title: 'QA', workspaceRoot: '/qa', scripts: [] }],
      threads: [{ id: 'thread', projectId: 'project', title: 'QA thread', worktreePath: null, branch: 'qa' }] },
    thread: { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 0,
      projection: { thread: { id: 'thread', projectId: 'project' }, runs: [], messages: [] } } });
  const owner = mobileStreamOwner(mobileClient), options: ThreadHeaderOptions = { routeKey: 'thread-visit',
    routeIdentity: `/threads/${mobileClient.environmentId}/thread`, focused: true, split: false, sidebarVisible: false,
    canGoBack: true, returnToChat: false, foreground: '#111111' };
  let serial = generation * 100, holdRelease = false, failRelease = false, cleaning = false;
  const releaseEntered = deferred<void>(), releaseAnswer = deferred<void>();
  const deviceData = () => ({ hosts: [{ id: 'host', label: 'QA host' }],
    devices: [{ hostId: 'host', id: 'device', name: 'QA device', version: 'iOS', platform: 'ios' }],
    sessions: [{ hostId: 'host', deviceId: 'device', threadId: mobileClient.threadId, platform: 'ios' },
      { hostId: 'host', deviceId: 'unrelated', threadId: 'another-thread', platform: 'ios' }] });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'http' && request.path === '/api/auth/session') {
      if (cleaning) return { ok: true, generation: request.generation, value: { authenticated: true,
        permissions: ['orchestration:read', 'diagnostics:read', 'terminal:read', 'filesystem:read'] } };
      const item = { request, answer: deferred<unknown>(), done: false }; auth.push(item); pulse(); return item.answer.promise;
    }
    pulse(); let value: unknown = {};
    if (request.op === 'subscribe') {
      const id = `${generation}-${++serial}`; value = { id };
      if (request.method === 'subscribeDeviceState') deviceStateEvent(mobileClient, { key: DEVICE_STATE_KEY, generation,
        subscriptionId: id, seq: 1, value: deviceData() });
      else if (request.method === 'subscribeTerminalMetadata') terminalMetadataEvent(mobileClient, { key: 'terminal-metadata', generation,
        subscriptionId: id, seq: 1, value: { type: 'snapshot', terminals: [] } });
      else expect(['subscribePreviewEvents', 'subscribeVcsStatus']).toContain(request.method);
    } else if (request.method === 'preview.list') value = { serverEpoch: `${generation}`, revision: 1, sessions: [
      { tabId: 'tab', runtime: 'server', navStatus: { _tag: 'Loaded', title: 'QA tab', url: 'https://preview.test' }, updatedAt: '2026-10-17' }] };
    else if (request.method === 'vcs.refreshStatus') value = { isRepo: true, refName: 'qa', hasPrimaryRemote: false,
      hasWorkingTreeChanges: false, workingTree: { files: [] }, branchChanges: { baseRef: 'main' } };
    else if (request.method === 'projects.readFile') value = { contents: '# QA file', byteLength: 9, truncated: false };
    else if (request.method === 'projects.listEntries') value = { entries: [{ kind: 'file', path: 'README.md' }] };
    else if (request.op === 'terminalRetain') expect(request.sessions).toEqual([]);
    else if (request.op === 'unsubscribe') {
      expect(request.key).toBe(BROWSER_EVENTS); releaseEntered.resolve();
      if (holdRelease) await releaseAnswer.promise;
      if (failRelease) return { ok: false, generation: request.generation, error: { kind: 'transport', message: 'Unsubscribe failed', uncertain: false } };
    } else throw new Error(`Unexpected preparation request: ${JSON.stringify(request)}`);
    return { ok: true, generation: request.generation, value };
  } };
  const track = <T>(promise: Promise<T>): Promise<T> => { pending.push(promise.then(() => {}, () => {})); return promise; };
  const prepare = (allowed = true, visit = 'thread-visit', kind = 'thread') => track(mobilePreviewPrepare(owner, visit, kind, allowed, native));
  const header = () => track(mobileThreadHeaderPrepare(mobileThreadHeaderSnapshot(options).owner, native, 10));
  const count = (method: string) => calls.filter(call => call.method === method).length;
  const grant = (index: number, mode = 'allowed') => {
    const item = auth[index - 1]!; expect(item.done).toBe(false); item.done = true;
    const permissions = mode === 'denied' ? [] : ['orchestration:read', 'diagnostics:read', 'terminal:read', 'filesystem:read'];
    if (mode === 'aborted') item.answer.reject(Object.assign(new Error('Answer was let go.'), { name: 'FetchError', kind: 'Aborted' }));
    else item.answer.resolve(mode === 'transport' ? { ok: false, generation: item.request.generation,
      error: { kind: 'transport', message: 'Session unavailable', uncertain: false } }
      : { ok: true, generation: item.request.generation, value: { authenticated: true, permissions } });
  };
  const authEntered = (index: number) => reached(() => auth.length >= index);
  const discover = async (visit = 'thread-visit', kind = 'thread') => {
    const first = auth.length + 1, running = prepare(true, visit, kind);
    await authEntered(first); grant(first); await authEntered(first + 1); grant(first + 1); return await running;
  };
  const finish = async () => {
    cleaning = true; releaseAnswer.resolve();
    for (let index = 0; index < auth.length; index++) if (!auth[index]!.done) grant(index + 1);
    await Promise.all(pending);
    failRelease = false;
    await mobilePreviewPrepare('', '', '', false, native);
    Object.assign(mobileClient, saved);
  };
  return { owner, generation, native, options, calls, auth, prepare, header, track, authEntered, grant, discover, count, finish,
    holdRelease() { holdRelease = true; }, releaseEntered: releaseEntered.promise, release() { releaseAnswer.resolve(); },
    failRelease() { failRelease = true; }, home: () => track(mobilePreviewPrepare('', '', '', false, native)),
    seedShared() { deviceStateOf(mobileClient); deviceStateEvent(mobileClient, { key: DEVICE_STATE_KEY, generation,
      subscriptionId: `${generation}-${++serial}`, seq: 1, value: deviceData() }); } };
}
function preparationTest(name: string, body: (fixture: ReturnType<typeof preparationFixture>) => Promise<void>) {
  test(name, async () => { const fixture = preparationFixture(); try { await body(fixture); } finally { await fixture.finish(); } });
}
preparationTest('ordinary-thread discovery waits for header admission, then publishes both floating entries', async f => {
  expect(mobilePreviewOwner(false, 'unused', 'thread')).toEqual({ owner: '', visit: '', kind: '', admissionKey: '' });
  expect(await f.prepare(false)).toMatchObject({ retryable: false }); expect(f.calls).toEqual([]);
  const header = f.header(); await f.authEntered(1);
  await f.prepare(false); expect(f.auth).toHaveLength(1);
  f.grant(1); await f.authEntered(2); expect(f.count('subscribeTerminalMetadata')).toBe(1);
  f.grant(2); await header; expect(f.count('vcs.refreshStatus')).toBe(1);
  const prepared = f.prepare(); await f.authEntered(3);
  expect(f.count('subscribePreviewEvents')).toBe(0); expect(f.count('subscribeDeviceState')).toBe(0);
  f.grant(3); await f.authEntered(4);
  expect(f.count('subscribePreviewEvents')).toBe(1); expect(f.count('preview.list')).toBe(1); expect(f.count('subscribeDeviceState')).toBe(0);
  f.grant(4); expect(await prepared).toMatchObject({ retryable: false });
  expect(mobileWorkingControl(mobileClient.environmentId, 'thread', 10)).toMatchObject({ visible: true,
    browserCount: 1, deviceCount: 1, browserAccessibility: 'View browser', deviceAccessibility: 'View device' });
  expect(JSON.parse(mobileDevicesSnapshot(f.owner).source).operate).toBe(false);
});
for (const source of [false, true]) preparationTest(`opening ${source ? 'source' : 'Files'} does not start preview authorization over its file read`, async f => {
  const active = await contractFn('mobile/app-shapes.contract', 'previewDiscoveryActive', ['name', 'inspector', 'filesError']);
  const read = source ? mobileFileRead('README.md', f.native) : mobileFilesRead('', '', f.native);
  const file = f.track(read).then(value => ({ value }), error => ({ error }));
  await f.authEntered(1);
  const route = source ? 'threadFile' : 'threadFiles';
  const owner = mobilePreviewOwner(active(route, false, '') === true, 'file-visit', route);
  const preview = f.track(mobilePreviewPrepare(owner.owner, owner.visit, owner.kind, !!owner.owner, f.native));
  expect(f.auth).toHaveLength(1);
  f.grant(1); await preview;
  expect(await file).toMatchObject({ value: source ? { contents: '# QA file', error: '', rows: [{ text: '# QA file' }] }
    : { error: '', rows: [{ path: 'README.md' }] } });
  expect(f.count('subscribePreviewEvents')).toBe(0); expect(f.count('subscribeDeviceState')).toBe(0);
});
test('preview discovery follows chat and viewer visibility, including the tablet file inspector', async () => {
  const active = await contractFn('mobile/app-shapes.contract', 'previewDiscoveryActive', ['name', 'inspector', 'filesError']);
  for (const route of ['thread', 'threadBrowser', 'threadDevices']) expect(active(route, false, '')).toBe(true);
  expect(active('threadFiles', true, '')).toBe(true);
  expect(active('threadFiles', true, 'Files unavailable')).toBe(false);
  for (const route of ['threadFiles', 'threadFile', 'threadTerminal', 'threadReview', 'threadQueue', 'threadModel', 'home']) expect(active(route, false, '')).toBe(false);
});
preparationTest('shared device state alone does not admit the ordinary-thread floating entry', async f => {
  f.seedShared(); expect(mobileDevicesSnapshot(f.owner).count).toBe(0);
  expect(mobileWorkingControl(mobileClient.environmentId, 'thread', 10).deviceCount).toBe(0);
  await f.discover(); expect(f.count('subscribeDeviceState')).toBe(0);
  expect(mobileWorkingControl(mobileClient.environmentId, 'thread', 10)).toMatchObject({ browserCount: 1, deviceCount: 1 });
});
preparationTest('a new viewer visit and kind each receive fresh serial admission with the same stream owner', async f => {
  const ordinary = await f.discover(), device = await f.discover('device-visit', 'threadDevices'), browser = await f.discover('browser-visit', 'threadBrowser');
  expect(new Set([ordinary.admissionKey, device.admissionKey, browser.admissionKey]).size).toBe(3);
  expect(f.auth).toHaveLength(6); expect(f.count('subscribeDeviceState')).toBe(1);
  expect(mobileWorkingControl(mobileClient.environmentId, 'thread', 10)).toMatchObject({ browserCount: 1, deviceCount: 1 });
});
for (const phase of ['Browser', 'Devices']) preparationTest(`${phase} session supersession returns a current retry receipt and a fresh answer can complete discovery`, async f => {
  const old = f.prepare(); await f.authEntered(1);
  const active = phase === 'Browser' ? 1 : 2;
  if (active === 2) { f.grant(1); await f.authEntered(2); }
  const successor = f.track(mobileClient.http(mobileNative(f.native), '/api/auth/session'));
  await f.authEntered(active + 1); f.grant(active + 1); await successor; f.grant(active);
  const receipt = await old; expect(receipt).toMatchObject({ retryable: true });
  expect(receipt.admissionKey).toBe(mobilePreviewOwner(true, 'thread-visit', 'thread').admissionKey);
  expect(f.count('subscribePreviewEvents')).toBe(active === 1 ? 0 : 1); expect(f.count('subscribeDeviceState')).toBe(0);
  expect(await f.discover()).toMatchObject({ retryable: false, admissionKey: receipt.admissionKey });
  expect(mobileDevicesSnapshot(f.owner).title).toBe('QA device');
});
for (const phase of ['Browser', 'Devices']) preparationTest(`raw ${phase} answer abort stops the serial pass without a retry receipt or later native call`, async f => {
  const running = f.prepare(); await f.authEntered(1);
  const active = phase === 'Browser' ? 1 : 2;
  if (active === 2) { f.grant(1); await f.authEntered(2); }
  const callCount = f.calls.length; f.grant(active, 'aborted');
  await expect(running).rejects.toHaveProperty('kind', 'superseded');
  expect(f.calls).toHaveLength(callCount); expect(f.auth).toHaveLength(active);
  expect(f.count('subscribePreviewEvents')).toBe(active === 1 ? 0 : 1); expect(f.count('subscribeDeviceState')).toBe(0);
});
for (const failure of ['transport', 'denied']) preparationTest(`ordinary Browser ${failure} retains its error and allows Device discovery`, async f => {
  const running = f.prepare(); await f.authEntered(1); f.grant(1, failure);
  await f.authEntered(2); f.grant(2); expect(await running).toMatchObject({ retryable: false });
  expect(mobileBrowserSnapshot(f.owner).error).toBe(failure === 'transport' ? 'Session unavailable' : 'This connection cannot view previews.');
  expect(mobileDevicesSnapshot(f.owner)).toMatchObject({ count: 1, error: '' });
});
preparationTest('ordinary Device denial remains an error without replacing accepted Browser discovery', async f => {
  const running = f.prepare(); await f.authEntered(1); f.grant(1);
  await f.authEntered(2); f.grant(2, 'denied'); expect(await running).toMatchObject({ retryable: false });
  expect(mobileBrowserSnapshot(f.owner).count).toBe(1);
  expect(mobileDevicesSnapshot(f.owner)).toMatchObject({ count: 0, error: 'This connection cannot view previews.' });
});
for (const replacement of ['visit', 'kind', 'thread', 'generation']) preparationTest(`replacing ${replacement} retires an old preparation without retrying it`, async f => {
  const running = f.prepare(); await f.authEntered(1);
  if (replacement === 'thread') mobileClient.threadId = 'replacement';
  if (replacement === 'generation') mobileClient.generation++;
  const owner = mobileStreamOwner(mobileClient);
  await mobilePreviewPrepare(owner, replacement === 'visit' ? 'next-visit' : 'thread-visit', replacement === 'kind' ? 'threadDevices' : 'thread', false, f.native);
  f.grant(1); await expect(running).rejects.toHaveProperty('kind', 'superseded');
  expect(f.auth).toHaveLength(1); expect(f.count('subscribeDeviceState')).toBe(0);
});
preparationTest('Home cleanup runs while the departing header is pending and does not start discovery', async f => {
  await f.discover(); const header = f.header(); await f.authEntered(3);
  f.options.focused = false; f.options.routeKey = 'home'; mobileThreadHeaderSnapshot(f.options);
  f.holdRelease(); const home = f.home(); await f.releaseEntered;
  expect(f.calls.filter(call => call.op === 'unsubscribe')).toHaveLength(1); expect(f.auth).toHaveLength(3);
  f.release(); expect(await home).toMatchObject({ admissionKey: '', retryable: false });
  f.grant(3); await expect(header).rejects.toHaveProperty('kind', 'superseded');
  expect(f.auth).toHaveLength(3);
});
preparationTest('owner replacement keeps the existing guard against unsubscribing a successor', async f => {
  await f.discover(); mobileClient.threadId = 'replacement';
  await mobilePreviewPrepare(mobileStreamOwner(mobileClient), 'next', 'thread', false, f.native);
  expect(f.calls.filter(call => call.op === 'unsubscribe')).toHaveLength(0);
  expect(mobileDevicesSnapshot(mobileStreamOwner(mobileClient)).count).toBe(0);
});
preparationTest('a release failure keeps the existing one-observation cleanup behavior', async f => {
  await f.discover(); f.failRelease();
  await expect(f.home()).rejects.toHaveProperty('message', 'Unsubscribe failed');
  expect(await f.home()).toMatchObject({ admissionKey: '', retryable: false });
  expect(f.calls.filter(call => call.op === 'unsubscribe')).toHaveLength(1);
});
preparationTest('an explicit refresh retires an older preparation rather than granting it a retry', async f => {
  const preparing = f.prepare(); await f.authEntered(1);
  const refresh = f.track(mobilePreviewAction('devices', f.owner, 'refresh', '', f.native));
  await f.authEntered(2); f.grant(2); await refresh;
  f.grant(1); await expect(preparing).rejects.toHaveProperty('kind', 'superseded');
  expect(f.count('subscribeDeviceState')).toBe(1); expect(mobileDevicesSnapshot(f.owner).title).toBe('QA device');
});
test('an obsolete native menu cannot dispatch into a different preview', async () => {
  const calls: unknown[] = [];
  const native: Native = { available: true, watch() {}, async later(input) { calls.push(input); return {}; } };
  await mobilePreviewMenuAction('browser', JSON.stringify({ owner: 'old', target: 'old', operation: 'clear:cache' }), 'new', native);
  expect(calls).toHaveLength(0);
});
test.each([1, 2])('device options show a picker only for multiple open previews: %i', async count => {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://device-menu.test', environmentId: 'env', threadId: 'thread', generation: 4,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  const calls: { op: string; target: string }[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push({ op: String(request.op), target: String(request.path ?? request.method) });
    return { ok: true, generation: 4, value: request.op === 'http'
      ? { authenticated: true, permissions: ['orchestration:read'] } : { id: '4-1' } };
  } };
  const owner = mobileStreamOwner(client); await mobileDevicesRead(owner, native, client);
  deviceStateEvent(client, { key: DEVICE_STATE_KEY, subscriptionId: '4-1', value: {
    hosts: [{ id: 'host', label: 'Host' }], devices: [], supportsToolInspection: true,
    sessions: [
      { hostId: 'host', deviceId: 'first', threadId: 'thread', platform: 'ios' },
      ...(count === 2 ? [{ hostId: 'host', deviceId: 'second', threadId: 'thread', platform: 'android' }] : []),
      { hostId: 'host', deviceId: 'unrelated', threadId: 'another-thread', platform: 'ios' },
    ],
  } });
  const snapshot = mobileDevicesSnapshot(owner, client), menus = mobilePreviewMenus('devices', snapshot);
  const selection = JSON.parse(menus.selection), options = JSON.parse(menus.options);
  expect(snapshot.count).toBe(count);
  expect(snapshot.devices.map(item => item.id)).toEqual(count === 2 ? ['["host","first"]', '["host","second"]'] : ['["host","first"]']);
  expect(selection).toMatchObject({ owner, target: snapshot.source });
  expect(options).toMatchObject({ owner, target: snapshot.source });
  expect(selection.items).toHaveLength(count);
  expect(selection.items.map((item: { operation: string; value: string; selected: boolean }) => [item.operation, item.value, item.selected]))
    .toEqual(snapshot.devices.map(item => ['select', item.id, item.selected]));
  const controls = options.items.filter((item: { group: string }) => item.group !== 'Devices');
  expect(controls.map((item: { id: string; disabled: boolean; operation: string; value: string }) => [item.id, item.disabled, item.operation, item.value]))
    .toEqual([['inspect', false, 'inspect', ''], ['tools', false, 'tools', ''], ['reload', false, 'reload', ''],
      ['appSwitcher', true, 'appSwitcher', ''], ['rotate', true, 'rotate', ''], ['shutdown', true, 'shutdown', '']]);
  expect(calls).toEqual([{ op: 'http', target: '/api/auth/session' }, { op: 'subscribe', target: 'subscribeDeviceState' }]);
  expect(options.items.filter((item: { group: string }) => item.group === 'Devices')).toEqual(count === 2
    ? selection.items.map((item: object) => ({ ...item, group: 'Devices' })) : []);
});
test('mobile event hook reassembles large native batches before shared acknowledgment', async () => {
  const saved = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, generation: mobileClient.generation,
    connection: mobileClient.connection, configLive: mobileClient.configLive, shellLive: mobileClient.shellLive, threadLive: mobileClient.threadLive };
  const calls: string[] = [];
  Object.assign(mobileClient, { origin: 'https://menu-fixture.test', environmentId: 'event-env', threadId: 'event-thread', generation: 4,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  try {
    const owner = mobileStreamOwner(mobileClient);
    const batch = { events: [{ seq: 1, generation: 4, key: BROWSER_EVENTS, subscriptionId: '4-7', value: { type: 'changed', serverEpoch: 'a', revision: 2, threadId: 'event-thread', tabId: 'new', snapshot: { tabId: 'new', runtime: 'server', navStatus: { _tag: 'Loaded', title: 'New tab' }, updatedAt: '2026-01-01' } } }] };
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input); calls.push(String(request.op));
      const value = request.op === 'http' ? { authenticated: true, permissions: ['orchestration:read', 'preview:operate'] }
        : request.op === 'subscribe' ? { id: '4-7' }
        : request.method === 'preview.list' ? { serverEpoch: 'a', revision: 1, sessions: [] }
        : request.op === 'events' ? { _nativeTransfer: { id: 'large', parts: 1 } }
        : request.op === 'readChunk' ? { text: JSON.stringify(batch) } : {};
      return { ok: true, generation: 4, value };
    } };
    await mobileBrowserRead(owner, native);
    const response = obj(await mobileNative(native).later({ op: 'events', after: 0 }));
    expect(response.value).toEqual(batch);
    expect(calls.slice(-3)).toEqual(['events', 'readChunk', 'releaseChunk']);
    expect(mobileBrowserSnapshot(owner).title).toBe('New tab');
    expect(mobileBrowserSnapshot(owner).count).toBe(1);
  } finally { Object.assign(mobileClient, saved); }
});
