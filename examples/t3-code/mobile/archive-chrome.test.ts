import { describe, expect, test } from 'bun:test';
import { archiveChromeEvent, archiveChromeView } from './archive-chrome';
import { answer } from './app';

const environments = [{ id: 'one:remote', label: 'Alpha' }, { id: 'two', label: 'Beta' }];
const view = (width = 393, glass = true, query = '', environment = '', sort = 'newest', rows = environments) =>
  archiveChromeView(['archive-1', width, 34, glass, query, environment, sort, rows]);
const event = (kind: string, value: string, routeKey = 'archive-1') => JSON.stringify({ routeKey, kind, value });

describe('Archive native chrome presentation', () => {
  test('compact decision follows window width and glass support, independent of empty content', () => {
    const empty = view(393, true, '', '', 'newest', []);
    expect(empty.routeKey).toBe('archive-1'); expect(empty.toolbarHeight).toBe(90);
    expect(JSON.parse(empty.configuration)).toMatchObject({ compact: true, environments: [], query: '' });
    expect(view(699).toolbarHeight).toBe(90);
    expect(view(700).toolbarHeight).toBe(0); expect(JSON.parse(view(700).configuration).compact).toBe(false);
    expect(view(1024).toolbarHeight).toBe(0); expect(view(393, false).toolbarHeight).toBe(0);
    expect(archiveChromeView(['archive', 393, -40, true, '', '', 'newest', []]).toolbarHeight).toBe(56);
  });
  test('query and missing selected scope remain recoverable on empty/filtered archive', () => {
    const query = 'No hits\n🦊 "repo"', result = view(393, true, query, 'forgotten', 'oldest', []);
    expect(JSON.parse(result.configuration)).toMatchObject({ query, environmentId: 'forgotten', sortOrder: 'oldest', environments: [] });
    expect(result.toolbarHeight).toBe(90);
    expect(archiveChromeEvent(event('search', ''), 'archive-1', [])).toEqual({ routeKey: 'archive-1', kind: 'search', value: '' });
    expect(archiveChromeEvent(event('environment', ''), 'archive-1', [])).toEqual({ routeKey: 'archive-1', kind: 'environment', value: '' });
  });
  test('sort menu selection is idempotent and exact; all environment IDs are preserved', () => {
    const selected = archiveChromeEvent(event('sort', 'oldest'), 'archive-1', environments);
    expect(selected.value).toBe('oldest'); expect(archiveChromeEvent(event('sort', 'oldest'), 'archive-1', environments)).toEqual(selected);
    expect(archiveChromeEvent(event('sort', 'newest'), 'archive-1', environments).value).toBe('newest');
    expect(archiveChromeEvent(event('environment', 'one:remote'), 'archive-1', environments).value).toBe('one:remote');
    expect(JSON.parse(view().configuration).environments).toEqual(environments);
  });
  test('late closed-route events, stale scopes and malformed values are refused', () => {
    const blank = { routeKey: '', kind: '', value: '' };
    expect(archiveChromeEvent(event('search', 'old'), 'archive-2', environments)).toEqual(blank);
    expect(archiveChromeEvent(event('search', 'old'), '', environments)).toEqual(blank);
    for (const input of ['{bad', 'null', '42', event('settings', ''), event('sort', 'toggle'), event('environment', 'gone'), JSON.stringify({ routeKey: 'archive-1', kind: 'search', value: 1 })]) {
      expect(archiveChromeEvent(input, 'archive-1', environments)).toEqual(blank);
    }
  });
  test('actual root exporters return serializable configuration and decode without native calls', () => {
    let calls = 0;
    const native = { available: true, watch() { calls++; }, async later() { calls++; throw new Error('pure chrome requested transport'); } };
    const args = ['archive-1', 393, 34, true, 'a', '', 'newest', environments];
    expect(answer('archiveChrome', args, undefined, undefined, native)).toEqual(archiveChromeView(args));
    expect(answer('archiveChromeEvent', [event('sort', 'newest'), 'archive-1', environments], undefined, undefined, native)).toEqual({ routeKey: 'archive-1', kind: 'sort', value: 'newest' });
    expect(calls).toBe(0);
  });
});
