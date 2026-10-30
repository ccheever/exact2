// Integrations › Devices › EAS Simulator: easDeviceHostsSettings.logic.test.ts (T3 Code 74541bab17) and the
// editor's account check and writes (EasDeviceHostsSettings.tsx).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { easAccountCheck, easEditorView, easHostProblem, easHostsCommand, easHostsView, easSessionLimits, newEasDeviceHost, updateEasDeviceHosts } from '../../src/device/settings-eas-hosts';
import { toasts } from '../../src/shell/toast';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const REDACTED = '••••••';

describe('EAS device host settings', () => {
  test('adds, edits, and removes one EAS host without touching the others', () => {
    const other = newEasDeviceHost('eas-0'), eas = newEasDeviceHost('eas-1');
    const added = updateEasDeviceHosts([other], eas, false);
    expect(added).toEqual([other, eas]);
    const edited = { ...eas, label: 'Cloud iPhone', maxConcurrentSessions: 1 };
    expect(updateEasDeviceHosts(added, edited, false)).toEqual([other, edited]);
    expect(updateEasDeviceHosts(added, eas, true)).toEqual([other]);
  });

  test('rejects limits EAS does not allow', () => {
    expect(easHostProblem(newEasDeviceHost('eas'))).toBe('');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), maxSessionMinutes: 10 })).toBe('Idle minutes must be smaller than session minutes.');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), maxSessionMinutes: 600 })).toBe('Give the host a label and limits within the ranges shown.');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), label: ' ' })).toBe('Give the host a label and limits within the ranges shown.');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), maxConcurrentSessions: 1.5 })).toBe('Give the host a label and limits within the ranges shown.');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), idleMinutes: 0 })).toBe('Give the host a label and limits within the ranges shown.');
    expect(easHostProblem({ ...newEasDeviceHost('eas'), idleMinutes: 61, maxSessionMinutes: 115 })).toBe('Give the host a label and limits within the ranges shown.');
    expect(easHostProblem({ ...newEasDeviceHost('local') })).toBe('Give the host a label and limits within the ranges shown.');
  });

  test('describes a host\'s bounds in one line', () => {
    expect(easSessionLimits({ ...newEasDeviceHost('eas'), maxConcurrentSessions: 1 })).toBe('Up to 1 session at once, each stopped after 30 min or 10 idle min');
    expect(easSessionLimits(newEasDeviceHost('eas'))).toBe('Up to 2 sessions at once, each stopped after 30 min or 10 idle min');
  });

  test('the dialog\'s problem line agrees with Save\'s check over the fields\' text', () => {
    const fns = contractFns('src/device/settings-eas-hosts.contract', { floor: Math.floor });
    // `match parseNumber(text)` is not JavaScript; easCount here is parseNumber's reading of the same text.
    fns.easCount = (text: string) => text.trim() === '' ? 0 : /^\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?\s*$/.test(text) ? Number(text) : -1;
    const problem = fns.easProblem as (label: string, sessions: string, sessionMinutes: string, idleMinutes: string) => string;
    const cases: [string, string, string, string][] = [['EAS Simulator', '2', '30', '10'], ['EAS Simulator', '2', '10', '10'], ['EAS Simulator', '2', '10', '30'], [' ', '2', '30', '10'],
      ['Cloud', '0', '30', '10'], ['Cloud', '21', '30', '10'], ['Cloud', '2', '116', '10'], ['Cloud', '2', '115', '60'], ['Cloud', '2', '', '10'], ['Cloud', '2', '30', ''],
      ['Cloud', '1.5', '30', '10'], ['Cloud', '2', 'abc', '10'], ['Cloud', '2', '30', '61']];
    for (const [label, sessions, sessionMinutes, idleMinutes] of cases) {
      const host = { ...newEasDeviceHost('eas'), label, maxConcurrentSessions: Number(sessions || 0), maxSessionMinutes: Number(sessionMinutes || 0), idleMinutes: Number(idleMinutes || 0) };
      expect([label, sessions, sessionMinutes, idleMinutes, problem(label, sessions, sessionMinutes, idleMinutes)]).toEqual([label, sessions, sessionMinutes, idleMinutes, easHostProblem(host)]);
    }
  });
});

