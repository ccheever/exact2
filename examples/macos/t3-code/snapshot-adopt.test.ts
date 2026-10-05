import { test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';
import { snapshotDefaultProject, snapshotDestinationExists, snapshotIdentity, withoutSnapshot } from './snapshot-adopt';

const capture = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', unrelated = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
type Gate = { fail: boolean; release: () => void; entered: Promise<void> };
// Writes land in call order, as the native adapter's serial queue does; a gate
// holds one write (and every later write behind it) until released.
function fixture() {
  const client = new T3Client();
  Object.assign(client, { generation: 1, environmentId: 'env', origin: 'http://fixture', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.shell = { sequence: 1, projects: [{ id: 'p', workspaceRoot: '/p' }], threads: [{ id: 't', projectId: 'p' }] };
  const calls: Obj[] = [], disk: string[] = [];
  let queue = Promise.resolve(), gate: (Gate & { match: (saved: Obj) => boolean }) | null = null, catalog = () => ({ projects: client.shell.projects, threads: client.shell.threads });
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk.at(-1) || '').buffer; }, atomicWriteFile(_path, bytes) {
    const text = new TextDecoder().decode(bytes), held = gate && gate.match(obj(JSON.parse(text))) ? gate : null;
    if (held) gate = null;
    const write = queue.then(async () => {
      if (held) { (held as unknown as { enter: () => void }).enter(); await new Promise<void>(resolve => { held.release = resolve; }); if (held.fail) throw new Error('disk full'); }
      disk.push(text);
    });
    queue = write.catch(() => undefined);
    return write;
  } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    let value: unknown = {};
    if (request.op === 'snapshotState') value = { captures: [{ id: capture, owner: client.snapshotOwner }] };
    if (request.op === 'http') value = catalog();
    if (request.op === 'snapshotRead') value = { id: capture, owner: request.owner, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: { kind: 'snap-shot', appName: 'Fixture app', windowTitle: 'Fixture', capturedAt: '2026-10-03T00:00:00Z' } };
    return { ok: true, generation: client.generation, value };
  } };
  const hold = (match: (saved: Obj) => boolean, fail = false): Gate => {
    let enter!: () => void; const entered = new Promise<void>(resolve => { enter = resolve; });
    const next = { fail, release: () => {}, entered, match, enter } as Gate & { match: (saved: Obj) => boolean };
    gate = next; return next;
  };
  return { client, native, files, calls, disk: () => obj(JSON.parse(disk.at(-1) || '{}')), hold, setCatalog: (next: () => Obj) => { catalog = next as typeof catalog; } };
}
const holdsCapture = (saved: Obj) => (obj(saved.snapshotDrafts)['env:t'] as Obj[] | undefined)?.some(image => image.id === capture) === true;
const image = (id: string) => ({ id, name: `${id}.png`, mimeType: 'image/png', sizeBytes: 68, source: {} });

test('failed deferred persist removes only this capture; an unrelated removal is never resurrected', async () => {
  const f = fixture(); await f.client.command('draft', '', 'kept text', 0, f.native, f.files);
  f.client.local.snapshotDrafts['env:t'] = [image(unrelated)];
  const gate = f.hold(holdsCapture, true);
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await gate.entered;
  const removal = f.client.command('remove-snapshot', unrelated, '', 0, f.native, f.files);
  gate.release(); await adoption; await removal;
  expect(f.client.local.snapshotDrafts['env:t']).toEqual([]);
  expect(f.client.error).toBe(`Snapshot failed. Capture ${capture}: disk full`);
  expect(f.calls.some(call => call.op === 'snapshotAcknowledge')).toBe(false);
  expect(f.calls.filter(call => call.op === 'snapshotDraftRemove').map(call => call.id).sort()).toEqual([capture, unrelated].sort());
  // The last durable state matches memory: neither the removed image nor the failed capture.
  expect(obj(f.disk().snapshotDrafts)['env:t']).toEqual([]);
  expect(f.client.local.drafts['env:t']).toBe('kept text');
});

test('destination lost after a deferred successful persist compensates against the current array', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  f.client.local.snapshotDrafts['env:t'] = [image(unrelated)];
  const gate = f.hold(holdsCapture);
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await gate.entered;
  const removal = f.client.command('remove-snapshot', unrelated, '', 0, f.native, f.files);
  f.setCatalog(() => ({ projects: [{ id: 'p' }], threads: [] }));
  gate.release(); await adoption; await removal;
  expect(f.client.local.snapshotDrafts['env:t']).toEqual([]);
  expect(obj(f.disk().snapshotDrafts)['env:t']).toEqual([]);
  expect(f.calls.some(call => call.op === 'snapshotAcknowledge')).toBe(false);
  expect(f.calls.find(call => call.op === 'snapshotDismiss')).toMatchObject({ id: capture });
});

