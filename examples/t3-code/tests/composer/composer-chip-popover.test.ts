// A skill chip's details popover, against T3 Code 1e2ecbd975
// (MIT, LICENSE-T3) ComposerPromptEditorTiptap.tsx ComposerSkillNodeView: the label, the skill's
// description or "No description is available for this skill.", and View instructions for a skill with a path.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldObserved } from '../../src/composer/composer-field-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { composerChipOpen, composerChipPreview } from '../../src/composer/composer-chip-bridge';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { chipPopover, chipPopoverLocal, chipPopoverView, NO_DESCRIPTION } from '../../src/composer/composer-chip-popover';
import { selectedProvider } from '../../src/composer/composer-editor';
import { appPath } from '../support/app-files';
import { observeComposer } from '../support/composer-field-fixture';
import { NO_CHIP_FINISH } from '../../src/composer/composer-chip-store';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const press = (extra: Obj = {}): Obj => ({ seq: 3, open: true, surface: 'composer', owner: 'draft:t1', kind: 'skill', name: 'frontend-design', label: 'Frontend Design', frame: [120, 640, 128, 19.5], ...extra });
const skills: Obj[] = [{ name: 'imagegen', displayName: 'Image Gen', description: 'Generate images.', path: '/Users/me/.codex/skills/imagegen/SKILL.md' },
  { name: 'frontend-design', description: 'Design web pages.', path: '/Users/me/.codex/skills/frontend-design/SKILL.md' }, { name: 'no-text' }];

