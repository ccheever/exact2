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
  // realinput-1010c-fixes RC-4: Base UI's Popover dismisses on Escape anywhere in the document. The native editor heard
  // Escape only while its text view had the focus (a real Escape after a click on the heading left the details open), and
  // Settings' Back took it first over the prompt sample (Escape left Settings too).
  test('Escape closes the details from any focus, before Settings\' Back, and not while the palette or a menu owns it (RC-4)', async () => {
    const layer = await source('composer-chip-popover.contract');
    expect(layer).toContain('button press=dismiss aria-keyshortcuts=(held ? "Escape" : "") aria-hidden=true tabindex=-1 testId="chip-popover-escape"');
    // The host gives an Escape to the oldest shortcut button (the lowest node id), so the details' Escape is the window's
    // first node: the right panel's toggle, made before the chip opened, took the key while a panel showed.
    const window = await source('app-window.contract');
    const view = window.slice(window.indexOf('\n  view\n    main testId="t3-code"'));
    const firstChild = view.split('\n').slice(3).find(row => !row.trimStart().startsWith('//')) ?? '';
    expect(firstChild).toBe('      ChipPopoverEscape(held=(chipShown and not (sshPrompt.open or paletteOpen or modelsOpen or optionsOpen != "" or projectsOpen or confirmOp != "")), dismiss=chipClose(false))');
    expect(window).not.toContain('dismiss=chipClose(false), escape=');
    const settings = await source('app-settings.contract');
    expect(settings).toContain('derive escapeOwned = settingsMenu != "" or modelsOpen or paletteOpen or chipOpen or ');
    expect(await source('app-window.contract')).toContain('paletteOpen=paletteOpen, chipOpen=chipShown, ');
  });
  test('Settings or a page over the composer\'s chip hides it at once and closes the native press; the palette does not', async () => {
    const window = await source('app-window.contract');
    expect(window).toContain('derive chipShown = chip.open and chip.seq != chipClosed and not chipCovered');
    const root = await source('app.contract');
    // The composer stays mounted under Settings and the pages (T3 Code unmounts it on the route change), so ⌘, alone left it open.
    expect(root).toContain('derive chipCovered = chip.open and chip.surface == "composer" and (settingsOpen or pageCover)');
    expect(root).toContain('task chipCover when chipCovered key=chip.seq\n    after(1, chipCoverClose)\n  action chipCoverClose\n    editorOp("chip-close", `${chip.seq}`, "")');
    expect(root).toContain('chip=chip, chipCovered=chipCovered,');
    // T3 Code keeps the popover open above the palette and after its Escape (both surfaces): no paletteOpen or modal term.
    expect(root).not.toMatch(/derive chipCovered = [^\n]*(paletteOpen|modal)/);
  });
});
