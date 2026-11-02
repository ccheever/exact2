import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
import { contractFns } from '../support/contract-fn-fixture';
import { contractComponent, value } from '../support/contract-action-fixture';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { inlineDocumentEvent, inlineDocumentKey, inlineDocumentView, inlineReference, syncInlineDocuments } from '../../src/timeline/inline-documents';

const window = contractComponent('app-window.contract', 'T3Window');
test('global document confirmation focus closes and opens owners in separate commits', () => {
  const source = readFileSync(appPath('app-window.contract'), 'utf8');
  const widthExpression = /^  derive documentPermissionFocusWidth = (.+)$/m.exec(source)![1]!;
  const state = { documentPermissionFocusOwner: '' }, calls: string[] = [];
  const width = (owner: string) => value(widthExpression, { ...state, documentPermissionOwner: owner });
  expect(width('')).toBe(1);
  expect(width('doc-a:1')).toBe(2);
  const scope = (owner: string) => ({ documentPermissionOwner: owner, documentPermissionOpened: (id: string) => calls.push('open:' + id), dialogClosed: (id: string) => calls.push('close:' + id) });
  window.call('syncDocumentPermissionFocus', state, scope('doc-a:1'), [width('doc-a:1'), 1]);
  expect(calls.splice(0)).toEqual(['open:doc-a:1']);
  expect(width('doc-a:1')).toBe(1);
  expect(width('doc-b:2')).toBe(3);
  window.call('syncDocumentPermissionFocus', state, scope('doc-b:2'), [width('doc-b:2'), 1]);
  expect(calls.splice(0)).toEqual(['close:doc-a:1']);
  expect(state.documentPermissionFocusOwner).toBe('');
  expect(width('doc-b:2')).toBe(2);
  // A queued owner may disappear before the opening commit: open only the current one.
  window.call('syncDocumentPermissionFocus', state, scope('doc-c:3'), [width('doc-c:3'), 1]);
  expect(calls.splice(0)).toEqual(['open:doc-c:3']);
  window.call('syncDocumentPermissionFocus', state, scope(''), [width(''), 1]);
  expect(calls.splice(0)).toEqual(['close:doc-c:3']);
  window.call('syncDocumentPermissionFocus', state, scope(''), [width(''), 1]);
  expect(calls).toEqual([]);
  expect(width('')).toBe(1);
  expect(source).toContain('resize=syncDocumentPermissionFocus');
});

test('only the oldest permission is outside the inert background and SSH has priority', () => {
  const source = readFileSync(appPath('app-window.contract'), 'utf8');
  const expression = (name: string) => new RegExp(`^  derive ${name} = (.+)$`, 'm').exec(source)![1]!;
  const documents = [{ key: 'a', permission: 'later', permissionSerial: 9 }, { key: 'b', permission: 'first', permissionSerial: 4 }];
  const pending = value(expression('documentPermissions'), { data: { messages: [{ inlineDocuments: documents }] }, sortBy: (items: Obj[], key: (item: Obj) => number) => [...items].sort((a, b) => key(a) - key(b)) });
  const scope = { documentPermissions: pending, sshPrompt: { open: false }, confirmOp: '', data: { serverUpdate: { confirmTitle: '' }, updateEverything: { confirm: false } } };
  expect(value(expression('activeDocumentPermissions'), scope)).toEqual([documents[1]]);
  expect(value(expression('activeDocumentPermissions'), { ...scope, sshPrompt: { open: true } })).toEqual([]);
  expect(source).toContain('inert=(sshPrompt.open or activeDocumentPermissions.length > 0) testId="ssh-background"');
  expect(source).toContain('key=`${document.key}:${document.permissionSerial}`');
  expect(source).toContain('cancel=chatLocal("inline-doc-permission", document.key, `${document.permissionSerial}:deny`)');
  expect(source).toContain('confirm=chatLocal("inline-doc-permission", document.key, `${document.permissionSerial}:allow`)');
});

