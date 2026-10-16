// Keybindings follow T3 f90b77d809's defaults (upstream 5c0429b157
// mod+alt+enter sends and opens a new thread; 263097a8da mod+enter starts in the background).
import { describe, expect, test } from 'bun:test';
import { keybindingCommands, keybindingDefaults } from './keybinding-settings';
import { buildRows, commandLabel, commandOptions } from './keybinding-view';
import { commandLabel as searchLabel } from './settings-search';
import { contractFns } from './contract-fn-fixture';
import { contractComponent } from './contract-action-fixture';

// The server's resolved rule: shortcut flags and the when-expression as an AST.
const binding = (command: string, key: string) => ({ command, shortcut: Object.fromEntries(key.split('+').map(part => part === 'mod' ? ['modKey', true] : part === 'alt' ? ['altKey', true] : ['key', part])),
  whenAst: { type: 'and', left: { type: 'identifier', name: 'composerFocus' }, right: { type: 'identifier', name: 'draftThreadRoute' } } });

describe('composer send shortcuts', () => {
  test('the defaults carry both new rules in the reference order', () => {
    const composer = keybindingDefaults.filter(rule => rule.command.startsWith('composer.send'));
    expect(composer).toEqual([
      { key: 'mod+enter', command: 'composer.sendAlternate', when: 'composerFocus && turnRunning' },
      { key: 'mod+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
      { key: 'mod+alt+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
      { key: 'mod+alt+enter', command: 'composer.sendAndNewThread', when: 'composerFocus && !draftThreadRoute' },
    ]);
    expect(keybindingCommands.slice(keybindingCommands.indexOf('composer.sendAlternate'), keybindingCommands.indexOf('composer.host')))
      .toEqual(['composer.sendAlternate', 'composer.sendBackground', 'composer.sendAndNewThread']);
  });
  test('the new command is labelled everywhere it is listed', () => {
    expect(commandLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
    expect(searchLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
    expect(commandOptions([]).some(option => option.value === 'composer.sendAndNewThread' && option.label === 'Composer: Send and Start New Thread')).toBe(true);
  });
  test('the rules a HEAD server migrates into configs read as defaults, not customizations', () => {
    const rows = buildRows([binding('composer.sendBackground', 'mod+enter')], '');
    expect(rows.map(row => [row.label, row.source])).toEqual([['Composer: Start in Background', 'Default']]);
  });
});

// The keybinding recorder is a Contract focus target (key-recorder.contract), not the
// native T3KeyRecorder view. Its chord is the reference's keybindingFromKeyboardEvent over the KeyboardEvent the host
// sends (KeybindingsSettings.logic.test.ts's table, then the cases the native T3KeyChord normalized by hand).
describe('the keybinding recorder (key-recorder.contract)', () => {
  const fns = contractFns('key-recorder.contract');
  const chord = (key: string, code: string, mods = '') =>
    fns.recorderBinding!(key as never, code as never, mods.includes('meta') as never, mods.includes('ctrl') as never, mods.includes('alt') as never, mods.includes('shift') as never);
  test('records the reference\'s bindings', () => {
    expect(chord('K', 'KeyK', 'meta shift')).toBe('mod+shift+k');
    expect(chord('k', 'KeyK')).toBe('k');
    expect(chord('Tab', 'Tab')).toBe('tab');
    expect(chord('F5', 'F5')).toBe('f5');
    // Shifted punctuation by its physical key; a Latin layout's own letter stays the key (AZERTY's m at Semicolon).
    expect(chord('@', 'Digit2', 'meta shift')).toBe('mod+shift+2');
    expect(chord('"', 'Digit2', 'meta shift')).toBe('mod+shift+2');
    expect(chord('@', 'Quote', 'meta shift')).toBe("mod+shift+'");
    expect(chord('m', 'Semicolon', 'meta')).toBe('mod+m');
    expect(chord('a', 'KeyQ', 'meta')).toBe('mod+a');
  });
  test('a modifier alone records nothing; named, function and arrow keys by name', () => {
    for (const [key, code] of [['Meta', 'MetaLeft'], ['Shift', 'ShiftRight'], ['Alt', 'AltLeft'], ['Control', 'ControlLeft'], ['CapsLock', 'CapsLock']]) {
      expect(chord(key!, code!, 'meta shift')).toBe('');
    }
    expect(chord('Escape', 'Escape', 'meta')).toBe('mod+esc');
    expect(chord(' ', 'Space', 'ctrl')).toBe('ctrl+space');
    expect(chord('ArrowUp', 'ArrowUp', 'ctrl')).toBe('ctrl+arrowup');
    expect(chord('Backspace', 'Backspace')).toBe('backspace');
    expect(chord('Delete', 'Delete')).toBe('delete');
    expect(chord('PageDown', 'PageDown', 'alt')).toBe('alt+pagedown');
    expect(chord('F12', 'F12', 'meta alt')).toBe('mod+alt+f12');
    expect(chord('F35', 'F35')).toBe('f35');
    expect(chord('+', 'Equal', 'meta shift')).toBe('mod+shift+=');
    expect(chord('Unidentified', '')).toBe('');
  });
  test('a key that types no a–z letter is its physical Latin key (Korean 2-Set, Option\'s character), in modifier order', () => {
    expect(chord('ㅠ', 'KeyB', 'meta')).toBe('mod+b');
    expect(chord('å', 'KeyA', 'alt')).toBe('alt+a');
    expect(chord('ö', 'Semicolon', 'meta')).toBe('mod+;');
    expect(chord('K', 'KeyK', 'shift alt ctrl meta')).toBe('mod+ctrl+alt+shift+k');
  });
  test('its capture handler takes every key: Escape cancels, a chord is reported once, a modifier alone waits', () => {
    const recorder = contractComponent('key-recorder.contract', 'KeyRecorder');
    const run = (key: string, code: string, mods = '', recording = true) => {
      const calls: string[] = [];
      const scope = { ...fns, recording, capture: (value: string) => calls.push(`capture:${value}`), preventDefault: () => calls.push('prevent'), stopPropagation: () => calls.push('stop') };
      const event = { key, code, metaKey: mods.includes('meta'), ctrlKey: mods.includes('ctrl'), altKey: mods.includes('alt'), shiftKey: mods.includes('shift'), repeat: false };
      recorder.call('keys', {}, scope, [key, event]);
      return calls;
    };
    expect(run('Escape', 'Escape')).toEqual(['prevent', 'stop', 'capture:escape']);
    expect(run('K', 'KeyK', 'meta shift')).toEqual(['prevent', 'stop', 'capture:mod+shift+k']);
    // Tab is recorded, its focus move cancelled; a ⌘ chord's default is cancelled too, so Exact's monitor drops the event
    // before the main menu (T3Menus' own quit-hold and held-⌘W monitor may still see ⌘Q or a repeat first, as with the native view).
    expect(run('Tab', 'Tab')).toEqual(['prevent', 'stop', 'capture:tab']);
    expect(run('q', 'KeyQ', 'meta')).toEqual(['prevent', 'stop', 'capture:mod+q']);
    expect(run('Meta', 'MetaLeft', 'meta')).toEqual(['prevent', 'stop']);
    // After a chord the field still has the focus but no longer records: Tab moves the focus on (the reference's
    // captureKeybinding returns before preventDefault), while Escape and any other chord are still the field's.
    expect(run('Tab', 'Tab', '', false)).toEqual([]);
    expect(run('Escape', 'Escape', '', false)).toEqual(['prevent', 'stop', 'capture:escape']);
    expect(run('j', 'KeyJ', 'meta', false)).toEqual(['prevent', 'stop', 'capture:mod+j']);
  });
  test('both recorders mount it in their modal row; the action editor\'s Escape and blur keep the old binding', async () => {
    const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
    const settings = await source('settings-keybindings.contract'), actions = await source('settings-b-actions.contract');
    expect(settings).toContain('KeyRecorder(prompt=(recording ? "Press shortcut" : "Unassigned"), value=(recording ? "" : value), recording=recording, autofocus=true, label=`Keybinding for ${label}`, testId=`keybinding-recorder-${rowId}`, capture=took, cancel=took("blur"))');
    expect(settings).toContain('KeyRecorder(prompt="Unassigned", value=value, recording=false, autofocus=false, label=`Keybinding for ${label}`, testId=`keybinding-field-${rowId}`, capture=took, cancel=idleLeft)');
    expect(actions).toContain('KeyRecorder(prompt="Press shortcut", value="", recording=true, autofocus=true, label="Keybinding", testId="action-keybinding-recorder", capture=capture, cancel=capture("escape"))');
    expect(actions).toContain('row class=AeField align-items="center" aria-modal=true testId="action-keybinding-recording"');
    const view = await source('key-recorder.contract');
    // The reference's Input: a text field named for its command, the prompt its placeholder (VoiceOver reads both).
    expect(view).toContain('input id=testId autofocus=autofocus keycapture=keys blur=cancel aria-label=label testId=testId value=value placeholder=prompt');
    const editor = contractComponent('settings-b-actions.contract', 'ProjectActionEditor');
    const state = (keybinding: string) => ({ ...editor.state({ item: { keybinding, previewUrl: '', setup: false, wait: false, autoOpen: false, name: '', command: '', icon: 'play', id: '' } }), keybinding, recording: true });
    for (const [value, expected] of [['escape', 'mod+k'], ['esc', 'mod+k'], ['tab', 'mod+k'], ['backspace', ''], ['mod+shift+j', 'mod+shift+j']] as const) {
      const now = state('mod+k');
      editor.call('capture', now, {}, [value]);
      expect([value, now.keybinding, now.recording]).toEqual([value, expected, false]);
    }
    // The action editor's Keybinding press moves the focus into the recorder itself (its Name field keeps it from autofocus).
    const starts: string[] = [];
    const idle = { ...state('mod+k'), recording: false };
    editor.call('record', idle, { focus: (id: string) => starts.push(id) }, []);
    expect(idle.recording).toBe(true);
    expect(starts).toEqual(['action-keybinding-recorder']);
  });
});

// KeybindingsSettings.tsx's Input (an existing binding's, and NewKeybindingKeyInput): a text field whose onFocus starts
// recording and whose onBlur stops it, the field staying.
describe('the Settings capture field (settings-keybindings.contract CaptureField)', () => {
  const field = contractComponent('settings-keybindings.contract', 'CaptureField');
  const run = (action: string, args: unknown[], recording: boolean, taking = false) => {
    const calls: string[] = [];
    const state = { ...field.state({ rowId: 'new', original: 'mod+n', recording }), taking };
    const scope = {
      rowId: 'new', original: 'mod+n', recording,
      record: () => calls.push('record'),
      capture: (original: string, key: string) => calls.push(`capture:${original}:${key}`),
      held: (id: string, on: boolean) => calls.push(`held:${id}:${on}`),
    };
    field.call(action, state, scope, args);
    return { calls, taking: state.taking };
  };
  test('taking the focus starts recording, and the window learns which field has it', () => {
    expect(run('entered', [], false).calls).toEqual(['held:keybinding-recorder-new:true', 'record']);
    // An existing binding's field mounts from its pill already recording (autoFocus={isRecording}).
    expect(run('entered', [], true).calls).toEqual(['held:keybinding-recorder-new:true']);
  });
  test('a chord or Escape keeps the field, leaving it ends recording only for the row that records', () => {
    expect(run('took', ['escape'], true)).toEqual({ calls: ['capture:mod+n:escape'], taking: true });
    expect(run('took', ['mod+j'], true)).toEqual({ calls: ['capture:mod+n:mod+j'], taking: true });
    expect(run('took', ['blur'], true, true)).toEqual({ calls: ['capture:mod+n:blur', 'held:keybinding-recorder-new:false'], taking: false });
    // Another row's pill started recording as this field gave up the focus: its blur leaves that recording alone.
    expect(run('took', ['blur'], false, true)).toEqual({ calls: ['held:keybinding-recorder-new:false'], taking: false });
  });
  test('the recorder is up, the window\'s modal scope, only while the field records or takes keys; else an idle field shows', () => {
    const drawn = (marker: string, recording: boolean, taking: boolean) => field.drawn(marker, { taking }, { rowId: 'new', recording });
    expect(drawn('aria-modal=true focusin=entered', true, false)).toBe(true);
    expect(drawn('aria-modal=true focusin=entered', false, true)).toBe(true);
    expect(drawn('aria-modal=true focusin=entered', false, false)).toBe(false);
    expect(drawn('testId=`keybinding-capture-${rowId}`', false, false)).toBe(true);
  });
  test('the new row\'s field is always shown', async () => {
    const text = await Bun.file(new URL('./settings-keybindings.contract', import.meta.url)).text();
    expect(text).toContain('CaptureField(rowId="new", ');
    expect(text).not.toContain('button press=record aria-label=`Keybinding for ${label}`');
  });
});
