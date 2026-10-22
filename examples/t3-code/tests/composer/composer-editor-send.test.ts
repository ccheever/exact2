// Lane composer-editor: the send gestures of 5c0429b (⌥⌘↩ sends and opens a new
// thread) and 263097a (⌘↩ starts a draft in the background), end to end
// through the client: the press's gesture, the server's keybindings, the writes.
import { STAGED } from '../../src/composer/composer-staged-copies';
import { memoryFiles, text } from '../support/app-files-fixture';
import { describe, expect, test } from 'bun:test';
import { connected, opened, running } from '../support/composer-controls-fixture';
import { snapshot } from '../../src/client/presentation';
import { DEFAULT_SEND_RULES, pressGesture, sendIntent } from '../../src/composer/composer-editor-intent';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { toasts } from '../../src/shell/toast';
import { appPath } from '../support/app-files';
import { obj } from '../../src/client/domain';
import { contextId, contextLink } from '../../src/composer/composer-editor-menu';
import { ATTACHMENT_ONLY_PROMPT } from '../../src/composer/queued-edit-attachments';
import { acceptAttachedFile } from '../../src/composer/composer-editor-attach';
import { fileChipLink } from '../../src/composer/composer-editor-files';

describe('submission text (ChatView onSend, 9c161462c9)', () => {
  for (const existing of [true, false]) {
    const setup = existing ? opened : connected;
    const route = existing ? 'an existing thread' : 'a new thread';
    const sentMessage = (payload: unknown) => existing ? obj(payload) : obj(obj(payload).initialMessage);
    test.each([
      { label: 'plain text', prompt: '  fresh spaced text  ', text: 'fresh spaced text', title: 'fresh spaced text' },
      { label: 'multiline Markdown', prompt: '\t\n  # Heading\n\nKeep  two spaces.  \nNext line.\n\n```ts\n  indent();\n```\n \t', text: '# Heading\n\nKeep  two spaces.  \nNext line.\n\n```ts\n  indent();\n```' },
    ])(`trims outer whitespace for ${route} while retaining inner whitespace: $label`, async ({ prompt, text, title }) => {
      const { client, native, command } = await setup();
      await command('draft', '', prompt);
      await command('send', '', prompt);
      const sent = native.committed.at(-1);
      expect(client.error).toBe('');
      expect(sentMessage(sent)).toMatchObject({ text, attachments: [] });
      if (title) expect(existing ? sent?.titleSeed : sent?.title).toBe(title);
      expect(client.draft).toBe('');
    });
    test(`preserves context references and their records for ${route} after trimming`, async () => {
      const { client, native, command } = await setup();
      const id = contextId('thread', 't2'), link = contextLink('thread', id, 'Thread t2');
      await command('send', '', ` \n ${link}  review this \n `);
      expect(client.error).toBe('');
      expect(sentMessage(native.committed.at(-1))).toMatchObject({
        text: `${link}  review this`,
        context: { version: 1, records: [{ kind: 'thread', contextId: id, threadId: 't2', environmentId: 'env1' }] },
      });
    });
    test(`trims whitespace exposed by an expired terminal reference for ${route}`, async () => {
      const { client, native, command } = await setup();
      await command('send', '', ' [old terminal](t3-context://v1/terminal/terminal_old)\n  Keep  this  ');
      expect(client.error).toBe('');
      expect(sentMessage(native.committed.at(-1))).toMatchObject({ text: 'Keep  this' });
      expect(sentMessage(native.committed.at(-1)).context).toBeUndefined();
    });
    test(`sends the attachment-only prompt for ${route} titled "Attached image" (#19822)`, async () => {
      const { client, native, command } = await setup();
      const key = client.draftKey;
      client.local.snapshotDrafts[key] = [{ id: 'image-local', uploadId: 'image-upload', name: 'screen.png', mimeType: 'image/png', sizeBytes: 64 }];
      await command('send', '', ' \t\n ');
      const sent = native.committed.at(-1);
      expect(client.error).toBe('');
      expect(sentMessage(sent)).toMatchObject({ text: ATTACHMENT_ONLY_PROMPT, attachments: [{ id: 'image-upload', type: 'image', name: 'screen.png' }],
        context: { version: 1, records: [{ kind: 'image', contextId: 'image_image-local', attachmentId: 'image-upload', name: 'screen.png', sizeBytes: 64 }] } });
      expect(existing ? sent?.titleSeed : sent?.title).toBe('Attached image');
      expect(client.local.snapshotDrafts[key]).toEqual([]);
      expect(client.draft).toBe('');
    });
    test(`rejects whitespace without attachments for ${route}`, async () => {
      const { native, command } = await setup();
      const before = native.committed.length;
      const result = await command('send', '', ' \t\n ');
      expect(result.message).toBe('Write a message or attach an image first.');
      expect(native.committed.length).toBe(before);
    });
    test.each([{ label: 'file only', withImage: false }, { label: 'file and image', withImage: true }])(`uploads and binds attachments for ${route}: $label`, async ({ withImage }) => {
      const { client, native, command } = await setup();
      const file = acceptAttachedFile(client, { kind: 'file', id: '00000000-0000-4000-8000-000000000001', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 5 }).file!;
      const later = native.later.bind(native), uploaded: unknown[] = [];
      const { released } = memoryFiles({ [`${STAGED}/${file.id}`]: text('hello') });
      native.later = async input => {
        const request = obj(input);
        if (request.op === 'request' && request.method === 'attachments.createUploadUrl') return native.ok({ attachmentId: 'file-upload', relativeUrl: '/api/attachments/upload/file-upload' });
        if (request.op === 'uploadAttachment') { uploaded.push(request); return native.ok({}); }
        return later(input);
      };
      if (withImage) client.local.snapshotDrafts[client.draftKey] = [{ id: 'image-local', uploadId: 'image-upload', name: 'screen.png', mimeType: 'image/png', sizeBytes: 64 }];
      await command('send', '', ` \n ${fileChipLink(file)}  `);
      const sent = sentMessage(native.committed.at(-1));
      expect(client.error).toBe('');
      expect(sent).toMatchObject({
        text: fileChipLink(file),
        context: { version: 1, records: [...(withImage ? [{ kind: 'image', contextId: 'image_image-local', attachmentId: 'image-upload', name: 'screen.png', sizeBytes: 64 }] : []),
          { kind: 'file', contextId: file.contextId, attachmentId: 'file-upload', sizeBytes: 5 }] },
      });
      expect(sent.attachments).toEqual([...(withImage ? [expect.objectContaining({ id: 'image-upload', type: 'image' })] : []),
        { type: 'file', id: 'file-upload', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 5 }]);
      expect(uploaded).toMatchObject([{ base64: 'aGVsbG8=', contentType: 'text/plain', path: '/api/attachments/upload/file-upload' }]);
      expect(released()).toEqual([file.id]);
      expect(client.draft).toBe('');
    });
  }
});

