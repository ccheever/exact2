import { test, expect, mock } from 'bun:test';

mock.module('./rt.js', () => ({
  journal: [], clock: { now: 0, agent: true }, nextTicket: () => 1,
  inflight: { n: 0 }, Hosts: {}, OnHooks: {}, viewId: () => 1, Views: new Map(), data: {},
}));
mock.module('./pointer.js', () => ({ record: () => {} }));
mock.module('./navigation.js', () => ({ reportPlace: () => '' }));
mock.module('./admission.js', () => ({ appGrantSet: () => new Set(), coversPath: (_, cap, path) => cap === 'fs.read' && path.startsWith('app:/data/') }));

const rt = await import('./rt.js');
const { accepts, deliveredType, tokens, holds } = await import('./files.js');

test('file input accept admits any type only when unrestricted', () => {
  const absent = tokens({ getAttribute: () => null });
  const empty = tokens({ getAttribute: () => '' });
  const mixed = tokens({ getAttribute: () => 'image/jpeg, */*' });
  expect(absent).toEqual([]);
  expect(empty).toEqual([]);
  expect(mixed).toEqual(['image/jpeg', '*/*']);
  for (const name of ['archive.xyz', 'notes.txt']) {
    expect(accepts(absent, name, 'application/octet-stream')).toBe(true);
    expect(accepts(empty, name, 'application/octet-stream')).toBe(true);
    expect(accepts(['*/*'], name, 'application/octet-stream')).toBe(true);
    expect(accepts(mixed, name, 'application/octet-stream')).toBe(true);
  }
  expect(accepts(['image/jpeg', '.txt'], 'archive.xyz', 'application/octet-stream')).toBe(false);
  expect(accepts(['image/jpeg', '.txt'], 'notes.txt', 'text/plain')).toBe(true);
});

test('a wildcard in a mixed accept preserves the original HEIC type', () => {
  expect(deliveredType('image/heic', ['image/jpeg', '*/*'])).toBe('image/heic');
  expect(deliveredType('image/heif', ['image/jpeg', '*/*'])).toBe('image/heif');
  expect(deliveredType('image/heic', ['image/jpeg'])).toBe('image/jpeg');
});

// LLP 1125 D12: the JS target's share holds its files under the agent, and
// refuses a picture of an element, a file outside `fs.read`, a path that is
// no app file, and nothing to share, in the runner's words (share.rs).
test('a share holds its files under the agent and refuses what the web cannot share', () => {
  rt.journal.length = 0;
  globalThis.Element ??= class {}; // no DOM: no pressed node to anchor to
  rt.Hosts.share('T', null, null, ['app:/data/a.txt']);
  expect(holds().at(-1).device).toEqual({ capability: 'share', args: { title: 'T', text: null, url: null, files: ['app:/data/a.txt'], anchor: null } });
  rt.Hosts.share(null, 'x', null, undefined);
  expect(holds().at(-1).device.args.files).toEqual([]);
  rt.Hosts.share(null, 'x', null, ['#card']);
  rt.Hosts.share(null, null, null, ['app:/cache/a.txt']);
  rt.Hosts.share(null, null, null, ['/etc/passwd']);
  rt.Hosts.share(null, null, null, []);
  expect(rt.journal.filter(l => l.includes('refused')).map(l => l.replace(/^t=\d+ /, ''))).toEqual([
    'share: refused: a picture of an element is not available on the web',
    "share: refused: app:/cache/a.txt is outside the app's fs.read grants",
    "share: refused: /etc/passwd is neither an app:/ file nor an element's #id",
    'share: refused: needs text=, url= or files=',
  ]);
});

// The web glue (picker-glue.js) reads each app file into a File named by
// its last segment and typed by its extension, and shares where
// `navigator.canShare` takes them; a fake navigator stands in.
test('app files reach navigator.share as typed Files, guarded by canShare', async () => {
  globalThis.exact ??= {};
  await import(`../web/picker-glue.js?share=${Date.now()}`);
  const api = globalThis.exact.picker({ appId: 'test.share', dispatch() {}, pickedPath: async () => 'app:/tmp/picked/1.png', log() {} });
  await api.answer(5, [{ name: 'card.png', bytes: btoa('png') }]);
  const was = Object.getOwnPropertyDescriptor(globalThis, 'navigator'), shared = [];
  let takes = true, fails = null;
  Object.defineProperty(globalThis, 'navigator', { configurable: true, writable: true, value: {
    canShare: d => takes && d.files.every(f => f instanceof File),
    share: async d => { if (fails) throw Object.assign(new Error(fails), { name: fails }); shared.push(d); },
  } });
  try {
    const picked = [{ path: 'app:/tmp/picked/1.png' }];
    expect(await api.share({ title: 'T', text: null, url: null }, picked)).toBe('share: shared');
    expect(Object.keys(shared[0])).toEqual(['title', 'files']);
    expect(shared[0].files.map(f => [f.name, f.type, f.size])).toEqual([['1.png', 'image/png', 3]]);
    fails = 'AbortError';
    expect(await api.share({ title: 'T' }, picked)).toBe('share: dismissed');
    fails = 'NotAllowedError';
    expect(await api.share({ title: 'T' }, picked)).toBe('share: refused: NotAllowedError');
    takes = false;
    expect(await api.share({ title: 'T' }, picked)).toBe('share: refused: this browser cannot share these files');
    expect(await api.share({}, [{ path: 'app:/data/missing.txt' }])).toBe('share: refused: this page has no app files');
    expect(shared.length).toBe(1);
  } finally { Object.defineProperty(globalThis, 'navigator', was); }
});
