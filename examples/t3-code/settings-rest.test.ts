import { redactedPlaceholder } from './redacted-text';
import { describe, test, expect } from 'bun:test';
import { params, storagePatch, resolveWorktreeCleanup, keybindingPayload } from './settings-rest-commands';
import { commandLabel, pillParts, shortcutLabel, parseWhen, printWhen, whenEditor, whenVariables, buildRows, commandOptions, conflictLabels, conflictText, rowId } from './keybinding-view';
import { relativeTimeLabel } from './settings-data';
import type { Obj } from './domain';

const shortcut = (key: string, mods: Obj = {}): Obj => ({ key, modKey: false, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });
const binding = (command: string, key: string, mods: Obj = {}, whenAst?: Obj): Obj => ({ command, shortcut: shortcut(key, mods), ...(whenAst ? { whenAst } : {}) });
const notTerminal = { type: 'not', node: { type: 'identifier', name: 'terminalFocus' } };

describe('settings-rest payloads', () => {
  test('params decodes Contract encodeURIComponent pairs, including empty and reserved characters', () => {
    expect(params(`key=mode&value=${encodeURIComponent('a&b=c "q"')}&empty=`)).toEqual({ key: 'mode', value: 'a&b=c "q"', empty: '' });
    expect(params('')).toEqual({});
  });
});

describe('storage cleanup (StorageSettings.tsx)', () => {
  const settings: Obj = { storageCleanup: { worktreeOnDelete: false, worktreeAfterDays: null, worktreeOnMerge: true, worktreeUnchanged: false, browserArtifactsAfterDays: null, logsAfterDays: 14 },
    projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' } } };
  test('environment writes are partial storageCleanup patches; retention clamps to 1–3650', () => {
    expect(storagePatch(settings, '', 'logsAfterDays', '8')).toEqual({ storageCleanup: { logsAfterDays: 8 } });
    expect(storagePatch(settings, '', 'logsAfterDays', '9999')).toEqual({ storageCleanup: { logsAfterDays: 3650 } });
    expect(storagePatch(settings, '', 'logsAfterDays', '0')).toEqual({ storageCleanup: { logsAfterDays: 1 } });
    expect(storagePatch(settings, '', 'worktreeAfterDays', 'null')).toEqual({ storageCleanup: { worktreeAfterDays: null } });
    expect(storagePatch(settings, '', 'worktreeOnMerge', 'false')).toEqual({ storageCleanup: { worktreeOnMerge: false } });
    expect(() => storagePatch(settings, '', 'logsAfterDays', 'abc')).toThrow('number of days');
    expect(() => storagePatch(settings, '', 'worktreeOnMerge', 'maybe')).toThrow('On or Off');
    expect(() => storagePatch(settings, '', 'unknown', 'true')).toThrow('Unsupported');
  });
  test('an environment with a stored worktree policy keeps it in step', () => {
    const custom = { ...settings, worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true, worktreeAfterDays: 3, worktreeOnMerge: false, worktreeUnchanged: false } } };
    expect(storagePatch(custom, '', 'worktreeOnMerge', 'true')).toEqual({ storageCleanup: { worktreeOnMerge: true }, worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true, worktreeAfterDays: 3, worktreeOnMerge: true, worktreeUnchanged: false } } });
    expect(storagePatch(custom, '', 'logsAfterDays', '2')).toEqual({ storageCleanup: { logsAfterDays: 2 } });
  });
  test('project mode inherit/off/custom keeps sibling override keys; rules need Custom', () => {
    expect(storagePatch(settings, 'p1', 'mode', 'off')).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', worktreeCleanup: { mode: 'off' } } } });
    expect(storagePatch(settings, 'p1', 'mode', 'custom')).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', worktreeCleanup: { mode: 'custom', rules: resolveWorktreeCleanup(settings, 'p1') } } } });
    expect(() => storagePatch(settings, 'p1', 'worktreeOnDelete', 'true')).toThrow('Choose Custom');
    expect(() => storagePatch(settings, 'p1', 'logsAfterDays', '8')).toThrow('environment');
    const customised = { ...settings, projectSettingsOverrides: { p1: { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: false, worktreeAfterDays: null, worktreeOnMerge: true, worktreeUnchanged: false } } } } };
    expect(storagePatch(customised, 'p1', 'worktreeAfterDays', '5')).toEqual({ projectSettingsOverrides: { p1: { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: false, worktreeAfterDays: 5, worktreeOnMerge: true, worktreeUnchanged: false } } } } });
    expect(storagePatch(customised, 'p1', 'mode', 'inherit')).toEqual({ projectSettingsOverrides: { p1: null } });
    expect(() => storagePatch(settings, '', 'mode', 'off')).toThrow('Choose a project');
  });
  test('resolveWorktreeCleanup: off clears every rule, inherit reads storageCleanup', () => {
    expect(resolveWorktreeCleanup({ ...settings, projectSettingsOverrides: { p1: { worktreeCleanup: { mode: 'off' } } } }, 'p1')).toEqual({ worktreeAfterDays: null, worktreeOnMerge: false, worktreeOnDelete: false, worktreeUnchanged: false });
    expect(resolveWorktreeCleanup(settings, 'p2')).toEqual({ worktreeAfterDays: null, worktreeOnMerge: true, worktreeOnDelete: false, worktreeUnchanged: false });
  });
});