describe('a skill chip\'s details popover', () => {
  test('root preview completion allows Appearance settings and its own generation while refusing other covers', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const finish = { ...NO_CHIP_FINISH, seq: 4, generation: 7, snapshotOwner: 'snapshot', owner: 'preview-owner', value: '$skill ', inputSerial: 3,
      surface: 'preview', start: 0, end: 6, focus: 'select' };
    const run = (patch: Env = {}, item = finish) => {
      const calls: string[] = [];
      const scope = { modal: true, settingsOpen: true, connecting: false, projectsOpen: false, paletteOpen: false, providerDialog: '', restEditor: '', welcome: { show: false },
        chip: { finish }, data: { snapshotOwner: 'snapshot' }, richComposer: { generation: 99 }, richPromptPreview: { generation: 7 },
        promptPreviewVisible: true, promptPreviewOwner: 'preview-owner', promptPreviewText: '$skill ', promptPreviewSerial: 3, promptPreviewComposing: false,
        querySelectorAll: (selector: string) => { expect(selector).toBe('[data-dialog-focus~="open"]'); return []; },
        frame: (target: string) => { expect(target).toBe('prompt-font-preview'); return { width: 320 }; },
        focus: (target: string) => calls.push(`focus:${target}`),
        setSelectionRange: (target: string, start: number, end: number) => calls.push(`select:${target}:${start}:${end}`), ...patch };
      root.call('chipComplete', {}, scope, [item, '']);
      return calls;
    };
    expect(run()).toEqual(['focus:prompt-font-preview', 'select:prompt-font-preview:0:6']);
    for (const patch of [{ connecting: true }, { projectsOpen: true }, { paletteOpen: true }, { providerDialog: 'provider' }, { restEditor: 'edit' }, { welcome: { show: true } },
      { querySelectorAll: () => ['theme-remove-dialog'] }, { promptPreviewVisible: false }, { promptPreviewComposing: true }]) expect(run(patch)).toEqual([]);
    expect(run({}, { ...finish, generation: 99 })).toEqual([]);
    expect(run({ richComposer: { generation: 7 }, richPromptPreview: { generation: 8 } })).toEqual([]);
    expect(run({}, { ...finish, surface: 'composer' })).toEqual([]);
  });
  test('the composer\'s chip: its label, the skill\'s description and its instructions file, at the chip', () => {
    expect(chipPopover(press(), skills)).toEqual({ finish: NO_CHIP_FINISH, seq: 3, open: true, surface: 'composer', label: 'Frontend Design', title: 'Skill Frontend Design', description: 'Design web pages.',
      path: '/Users/me/.codex/skills/frontend-design/SKILL.md', x: 120, y: 640, width: 128, height: 19.5, kind: 'skill', comment: '', anchor: '', ax: 0, ay: 0, aw: 0, ah: 0, message: '', quote: '', at: -1, lines: '', rows: 0, diff: '', image: '', aspect: 0, act: '' });
  });

  test('the Settings sample and a skill the provider does not list: no description, no View instructions', () => {
    expect(chipPopover(press({ surface: 'preview', owner: '' }), [])).toMatchObject({ open: true, label: 'Frontend Design', title: 'Skill Frontend Design', description: NO_DESCRIPTION, path: '' });
    expect(NO_DESCRIPTION).toBe('No description is available for this skill.');
    expect(chipPopover(press({ name: 'no-text', label: 'No Text' }), skills)).toMatchObject({ description: NO_DESCRIPTION, path: '' });
    // Names match exactly, as the reference's `find` does.
    expect(chipPopover(press({ name: 'Frontend-Design' }), skills).description).toBe(NO_DESCRIPTION);
  });

  test('a closed press, or one that is not a skill, shows nothing but keeps its sequence', () => {
    expect(chipPopover({ seq: 4, open: false }, skills)).toMatchObject({ seq: 4, open: false, label: '' });
    expect(chipPopover(press({ kind: 'mention' }), skills).open).toBe(false);
  });

  test('the resource reads shared source presses without a native painter, with workspace skills and isolated preview', () => {
    const client = new T3Client(), value = '$frontend-design ';
    client.providerId = 'codex'; client.projectId = 'p1';
    client.shell.projects = [{ id: 'p1', workspaceRoot: '/work' }];
    client.config.providers = [{ instanceId: 'codex', skills: [], workspaceSnapshots: [{ cwd: '/work', skills }] }];
    const snapshot = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner: composerFieldOwner(client), value,
      start: 0, end: 0, composing: false, focused: true, live: true, inputSerial: 0 };
    composerFieldObserved(client, [snapshot, false], snapshot.owner);
    composerSourceSession(client).observe(snapshot.owner, value);
    const chip = composerSourceSession(client).chips[0]!;
    const target = { ...snapshot, ...chip, surface: 'composer', comment: false, frame: { unavailable: false, x: 120, y: 640, width: 128, height: 19.5 } };
    composerChipOpen(client, target);
    expect(chipPopoverView(client)).toMatchObject({ open: true, description: 'Design web pages.', path: '/Users/me/.codex/skills/frontend-design/SKILL.md' });
    const preview = { ...snapshot, owner: `${client.snapshotOwner}:prompt-preview`, inputSerial: 1 };
    composerSourceSession(client, 'preview').observe(preview.owner, value, '', 'plain');
    composerChipPreview(client, preview);
    composerChipOpen(client, { ...target, ...preview, start: chip.start, end: chip.end, surface: 'preview' });
    expect(chipPopoverView(client)).toMatchObject({ open: true, description: NO_DESCRIPTION, path: '' });
    composerChipPreview(client, { ...preview, live: false });
    expect(chipPopoverView(client).open).toBe(false);
    client.generation++;
    expect(chipPopoverView(client).open).toBe(false);
  });

  test('details use the same selected provider as the source menu', async () => {
    const client = new T3Client(), value = '$frontend-design ';
    const claudeSkills = [{ name: 'frontend-design', description: "Claude's design skill.", path: '/Users/me/.claude/skills/frontend-design/SKILL.md' }];
    client.providerId = 'claudeAgent'; client.projectId = 'p1';
    client.shell.projects = [{ id: 'p1', workspaceRoot: '/work' }];
    client.config.providers = [{ instanceId: 'codex', skills: [], workspaceSnapshots: [{ cwd: '/work', skills }] },
      { instanceId: 'claudeAgent', skills: [], workspaceSnapshots: [{ cwd: '/work', skills: claudeSkills }] }];
    const snapshot = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner: composerFieldOwner(client), value,
      start: 0, end: 0, composing: false, focused: true, live: true, inputSerial: 0 };
    composerFieldObserved(client, [snapshot, false], snapshot.owner);
    composerSourceSession(client).observe(snapshot.owner, value);
    composerChipOpen(client, { ...snapshot, ...composerSourceSession(client).chips[0]!, surface: 'composer', comment: false,
      frame: { unavailable: false, x: 120, y: 640, width: 128, height: 19.5 } });
    expect(selectedProvider(client).instanceId).toBe('claudeAgent');
    expect(chipPopoverView(client)).toMatchObject({ description: "Claude's design skill.", path: '/Users/me/.claude/skills/frontend-design/SKILL.md' });
    client.providerId = 'codex';
    expect(chipPopoverView(client)).toMatchObject({ description: 'Design web pages.' });
    expect(await Bun.file(appPath('composer-chip-popover.ts')).text()).not.toContain('client.providerId');
  });

  test('close retains guarded public focus metadata and instructions require the current skill path', async () => {
    const calls: Obj[] = [];
    const native = { available: true, later: async (request: Obj) => { calls.push(request); return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    const client = new T3Client(), value = '$frontend-design ';
    client.providerId = 'codex'; client.config.providers = [{ instanceId: 'codex', skills }];
    const snapshot = observeComposer(client, value), chip = composerSourceSession(client).chips[0]!;
    const open = () => composerChipOpen(client, { ...snapshot, ...chip, surface: 'composer', comment: false, frame: { unavailable: false, x: 1, y: 2, width: 100, height: 20 } })!;
    const first = open();
    await chipPopoverLocal(client, native, 'chip-close', `${first.seq}`, 'escape');
    expect(chipPopoverView(client)).toMatchObject({ open: false, finish: { seq: first.seq, value, focus: 'select', start: chip.start, end: chip.end } });
    // An outside press leaves the focus where it put it: the finish takes none (FloatingFocusManager returns it only to a
    // tabbable trigger, and an editor chip is no tab stop).
    const outside = open();
    await chipPopoverLocal(client, native, 'chip-close', `${outside.seq}`, '');
    expect(chipPopoverView(client)).toMatchObject({ open: false, finish: { seq: outside.seq, focus: 'none' } });
    const next = open();
    await chipPopoverLocal(client, native, 'chip-close', `${first.seq}`, 'escape');
    expect(chipPopoverView(client)).toMatchObject({ open: true, seq: next.seq });
    await chipPopoverLocal(client, native, 'chip-instructions', `${next.seq}`, '/unrelated/SKILL.md');
    expect(chipPopoverView(client).open).toBe(false);
    expect(calls).toEqual([]);
    const last = open();
    // A legitimate current path reaches the ordinary file action's missing-project guard.
    await expect(chipPopoverLocal(client, native, 'chip-instructions', `${last.seq}`, String(skills[1]!.path))).rejects.toThrow();
    expect(chipPopoverView(client).open).toBe(false);
    expect(calls.some(call => String(call.op).startsWith('editorChip'))).toBe(false);
    await expect(chipPopoverLocal(client, native, 'chip-open', '1', '')).rejects.toThrow('Unknown editor action: chip-open');
  });
});
