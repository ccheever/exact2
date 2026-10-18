// Lane composer-editor: the send gestures of 5c0429b (⌥⌘↩ sends and opens a new
// thread) and 263097a (⌘↩ starts a draft in the background), end to end
// through the client: the native gesture, the server's keybindings, the writes.
import { describe, expect, test } from 'bun:test';
import { connected, opened, running } from '../support/composer-controls-fixture';
import { snapshot } from '../../src/client/presentation';
import { DEFAULT_SEND_RULES, pressGesture, sendIntent } from '../../src/composer/composer-editor-intent';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { toasts } from '../../src/shell/toast';
import { appPath } from '../support/app-files';

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

// A pointer press carries its own modifiers (its MouseEvent) to `send`, as
// `pointer:meta+alt`; any other press (a plain click, Return, a Send chord, an assistive press) reads the module's newest
// Return gesture, which a click retires (T3ComposerIntent, the intent AppKit suite).
describe('a Send press carries its own pointer modifiers', () => {
  test('pressGesture: modifiers make a pointer gesture of this press; none leaves it to the Return gesture', () => {
    expect(pressGesture('')).toBeNull();
    expect(pressGesture('pointer:')).toBeNull();
    expect(pressGesture('pointer:meta')).toEqual({ modifiers: 'meta', source: 'pointer', ageMs: 0 });
    expect(pressGesture('pointer:meta+alt+shift')).toEqual({ modifiers: 'meta+alt+shift', source: 'pointer', ageMs: 0 });
    expect(pressGesture('pointer:control+bogus')).toEqual({ modifiers: 'control', source: 'pointer', ageMs: 0 });
    // A ⌘-click while a turn runs is the alternate; a pointer never resolves a key chord (⌥⌘-click in a draft stays foreground).
    expect(sendIntent({}, pressGesture('pointer:meta')!, true, false)).toBe('alternate');
    expect(sendIntent({}, pressGesture('pointer:control')!, true, false)).toBe('alternate');
    expect(sendIntent({ keybindings: DEFAULT_SEND_RULES }, pressGesture('pointer:meta+alt')!, false, true)).toBe('foreground');
    expect(sendIntent({}, pressGesture('pointer:shift')!, true, false)).toBe('foreground');
  });
  test('the client takes a ⌘-click from the press and leaves the recorded Return alone', async () => {
    const { client, native, command } = await opened();
    running(client);
    native.gesture = { modifiers: 'meta+alt', source: 'key', ageMs: 4 }; // a Return gesture that is not this click's
    await command('send', 'pointer:meta', 'Steer by click');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer', dispatchMode: { type: 'start_immediately' } });
    expect(native.gesture).toEqual({ modifiers: 'meta+alt', source: 'key', ageMs: 4 }); // never asked for
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
    const app = await Bun.file(appPath('app.contract')).text();
    expect(app).toContain('  action send(held: string)');
    expect(app).toContain('send changed = command("send", held == "" ? "" : `pointer:${held}`, composerText, 0)');
    expect(app).toContain('    else\n      send("")\n'); // the annotation's send: no press
  });
});