describe('archive (formatRelativeTimeLabel)', () => {
  test('relative labels match the reference', () => {
    const now = Date.parse('2026-10-03T12:00:00Z');
    expect(relativeTimeLabel('2026-10-03T11:59:30Z', now)).toBe('just now');
    expect(relativeTimeLabel('2026-10-03T11:55:00Z', now)).toBe('5m ago');
    expect(relativeTimeLabel('2026-10-03T09:00:00Z', now)).toBe('3h ago');
    expect(relativeTimeLabel('2026-09-30T12:00:00Z', now)).toBe('3d ago');
    expect(relativeTimeLabel('2026-10-03T13:00:00Z', now)).toBe('just now');
    expect(relativeTimeLabel('nope', now)).toBe('');
  });
});

describe('keybindings view (KeybindingsSettings.logic.ts)', () => {
  test('command labels, pills and shortcut labels', () => {
    expect(commandLabel('appearance.cycle')).toBe('Appearance: Cycle');
    expect(commandLabel('chat.newWithoutProject')).toBe('Chat: New Without Project');
    expect(commandLabel('composer.sendAlternate')).toBe('Composer: Opposite Queue or Steer Action');
    expect(commandLabel('usage.period.month')).toBe('Usage: Period: 30 days');
    expect(commandLabel('thread.jump.3')).toBe('Thread: Jump: 3');
    expect(commandLabel('script.build-app.run')).toBe('Run Script: Build App');
    expect(pillParts('mod+alt+shift+a')).toEqual(['⌘', '⌥', '⇧', 'A']);
    expect(pillParts('mod+enter')).toEqual(['⌘', 'enter']);
    expect(shortcutLabel('mod+alt+shift+a')).toBe('⌥⇧⌘A');
    expect(shortcutLabel('mod+shift+arrowup')).toBe('⇧⌘Up');
    expect(shortcutLabel('ctrl+space')).toBe('⌃Space');
  });
  test('rows: defaults vs custom, sorted by command then key, searchable, conflicts by overlapping when', () => {
    const bindings = [binding('sidebar.toggle', 'b', { modKey: true }), binding('chat.new', 'n', { modKey: true }, notTerminal), binding('chat.new', 'o', { modKey: true, shiftKey: true }, notTerminal), binding('diff.toggle', 'n', { modKey: true })];
    const rows = buildRows(bindings, '');
    expect(rows.map(row => row.label)).toEqual(['Chat: New', 'Chat: New', 'Diff: Toggle', 'Sidebar: Toggle']);
    expect(rows.map(row => row.key)).toEqual(['mod+n', 'mod+shift+o', 'mod+n', 'mod+b']);
    expect(rows[0].first && !rows[1].first).toBe(true);
    expect(rows.find(row => row.command === 'diff.toggle')!.source).toBe('Custom');
    expect(rows.find(row => row.command === 'diff.toggle')!.defaultKey).toBe('mod+d');
    expect(rows.find(row => row.command === 'diff.toggle')!.canReset).toBe(true);
    expect(rows[0].source).toBe('Default');
    expect(rows[0].canRemove).toBe(false);
    expect(rows[0].conflict).toBe('Conflicts with Diff: Toggle.');
    expect(rows[0].condition).toBe('!terminalFocus');
    expect(rows[3].whenLabel).toBe('Always');
    expect(buildRows(bindings, 'sidebar').map(row => row.command)).toEqual(['sidebar.toggle']);
    expect(buildRows(bindings, 'custom').map(row => row.command)).toEqual(['diff.toggle']);
    expect(conflictLabels([{ id: 'a', command: 'chat.new', key: 'mod+n', when: 'x' }], { id: 'b', key: 'mod+n', when: 'y' })).toEqual([]);
    expect(conflictText(['A', 'B', 'C', 'D'])).toBe('Conflicts with A, B, C, and more.');
  });
  test('command options: catalog plus configured commands, usage block in page order', () => {
    const options = commandOptions([binding('script.lint.run', 'l', { modKey: true })]).map(option => option.value);
    expect(options).toContain('script.lint.run');
    expect(options.indexOf('usage.open')).toBeLessThan(options.indexOf('usage.cost'));
    expect(options.indexOf('usage.cost')).toBe(options.indexOf('usage.open') + 1);
    expect(options.slice(0, 2)).toEqual(['appearance.cycle', 'chat.new']);
  });
  test('save/remove/reset payloads replace the exact binding and validate the grammar', () => {
    const bindings = [binding('chat.new', 'n', { modKey: true }, notTerminal)];
    const id = rowId('chat.new', 'mod+n', '!terminalFocus');
    expect(keybindingPayload(bindings, { action: 'save', previous: id, command: 'chat.new', key: 'mod+shift+k', when: '' })).toEqual({ command: 'chat.new', key: 'mod+shift+k', replace: { command: 'chat.new', key: 'mod+n', when: '!terminalFocus' } });
    expect(keybindingPayload(bindings, { action: 'remove', previous: id })).toEqual({ command: 'chat.new', key: 'mod+n', when: '!terminalFocus' });
    expect(keybindingPayload(bindings, { action: 'save', command: 'thread.stop', key: 'mod+.', when: 'composerFocus && !turnRunning' })).toEqual({ command: 'thread.stop', key: 'mod+.', when: 'composerFocus && !turnRunning' });
    expect(() => keybindingPayload(bindings, { action: 'remove', previous: rowId('chat.new', 'mod+x', '') })).toThrow('no longer available');
    expect(() => keybindingPayload(bindings, { action: 'save', command: 'chat.new', key: 'mod+shift', when: '' })).toThrow('one key');
    expect(() => keybindingPayload(bindings, { action: 'save', command: 'chat.new', key: 'mod+k', when: 'a &&' })).toThrow('parentheses');
    expect(() => keybindingPayload(bindings, { action: 'save', command: '', key: 'mod+k', when: '' })).toThrow('command');
  });
});

