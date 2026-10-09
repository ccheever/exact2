import { test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';
import { snapshotDraftTiles, snapshotSettings, snapshotSetupLabel, snapshotShortcutStatus, snapshotStatus, isSnapshotPermissionMessage, type SnapshotNativeState } from './snapshot-settings';
import { commandLabel, snapshotKeyLabels, snapshotShortcutAria, snapshotSystemConflict, sameSnapshotShortcut } from './snapshot-shortcut';

const ready: SnapshotNativeState = { available: true, registered: true, verified: false, message: '', screenRecording: true, accessibility: true, recording: false, candidate: '' };

test('main row status and setup button follow the reference macOS state machine', () => {
  expect(snapshotStatus({ ...ready, available: false }, true)).toBe('Checking snapshots…');
  expect(snapshotStatus(ready, false)).toBe('Turn this on to set up snapshots.');
  expect(snapshotStatus({ ...ready, registered: false, message: 'Allow Screen Recording in System Settings, then restart T3 Code.' }, true)).toBe('Capture needs attention');
  expect(snapshotStatus(ready, true)).toBe('Shortcut saved');
  expect(snapshotStatus({ ...ready, verified: true }, true)).toBe('Ready to capture');
  expect(snapshotStatus({ ...ready, registered: false }, true)).toBe('Finish shortcut setup');
  expect(snapshotSetupLabel(ready, false, true)).toBe('');
  expect(snapshotSetupLabel(ready, true, true)).toBe('');
  expect(snapshotSetupLabel({ ...ready, message: 'Allow Accessibility in System Settings, then restart T3 Code.', accessibility: false, registered: false }, true, true)).toBe('Continue setup');
  expect(snapshotSetupLabel({ ...ready, accessibility: false }, true, false)).toBe('');
  expect(snapshotSetupLabel({ ...ready, registered: false }, true, true)).toBe('Manage capture');
  expect(isSnapshotPermissionMessage('Allow Accessibility and Screen Recording in System Settings, then restart T3 Code.')).toBe(true);
  expect(isSnapshotPermissionMessage('Monitor installation failed.')).toBe(false);
});

test('shortcut status chain: recording, keybinding conflict, availability, saved', () => {
  expect(snapshotShortcutStatus({ ...ready, recording: true }, 'thread.new', null)).toBe('Press your shortcut. Esc cancels.');
  expect(snapshotShortcutStatus(ready, 'thread.new', { available: true, message: '' })).toBe('T3 Code already uses this for "Thread: New".');
  expect(snapshotShortcutStatus(ready, '', { available: true, message: '' })).toBe('Ready to save.');
  expect(snapshotShortcutStatus(ready, '', { available: false, message: 'This shortcut is already used by the system or another app.' })).toBe('This shortcut is already used by the system or another app.');
  expect(snapshotShortcutStatus(ready, '', null)).toBe('Shortcut saved.');
  expect(snapshotShortcutStatus({ ...ready, registered: false }, '', null)).toBe('');
});

test('macOS key labels, accessible names, system conflicts and command labels match the reference', () => {
  expect(snapshotKeyLabels('shift+shift')).toEqual(['⇧', '⇧']);
  expect(snapshotKeyLabels('alt+alt')).toEqual(['⌥', '⌥']);
  expect(snapshotKeyLabels('meta+ctrl+alt+shift+k')).toEqual(['⌃', '⌥', '⇧', '⌘', 'K']);
  expect(snapshotKeyLabels('ctrl+alt+plus')).toEqual(['⌃', '⌥', '+']);
  expect(snapshotKeyLabels('meta+shift+f5')).toEqual(['⇧', '⌘', 'F5']);
  expect(snapshotShortcutAria('shift+shift')).toBe('Shift + Shift');
  expect(snapshotShortcutAria('meta+meta')).toBe('Command + Command');
  expect(snapshotShortcutAria('ctrl+alt+k')).toBe('⌃⌥K');
  expect(sameSnapshotShortcut('Command+Shift+A', 'meta+shift+a')).toBe(true);
  expect(sameSnapshotShortcut('shift+shift', 'meta+meta')).toBe(false);
  expect(snapshotSystemConflict('shift+k')).toBe('Shift combinations are used for typing and text selection. Add another modifier.');
  expect(snapshotSystemConflict('meta+c')).toBe('This shortcut is Copy in most apps.');
  expect(snapshotSystemConflict('ctrl+d')).toBe('This shortcut controls running commands in terminals.');
  expect(snapshotSystemConflict('alt+tab')).toBe('The system uses Alt+Tab to switch apps.');
  expect(snapshotSystemConflict('meta+shift+c')).toBe('');
  expect(snapshotSystemConflict('shift+shift')).toBe('');
  expect(commandLabel('thread.new')).toBe('Thread: New');
  expect(commandLabel('script.build-app.run')).toBe('Run Script: Build App');
  expect(commandLabel('usage.period.week')).toBe('Usage: Period: 7 days');
  expect(commandLabel('sidebar.toggleProjects')).toBe('Sidebar: Toggle Projects');
});

function fixture(state: Obj, check: Obj = { available: true }) {
  const client = new T3Client();
  Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.local.deviceSettings.snapShotEnabled = true;
  client.config = { keybindings: [{ command: 'thread.new', shortcut: { key: 'n', modKey: true, shiftKey: true } }] };
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.op === 'snapshotState' ? state : request.op === 'snapshotCheckShortcut' ? check : {};
    return { ok: true, generation: 1, value };
  } };
  return { client, native, calls };
}

