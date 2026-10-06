import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { noteNow } from './composer-controls';
import { handleTerminalMessage } from './terminal-drawer-view';
import { insertContext, messageContext } from './composer-editor';
import { obj } from './domain';
import { toasts } from './toast';
import { adoptTerminalContexts, buildExpiredTerminalContextToastCopy, canRunShellCommand, formatTerminalContextLabel,
  formatTerminalContextReference, isTerminalContextExpired, migrateLegacyTerminalContextPlaceholders, normalizeTerminalContextText,
  omitExpiredTerminalContexts, parseTerminalContext, runnableShellCommands, shouldClearTerminalSelectionAction,
  terminalLinkTarget, terminalContextMenuItems, terminalContextRecord, terminalSelectionLineRange, terminalSelectionMenuItems } from './terminal-integrations';

const selection = { id: 'selected-1', threadId: 't1', createdAt: '2026-10-06T00:00:00Z', terminalId: 'term-1', terminalLabel: 'Terminal 1', lineStart: 3, lineEnd: 5, text: '\nhello\r\nworld\n' };
describe('terminal context reference ports', () => {
  test('formatTerminalContextLabel and reference', () => {
    expect(formatTerminalContextLabel(selection)).toBe('Terminal 1 lines 3-5');
    expect(formatTerminalContextLabel({ ...selection, lineEnd: 3 })).toBe('Terminal 1 line 3');
    expect(formatTerminalContextReference(selection)).toBe('[Terminal 1 lines 3-5](t3-context://v1/terminal/terminal_selected-1)');
  });
  test('normalizeTerminalContextText and isTerminalContextExpired', () => {
    expect(normalizeTerminalContextText(selection.text)).toBe('hello\nworld');
    expect(isTerminalContextExpired({ text: '\r\n\n' })).toBe(true);
    expect(isTerminalContextExpired({ text: ' ' })).toBe(false);
    expect(terminalContextRecord(selection)).toMatchObject({ contextId: 'terminal_selected-1', kind: 'terminal', text: 'hello\nworld', lineStart: 3 });
  });
  test('legacy ordinal placeholders migrate in order and extra placeholders disappear', () => {
    expect(migrateLegacyTerminalContextPlaceholders('a \uFFFC b \uFFFC', [selection])).toBe(`a ${formatTerminalContextReference(selection)} b `);
  });
  test('external records validate and normalize without a client text cap', () => {
    expect(parseTerminalContext({ ...selection, lineStart: -2, lineEnd: -4 })?.lineEnd).toBe(1);
    expect(parseTerminalContext({ ...selection, lineStart: NaN })).toBeNull();
    expect(parseTerminalContext({ ...selection, text: 'a'.repeat(64001) })?.text.length).toBe(64001);
  });
  test('captured selection inserts at native caret, survives storage reload and is sent as a record', async () => {
    const client = new T3Client(); const inserted: string[] = [];
    await insertContext(client, { available: true, watch() {}, async later(input) { inserted.push(String(obj(input).text)); return { ok: true, generation: 1, value: { applied: true } }; } }, 'terminal', JSON.stringify(selection));
    expect(inserted).toEqual([formatTerminalContextReference(selection)]); // Native insertion owns the one trailing separator.
    const restored = new T3Client(); adoptTerminalContexts(restored.local, JSON.parse(JSON.stringify(client.local)));
    expect(messageContext(restored, inserted[0] ?? '')).toEqual({ version: 1, records: [terminalContextRecord(selection)] });
  });
  test('native Add to chat uses the injected snapshot clock without ambient Date access', async () => {
    const client = new T3Client();
    client.environmentId = 'env-clock'; client.threadId = 't1';
    const now = 1791302400123;
    noteNow(client, now);
    const inserted: string[] = [], instants: unknown[] = [];
    const native = { available: true, watch() {}, async later(input: unknown) {
      inserted.push(String(obj(input).text)); return { ok: true, generation: 1, value: { applied: true } };
    } };
    const original = globalThis.Date;
    globalThis.Date = new Proxy(original, {
      construct(target, args) {
        if (args.length === 0) throw new Error('new Date() is unavailable in data sources');
        instants.push(args[0]);
        return Reflect.construct(target, args);
      },
      get(target, key, receiver) {
        if (key === 'now') return () => { throw new Error('Date.now() is unavailable in data sources'); };
        return Reflect.get(target, key, receiver);
      },
    });
    try {
      expect(await handleTerminalMessage(client, native, { ...selection, type: 'selectionAction', action: 'add-to-chat' })).toBe(true);
    } finally { globalThis.Date = original; }
    expect(instants).toEqual([now]);
    expect(inserted).toHaveLength(1);
    expect(inserted[0]).toContain('Terminal 1 lines 3-5');
    expect(messageContext(client, inserted[0] ?? '').records).toMatchObject([{ text: 'hello\nworld', lineStart: 3, lineEnd: 5 }]);
  });
  test('expired-only sends refuse; mixed sends omit missing chips with guidance', () => {
    const client = new T3Client(), reference = formatTerminalContextReference(selection);
    expect(omitExpiredTerminalContexts(client, reference, false)).toEqual({ text: '', empty: true });
    expect(toasts(client).at(-1)?.title).toBe("Expired terminal context won't be sent");
    expect(omitExpiredTerminalContexts(client, `explain ${reference}`, false)).toEqual({ text: 'explain ', empty: false });
    expect(toasts(client).at(-1)?.title).toBe('Expired terminal context omitted from message');
    expect(buildExpiredTerminalContextToastCopy(2, 'omitted').title).toBe('Expired terminal contexts omitted from message');
  });
});
describe('terminal selection menu ports', () => {
  test('popup and context menus omit Add to chat without target and disable empty selection actions', () => {
    expect(terminalSelectionMenuItems()).toEqual([{ id: 'add-to-chat', label: 'Add to chat' }, { id: 'copy', label: 'Copy' }]);
    expect(terminalContextMenuItems({ hasSelection: false, canAddToChat: false })).toEqual([{ id: 'copy', label: 'Copy', disabled: true }, { id: 'paste', label: 'Paste' }]);
  });
  test('superseded selection popup cannot clear the newer context menu', () => {
    expect(shouldClearTerminalSelectionAction({ actionPending: false, openMenuRequestId: 1, currentRequestId: 2 })).toBe(false);
    expect(shouldClearTerminalSelectionAction({ actionPending: true, openMenuRequestId: 1, currentRequestId: 2 })).toBe(true);
    expect(terminalSelectionLineRange({ start: { y: 2 }, end: { y: 4 } })).toEqual({ lineStart: 3, lineEnd: 5 });
  });
});
describe('Run in terminal', () => {
  test('only a complete single-line shell command without controls can run', () => {
    for (const language of ['sh', 'bash', 'zsh', 'fish', 'shell', 'powershell', 'pwsh']) expect(canRunShellCommand(language, 'echo hello\n', false)).toBe(true);
    for (const code of ['echo hi', 'echo hi\nnext\n', 'echo \\n', 'echo\t hi\n', 'echo \u200bhi\n', '\n']) expect(canRunShellCommand('bash', code, false)).toBe(false);
    expect(canRunShellCommand('python', 'print(1)\n', false)).toBe(false);
    expect(canRunShellCommand('bash', 'echo hi\n', true)).toBe(false);
    expect(canRunShellCommand('bash', 'echo hi\n', false, false)).toBe(false);
  });
  test('fences must close with matching character and sufficient width', () => {
    expect(runnableShellCommands('```bash\necho hi\n```\n\n~~~pwsh\nGet-Date\n~~~', false)).toEqual(['bash:echo hi', 'pwsh:Get-Date']);
    expect(runnableShellCommands('````bash\necho hi\n```', false)).toEqual([]);
    expect(runnableShellCommands('```bash\necho hi\n', false)).toEqual([]);
    expect(runnableShellCommands('```bash\necho hi\n```', true)).toEqual([]);
  });
});