describe('When builder (WhenExpressionBuilder)', () => {
  test('parse and print round-trip with the reference wrapping', () => {
    for (const text of ['a', '!a', 'a && b', 'a || b && c', '(a || b) && !c', '!(a && b)']) expect(printWhen(parseWhen(text)!)).toBe(text === 'a || b && c' ? 'a || (b && c)' : text);
    expect(parseWhen('a &&')).toBeNull();
    expect(parseWhen('(a')).toBeNull();
  });
  test('variables: core first, then identifiers from the defaults', () => {
    const variables = whenVariables();
    expect(variables.slice(0, 6)).toEqual(['terminalFocus', 'terminalOpen', 'isWeb', 'isDesktop', 'true', 'false']);
    expect(variables).toContain('composerFocus');
    expect(variables).toContain('modelPickerOpen');
  });
  test('every control precomputes its next expression', () => {
    const empty = whenEditor('');
    expect(empty.lines).toEqual([]);
    expect(empty.addRootCondition).toBe('terminalFocus');
    expect(empty.addRootGroup).toBe('terminalFocus || !terminalFocus');
    const view = whenEditor('composerFocus && !turnRunning');
    expect(view.lines.map(line => [line.id, line.kind, line.depth, line.label])).toEqual([['root', 'group', 0, ''], ['0', 'condition', 1, 'composerFocus'], ['1', 'condition', 1, 'turnRunning']]);
    const [root, first, second] = view.lines;
    expect(root.operators.map(option => option.value)).toEqual(['composerFocus && !turnRunning', 'composerFocus || !turnRunning']);
    // whenAstToExpression parenthesises every nested binary node, as the reference prints.
    expect(root.addCondition).toBe('(composerFocus && !turnRunning) && terminalFocus');
    expect(root.addGroup).toBe('(composerFocus && !turnRunning) && (terminalFocus || !terminalFocus)');
    expect(root.remove).toBe('');
    expect(first.negate).toBe('!composerFocus && !turnRunning');
    expect(second.negate).toBe('composerFocus && turnRunning');
    expect(second.negated).toBe(true);
    expect(first.remove).toBe('!turnRunning');
    expect(first.options.find(option => option.label === 'isDesktop')!.value).toBe('isDesktop && !turnRunning');
    expect(second.options.find(option => option.label === 'isDesktop')!.value).toBe('composerFocus && !isDesktop');
    expect(view.addRootCondition).toBe('(composerFocus && !turnRunning) && terminalFocus');
    expect(whenEditor('a &&').error).toBe('Use variables with !, &&, ||, and parentheses.');
    expect(whenEditor('mysteryFlag').unknown).toBe('Unknown condition: mysteryFlag');
    const notGroup = whenEditor('!(a || b)');
    expect(notGroup.lines.map(line => line.kind)).toEqual(['not', 'group', 'condition', 'condition']);
    expect(notGroup.lines[0].negate).toBe('a || b');
    expect(notGroup.lines[2].remove).toBe('!b');
  });
});