test('reconnect during deferred persist keeps the durable association and acknowledges on the next generation', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const gate = f.hold(holdsCapture);
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await gate.entered; f.client.generation = 2; gate.release(); await adoption;
  expect(f.client.local.snapshotDrafts['env:t'].map(saved => saved.id)).toEqual([capture]);
  expect(f.calls.some(call => ['snapshotAcknowledge', 'snapshotDismiss', 'snapshotDraftRemove'].includes(String(call.op)))).toBe(false);
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.calls.filter(call => call.op === 'snapshotDraftSave')).toHaveLength(1);
  expect(f.calls.find(call => call.op === 'snapshotAcknowledge')).toMatchObject({ id: capture, focus: true, focusOwner: JSON.stringify(['http://fixture', 'env', 'p', 't']) });
});

test('a disconnect is not a lost destination: no compensation, release or acknowledgement', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const gate = f.hold(holdsCapture);
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await gate.entered; f.client.connection = 'reconnecting'; gate.release(); await adoption;
  expect(f.client.local.snapshotDrafts['env:t'].map(saved => saved.id)).toEqual([capture]);
  expect((obj(f.disk().snapshotDrafts)['env:t'] as Obj[]).map(saved => saved.id)).toEqual([capture]);
  expect(f.calls.some(call => ['snapshotAcknowledge', 'snapshotDismiss', 'snapshotDraftRemove'].includes(String(call.op)))).toBe(false);
});

test('no eligible project: cached empty shell dismisses without fetching, reports once, delivers once a project exists', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  Object.assign(f.client, { projectId: '', threadId: '', shellLoaded: true }); f.client.shell.projects = []; f.client.shell.threads = [];
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.calls.some(call => call.op === 'http')).toBe(false);
  expect(f.calls.find(call => call.op === 'snapshotDismiss')).toMatchObject({ id: capture });
  expect(f.client.error).toBe('Snapshot taken, but no project is available. Add a project, then capture the window again.');
  f.client.error = ''; await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.error).toBe('');
  // Reference drains again later: the first ordered project receives it as a new draft.
  f.client.shell.projects = [{ id: 'q', workspaceRoot: '/q' }, { id: 'p', workspaceRoot: '/p' }];
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.projectId).toBe('q');
  expect(f.client.local.snapshotDrafts['env:new:q'].map(saved => saved.id)).toEqual([capture]);
  expect(f.calls.find(call => call.op === 'snapshotAcknowledge')).toMatchObject({ focus: true, focusOwner: JSON.stringify(['http://fixture', 'env', 'q', '']) });
});

test('focus is requested only for the destination that is still selected', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  f.client.shell.threads.push({ id: 'u', projectId: 'p' });
  const gate = f.hold(holdsCapture);
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await gate.entered; f.client.threadId = 'u'; gate.release(); await adoption;
  expect(f.calls.find(call => call.op === 'snapshotAcknowledge')).toMatchObject({ focus: false, focusOwner: JSON.stringify(['http://fixture', 'env', 'p', 't']) });
});

test('default destination follows reference preferred project order, then source order', () => {
  const projects = [{ id: 'a', workspaceRoot: '/repo/a/' }, { id: 'b', workspaceRoot: 'C:\\repo\\b' }, { id: 'c', workspaceRoot: '/repo/c' }];
  expect(snapshotDefaultProject(projects, 'env')).toBe('a');
  expect(snapshotDefaultProject(projects, 'env', ['env:/repo/c'])).toBe('c');
  expect(snapshotDefaultProject(projects, 'env', ['legacy-project-cwd:C:/repo/b', 'env:/repo/c'])).toBe('b');
  expect(snapshotDefaultProject(projects, 'env', ['other:/repo/c'])).toBe('a');
  expect(snapshotDefaultProject([], 'env', ['env:/repo/c'])).toBe('');
  expect(snapshotDestinationExists({ projects: [{ id: 'p' }], threads: [{ id: 't', projectId: 'other' }] }, 'p', 't')).toBe(false);
  expect(snapshotDestinationExists({ projects: [{ id: 'p' }] }, 'p', '')).toBe(true);
  expect(snapshotDestinationExists({ projects: [] }, '', '')).toBe(false);
  expect(snapshotIdentity(JSON.stringify(['o', 'env', 'p', 't']), 'o', 'env')).toEqual(['o', 'env', 'p', 't']);
  expect(snapshotIdentity(JSON.stringify(['o', 'other', 'p', 't']), 'o', 'env')).toBeNull();
  expect(snapshotIdentity('not json', 'o', 'env')).toBeNull();
  const drafts: Record<string, Obj[]> = { k: [image('x'), image('y')] };
  expect(withoutSnapshot(drafts, 'k', 'x')).toBe(true); expect(drafts.k.map(saved => saved.id)).toEqual(['y']);
  expect(withoutSnapshot(drafts, 'k', 'x')).toBe(false); expect(withoutSnapshot(drafts, 'missing', 'x')).toBe(false);
});