describe('terminal links', () => {
  test('original browser preference and modifier bypass remain distinct', () => {
    expect(terminalLinkTarget({ text: 'https://example.com' }, 'app', true)).toBe('app');
    expect(terminalLinkTarget({ text: 'https://example.com', metaKey: true }, 'app', true)).toBe('system');
    expect(terminalLinkTarget({ text: 'https://example.com', ctrlKey: true }, 'app', true)).toBe('system');
    expect(terminalLinkTarget({ text: 'https://example.com' }, 'system', true)).toBe('system');
    expect(terminalLinkTarget({ text: 'https://example.com' }, 'app', false)).toBe('system');
    expect(terminalLinkTarget({ text: 'src/a.ts:10:5' }, 'system', true)).toBe('path');
    expect(terminalLinkTarget({ text: 'javascript:alert(1)' }, 'system', true)).toBe('unsupported');
  });
});

import { runProjectTerminalScript, lastInvokedProjectScript, cardScripts } from './r6-polish-scripts';
import type { Files, Native } from './protocol';
import type { Obj } from './domain';
const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } };
const storage: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
function scriptClient(fail = '') {
  const client = new T3Client(), calls: { method: string; payload: Obj }[] = [];
  client.environmentId = 'e'; client.threadId = 't'; client.projectId = 'p';
  client.shell = { projects: [{ id: 'p', workspaceRoot: '/repo', scripts: [{ id: 'test', name: 'Test', command: 'bun test', icon: 'play' }, { id: 'build', name: 'Build', command: 'bun build', icon: 'play' }] }], threads: [{ id: 't', projectId: 'p' }], sequence: 0 };
  client.request = async (_native, method, payload) => { calls.push({ method, payload }); if (method === fail) throw new Error(`${method} refused`); return {}; };
  return { client, calls };
}
describe('project script integration', () => {
  test('script runs through open/write, remembers secondary choice and updates primary label', async () => {
    const { client, calls } = scriptClient();
    await runProjectTerminalScript(client, native, storage, 'build');
    expect(calls.map(call => call.method)).toEqual(['terminal.open', 'terminal.write']);
    expect(calls[1]?.payload).toMatchObject({ threadId: 't', data: 'bun build\r' });
    expect(lastInvokedProjectScript(client)).toBe('build');
    expect(cardScripts({}, client.shell.projects[0], lastInvokedProjectScript(client)).scriptName).toBe('Build');
    await runProjectTerminalScript(client, native, storage, '');
    expect(calls[3]?.payload.data).toBe('bun build\r');
    expect(calls[2]?.payload.terminalId).toBe(calls[0]?.payload.terminalId);
  });
  test('autoOpenPreview runs the script but reports the unmatched Browser dependency', async () => {
    const { client, calls } = scriptClient();
    const project = client.shell.projects[0];
    if (!project) throw new Error('fixture project missing');
    project.scripts = [{ id: 'preview', name: 'Preview', command: 'bun dev', autoOpenPreview: true, previewUrl: 'http://localhost:3000' }];
    await runProjectTerminalScript(client, native, storage, 'preview');
    expect(calls.map(call => call.method)).toEqual(['terminal.open', 'terminal.write']);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Could not open preview', description: 'The in-app Browser is not available in this build.' });
  });
  test('open failures never write and write failures surface original server message', async () => {
    const failedOpen = scriptClient('terminal.open');
    await expect(runProjectTerminalScript(failedOpen.client, native, storage, 'test')).rejects.toThrow('terminal.open refused');
    expect(failedOpen.calls.map(call => call.method)).toEqual(['terminal.open']);
    const failedWrite = scriptClient('terminal.write');
    await expect(runProjectTerminalScript(failedWrite.client, native, storage, 'test')).rejects.toThrow('terminal.write refused');
    expect(failedWrite.calls).toHaveLength(2);
  });
});
