// Ported from T3 Code 7c3b3b3605ef apps/web/src/textDirection.test.ts (MIT, see LICENSE-T3), and the sidebar row's
// title (SidebarThreadRow `dir={textDirection(thread.title)}`, the rename field's `dir="auto"`).
import { describe, expect, it } from 'bun:test';
import { autoDirection, textDirection } from '../../src/sidebar/title-direction';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import type { T3Client } from '../../src/client/client';

describe('textDirection', () => {
  it.each([
    ['Arabic', 'اكتب دالة تجمع الأرقام', 'rtl'],
    ['Hebrew', 'תקן את הבאג', 'rtl'],
    ['English', 'fix the failing test', 'ltr'],
    ['Arabic that opens with an English term', 'StrictMode: يعيد الـ render مرتين في التطوير', 'rtl'],
    ['English quoting one Arabic word', 'translate the label مرحبا for the login screen', 'ltr'],
    ['a tie, by its first word', 'React مكتبة', 'ltr'],
    ['a tie that opens in Arabic', 'مكتبة React', 'rtl'],
    ['Arabic around numbers and punctuation', 'عدّل السطر 42، ثم شغّل الاختبارات (3)', 'rtl'],
  ])('reads %s', (_, text, direction) => {
    expect(textDirection(text)).toBe(direction);
  });
  it('leaves text without letters to its surroundings', () => {
    expect(textDirection('42 — 7 ✓')).toBeUndefined();
  });
  it('dir="auto" follows the first strong letter', () => {
    expect(autoDirection('StrictMode: يعيد الـ render مرتين في التطوير')).toBe('ltr');
    expect(autoDirection('42 مرحبا')).toBe('rtl');
  });
});

it('an Arabic title reads right to left in its row, and so does its rename field', () => {
  const client = {
    shell: { projects: [{ id: 'p1', title: 'P' }], threads: [{ id: 'a', projectId: 'p1', title: 'مرحبا بالعالم', createdAt: '2026-10-27T10:00:00.000Z',
      updatedAt: '2026-10-27T10:00:00.000Z', latestRunId: null, status: 'idle', archivedAt: null, lineage: { relationshipToParent: null } }], sequence: 1 },
    config: { environment: { capabilities: {} }, providers: [], keybindings: [] }, environmentId: 'env', threadId: '', query: '', presentation: {},
    local: { drafts: {}, clientSettings: {}, deviceSettings: {} }, projectGroups: () => [{ key: 'g', name: 'P', members: [{ id: 'p1' }] }],
  } as unknown as T3Client;
  const helpers = { projectIdentity: () => ({ projectMark: '', projectInk: '', projectSurface: '' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
  expect(sidebarSnapshot(client, Date.parse('2026-10-27T12:00:00.000Z'), helpers).threads[0]).toMatchObject({ titleDir: 'rtl', renameDir: 'rtl' });
});