describe('send gestures through the server keybindings', () => {
  test('⌥⌘↩ on an existing thread sends there, then opens a fresh composer in its project', async () => {
    const { client, native, command } = await opened();
    const project = client.projectId;
    await command('send', 'key:meta+alt', 'Keep going');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't1', text: 'Keep going', dispatchMode: { type: 'start_immediately' } });
    expect(client.threadId).toBe('');
    expect(client.projectId).toBe(project);
    expect(client.error).toBe('');
  });
  test('⌥⌘↩ while the thread runs follows the follow-up behaviour (queue), then opens the fresh composer', async () => {
    const { client, native, command } = await opened();
    running(client);
    await command('send', 'key:meta+alt', 'After this');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't1', dispatchMode: { type: 'queue_after_active' } });
    expect(client.threadId).toBe('');
  });
  test('a plain send or ⌘↩ on an idle thread stays on it', async () => {
    const { client, native, command } = await opened();
    await command('send', '', 'Plain');
    expect(client.threadId).toBe('t1');
    await command('send', 'key:meta', 'Command return');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Command return' });
    expect(client.threadId).toBe('t1');
  });
  test('⌘↩ in a draft starts the thread in the background (a HEAD server binds it)', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES;
    await command('send', 'key:meta', 'Background from command return');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Started in background' });
  });
  test('an older server that binds only ⌥⌘↩ keeps ⌘↩ in a draft a foreground send', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES.filter((rule, index) => index !== 1);
    await command('send', 'key:meta', 'Foreground');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).not.toBe('');
  });
  test('the composer takes ⌥⌘↩ on an existing thread', async () => {
    const { client } = await opened();
    client.config.keybindings = DEFAULT_SEND_RULES;
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
  });
  test('a new thread is titled from its prose with each chip read as its label (#19822)', async () => {
    const { native, command } = await connected();
    await command('send', '', 'Look at [notes.md](t3-context://v1/file/file_x) please');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', title: 'Look at notes.md please' });
  });
});

