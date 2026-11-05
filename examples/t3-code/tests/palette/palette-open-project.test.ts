// CommandPalette.browser.test.tsx at T3 Code v0.1.7 (#29891), by name.
import { expect, test } from 'bun:test';
import { openProject } from '../../src/palette/palette-commands';

test("opens a searched project's latest listed thread, not a newer subagent", () => {
  const thread = (id: string, updatedAt: string, lineage: object = {}) => ({ id, projectId: 'acme', updatedAt, lineage });
  const client = { revision: 1, shell: { threads: [thread('parent', '2026-03-01T00:00:00.000Z'),
    thread('subagent', '2026-03-02T00:00:00.000Z', { relationshipToParent: 'subagent' }), thread('side', '2026-03-03T00:00:00.000Z', { sideChat: true })] } };
  expect(openProject(client as never, 'acme')).toMatchObject({ close: true, thread: 'parent' });
});