describe('keyboard dispatch (resolveShortcutCommand)', () => {
  test('chords follow the host grammar and the last matching rule wins', async () => {
    const { ariaChord, chordWinners } = await import('./keyboard-dispatch');
    expect(ariaChord(shortcut('a', { modKey: true, altKey: true, shiftKey: true }))).toBe('Meta+Alt+Shift+a');
    expect(ariaChord(shortcut('arrowup', { modKey: true, shiftKey: true }))).toBe('Meta+Shift+ArrowUp');
    expect(ariaChord(shortcut('+', { modKey: true }))).toBe('Meta+Plus');
    const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
    const bindings = [binding('thread.jump.1', '1', { modKey: true }, { type: 'identifier', name: 'isDesktop' }),
      binding('modelPicker.jump.1', '1', { modKey: true }, { type: 'and', left: { type: 'identifier', name: 'modelPickerOpen' }, right: { type: 'identifier', name: 'isDesktop' } }),
      binding('thread.stop', '.', { modKey: true }, { type: 'identifier', name: 'mysteryContext' })];
    expect(chordWinners(bindings, context).get('Meta+1')).toBe('thread.jump.1');
    expect(chordWinners(bindings, { ...context, modelPickerOpen: true }).get('Meta+1')).toBe('modelPicker.jump.1');
    expect(chordWinners(bindings, context).get('Meta+.')).toBe('');
  });
  test('dispatch lists counterparts in sidebar order and context', async () => {
    const { keyboardDispatch } = await import('./keyboard-dispatch');
    const notTerminalBinding = (command: string, key: string, mods: Obj) => binding(command, key, mods, notTerminal);
    const client = { config: { keybindings: [binding('thread.next', ']', { modKey: true, shiftKey: true }), binding('thread.previous', '[', { modKey: true, shiftKey: true }),
      binding('thread.jump.1', '1', { modKey: true }, { type: 'identifier', name: 'isDesktop' }), binding('thread.jump.2', '2', { modKey: true }, { type: 'identifier', name: 'isDesktop' }),
      notTerminalBinding('appearance.cycle', 'a', { modKey: true, altKey: true, shiftKey: true }), notTerminalBinding('chat.newLocal', 'n', { modKey: true, shiftKey: true }),
      binding('rightPanel.close', 'w', { modKey: true }, notTerminal), binding('navigation.back', '[', { modKey: true }, notTerminal)], providers: [] },
      local: { deviceSettings: { appearanceMode: 'light' }, favoriteModels: [] }, providerId: '', modelId: '' } as never;
    const threads: Obj[] = [{ id: 'settled', section: 'settled', selected: false }, { id: 'b', section: 'active', selected: true }, { id: 'a', section: 'pinned', selected: false }];
    const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
    const items = keyboardDispatch(client, threads, '', '', context);
    const target = (command: string) => items.find(item => item.command === command);
    expect(target('thread.jump.1')!.target).toBe('a');
    expect(target('thread.jump.2')!.target).toBe('b');
    expect(target('thread.next')!.target).toBe('settled');
    expect(target('thread.previous')!.target).toBe('a');
    expect(target('appearance.cycle')).toMatchObject({ target: 'dark', chord: 'Meta+Alt+Shift+a' });
    expect(target('chat.newLocal')!.chord).toBe('Meta+Shift+n');
    expect(target('rightPanel.close')).toBeUndefined();
    expect(keyboardDispatch(client, threads, '', '', { ...context, diffOpen: true }).some(item => item.command === 'rightPanel.close')).toBe(true);
    expect(keyboardDispatch(client, threads, '', '', { ...context, modalOpen: true })).toEqual([]);
    // thread-commands-and-keys: the palette provider's chords (here appearance.cycle) stay live in Settings.
    expect(keyboardDispatch(client, threads, '', '', { ...context, modalOpen: true, settingsOpen: true }).map(item => item.command)).toEqual(['settings.open', 'navigation.back', 'appearance.cycle']);
  });
});