// A pointer press carries its own modifiers (its MouseEvent) to `send`, as `pointer:meta+alt`, and a send chord typed
// in the composer its own, as `key:meta+alt` (app.contract composerKeyEvent); any other press is a plain send.
describe('a Send press carries its own pointer modifiers', () => {
  test('pressGesture: a press names its gesture; a plain one is a plain send', () => {
    expect(pressGesture('')).toEqual({});
    expect(pressGesture('pointer:')).toEqual({});
    expect(pressGesture('key:meta+alt')).toEqual({ modifiers: 'meta+alt', source: 'key', ageMs: 0 });
    expect(pressGesture('key:')).toEqual({ modifiers: '', source: 'key', ageMs: 0 });
    expect(pressGesture('pointer:meta')).toEqual({ modifiers: 'meta', source: 'pointer', ageMs: 0 });
    expect(pressGesture('pointer:meta+alt+shift')).toEqual({ modifiers: 'meta+alt+shift', source: 'pointer', ageMs: 0 });
    expect(pressGesture('pointer:control+bogus')).toEqual({ modifiers: 'control', source: 'pointer', ageMs: 0 });
    // A ⌘-click while a turn runs is the alternate; a pointer never resolves a key chord (⌥⌘-click in a draft stays foreground).
    expect(sendIntent({}, pressGesture('pointer:meta')!, true, false)).toBe('alternate');
    expect(sendIntent({}, pressGesture('pointer:control')!, true, false)).toBe('alternate');
    expect(sendIntent({ keybindings: DEFAULT_SEND_RULES }, pressGesture('pointer:meta+alt')!, false, true)).toBe('foreground');
    expect(sendIntent({}, pressGesture('pointer:shift')!, true, false)).toBe('foreground');
  });
  test('the client takes a ⌘-click from the press', async () => {
    const { client, native, command } = await opened();
    running(client);
    await command('send', 'pointer:meta', 'Steer by click');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer', dispatchMode: { type: 'start_immediately' } });
    expect(client.threadId).toBe('t1');
  });
  test('each Send button presses `sendPress`, which hands `send` its MouseEvent\'s modifiers', async () => {
    const fns = contractFns('press-modifiers.contract');
    const mouse = (mods: string) => ({ metaKey: mods.includes('meta'), ctrlKey: mods.includes('ctrl'), altKey: mods.includes('alt'), shiftKey: mods.includes('shift') });
    for (const name of ['PrimaryActions', 'ComposerFooter']) {
      const component = contractComponent('composer-controls.contract', name);
      for (const [mods, held] of [['', ''], ['meta', 'meta'], ['meta alt', 'meta+alt'], ['ctrl shift', 'control+shift'], ['meta ctrl alt shift', 'meta+control+alt+shift']]) {
        const sent: string[] = [];
        component.call('sendPress', {}, { ...fns, send: (value: string) => sent.push(value) }, [mouse(mods!)]);
        expect([name, sent]).toEqual([name, [held]]);
      }
    }
    const controls = await Bun.file(appPath('composer-controls.contract')).text();
    expect(controls.split('button press=sendPress ').length - 1).toBe(3); // plan refine, Send, the resting Send
    expect(controls).not.toContain('button press=send ');
    // The reference resolves its send chords only in the composer's editor (composerSubmissionIntentForKey): with the
    // composer unfocused ⌘↩ sends nothing, so Send declares no chord (the composer's key action takes them).
    expect(controls.split('\n').filter(line => line.includes('button press=sendPress ') && line.includes('aria-keyshortcuts='))).toEqual([]);
    const app = await Bun.file(appPath('app.contract')).text();
    expect(app).toContain('  action send(held: string)');
    expect(app).toContain('    submit(held == "" ? "" : `pointer:${held}`)');
    expect(app).toContain('send changed = command("send", gesture, composerText, 0)');
    expect(app).toContain('command("send-annotation", `${shell.panel.annotationSend}`, "", 0)');
  });
});