describe('the EAS hosts row and editor', () => {
  const fixture = () => {
    const requests: [string, Obj][] = [];
    const saved = { kind: 'eas', id: 'cloud', label: 'Cloud iPhone', accessToken: REDACTED, maxSessionMinutes: 40, idleMinutes: 5, maxConcurrentSessions: 1 };
    const settings: Obj = { easDeviceHosts: [saved] };
    const replies: Record<string, (payload: Obj) => Obj> = {};
    const client = { ready: true, environmentId: 'env1', generation: 1, config: { environment: { label: 'Studio' }, settings },
      restAccess: () => ({ request: async (method: string, payload: Obj) => {
        requests.push([method, payload]);
        if (replies[method]) return replies[method]!(payload);
        if (method === 'server.getSettings') return settings;
        if (method === 'server.updateSettings') return { ...settings, ...(payload.patch as Obj) };
        return {};
      } }) } as unknown as T3Client;
    return { client, requests, replies, settings, saved };
  };
  const native = { available: true } as unknown as Native;

  test('the row shows only on servers that support EAS hosts, and is off where the reference disables it', () => {
    const { client, settings } = fixture();
    expect(easHostsView(client, settings, { hosts: [] }, false, true, true).supported).toBe(false);
    const view = easHostsView(client, settings, { hosts: [], supportsEasHosts: true }, false, true, true);
    expect(view).toEqual({ supported: true, disabled: false, editing: false, hosts: [{ id: 'cloud', label: 'Cloud iPhone', limits: 'Up to 1 session at once, each stopped after 40 min or 5 idle min', first: true }] });
    expect(easHostsView(client, settings, { supportsEasHosts: true }, true, true, true).disabled).toBe(true);
    expect(easHostsView(client, settings, { supportsEasHosts: true }, false, false, true).disabled).toBe(true);
    expect(easHostsView(client, settings, { supportsEasHosts: true }, false, true, false).disabled).toBe(true);
  });

  test('a host saved while the save ran draws its Edit and Remove at disabled:opacity-64, then at 1 once it ends', () => {
    const row = contractComponent('src/device/settings-eas-hosts.contract', 'EasDeviceHostsRow');
    const button = contractComponent('src/device/settings-eas-hosts.contract', 'EasGhostButton');
    const opacity = (label: string, busy: boolean) => button.attribute('button id=buttonId', 'opacity', { over: false },
      { destructive: label === 'Remove', disabled: row.attribute(`EasGhostButton(label="${label}"`, 'disabled', {}, { busy, view: { disabled: false } }) });
    expect([opacity('Edit', true), opacity('Remove', true), opacity('Edit', false), opacity('Remove', false)]).toEqual([0.64, 0.64, 1, 1]);
  });

  test('Add opens a new host with the defaults and checks the account; a token edit or a later check makes an answer stale', async () => {
    const { client, requests, replies } = fixture();
    await expect(easHostsCommand(client, native, 'eas-open', {}, true)).rejects.toThrow('EAS hosts belong to the environment.');
    await easHostsCommand(client, native, 'eas-open', {}, false);
    const editor = easEditorView(client);
    expect(editor).toMatchObject({ open: true, isNew: true, title: 'Add EAS host', label: 'EAS Simulator', tokenSaved: false, appId: '', sessions: '2', sessionMinutes: '30', idleMinutes: '10', accountSeq: 0, accountStatus: 'checking', apps: [] });
    expect(editor.id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    expect(easHostsView(client, {}, { supportsEasHosts: true }, false, true, true).editing).toBe(true);
    // The dialog's first check: no token yet, so the server's EXPO_TOKEN or eas login answers.
    replies['device.easAccount'] = () => ({ signedInAs: 'fixture-user', credentialSource: 'env', apps: [{ id: 'app-1', fullName: '@fixture/app' }] });
    await easAccountCheck(client, native, { serial: editor.serial, seq: '1', tokenKind: 'cleared', token: '' });
    expect(requests.at(-1)).toEqual(['device.easAccount', { accessToken: '' }]);
    expect(easEditorView(client)).toMatchObject({ accountSeq: 1, accountStatus: 'signed-in', accountMessage: 'Signed in as fixture-user with EXPO_TOKEN on the server.', apps: [{ id: 'app-1', fullName: '@fixture/app' }] });
    // A typed token is checked as given; an answer to an older check never replaces a newer one.
    replies['device.easAccount'] = () => { throw new Error('Expo rejected the credential (HTTP 401).'); };
    await easAccountCheck(client, native, { serial: editor.serial, seq: '3', tokenKind: 'new', token: 'tok' });
    expect(requests.at(-1)).toEqual(['device.easAccount', { accessToken: 'tok' }]);
    expect(easEditorView(client)).toMatchObject({ accountSeq: 3, accountStatus: 'failed', accountMessage: 'Expo rejected the credential (HTTP 401).', apps: [] });
    replies['device.easAccount'] = () => ({ signedInAs: 'late', credentialSource: 'settings', apps: [] });
    await easAccountCheck(client, native, { serial: editor.serial, seq: '2', tokenKind: 'new', token: 'old' });
    expect(easEditorView(client).accountSeq).toBe(3);
    // A check from an editor that has since closed answers nothing.
    await easAccountCheck(client, native, { serial: String(Number(editor.serial) - 1), seq: '9', tokenKind: 'cleared', token: '' });
    expect(easEditorView(client).accountSeq).toBe(3);
  });

  test('a dialog that mounts again over the open editor starts its checks over; the earlier dialog\'s answers are dropped', async () => {
    const { client, replies } = fixture();
    await easHostsCommand(client, native, 'eas-open', {}, false);
    const { serial } = easEditorView(client);
    replies['device.easAccount'] = () => ({ signedInAs: 'first', credentialSource: 'env', apps: [] });
    for (const seq of ['1', '2', '3']) await easAccountCheck(client, native, { serial, seq, tokenKind: 'cleared', token: '' });
    expect(easEditorView(client)).toMatchObject({ accountSeq: 3, accountStatus: 'signed-in' });
    // The earlier dialog's check 4 is still in flight when Settings closes and opens again.
    let answerLate: (value: Obj) => void = () => {};
    replies['device.easAccount'] = () => new Promise<Obj>(resolve => { answerLate = resolve; }) as unknown as Obj;
    const late = easAccountCheck(client, native, { serial, seq: '4', tokenKind: 'cleared', token: '' });
    replies['device.easAccount'] = () => ({ signedInAs: 'second', credentialSource: 'env', apps: [] });
    const remount = easAccountCheck(client, native, { serial, seq: '1', tokenKind: 'cleared', token: '' });
    expect(easEditorView(client)).toMatchObject({ accountSeq: 0, accountStatus: 'checking' });
    await remount;
    expect(easEditorView(client)).toMatchObject({ accountSeq: 1, accountStatus: 'signed-in', accountMessage: 'Signed in as second with EXPO_TOKEN on the server.' });
    answerLate({ signedInAs: 'first', credentialSource: 'env', apps: [] });
    await late;
    expect(easEditorView(client)).toMatchObject({ accountSeq: 1, accountMessage: 'Signed in as second with EXPO_TOKEN on the server.' });
  });

  test('Enter in Name, the token or a limit saves a valid draft, as the reference\'s form submits', async () => {
    const dialog = contractComponent('src/device/settings-eas-hosts.contract', 'EasHostDialog');
    const sent: [string, string][] = [];
    const rest = (op: string, value: string) => { sent.push([op, value]); };
    for (const [problem, busy] of [['', false], ['Idle minutes must be smaller than session minutes.', false], ['', true]] as const)
      dialog.call('submitDraft', dialog.state(), { problem, busy, saveInput: 'serial=1&label=Cloud', rest }, []);
    expect(sent).toEqual([['eas-save', 'serial=1&label=Cloud']]);
    const source = await Bun.file(appPath('src/device/settings-eas-hosts.contract')).text();
    for (const field of ['id="eas-host-name"', 'id="eas-host-token"']) expect(source.split('\n').find(line => line.includes(field))).toContain('submit=submitDraft');
    expect(source.split('\n').filter(line => line.includes('EasLimitField(label=') && line.includes('submit=submitDraft')).length).toBe(3);
    expect(source.split('\n').find(line => line.includes('input id=fieldId'))).toContain('submit=submit ');
  });

  test('Save writes the draft beside the other hosts; an invalid draft is refused with the editor\'s reason', async () => {
    const { client, requests, saved } = fixture();
    await easHostsCommand(client, native, 'eas-open', {}, false);
    const { serial, id } = easEditorView(client);
    const draft = { serial, label: ' Cloud iPad ', tokenKind: 'new', token: 'expo-token', appId: 'app-1', sessions: '3', sessionMinutes: '20', idleMinutes: '20' };
    await expect(easHostsCommand(client, native, 'eas-save', draft, false)).rejects.toThrow('Idle minutes must be smaller than session minutes.');
    await easHostsCommand(client, native, 'eas-save', { ...draft, idleMinutes: '15' }, false);
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { easDeviceHosts: [saved,
      { kind: 'eas', id, label: 'Cloud iPad', appId: 'app-1', accessToken: 'expo-token', maxSessionMinutes: 20, idleMinutes: 15, maxConcurrentSessions: 3 }] } }]);
    expect(easEditorView(client).open).toBe(false);
  });

  test('Edit keeps a saved token by sending it back redacted, checks it by host id, and clears it on request', async () => {
    const { client, requests, replies, saved } = fixture();
    await easHostsCommand(client, native, 'eas-edit', { id: 'cloud' }, false);
    const editor = easEditorView(client);
    expect(editor).toMatchObject({ isNew: false, title: 'Edit EAS host', label: 'Cloud iPhone', tokenSaved: true, sessions: '1', sessionMinutes: '40', idleMinutes: '5' });
    replies['device.easAccount'] = () => ({ signedInAs: 'fixture-user', credentialSource: 'settings', apps: [] });
    await easAccountCheck(client, native, { serial: editor.serial, seq: '1', tokenKind: 'saved', token: '' });
    expect(requests.at(-1)).toEqual(['device.easAccount', { hostId: 'cloud' }]);
    expect(easEditorView(client).accountMessage).toBe('Signed in as fixture-user with the access token saved here.');
    const fields = { serial: editor.serial, label: 'Cloud iPhone', appId: '', sessions: '1', sessionMinutes: '40', idleMinutes: '5' };
    await easHostsCommand(client, native, 'eas-save', { ...fields, tokenKind: 'saved', token: '' }, false);
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { easDeviceHosts: [saved] } }]);
    await easHostsCommand(client, native, 'eas-edit', { id: 'cloud' }, false);
    await easHostsCommand(client, native, 'eas-save', { ...fields, serial: easEditorView(client).serial, tokenKind: 'cleared', token: '' }, false);
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { easDeviceHosts: [{ ...saved, accessToken: '' }] } }]);
  });

  test('Remove drops one host; a refused write toasts and keeps the editor open', async () => {
    const { client, requests, replies, settings } = fixture();
    await easHostsCommand(client, native, 'eas-remove', { id: 'cloud' }, false);
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { easDeviceHosts: [] } }]);
    expect(easHostsView(client, client.config.settings as Obj, { supportsEasHosts: true }, false, true, true).hosts).toEqual([]);
    client.config = { ...client.config, settings };
    replies['server.updateSettings'] = () => { throw new Error('Device host ids must be unique.'); };
    await easHostsCommand(client, native, 'eas-remove', { id: 'cloud' }, false);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'EAS host not removed', description: 'Device host ids must be unique.' });
    await easHostsCommand(client, native, 'eas-open', {}, false);
    const { serial } = easEditorView(client);
    await easHostsCommand(client, native, 'eas-save', { serial, label: 'Cloud', tokenKind: 'cleared', token: '', appId: '', sessions: '2', sessionMinutes: '30', idleMinutes: '10' }, false);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'EAS host not saved', description: 'Device host ids must be unique.' });
    expect(easEditorView(client).open).toBe(true);
    await easHostsCommand(client, native, 'eas-close', {}, false);
    expect(easEditorView(client).open).toBe(false);
  });
});
