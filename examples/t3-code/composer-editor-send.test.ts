// Lane composer-editor: the send gestures of 5c0429b (⌥⌘↩ sends and opens a new
// thread) and 263097a (⌘↩ starts a draft in the background), end to end
// through the client: the native gesture, the server's keybindings, the writes.
import { describe, expect, test } from 'bun:test';
import { connected, opened, running } from './composer-controls-fixture';
import { snapshot } from './presentation';
import { DEFAULT_SEND_RULES } from './composer-editor-intent';
import { toasts } from './toast';

describe('send gestures through the server keybindings', () => {
  test('⌥⌘↩ on an existing thread sends there, then opens a fresh composer in its project', async () => {
    const { client, native, command } = await opened();
    const project = client.projectId;
    native.gesture = { modifiers: 'meta+alt', source: 'key', ageMs: 4 };
    await command('send', '', 'Keep going');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't1', text: 'Keep going', dispatchMode: { type: 'start_immediately' } });
    expect(client.threadId).toBe('');
    expect(client.projectId).toBe(project);
    expect(client.error).toBe('');
  });
  test('⌥⌘↩ while the thread runs follows the follow-up behaviour (queue), then opens the fresh composer', async () => {
    const { client, native, command } = await opened();
    running(client);
    native.gesture = { modifiers: 'meta+alt', source: 'key', ageMs: 4 };
    await command('send', '', 'After this');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't1', dispatchMode: { type: 'queue_after_active' } });
    expect(client.threadId).toBe('');
  });
  test('a plain send or ⌘↩ on an idle thread stays on it', async () => {
    const { client, native, command } = await opened();
    await command('send', '', 'Plain');
    expect(client.threadId).toBe('t1');
    native.gesture = { modifiers: 'meta', source: 'key', ageMs: 4 };
    await command('send', '', 'Command return');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Command return' });
    expect(client.threadId).toBe('t1');
  });
  test('⌘↩ in a draft starts the thread in the background (a HEAD server binds it)', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES;
    native.gesture = { modifiers: 'meta', source: 'key', ageMs: 3 };
    await command('send', '', 'Background from command return');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Started in background' });
  });
  test('an older server that binds only ⌥⌘↩ keeps ⌘↩ in a draft a foreground send', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES.filter((rule, index) => index !== 1);
    native.gesture = { modifiers: 'meta', source: 'key', ageMs: 3 };
    await command('send', '', 'Foreground');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).not.toBe('');
  });
  test('the send button takes ⌥⌘↩ on an existing thread', async () => {
    const { client } = await opened();
    client.config.keybindings = DEFAULT_SEND_RULES;
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
  });
  test('a new thread is titled from its prose, not its chips', async () => {
    const { native, command } = await connected();
    await command('send', '', 'Look at [notes.md](t3-context://v1/file/file_x) please');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', title: 'Look at  please' });
  });
});
