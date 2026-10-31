// Measured composer labels and menus, ref paging.
import { describe, expect, test } from 'bun:test';
import { effortBlockWidth, footerLayout } from '../../src/composer/composer-controls-view';
import { measuredLabels } from '../../src/composer/composer-measure';
import { effortMenuWidth, menuProbes, probeKey, runOnMenuWidth, runtimeMenuWidth, uniqueProbes } from '../../src/composer/composer-menus';
import { refsPageWanted } from '../../src/composer/ref-loading';
import { firstPage, mergePage, morePages, noteRefScrollEnd, pageWanted, refsStatus, scrollEnds } from '../../src/composer/ref-paging';

// Measured labels at 1280×840: GPT-5.6-Luna 93.22, "Medium" 52.89, "Full access" 73.05 in the 14pt toolbar.
const anchors = { 'm-model-sm': [0, 93.22], 'm-traits-sm': [0, 52.89], 'm-runtime-sm': [0, 73.05] };
const measure = measuredLabels({ anchors });
const traits = effortBlockWidth({ shown: true, word: 'Medium', bars: 0, speed: '', other: '' }, measure);
const base = { model: 'GPT-5.6-Luna', traits, runtime: 'Full access', runtimeShown: true, mode: true, plan: '', measure };

describe('composer labels compact at their widths (resolveRestingComposerControlsLayout, v0.1.5 geometry)', () => {
  test('blocks: picker 129.22 (pe-1, no chevron), effort 54.89 (-ms-1), Permissions 111.05 / icon 32, 2pt apart', () => {
    expect(traits).toBeCloseTo(54.89, 2);
    // Natural 129.22 + 54.89 + 111.05 + 2 × 2 = 299.16; the chip as its icon 220.11; the effort as its gauge too 185.22.
    expect(footerLayout({ ...base, host: 299.16 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false, measuredHidden: 0 });
    expect(footerLayout({ ...base, host: 299.1 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 220.11 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 220 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: true, measuredHidden: 0 });
    // Below that the chip alone in "…" (34pt) would take 187.22, wider than both icons, so both move into it (165.22).
    expect(footerLayout({ ...base, host: 185 })).toMatchObject({ measuredHidden: 2, controlsHidden: false });
  });
  test("an unavailable model's badge widens the picker by itself and the trigger's gap", () => {
    expect(footerLayout({ ...base, badge: 52, host: 299.16 + 58 })).toMatchObject({ runtimeIconOnly: false });
    expect(footerLayout({ ...base, badge: 52, host: 299.16 + 57 })).toMatchObject({ runtimeIconOnly: true });
  });
  test('at the default the mode block is gone, so only the effort can compact', () => {
    const atDefault = { ...base, runtimeShown: false, mode: false };
    expect(footerLayout({ ...atDefault, host: 129.22 + 54.89 + 2 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false });
    expect(footerLayout({ ...atDefault, host: 129.22 + 54.89 + 1 })).toMatchObject({ traitsIconOnly: true });
  });
  test('a promotion back to labels needs a point of slack over the last layout', () => {
    const host = 299.16 + 0.5;
    expect(footerLayout({ ...base, host })).toMatchObject({ runtimeIconOnly: false });
    expect(footerLayout({ ...base, host, previous: { sm: 1 } })).toMatchObject({ runtimeIconOnly: true });
    expect(footerLayout({ ...base, host: host + 1, previous: { sm: 1 } })).toMatchObject({ runtimeIconOnly: false });
  });
  test('unmeasured labels fall back to the estimate; an unmeasured host never compacts', () => {
    expect(footerLayout({ ...base, measure: measuredLabels({}), host: 0 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false, measuredHidden: 0 });
    expect(measuredLabels({ anchors: { 'm-model-sm': [3, 0] } })('model', false)).toBeNull();
  });
  test('bars, a bolt and other traits widen the effort chip', () => {
    expect(effortBlockWidth({ shown: true, word: '', bars: 4, speed: 'Fast', other: '', }, measure)).toBe(3 + 1 + 14 + 6 + 12);
    expect(effortBlockWidth({ shown: false, word: 'High', bars: 0, speed: '', other: '' }, measure)).toBe(0);
  });
});

describe('content-sized menus from measured texts (MenuPopup min-w-40)', () => {
  const traits = [{ kind: 'header', label: 'Reasoning', description: '', isDefault: false }, { kind: 'option', label: 'Medium', description: '', isDefault: true },
    { kind: 'option', label: 'Extra High', description: 'Thinks the longest before answering, for the hardest problems', isDefault: false }];
  test('the probes list each text once at its size and weight', () => {
    const probes = menuProbes(traits, [{ label: 'Full access', description: 'Run commands without asking' }], ['This Mac', 'Build box']);
    expect(probes.map(entry => entry.key)).toContain(probeKey('Default', 10, 500));
    expect(probes.map(entry => entry.key)).toContain(probeKey('Run on', 12, 500));
    expect(uniqueProbes([...probes, ...probes]).length).toBe(probes.length);
  });
  test('the effort menu grows past 160 for a description (capped at 224) and fits its widest row', () => {
    const presentation = { anchors: { [probeKey('Extra High', 14, 400)]: [0, 70], [probeKey('Thinks the longest before answering, for the hardest problems', 12, 400)]: [0, 340],
      [probeKey('Medium', 14, 400)]: [0, 50], [probeKey('Default', 10, 500)]: [0, 34], [probeKey('Reasoning', 12, 500)]: [0, 60] } };
    expect(effortMenuWidth(presentation, traits)).toBe(224 + 16 + 10);
    expect(effortMenuWidth(presentation, traits.slice(0, 2))).toBe(160);
  });
  test('the runtime Select and Run on take their widest row', () => {
    const presentation = { anchors: { [probeKey('Full access', 14, 500)]: [0, 73], [probeKey('Run commands without asking', 12, 400)]: [0, 300],
      [probeKey('A very long environment label for testing', 14, 400)]: [0, 290] } };
    expect(runtimeMenuWidth(presentation, [{ label: 'Full access', description: 'Run commands without asking' }])).toBe(326);
    expect(runOnMenuWidth(presentation, ['A very long environment label for testing'], ['Current checkout', 'New worktree'])).toBe(290 + 14 + 26);
  });
});

describe('ref pages (usePaginatedBranches, shouldLoadNextBranchPageAfterScroll)', () => {
  const page = (names: string[], next: number | null, total = 250) => ({ refs: names.map(name => ({ name })), nextCursor: next, totalCount: total });
  test('pages merge by name, keep the last cursor and the largest total', () => {
    const merged = mergePage(firstPage(page(['a', 'b'], 2), 0), page(['b', 'c'], null, 240));
    expect(merged.refs.map(ref => ref.name)).toEqual(['a', 'b', 'c']);
    expect([merged.nextCursor, merged.total]).toEqual([null, 250]);
  });
  test('each new scroll signal loads one page while a cursor remains', async () => {
    const calls: number[] = [];
    const fetch = async (cursor: number) => { calls.push(cursor); return page([`r${cursor}`], cursor < 200 ? cursor + 100 : null); };
    let pages = firstPage(page(['r0'], 100), 3);
    pages = await morePages(pages, { 'scroll:details-refs': 3 }, 'details-refs', fetch);
    expect(calls).toEqual([]);
    // Each signal is answered twice, first with "Loading more refs...", then with the page.
    const twice = async (ends: number) => {
      const reports = { 'scroll:details-refs': ends };
      const marked = await morePages(pages, reports, 'details-refs', fetch);
      if (marked.loadingMore) {
        expect(pageWanted(marked, reports, 'details-refs')).toBe(true);
        expect(refsStatus(marked, false)).toBe('Loading more refs...');
      }
      return morePages(marked, reports, 'details-refs', fetch);
    };
    pages = await twice(4);
    expect(calls).toEqual([100]);
    pages = await twice(5);
    pages = await twice(6);
    expect(calls).toEqual([100, 200]);
    expect(pageWanted(pages, { 'scroll:details-refs': 6 }, 'details-refs')).toBe(false);
    expect(pages.refs.map(ref => ref.name)).toEqual(['r0', 'r100', 'r200']);
    expect(refsStatus(pages, false)).toBe('');
    expect(refsStatus(firstPage(page(['x'], 100), 0), false)).toBe('Showing 1 of 250 refs');
    expect(scrollEnds({}, 'strip-refs')).toBe(0);
  });
});

describe('the strip and card pickers page past 100 refs', () => {
  test('a strip scroll signal loads the next page into the open picker', async () => {
    const { connected } = await import('../support/composer-controls-fixture');
    const { composerBranches } = await import('../../src/composer/composer-controls-branch');
    const { client, native } = await connected();
    const cursors: unknown[] = [];
    const later = native.later.bind(native);
    (native as { later: (request: unknown) => Promise<unknown> }).later = async (input: unknown) => {
      const request = input as Record<string, unknown>, payload = (request.payload ?? {}) as Record<string, unknown>;
      if (request.op === 'request' && request.method === 'vcs.listRefs') {
        cursors.push(payload.cursor);
        const start = typeof payload.cursor === 'number' ? payload.cursor : 0, end = Math.min(250, start + 100);
        return { ok: true, generation: request.generation, value: { isRepo: true, totalCount: 250, nextCursor: end < 250 ? end : null,
          refs: Array.from({ length: end - start }, (_, i) => ({ name: `topic-${start + i}` })) } };
      }
      return later(input);
    };
    let view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([100, 'Showing 100 of 250 refs']);
    noteRefScrollEnd(client, 'strip-refs');
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([100, 'Loading more refs...']);
    expect(refsPageWanted(client)).toBe(true);
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([200, 'Showing 200 of 250 refs']);
    expect(refsPageWanted(client)).toBe(false);
    // The workspace card reads the same repository (closed picker) without dropping the open picker's pages.
    await composerBranches(client, native, false, '', true);
    noteRefScrollEnd(client, 'strip-refs');
    await composerBranches(client, native, true, '');
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([250, '']);
    expect(cursors).toEqual([undefined, 100, 200]);
  });
});
