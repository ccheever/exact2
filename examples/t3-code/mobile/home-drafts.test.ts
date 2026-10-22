import { describe, expect, test } from 'bun:test';
import { homeDraftLocation, homeDraftTitle, projectHomeDrafts, type HomeDraftInput } from './home-drafts';
import { serializeAssistantCitation } from './shared/diff-citations';

const draft = (key: string, patch: Partial<HomeDraftInput> = {}): HomeDraftInput => ({
  key: `new-task:${key}`, environmentId: 'one', projectId: 'project', origin: 'http://offline.invalid',
  createdAt: '2026-10-08T12:00:00.000Z', text: key, images: [], files: [], workspace: null, ...patch,
});

describe('foreground Home Draft projection', () => {
  test('only stamped new-task user content enters the list; attachments count across images and files', () => {
    const input = [draft('blank', { text: ' \n ', workspace: { branch: 'main' } }), draft('missing-project', { projectId: '' }),
      draft('missing-environment', { environmentId: '' }), draft('thread', { key: 'one:thread' }),
      draft('outbox-editor', { key: 'pending-task:message' }), draft('image', { text: '', images: [{ id: 'i' }] }),
      draft('files', { text: '', files: [{ id: 'f' }, { id: 'g' }] }),
      draft('both', { text: '', images: [{ id: 'i' }], files: [{ id: 'f' }] })];
    const before = JSON.stringify(input);
    expect(projectHomeDrafts(input).map(row => [row.draftKey, row.title])).toEqual([
      ['new-task:both', '2 attachments'], ['new-task:files', '2 attachments'], ['new-task:image', '1 attachment'],
    ]);
    expect(JSON.stringify(input)).toBe(before);
  });

  test('titles collapse source whitespace, preserve the72 character limit and decode quotes', () => {
    expect(homeDraftTitle('  first\n\tsecond  ', 0)).toBe('first second');
    expect(homeDraftTitle('x'.repeat(72), 0)).toBe('x'.repeat(72));
    expect(homeDraftTitle('x'.repeat(68) + '  abcde', 0)).toBe('x'.repeat(68) + '...');
    const citation = serializeAssistantCitation({ version: 1, environmentId: 'one', threadId: 't', messageId: 'm',
      text: 'quoted\nwords', start: 0, end: 12, prefix: '', suffix: '', comment: 'my note' });
    expect(homeDraftTitle(citation, 0)).toBe('quoted words Comment: my note');
    expect(homeDraftTitle('[Assistant quote](t3-citation://v1/broken)', 0)).toBe('[Assistant quote](t3-citation://v1/broken)');
  });

  test('creation order and stable full-key ties survive later text edits', () => {
    const a = draft('a'), b = draft('b'), newer = draft('newer', { createdAt: '2026-10-08T13:00:00.000Z' });
    expect(projectHomeDrafts([b, newer, a]).map(row => row.draftKey)).toEqual(['new-task:newer', 'new-task:a', 'new-task:b']);
    expect(projectHomeDrafts([{ ...b, text: 'edited later' }, newer, a]).map(row => row.draftKey)).toEqual(['new-task:newer', 'new-task:a', 'new-task:b']);
  });

  test('scope uses exact environment/project pairs and query searches only the derived title', () => {
    const inputs = [draft('one', { text: 'Visible title' }), draft('two', { environmentId: 'two', text: 'other title' }),
      draft('tail', { text: 'x'.repeat(80) + ' hiddenword', workspace: { branch: 'hiddenword' } })];
    const refs = [{ environmentId: 'one', projectId: 'project' }, { environmentId: 'two', projectId: 'project' }];
    expect(projectHomeDrafts(inputs, { projectRefs: refs, query: ' TITLE ' }).map(row => row.draftKey)).toEqual(['new-task:one', 'new-task:two']);
    expect(projectHomeDrafts(inputs, { environmentId: 'two', projectRefs: refs })).toHaveLength(1);
    expect(projectHomeDrafts(inputs, { projectRefs: [] })).toEqual([]);
    expect(projectHomeDrafts(inputs, { query: 'hiddenword' })).toEqual([]);
    expect(projectHomeDrafts(inputs, { projectRefs: [{ environmentId: 'missing', projectId: 'project' }] })).toEqual([]);
  });

  test('offline drafts remain visible without a loaded project; saved connections control labels', () => {
    const row = draft('a', { workspace: { branch: 'feature/one' } });
    const envs = [{ environmentId: 'one', label: 'Offline Mac', machineSymbol: 'laptopcomputer' },
      { environmentId: 'two', label: 'Other Mac', machineSymbol: 'desktopcomputer' }];
    expect(projectHomeDrafts([row])[0]).toMatchObject({ projectPresent: false, projectTitle: '', origin: row.origin, branch: 'feature/one' });
    expect(projectHomeDrafts([row], { environments: envs.slice(0, 1) })[0]?.environmentLabel).toBe('');
    expect(projectHomeDrafts([row], { environments: envs, projects: [{ environmentId: 'one', projectId: 'project', title: 'Checkout', groupTitle: 'Repository' }] })[0])
      .toMatchObject({ projectPresent: true, projectTitle: 'Repository', environmentLabel: 'Offline Mac', machineSymbol: 'laptopcomputer' });
  });

  test('local row commands keep full encoded identities and exclude thread/swipe ownership', () => {
    const inputs = [draft('a'), draft('b')], rows = projectHomeDrafts(inputs);
    expect(rows.map(row => [row.showPendingDivider, row.trailingDivider])).toEqual([[true, true], [false, false]]);
    expect(rows[0]?.menuItems).toEqual([{ id: 'discard', parentId: '', label: 'Discard', operation: 'draft-discard', value: 'new-task:a',
      symbol: 'trash', subtitle: '', destructive: true, checked: false, disabled: false }]);
    expect(rows[0]).toMatchObject({ status: 'Draft', statusSymbol: 'square.and.pencil', accessibilityHint: 'Opens the draft in the new task composer' });
    expect('threadId' in rows[0]!).toBe(false);
    expect('swipe' in rows[0]!).toBe(false);
    const url = new URL(homeDraftLocation({ key: 'new-task:quote?&', environmentId: 'one&two', projectId: 'p/#' }), 'https://test.invalid');
    expect([...url.searchParams]).toEqual([['environmentId', 'one&two'], ['projectId', 'p/#'], ['draftId', 'new-task:quote?&']]);
    expect(JSON.parse(JSON.stringify(rows))).toEqual(rows);
  });
});