test('recorded candidate: keybinding conflict blocks without a native probe; a free chord is probed and saveable', async () => {
  const conflicted = fixture({ enabled: true, candidate: 'meta+shift+n', screenRecording: true, accessibility: true });
  const view = await snapshotSettings(conflicted.client, conflicted.native, true);
  expect(view).toMatchObject({ changed: true, canSave: false, shortcutStatus: 'T3 Code already uses this for "Thread: New".', keys: [{ label: '⇧' }, { label: '⌘' }, { label: 'N' }] });
  expect(conflicted.calls.some(call => call.op === 'snapshotCheckShortcut')).toBe(false);
  const free = fixture({ enabled: true, candidate: 'ctrl+alt+k', screenRecording: true, accessibility: true });
  expect(await snapshotSettings(free.client, free.native, true)).toMatchObject({ changed: true, canSave: true, shortcutStatus: 'Ready to save.', shortcutAria: 'Record snapshot shortcut, currently ⌃⌥K' });
  expect(free.calls.find(call => call.op === 'snapshotCheckShortcut')).toMatchObject({ shortcut: 'ctrl+alt+k', bindings: [] });
  const taken = fixture({ enabled: true, candidate: 'ctrl+alt+k' }, { available: false, message: 'This shortcut is already used by the system or another app.' });
  expect(await snapshotSettings(taken.client, taken.native, true)).toMatchObject({ canSave: false, shortcutStatus: 'This shortcut is already used by the system or another app.' });
  const typing = fixture({ enabled: true, candidate: 'shift+k' });
  expect(await snapshotSettings(typing.client, typing.native, true)).toMatchObject({ canSave: false, shortcutStatus: 'Shift combinations are used for typing and text selection. Add another modifier.' });
  expect(typing.calls.some(call => call.op === 'snapshotCheckShortcut')).toBe(false);
});

test('missing grants keep the saved choice On, report attention, and never surface as a red error', async () => {
  const revoked = fixture({ enabled: false, wanted: true, error: 'Allow Screen Recording in System Settings, then restart T3 Code.', screenRecording: false, accessibility: true });
  expect(await snapshotSettings(revoked.client, revoked.native, true)).toMatchObject({ enabled: true, statusText: 'Capture needs attention', setupLabel: 'Continue setup', error: '', message: 'Allow Screen Recording in System Settings, then restart T3 Code.', shortcutStatus: '' });
  const failed = fixture({ enabled: false, error: 'Could not register the snapshot shortcut handler.' });
  expect(await snapshotSettings(failed.client, failed.native, true)).toMatchObject({ error: 'Could not register the snapshot shortcut handler.', statusText: 'Finish shortcut setup' });
  const inactive = fixture({});
  expect(await snapshotSettings(inactive.client, inactive.native, false)).toMatchObject({ available: false, statusText: 'Checking snapshots…' });
  expect(inactive.calls).toHaveLength(0);
});