describe('source control and integrations (scoped rows)', () => {
  test('inheritance chain and summaries match SettingInheritance', async () => {
    const { inheritance, scopedPatch, sourceControlRows, fetchInterval, fetchIntervalPatch, discoveryRows, toolVersion } = await import('./source-control-view');
    const settings: Obj = { defaultAutoPull: true, pullRequestMergeMethod: null, branchNamingMode: 'static', branchNamePrefix: 't3code', sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: '', followChangeRequestTemplates: true },
      projectSettingsOverrides: { p1: { pullRequestMergeMethod: 'squash', defaultRuntimeMode: 'full-access' } } };
    expect(inheritance(settings, '', 'defaultAutoPull', 'Mac').summary).toBe('Set on the environment');
    expect(inheritance(settings, '', 'pullRequestMergeMethod', 'Mac').summary).toBe('Built-in default');
    expect(inheritance(settings, 'p1', 'pullRequestMergeMethod', 'Mac')).toMatchObject({ summary: 'Overridden for this project', state: 'overridden' });
    expect(inheritance(settings, 'p1', 'defaultAutoPull', 'Mac').summary).toBe('Inherited from Mac');
    expect(inheritance(settings, 'p1', 'pullRequestMergeMethod', 'Mac').layers.map(layer => [layer.label, layer.value, layer.effective])).toEqual([['Project', 'Squash and merge', true], ['Environment', 'Inherits', false], ['Default', 'Last selected', false]]);
    expect(scopedPatch(settings, '', 'pullRequestMergeMethod', 'last')).toEqual({ pullRequestMergeMethod: null });
    expect(scopedPatch(settings, '', 'sourceControlWritingStyle.followChangeRequestTemplates', 'false')).toEqual({ sourceControlWritingStyle: { followChangeRequestTemplates: false } });
    expect(scopedPatch(settings, 'p1', 'pullRequestMergeMethod', '__inherit__')).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' } } });
    expect(scopedPatch(settings, 'p1', 'defaultAutoPull', 'false')).toEqual({ projectSettingsOverrides: { p1: { pullRequestMergeMethod: 'squash', defaultRuntimeMode: 'full-access', defaultAutoPull: false } } });
    expect(scopedPatch(settings, '', 'defaultAutoPull', '__default__')).toEqual({ defaultAutoPull: false });
    expect(scopedPatch(settings, '', 'sourceControlWritingStyle', '__default__')).toEqual({ sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: '' } });
    expect(scopedPatch(settings, '', 'branchNamePrefix', '  feat/ ')).toEqual({ branchNamePrefix: 'feat/' });
    expect(() => scopedPatch(settings, 'p1', 'enableDeviceSupport', 'true')).toThrow('environment-wide');
    expect(() => scopedPatch(settings, '', 'pullRequestMergeMethod', 'fast-forward')).toThrow('merge method');
    expect(() => scopedPatch({}, '', 'sourceControlWriterModelSelection', 'default')).toThrow('No text generation');
    expect(scopedPatch({ textGenerationModelSelection: { instanceId: 'codex', model: 'gpt' } }, '', 'sourceControlWriterModelSelection', 'default')).toEqual({ sourceControlWriterModelSelection: { instanceId: 'codex', model: 'gpt' } });
    const rows = sourceControlRows(settings, '', 'Mac', [], true);
    expect(rows.repositories.map(row => [row.title, row.reset])).toEqual([['Automatically pull', 'key=defaultAutoPull&value=__default__'], ['Default merge method', '']]);
    expect(rows.text.map(row => row.title)).toEqual(['Worktree branch naming', 'Branch prefix', 'Source control writing style', 'Follow change request templates', 'Source control writer model']);
    expect(fetchInterval({})).toMatchObject({ seconds: '30', canReset: false });
    expect(fetchIntervalPatch({}, '45')).toEqual({ backgroundActivity: { schemaVersion: 1, profile: 'custom', baseProfile: 'balanced', overrides: { automaticGitFetchInterval: 45000 } } });
    expect(fetchInterval(fetchIntervalPatch({}, '45'))).toMatchObject({ seconds: '45', canReset: true });
    const found = discoveryRows({ versionControlSystems: [{ kind: 'jj', label: 'Jujutsu', implemented: false, status: 'available', version: { _tag: 'None' }, installHint: 'x' }],
      sourceControlProviders: [{ kind: 'github', label: 'GitHub', status: 'available', version: { _tag: 'Some', value: 'gh 2' }, installHint: 'x', executable: 'gh', auth: { status: 'authenticated', account: { _tag: 'Some', value: 'secret-account' } } }] });
    expect(found.vcs[0]).toMatchObject({ comingSoon: true, summary: 'Support for Jujutsu is coming soon.', dot: 'muted' });
    expect(found.providers[0]).toMatchObject({ version: 'gh 2', summary: 'Authenticated', account: true, enabled: true });
    // RedactedText draws the same-shape placeholder until this item's own reveal (provider-sign-in-and-install, D14).
    expect(found.providers[0]).toMatchObject({ accountValue: 'secret-account', accountPlaceholder: redactedPlaceholder('secret-account') });
    expect(found.vcs[0]).toMatchObject({ accountValue: '', accountPlaceholder: '' });
    expect(toolVersion(undefined)).toBe('Version unknown');
    expect(toolVersion({ installedVersions: [], requiredVersion: '1.2' })).toBe('Not installed');
    expect(toolVersion({ installedVersions: ['1.0', '1.10', '1.9'], requiredVersion: '2' })).toBe('v1.10');
  });
});
