import { describe, expect, test } from 'bun:test';
import { ThreadFind, foldFindCase, literalFindRanges, findCount } from '../../src/timeline/thread-find';
import { diffFindRanges, markFindSegments } from '../../src/diff/diff-find';
const response = { snapshotSequence: 1, totalMatches: 3, activeIndex: 0, match: { entryId: 'a', runId: 'r', occurrence: 0 }, navigation: [{ entryId: 'a', runId: 'r', startIndex: 0, count: 2 }, { entryId: 'b', runId: 'r', startIndex: 2, count: 1 }] };
describe('whole-thread search lifecycle', () => {
  test('debounces, clamps the wire query and discards superseded results', () => {
    const m = new ThreadFind('thread'); m.show(); m.edit('  needle  ', 10);
    expect(m.request('t', [], 109)).toBeNull();
    const first = m.request('t', [], 110)!; expect(first.payload).toEqual({ threadId: 't', query: 'needle', skills: [] });
    m.edit('next', 111); expect(m.accept(first.serial, response)).toBeFalse();
    const second = m.request('t', [], 211)!; expect(m.accept(second.serial, response)).toBeTrue();
    m.close(); expect(m.accept(second.serial, response)).toBeFalse();
  });
  test('progressive counts prevent navigation until complete', () => {
    const m = new ThreadFind('t'); m.show(); m.edit('a', 0); const req = m.request('t', [], 100)!;
    m.accept(req.serial, { ...response, complete: false }); expect(findCount(m.result)).toBe('1/…');
    m.move(1); expect(m.result?.activeIndex).toBe(0);
    m.accept(req.serial, response); m.move(-1); expect(m.result?.match).toEqual({ entryId: 'b', runId: 'r', occurrence: 0 });
    expect(m.pending).toBeFalse(); expect(m.due).toBe(0);
  });
  test('misses outside a navigation window ask relative to the selected occurrence', () => {
    const m = new ThreadFind('t'); m.show(); m.edit('a', 0); const req = m.request('t', [], 100)!;
    m.accept(req.serial, { ...response, navigation: [] }); m.move(1); m.move(1);
    expect(m.request('t', [], 100)!.payload).toMatchObject({ start: { entryId: 'a', occurrence: 0 }, offset: 2 });
  });
  test('live updates coalesce and retain the selected occurrence', () => {
    const m = new ThreadFind('t'); m.changed('1', 0); m.show(); m.edit('a', 0); const req = m.request('t', [], 100)!;
    m.changed('2', 150); m.accept(req.serial, response); expect(m.due).toBe(1);
    const next = m.request('t', [], 151)!; expect(next.payload.start).toEqual({ entryId: 'a', occurrence: 0 });
    m.accept(next.serial, response); m.changed('3', 200); expect(m.request('t', [], 499)).toBeNull(); expect(m.request('t', [], 500)).not.toBeNull();
  });
});
test('literal search retains UTF-16 offsets, folds sigma and never expands dotted I', () => {
  expect(foldFindCase('Σςİ')).toBe('σσİ');
  expect(literalFindRanges('😀**Σς** İ', 'σ')).toEqual([{ start: 4, end: 5 }, { start: 5, end: 6 }]);
  expect(literalFindRanges('aaaa', 'aa')).toEqual([{ start: 0, end: 2 }, { start: 2, end: 4 }]);
});
test('diff options match Pierre word separators, regex failures and empty matches', () => {
  const options = { caseSensitive: false, wholeWord: true, regex: false };
  expect(diffFindRanges('hello hello_world HELLO-world', 'hello', options)).toEqual([{ start: 0, end: 5 }, { start: 18, end: 23 }]);
  expect(diffFindRanges('hello', '[', { ...options, regex: true })).toEqual([]);
  expect(diffFindRanges('a', '^|a', { ...options, wholeWord: false, regex: true })).toEqual([]);
});
test('amber matches split across syntax runs without changing text or syntax', () => {
  const marked = markFindSegments([{ id: 'a', text: 'hel', syntax: 'kw' }, { id: 'b', text: 'lo world', syntax: 'str' }], [{ start: 1, end: 5 }], { start: 1, end: 5 });
  expect(marked.map(x => [x.text, x.find, x.syntax])).toEqual([['h', 0, 'kw'], ['el', 2, 'kw'], ['lo', 2, 'str'], [' world', 0, 'str']]);
});

test('diff search counts context once, includes folded files and preserves the active match across a refresh', async () => {
  const { DiffFind } = await import('../../src/diff/diff-find');
  const find = new DiffFind(); find.reset('scope'); find.action('open', ''); find.action('query', 'foo');
  const files = [{ path: 'folded.ts', lines: [{ change: 'context' as const, content: 'foo foo', oldLineNumber: 1, newLineNumber: 1 }] }];
  find.search(files); expect(find.matches).toHaveLength(2); find.action('next', ''); expect(find.active?.start).toBe(4);
  find.search(files); expect(find.active?.start).toBe(4); find.reset('another'); expect(find.open).toBeFalse(); expect(find.matches).toEqual([]);
});

test('split context highlights both old and new line numbers with one count', async () => {
  const { DiffFind } = await import('../../src/diff/diff-find'); const find = new DiffFind(); find.action('open', ''); find.action('query', 'needle');
  find.search([{ path: 'a', lines: [{ change: 'context', content: 'needle', oldLineNumber: 1, newLineNumber: 3 }] }]);
  expect(find.matches).toHaveLength(1); expect(find.mark([{ id: 't', text: 'needle' }], 'a', 'deletions', 1)[0]?.find).toBe(2);
  expect(find.mark([{ id: 't', text: 'needle' }], 'a', 'additions', 3)[0]?.find).toBe(2);
});
