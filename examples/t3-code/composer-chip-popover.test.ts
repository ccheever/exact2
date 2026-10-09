// settings-appearance-and-skill-chip (S1-12): a skill chip's details popover, against T3 Code 1e2ecbd975
// (MIT, LICENSE-T3) ComposerPromptEditorTiptap.tsx ComposerSkillNodeView: the label, the skill's
// description or "No description is available for this skill.", and View instructions for a skill with a path.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { chipPopover, chipPopoverLocal, chipPopoverView, NO_DESCRIPTION } from './composer-chip-popover';
import { selectedProvider } from './composer-editor';

const press = (extra: Obj = {}): Obj => ({ seq: 3, open: true, surface: 'composer', owner: 'draft:t1', kind: 'skill', name: 'frontend-design', label: 'Frontend Design', frame: [120, 640, 128, 19.5], ...extra });
const skills: Obj[] = [{ name: 'imagegen', displayName: 'Image Gen', description: 'Generate images.', path: '/Users/me/.codex/skills/imagegen/SKILL.md' },
  { name: 'frontend-design', description: 'Design web pages.', path: '/Users/me/.codex/skills/frontend-design/SKILL.md' }, { name: 'no-text' }];

describe('a skill chip\'s details popover (S1-12)', () => {
  test('the composer\'s chip: its label, the skill\'s description and its instructions file, at the chip', () => {
    expect(chipPopover(press(), skills)).toEqual({ seq: 3, open: true, surface: 'composer', label: 'Frontend Design', title: 'Skill Frontend Design', description: 'Design web pages.',
      path: '/Users/me/.codex/skills/frontend-design/SKILL.md', x: 120, y: 640, width: 128, height: 19.5 });
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

  test('the window\'s resource reads the newest press, the composer\'s against the selected provider\'s skills for the workspace', async () => {
    const calls: Obj[] = [];
    const watched: string[] = [];
    let reply: Obj = press();
    const native = { available: true, watch: (topic: string) => { watched.push(topic); }, later: async (request: Obj) => { calls.push(request); return { ok: true, generation: 0, value: reply }; } } as unknown as Native;
    const client = { snapshotOwner: 'draft:t1', providerId: 'codex', projectId: 'p1', projection: { thread: {} }, shell: { projects: [{ id: 'p1', workspaceRoot: '/work' }] },
      config: { providers: [{ instanceId: 'codex', skills: [], workspaceSnapshots: [{ cwd: '/work', skills }] }] } } as unknown as T3Client;
    expect(await chipPopoverView(client, native)).toMatchObject({ open: true, description: 'Design web pages.', path: '/Users/me/.codex/skills/frontend-design/SKILL.md' });
    expect(calls.at(-1)).toEqual({ op: 'editorChip' });
    expect(watched).toContain('t3.chip');
    // Another thread's composer press is not this thread's.
    reply = press({ owner: 'draft:t2' });
    expect(await chipPopoverView(client, native)).toMatchObject({ seq: 3, open: false });
    // The sample's chip never reads the composer's skills.
    reply = press({ surface: 'preview', owner: '' });
    expect(await chipPopoverView(client, native)).toMatchObject({ open: true, description: NO_DESCRIPTION, path: '' });
    expect(await chipPopoverView(client, { available: false } as unknown as Native)).toMatchObject({ seq: 0, open: false });
  });

  test('the details read the provider the `$` menu reads (composer-editor selectedProvider)', async () => {
    const native = { available: true, watch: () => undefined, later: async () => ({ ok: true, generation: 0, value: press() }) } as unknown as Native;
    const claudeSkills = [{ name: 'frontend-design', description: 'Claude\'s design skill.', path: '/Users/me/.claude/skills/frontend-design/SKILL.md' }];
    const client = { snapshotOwner: 'draft:t1', providerId: 'claudeAgent', projectId: 'p1', projection: { thread: {} }, shell: { projects: [{ id: 'p1', workspaceRoot: '/work' }] },
      config: { providers: [{ instanceId: 'codex', skills: [], workspaceSnapshots: [{ cwd: '/work', skills }] }, { instanceId: 'claudeAgent', skills: [], workspaceSnapshots: [{ cwd: '/work', skills: claudeSkills }] }] } } as unknown as T3Client;
    expect(selectedProvider(client).instanceId).toBe('claudeAgent');
    expect(await chipPopoverView(client, native)).toMatchObject({ description: 'Claude\'s design skill.', path: '/Users/me/.claude/skills/frontend-design/SKILL.md' });
    (client as unknown as { providerId: string }).providerId = 'codex';
    expect(await chipPopoverView(client, native)).toMatchObject({ description: 'Design web pages.' });
    // One lookup: the popover has no provider rule of its own.
    expect(await Bun.file(new URL('./composer-chip-popover.ts', import.meta.url)).text()).not.toContain('client.providerId');
  });

  test('the window\'s close names the press it saw; View instructions closes it and opens the file', async () => {
    const calls: Obj[] = [];
    const native = { available: true, later: async (request: Obj) => { calls.push(request); return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    await chipPopoverLocal({} as T3Client, native, 'chip-close', '7', '');
    expect(calls).toEqual([{ op: 'editorChipClose', seq: 7 }]);
    // openFileSurface needs a project: the close still happens first.
    const noProject = { config: {}, shell: { projects: [] }, projection: {}, projectId: '', ready: true } as unknown as T3Client;
    await expect(chipPopoverLocal(noProject, native, 'chip-instructions', '8', '/skills/a/SKILL.md')).rejects.toThrow();
    expect(calls.at(-1)).toEqual({ op: 'editorChipClose', seq: 8 });
    await expect(chipPopoverLocal({} as T3Client, native, 'chip-open', '1', '')).rejects.toThrow('Unknown editor action: chip-open');
  });
});

describe('the popover\'s Contract (composer-chip-popover.contract, app-window.contract)', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
  test('PopoverPopup side top, w-96 within the viewport, compact padding; a dialog named "Skill <label>"', async () => {
    const layer = await source('composer-chip-popover.contract');
    expect(layer).toContain('derive width = min(24 * rem, viewportWidth - 2 * rem)');
    expect(layer).toContain('bottom=(above ? viewportHeight - chip.y + 4 : "auto")');
    expect(layer).toContain('padding-top="0.5rem" padding-bottom="0.5rem" padding-left="0.75rem" padding-right="0.75rem"');
    expect(layer).toContain('role="dialog" aria-label=chip.title testId="chip-popover"');
    expect(layer).toContain('text "View instructions"');
  });
  test('a press outside the popover closes it; View instructions closes it with the file', async () => {
    const window = await source('app-window.contract');
    expect(window).toContain('when chipShown\n          ChipPopoverLayer(chip=chip, viewportWidth=viewport.width, viewportHeight=viewport.height, instructions=chipClose(true))');
    expect(window).toMatch(/if chipShown and e\.buttons == 1 and not \(e\.clientX >= frame\("chip-popover"\)\.x/);
    expect(window).toContain('editorOp(instructions ? "chip-instructions" : "chip-close", `${chip.seq}`, instructions ? chip.path : "")');
  });
  test('Settings, a page, the palette or a dialog over the chip hide it at once and close the native press', async () => {
    const window = await source('app-window.contract');
    expect(window).toContain('derive chipShown = chip.open and chip.seq != chipClosed and not chipCovered');
    const root = await source('app.contract');
    // The composer stays mounted under Settings and the palette, so nothing else would close it (⌘, or ⌘K from the keyboard).
    expect(root).toContain('derive chipCovered = chip.open and (paletteOpen or (chip.surface == "composer" and (modal or pageCover or confirmOp != "" or data.sidebar.dialog != "")))');
    expect(root).toContain('task chipCover when chipCovered key=chip.seq\n    after(1, chipCoverClose)\n  action chipCoverClose\n    editorOp("chip-close", `${chip.seq}`, "")');
    expect(root).toContain('chip=chip, chipCovered=chipCovered,');
    // `modal` covers Settings (settingsOpen), so the sample's chip is covered by the palette only.
    expect(root).toMatch(/derive modal = [^\n]*\bsettingsOpen\b/);
  });
});
