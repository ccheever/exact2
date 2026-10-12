// r5-composer: measured composer labels and menus, ref paging.
import { describe, expect, test } from 'bun:test';
import { footerLayout } from './composer-controls-view';
import { measuredLabels } from './r5-composer-measure';
import { effortMenuWidth, menuProbes, probeKey, runOnMenuWidth, runtimeMenuWidth, uniqueProbes } from './r5-composer-menus';
import { refsPageWanted } from './r6-polish-refs';
import { firstPage, mergePage, morePages, pageWanted, refsStatus, scrollEnds } from './r5-composer-paging';

// The served reference's measurements (Chrome, f90b77d809) at 1280×840: GPT-5.6-Luna 93.22, "Medium" 52.89,
// "Full access" 73.05 in the 14pt medium toolbar; the native probes report the same widths.
const anchors = { 'm-model-sm': [0, 93.22], 'm-traits-sm': [0, 52.89], 'm-runtime-sm': [0, 73.05], 'm-model-xs': [0, 80], 'm-traits-xs': [0, 45.13], 'm-runtime-xs': [0, 62.67] };
const base = { model: 'GPT-5.6-Luna', traits: 'Medium', traitsIcon: false, runtime: 'Full access', plan: '', measure: measuredLabels({ anchors }) };

describe('composer labels compact at the reference widths (resolveRestingComposerControlsLayout)', () => {
  test('blocks: picker 143.22 at -10, traits 99.89, mode 142.05 / icon-only 63, gaps 4', () => {
    // natural 143.22 + 4 + 99.89 + 4 + 142.05 = 393.16; mode icon-only 314.11; both icon-only 277.22.
    // The reference's host is this footer's controls row plus the 8pt gap before the actions.
    expect(footerLayout({ ...base, host: 393.16 - 8 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 393.1 - 8 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 314.11 - 8 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 314 - 8 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: true });
  });
  test('the Files panel at 1280: the reference keeps "Medium" with "Full access" as its icon (host 336)', () => {
    expect(footerLayout({ ...base, host: 336 - 8 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
  });
  test('a promotion back to labels needs a point of slack over the last layout', () => {
    const host = 393.16 - 8 + 0.5;
    expect(footerLayout({ ...base, host })).toMatchObject({ runtimeIconOnly: false });
    expect(footerLayout({ ...base, host, previous: { sm: 1, xs: 0 } })).toMatchObject({ runtimeIconOnly: true });
    expect(footerLayout({ ...base, host: host + 1, previous: { sm: 1, xs: 0 } })).toMatchObject({ runtimeIconOnly: false });
  });
  test('unmeasured labels fall back to the estimate; an unmeasured host never compacts', () => {
    expect(footerLayout({ ...base, measure: measuredLabels({}), host: 0 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false });
    expect(measuredLabels({ anchors: { 'm-model-sm': [3, 0] } })('model', false)).toBeNull();
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
    pages = await morePages(pages, { scrollEnds: { 'scroll:details-refs': 3 } }, 'details-refs', fetch);
    expect(calls).toEqual([]);
    // r6-polish: each signal is answered twice, first with "Loading more refs...", then with the page.
    const twice = async (ends: number) => {
      const presentation = { scrollEnds: { 'scroll:details-refs': ends } };
      const marked = await morePages(pages, presentation, 'details-refs', fetch);
      if (marked.loadingMore) {
        expect(pageWanted(marked, presentation, 'details-refs')).toBe(true);
        expect(refsStatus(marked, false)).toBe('Loading more refs...');
      }
      return morePages(marked, presentation, 'details-refs', fetch);
    };
    pages = await twice(4);
    expect(calls).toEqual([100]);
    pages = await twice(5);
    pages = await twice(6);
    expect(calls).toEqual([100, 200]);
    expect(pageWanted(pages, { scrollEnds: { 'scroll:details-refs': 6 } }, 'details-refs')).toBe(false);
    expect(pages.refs.map(ref => ref.name)).toEqual(['r0', 'r100', 'r200']);
    expect(refsStatus(pages, false)).toBe('');
    expect(refsStatus(firstPage(page(['x'], 100), 0), false)).toBe('Showing 1 of 250 refs');
    expect(scrollEnds({}, 'strip-refs')).toBe(0);
  });
});

describe('the strip and card pickers page past 100 refs', () => {
  test('a strip scroll signal loads the next page into the open picker', async () => {
    const { connected } = await import('./composer-controls-fixture');
    const { composerBranches } = await import('./composer-controls-branch');
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
    client.presentation = { ...client.presentation, scrollEnds: { 'scroll:strip-refs': 1 } };
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([100, 'Loading more refs...']);
    expect(refsPageWanted(client)).toBe(true);
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([200, 'Showing 200 of 250 refs']);
    expect(refsPageWanted(client)).toBe(false);
    // The workspace card reads the same repository (closed picker) without dropping the open picker's pages.
    await composerBranches(client, native, false, '', true);
    client.presentation = { ...client.presentation, scrollEnds: { 'scroll:strip-refs': 2 } };
    await composerBranches(client, native, true, '');
    view = await composerBranches(client, native, true, '');
    expect([view.refs.length, view.status]).toEqual([250, '']);
    expect(cursors).toEqual([undefined, 100, 200]);
  });
});
