// The thread details card's right-click menus (T3 Code v0.1.5 BranchToolbar.tsx showWorkspaceContextMenu, the
// RunContextSelector's onContextMenu in the panel form; ThreadDetailsPanel's items menu on the card): the Workspace row
// offers Copy full path while it has a path, and the rest of the card opens Show in thread details. Both are
// menu-shaped, so macOS presents them as its own menu at the pointer: a row with an icon box beside its label opened
// nothing on a right-click (exact2 issues/closed/20261031-macos-context-popover-not-menu-shaped.md).
import { describe, expect, test } from 'bun:test';
import { shellDetails } from '../../src/details/shell-details';
import { shellLocal } from '../../src/shell/shell-commands';
import { toasts } from '../../src/shell/toast';
import { setFanout } from '../../src/composer/composer-controls-fanout';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { connected } from '../support/composer-controls-fixture';

const body = contractComponent('shell-details.contract', 'DetailsBody');
const card = appText('src/details/shell-details.contract').split('\n');

/** The view lines of `component name`, each with its depth. */
function view(name: string): { depth: number; text: string }[] {
  const from = card.indexOf(`component ${name}`), to = card.findIndex((line, i) => i > from && line.startsWith('component '));
  const lines = card.slice(card.indexOf('  view', from) + 1, to < 0 ? undefined : to).filter(line => line.trim() !== '' && !line.trim().startsWith('//'));
  return lines.map(line => ({ depth: line.length - line.trimStart().length, text: line.trim() }));
}

describe('the card\'s menus are menu-shaped, so macOS presents them as its own menu at the pointer (LLP 1021 D3)', () => {
  for (const name of ['ItemsMenuPopover', 'PathMenuPopover']) test(name, () => {
    const lines = view(name);
    const rows = lines.filter(line => line.text.startsWith('button '));
    expect(rows.length).toBeGreaterThan(0);
    for (const row of rows) {
      const at = lines.indexOf(row);
      const inside = lines.slice(at + 1).filter((line, i, all) => all.slice(0, i + 1).every(entry => entry.depth > row.depth));
      // Each row is text alone: no box, icon or image beside its label, and any other text is aria-hidden.
      const texts = inside.map(line => line.text);
      expect([row.text.slice(0, 40), texts.map(line => line.split(' ')[0])]).toEqual([row.text.slice(0, 40), texts.map(() => 'text')]);
      for (const text of texts.slice(0, -1)) expect(text).toContain('aria-hidden=true');
    }
  });

  test('the items menu is headed by its label and checks each shown item; the path menu has no heading', () => {
    const items = view('ItemsMenuPopover').map(line => line.text), path = view('PathMenuPopover').map(line => line.text);
    // A menu-shaped popover's aria-label heads the Mac's menu (MenuHost.menu(of:)).
    expect(items.find(line => line.startsWith('column id="details-items-menu"'))).toContain('aria-label="Show in thread details"');
    expect(items.find(line => line.includes('id=`details-items-${item.id}`'))).toContain('role="menuitemcheckbox" aria-checked=item.checked');
    // The painted form shows the check too (the web, the agent's screenshots).
    expect(items.find(line => line.startsWith('text (item.checked ? "✓" : "")'))).toContain('aria-hidden=true');
    expect(path.find(line => line.startsWith('column id="details-path-menu"'))).not.toContain('aria-label');
  });
});

describe('the Workspace row\'s Copy full path', () => {
  test('the row has the path of its checkout, and none while a new worktree is still to be made', async () => {
    const { client, native, command } = await connected();
    const details = () => shellDetails(client, native, true, '', false);
    expect((await details()).workspacePath).toBe('/repo');
    await command('cclocal:env-mode', '', 'worktree');
    expect((await details()).workspacePath).toBe('');
    await command('cclocal:env-mode', '', 'local');
    setFanout(client, [{ instanceId: 'codex', model: 'model-a', options: [] }, { instanceId: 'claude', model: 'model-b', options: [] }]);
    expect((await details()).workspacePath).toBe(''); // each model starts in its own worktree
  });

  test('the menu names its popover only while there is a path; without one the right-click reaches the card\'s menu', () => {
    const derive = (workspacePath: string) => body.derives({ hovered: '' }, { details: { workspacePath, items: {}, git: { branch: {} }, servers: {}, automations: {}, usage: {}, lineage: [] }, density: 'full' }).pathMenu;
    expect([derive('/repo'), derive('')]).toEqual(['details-path-menu', '']);
    const select = card.find(line => line.includes('button id="details-workspace"'))!, locked = card.find(line => line.includes('testId="details-folder"'))!;
    for (const row of [select, locked]) expect(row).toContain('contextPopover=pathMenu');
    expect(card.find(line => line.includes('scroll width="100%" min-height=0'))).toContain('contextPopover="details-items-menu"');
  });

  test('its one item copies the path and toasts it, as the reference\'s does', async () => {
    const item = card.find(line => line.includes('id="details-copy-path"'))!;
    expect(item).toContain('press=act("shelllocal:details-copy-path", "", path)');
    expect(card[card.indexOf(item) + 1]!.trim()).toStartWith('text "Copy full path"');
    const copied: string[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { copied.push(String((request as Obj).text)); return { ok: true, generation: 0, value: { copied: true } }; } } as Native;
    const { client } = await connected();
    await shellLocal(client, native, 'details-copy-path', '', '/repo/worktrees/feature');
    expect(copied).toEqual(['/repo/worktrees/feature']);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Path copied', '/repo/worktrees/feature']]);
    const failing = { ...native, later: async () => ({ ok: false, generation: 0, error: 'denied' }) } as Native;
    await shellLocal(client, failing, 'details-copy-path', '', '/repo');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Failed to copy path' });
  });
});