test('attachment frames carry app, title, initial and verified accessibility contents', () => {
  const client = new T3Client(); Object.assign(client, { environmentId: 'env', threadId: 't' });
  client.local.snapshotDrafts['env:t'] = [
    { id: 'a', name: 'A.png', source: { appName: 'safari', windowTitle: 'Docs', accessibility: { format: 'flat-text', text: ' Hello ' } } },
    { id: 'b', name: 'B.png', source: { appName: 'Finder', windowTitle: '' } },
  ];
  expect(snapshotDraftTiles(client)).toEqual([
    { id: 'a', name: 'A.png', snapshot: true, app: 'safari', title: 'Docs', letter: 'S', included: true, contents: 'Hello', referenced: false },
    { id: 'b', name: 'B.png', snapshot: true, app: 'Finder', title: 'Captured window', letter: 'F', included: false, contents: '', referenced: false },
  ]);
  // An image picked with Attach files (composer-editor lane) is not a capture.
  client.local.snapshotDrafts['env:t'] = [{ id: 'c', name: 'photo.png', mimeType: 'image/png', sizeBytes: 9 }];
  expect(snapshotDraftTiles(client)).toEqual([{ id: 'c', name: 'photo.png', snapshot: false, app: 'Window', title: '', letter: 'W', included: false, contents: '', referenced: false }]);
});

test('setup wizard: Allow asks native only; continue tests capture and requests the missing grants before saving On; refusal changes nothing', async () => {
  let persisted = '', refuse = false;
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(persisted).buffer; }, async atomicWriteFile(_path, bytes) { persisted = new TextDecoder().decode(bytes); } } };
  const client = new T3Client(); Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't' });
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'snapshotSetup' && refuse) return { ok: false, generation: 1, error: { kind: 'SnapShot', message: 'Permission prompts and test captures are disabled in isolated testing.', uncertain: false } };
    return { ok: true, generation: 1, value: {} };
  } };
  expect((await client.command('setting-snapshot', 'setup', 'allow-screen-recording', 0, native, files)).message).toBe('');
  expect(calls.find(call => call.op === 'snapshotSetup')).toMatchObject({ action: 'allow-screen-recording' });
  expect(calls.some(call => call.op === 'snapshotConfigure')).toBe(false);
  expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect((await client.command('setting-snapshot', 'setup', 'continue', 0, native, files)).message).toBe('');
  const order = calls.map(call => String(call.op)).filter(op => op.startsWith('snapshot'));
  // Reference enableForSetup: setupSnapShot('test-mac-capture'), then requestSnapShotPermissions(includeAccessibility).
  expect(order.slice(-3)).toEqual(['snapshotSetup', 'snapshotRequestPermissions', 'snapshotConfigure']);
  expect(calls.filter(call => call.op === 'snapshotSetup').at(-1)).toMatchObject({ action: 'test-mac-capture' });
  expect(calls.find(call => call.op === 'snapshotRequestPermissions')).toMatchObject({ includeAccessibility: true });
  expect(calls.find(call => call.op === 'snapshotConfigure')).toMatchObject({ enabled: true });
  expect(obj(obj(JSON.parse(persisted)).deviceSettings).snapShotEnabled).toBe(true);
  client.local.deviceSettings.snapShotEnabled = false; refuse = true;
  const requests = calls.filter(call => call.op === 'snapshotRequestPermissions').length;
  expect((await client.command('setting-snapshot', 'setup', 'continue', 0, native, files)).message).toBe('Permission prompts and test captures are disabled in isolated testing.');
  expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect(calls.filter(call => call.op === 'snapshotRequestPermissions')).toHaveLength(requests);
  expect((await client.command('setting-snapshot', 'setup', 'install-extension', 0, native, files)).message).toBe('Unsupported snapshot setting.');
});

test('Include app text: turning it on while capture is on requests Accessibility first (reference saveIncludeAccessibility)', async () => {
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('').buffer; }, async atomicWriteFile() {} } };
  const client = new T3Client(); Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't' });
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) { calls.push(obj(input)); return { ok: true, generation: 1, value: {} }; } };
  const ops = () => calls.map(call => String(call.op)).filter(op => op.startsWith('snapshot'));
  client.local.deviceSettings.snapShotEnabled = false;
  await client.command('setting-snapshot', 'snapShotIncludeAccessibility', 'true', 0, native, files);
  expect(ops()).toEqual(['snapshotConfigure']);
  client.local.deviceSettings.snapShotEnabled = true; calls.length = 0;
  await client.command('setting-snapshot', 'snapShotIncludeAccessibility', 'false', 0, native, files);
  expect(ops()).toEqual(['snapshotConfigure']);
  calls.length = 0;
  await client.command('setting-snapshot', 'snapShotIncludeAccessibility', 'true', 0, native, files);
  expect(ops()).toEqual(['snapshotRequestPermissions', 'snapshotConfigure']);
  expect(calls.find(call => call.op === 'snapshotRequestPermissions')).toMatchObject({ includeAccessibility: true });
  expect(client.local.deviceSettings.snapShotIncludeAccessibility).toBe(true);
});