test('permissions collapse every fullscreen guest, queue by unique serial and reject an old confirmation', async () => {
  const native = { available: true } as Native, client = new T3Client(), writes: unknown[] = [];
  client.threadId = 'thread'; client.environmentId = 'env'; client.generation = 1; client.origin = 'http://127.0.0.1:1';
  const items = ['a', 'b'].map(id => ({ id, type: 'dynamic_tool', status: 'completed', toolName: 'weather.forecast', output: { t3McpApp: { attachmentId: id, server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } }));
  client.thread = { projection: { thread: { id: 'thread' }, visibleTurnItems: items.map(item => ({ sourceThreadId: 'thread', sourceItemId: item.id, item })) } };
  client.rpc = async (_native, method, payload, write) => {
    if (method === 'assets.createUrl') return { relativeUrl: '/api/assets/' + obj(obj(payload).resource).attachmentId, expiresAt: 10_000_000 };
    if (method === 'mcpApps.toolInfo') return { callable: true, readOnly: false };
    if (method === 'mcpApps.callTool') writes.push({ payload, write });
    return { content: [] };
  };
  await syncInlineDocuments(client, native, 0, 'light');
  const refs = items.map(item => inlineReference(item)!);
  const keys = items.map((item, i) => inlineDocumentKey(client, 'thread', item.id, refs[i]!));
  const view = (i: number) => inlineDocumentView(client, keys[i]!, refs[i]!);
  const receive = (i: number, id: number) => {
    const token = JSON.parse(decodeURIComponent(view(i).src.split('#')[1]!)).token;
    return inlineDocumentEvent(client, keys[i]!, 'message', JSON.stringify({ t3Document: token, message: { jsonrpc: '2.0', id, method: 'tools/call', params: { name: 'setForecast', arguments: {} } } }), native);
  };
  const sources = [view(0).src, view(1).src];
  await inlineDocumentEvent(client, keys[0]!, 'fullscreen', 'open', native);
  expect(view(0).fullscreen).toBe(true);
  await receive(1, 1);
  const first = view(1).permissionSerial;
  expect(view(0).fullscreen).toBe(false);
  await receive(0, 2);
  expect(view(0).permissionSerial).toBeGreaterThan(first);
  await inlineDocumentEvent(client, keys[1]!, 'permission', `${first}:deny`, native);
  await receive(1, 3);
  const next = view(1).permissionSerial;
  expect(next).toBeGreaterThan(view(0).permissionSerial);
  await inlineDocumentEvent(client, keys[1]!, 'permission', `${first}:allow`, native);
  expect(view(1).permissionSerial).toBe(next);
  expect(view(1).permission).not.toBe('');
  expect(writes).toEqual([]);
  await inlineDocumentEvent(client, keys[1]!, 'permission', `${next}:allow`, native);
  await syncInlineDocuments(client, native, 1, 'light');
  expect(writes).toMatchObject([{ payload: { threadId: 'thread', itemId: 'b', name: 'setForecast' }, write: true }]);
  expect([view(0).src, view(1).src]).toEqual(sources);
});


test('a replaced permission keeps its outside focus lineage until the final confirmation closes', () => {
  const helpers = contractFns('dialog-trap.contract', ['DialogFocusFrame', 'DialogFocusReturn']);
  const state = { focusEditable: false, documentPermissionFocusOwner: '', documentPermissionReturnKey: '', documentPermissionReturnSource: '', dialogFrames: [], dialogSeated: [], dialogFocusOwner: '', dialogFocusId: 'guest-frame', dialogFocusPopup: '', dialogFocusInvoker: '' };
  let owner = 'doc-a:1', live = [owner];
  const focused: string[] = [];
  const scope = () => ({ ...helpers, activeDocumentPermissions: [], data: { messages: [] }, documentPermissionOwner: owner, stopPropagation() {}, chipFocusChanged() {}, focus: (id: string) => focused.push(id),
    frame: (id: string) => ({ unavailable: !live.includes(id), width: live.includes(id) ? 300 : 0, height: live.includes(id) ? 160 : 0 }),
    querySelectorAll: () => live,
    documentPermissionOpened: (id: string) => window.call('documentPermissionOpened', state, scope(), [id]),
    dialogOpened: (id: string) => window.call('dialogOpened', state, scope(), [id]),
    dialogClosed: (id: string) => window.call('dialogClosed', state, scope(), [id]),
  });
  window.call('syncDocumentPermissionFocus', state, scope(), [1, 1]);
  window.call('dialogEntered', state, scope(), [owner, { target: owner + '-cancel' }]);
  owner = 'doc-b:2'; live = [owner];
  window.call('syncDocumentPermissionFocus', state, scope(), [1, 1]);
  expect(focused).toEqual([]);
  window.call('syncDocumentPermissionFocus', state, scope(), [1, 1]);
  window.call('dialogEntered', state, scope(), [owner, { target: owner + '-cancel' }]);
  expect(focused).toEqual([]);
  owner = ''; live = [];
  window.call('syncDocumentPermissionFocus', state, scope(), [1, 1]);
  expect(focused).toEqual(['guest-frame']);
});

function nativeFocusFixture(prior = '') {
  const helpers = contractFns('dialog-trap.contract', ['DialogFocusFrame', 'DialogFocusReturn']);
  const state = { focusEditable: false, documentPermissionFocusOwner: '', documentPermissionReturnKey: '', documentPermissionReturnSource: '', dialogFrames: [], dialogSeated: [], dialogFocusOwner: '', dialogFocusId: prior, dialogFocusPopup: '', dialogFocusInvoker: '' };
  const documents = [{ key: 'a', src: 'adapter#a-generation1', permissionSerial: 1 }, { key: 'b', src: 'adapter#b-generation1', permissionSerial: 2 }];
  const nodes = new Set(['inline-frame-a', 'inline-frame-b', ...(prior ? [prior] : [])]);
  const live = new Set<string>(), focused: string[] = [];
  let owner = '', active: typeof documents = [];
  const scope = () => ({ ...helpers, documentPermissionOwner: owner, activeDocumentPermissions: active, data: { messages: [{ inlineDocuments: documents }] }, stopPropagation() {}, chipFocusChanged() {}, focus: (id: string) => focused.push(id),
    frame: (id: string) => ({ unavailable: !nodes.has(id) && !live.has(id), width: nodes.has(id) || live.has(id) ? 300 : 0, height: nodes.has(id) || live.has(id) ? 160 : 0 }),
    querySelectorAll: () => [...live],
    documentPermissionOpened: (id: string) => window.call('documentPermissionOpened', state, scope(), [id]),
    dialogOpened: (id: string) => window.call('dialogOpened', state, scope(), [id]),
    dialogClosed: (id: string) => window.call('dialogClosed', state, scope(), [id]),
  });
  const call = (name: string, ...args: unknown[]) => window.call(name, state, scope(), args);
  const show = (index: number) => { active = [documents[index]!]; owner = `inline-permission-${active[0]!.key}-${active[0]!.permissionSerial}`; live.clear(); live.add(owner); };
  const sync = () => call('syncDocumentPermissionFocus', 2, 1);
  const enter = () => call('dialogEntered', owner, { target: owner + '-cancel' });
  const close = () => { owner = ''; active = []; live.clear(); sync(); };
  return { state, documents, nodes, live, focused, call, show, sync, enter, close };
}

test('an empty native WK ledger returns to the requesting guest, including focusin before layout', () => {
  for (const enteredFirst of [false, true]) {
    const f = nativeFocusFixture(); f.show(0);
    if (enteredFirst) f.enter();
    f.sync(); f.enter(); f.close();
    expect(f.focused).toEqual(['inline-frame-a']);
    expect(f.state.documentPermissionReturnSource).toBe('adapter#a-generation1');
  }
});

test('a real prior app control and a queued dialog keep their original outside focus lineage', () => {
  const control = nativeFocusFixture('composer-input'); control.show(0); control.sync(); control.enter(); control.close();
  expect(control.focused).toEqual(['composer-input']);
  const f = nativeFocusFixture(); f.show(0); f.sync(); f.enter();
  f.show(1); f.sync();
  expect(f.focused).toEqual([]);
  f.sync(); f.enter(); f.close();
  expect(f.focused).toEqual(['inline-frame-a']);
});

test('removed, unmounted and same-id replacement guests cannot receive an old focus return', () => {
  for (const change of ['removed', 'unmounted', 'replacement']) {
    const f = nativeFocusFixture(); f.show(0); f.sync(); f.enter();
    if (change === 'removed') f.documents.splice(0, 1);
    if (change === 'unmounted') f.nodes.delete('inline-frame-a');
    if (change === 'replacement') f.documents[0]!.src = 'adapter#a-generation2';
    f.close();
    expect(f.focused).toEqual([]);
    expect(f.state.dialogFrames).toEqual([]);
  }
});

test('a nested confirmation cannot return to a replaced guest inherited from its parent', () => {
  const f = nativeFocusFixture(); f.show(0); f.sync(); f.enter();
  f.live.add('nested'); f.call('dialogOpened', 'nested'); f.call('dialogEntered', 'nested', { target: 'nested-cancel' });
  f.documents[0]!.src = 'adapter#a-generation2';
  f.live.clear(); f.call('dialogClosed', 'nested');
  expect(f.focused).toEqual([]);
});
