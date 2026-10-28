// The Changes header's comparison
// target and its picker, the scope menu's turn rows, file header counts, and ⌘↩ in a line comment
// draft, driven through T3Client.command as the Contract drives them, against a scripted server.
import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import { obj, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { baseRefView, buildBaseRefChoices, emptyBaseRefPicker, filterBaseRefChoices, loadBaseRefs, normalizeBaseRef, type BaseRefPicker } from '../../src/diff/diff-base-ref';
import { headerStat } from '../../src/diff/diff';
import { appPath } from '../support/app-files';

const smallPatch = 'diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -3,3 +3,7 @@\n }\n-export const answer = 42;\n+export const answer = 43;\n+\n+export function bye(name: string): string {\n+  return `Bye, ${name}`;\n+}\n';
const local = [{ name: 'feature/audit', remoteName: null }, { name: 'main', remoteName: null }];
const remote = [{ name: 'origin/feature/audit', remoteName: 'origin' }, { name: 'origin/main', remoteName: 'origin' }, { name: 'origin/release/2026-10', remoteName: 'origin' }];

const otherPatch = 'diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1,1 +1,2 @@\n # Fixture\n+A line.\n';

function harness(large = false) {
  const calls: Obj[] = [];
  let patch = smallPatch;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const payload = obj(request.payload);
    if (request.op === 'request' && request.method === 'vcs.listRefs') {
      const query = typeof payload.query === 'string' ? payload.query : '';
      const refs = (payload.refKind === 'remote' ? remote : local).filter(ref => ref.name.includes(query));
      return { ok: true, generation: 1, value: { refs, isRepo: true, hasPrimaryRemote: true, nextCursor: null, totalCount: refs.length } };
    }
    if (request.op === 'request' && request.method === 'review.getDiffPreview') {
      const file = obj(payload.file);
      const baseRef = typeof payload.baseRef === 'string' ? payload.baseRef : 'origin/main';
      if (typeof file.path === 'string') return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: patch, truncated: false, diffHash: 'f', baseRef, headRef: 'feature/audit' }] } };
      return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'working-tree', diff: patch, truncated: false, diffHash: 'w' },
        large ? { kind: 'branch-range', diff: '', truncated: true, diffHash: 'h1', baseRef, headRef: 'feature/audit', files: [{ path: 'src/app.ts', previousPath: null, additions: 5, deletions: 1 }, { path: 'data/rows.txt', previousPath: null, additions: 2400, deletions: 0 }] }
          : { kind: 'branch-range', diff: patch, truncated: false, diffHash: patch === smallPatch ? 'h2' : 'h3', baseRef, headRef: 'feature/audit' }] } };
    }
    if (request.op === 'editorInsert' || request.op === 'editorEdit') return { ok: true, generation: 1, value: { applied: true } };
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  const previews = () => calls.filter(call => call.method === 'review.getDiffPreview' && !obj(obj(call.payload).file).path).map(call => obj(call.payload));
  const listRefs = () => calls.filter(call => call.method === 'vcs.listRefs').map(call => obj(call.payload));
  const serve = (next: string) => { patch = next; };
  return { client, command, previews, listRefs, calls, serve };
}

describe('the comparison target', () => {
  test('choices pair each local ref with its remote, leave the rest remote-only, and filter by any name', () => {
    const choices = buildBaseRefChoices([{ name: 'main', remoteName: null }], remote);
    expect(choices.map(choice => [choice.label, choice.local?.name ?? null, choice.remote?.name ?? null])).toEqual([
      ['main', 'main', 'origin/main'], ['origin/feature/audit', null, 'origin/feature/audit'], ['origin/release/2026-10', null, 'origin/release/2026-10']]);
    expect(filterBaseRefChoices(choices, ' REL ').map(choice => choice.label)).toEqual(['origin/release/2026-10']);
    expect(normalizeBaseRef('  ')).toBeNull();
    expect(normalizeBaseRef(' main ')).toBe('main');
  });

  test('the rows leave the head out, mark the remote column, and say when nothing matches', () => {
    const picker: BaseRefPicker = { query: '', cwd: '/repo', local, remote, reads: 1 };
    const view = baseRefView(picker, '/repo', 'feature/audit', null);
    expect(view.automatic).toBe(true);
    expect(view.empty).toBe(false);
    expect(view.rows.map(row => [row.label, row.value, row.remote, row.remoteOn, row.other, row.selected])).toEqual([
      ['main', 'main', 'switch', false, 'origin/main', false],
      ['origin/feature/audit', 'origin/feature/audit', 'only', false, '', false],
      ['origin/release/2026-10', 'origin/release/2026-10', 'only', false, '', false]]);
    // The remote side chosen: the row's value is the remote name, its switch on, and it picks the local name back.
    expect(baseRefView(picker, '/repo', 'feature/audit', 'origin/main').rows[0]).toMatchObject({ value: 'origin/main', remoteOn: true, other: 'main', selected: true });
    expect(baseRefView({ ...picker, query: 'zzz' }, '/repo', 'feature/audit', null)).toMatchObject({ rows: [], empty: true });
    // Lists read at another cwd are not this preview's.
    expect(baseRefView(picker, '/elsewhere', 'feature/audit', null).rows).toEqual([]);
  });

  test('Changes names head and base, the picker reads both lists, and a choice asks for the preview against it', async () => {
    const { client, command, previews, listRefs } = harness();
    await command('diff');
    let view = snapshot(client);
    expect(view).toMatchObject({ diffCompare: true, diffHead: 'feature/audit', diffBase: 'origin/main', diffBaseAutomatic: true });
    expect(previews().at(-1)).not.toHaveProperty('baseRef');
    await command('diffbase', 'open');
    expect(listRefs()).toEqual([{ cwd: '/repo', includeMatchingRemoteRefs: true, refKind: 'local', limit: 100 }, { cwd: '/repo', includeMatchingRemoteRefs: true, refKind: 'remote', limit: 100 }]);
    expect(snapshot(client).diffBaseRefs.map(row => row.label)).toEqual(['main', 'origin/feature/audit', 'origin/release/2026-10']);
    await command('diffbase', 'query', 'zzz');
    expect(listRefs().at(-1)).toMatchObject({ refKind: 'remote', query: 'zzz' });
    expect(snapshot(client)).toMatchObject({ diffBaseRefs: [], diffBaseEmpty: true });
    await command('diff-base', '', 'main');
    expect(previews().at(-1)).toMatchObject({ cwd: '/repo', baseRef: 'main' });
    view = snapshot(client);
    expect(view).toMatchObject({ diffCompare: true, diffBase: 'main', diffBaseAutomatic: false });
    // Uncommitted asks without a base and shows no comparison; Changes takes its base back.
    await command('diff-scope', '', 'unstaged');
    expect(previews().at(-1)).not.toHaveProperty('baseRef');
    expect(snapshot(client).diffCompare).toBe(false);
    await command('diff-scope', '', 'branch');
    expect(previews().at(-1)).toMatchObject({ baseRef: 'main' });
    await command('diff-base', '', '__automatic_base_ref__');
    expect(previews().at(-1)).not.toHaveProperty('baseRef');
    expect(snapshot(client)).toMatchObject({ diffBase: 'origin/main', diffBaseAutomatic: true });
  });

  test("only the newest read lands: another thread's older answer for the same query is dropped", async () => {
    const picker = emptyBaseRefPicker();
    const answers = new Map<string, (value: Obj) => void>();
    const send = (_method: string, payload: Obj) => new Promise<Obj>(resolve => answers.set(`${payload.cwd}:${payload.refKind}`, resolve));
    const refs = (names: string[]) => ({ refs: names.map(name => ({ name, remoteName: null })) });
    const older = loadBaseRefs(picker, '/a', send), newer = loadBaseRefs(picker, '/b', send);
    answers.get('/b:local')!(refs(['b-main'])); answers.get('/b:remote')!(refs([]));
    await newer;
    answers.get('/a:local')!(refs(['a-main'])); answers.get('/a:remote')!(refs([]));
    await older;
    expect(picker).toMatchObject({ cwd: '/b', local: [{ name: 'b-main', remoteName: null }], reads: 2 });
    expect(baseRefView(picker, '/b', null, null).rows.map(row => row.label)).toEqual(['b-main']);
  });

  // The picker's keys are Contract fns (diff.contract), read from the source and run as JavaScript, as
  // usage-pooled.test.ts runs usageHit. Expected values are the reference's (T3 Code 1e2ecbd975 over CDP,
  // base Automatic): its static Automatic row keeps keyboard index 0 under a query,
  // while Base UI counts only the matching refs; the clone matches it.
  const keyFns = async () => {
    const text = await Bun.file(appPath('diff.contract')).text();
    const names = ['dbStep', 'dbKeys', 'dbMove', 'dbReturn'];
    const decls = names.map(name => {
      const found = new RegExp(`^fn ${name}\\(([^)]*)\\): [^=]+ = (.+)$`, 'm').exec(text);
      if (!found) throw new Error(`diff.contract: no fn ${name}`);
      const params = found[1].split(',').map(param => param.split(':')[0]!.trim());
      const body = found[2]!.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
      return `function ${name}(${params.join(', ')}) { return ${body}; }`;
    });
    return new Function(`${decls.join('\n')}\nreturn { ${names.join(', ')} };`)() as {
      dbKeys: (refIds: string[], query: string) => string[];
      dbMove: (keys: string[], current: string, down: boolean, query: string) => string;
      dbReturn: (keys: string[], current: string, query: string) => string;
    };
  };
  const [main, feature, release] = ['local:main', 'remote:origin/feature/audit', 'remote:origin/release/2026-10'];
  /** The highlight after each ↓/↑ and Return's outcome ("stay" keeps the highlight and the popup), from `start` ("-" is none). */
  const drive = async (refIds: string[], query: string, ops: string, start = '-') => {
    const { dbKeys, dbMove, dbReturn } = await keyFns();
    const keys = dbKeys(refIds, query);
    let at = start;
    return ops.split(' ').map(op => op === 'Return' ? dbReturn(keys, at, query) : (at = dbMove(keys, at, op === '↓', query)));
  };

  test('with a query, ↓ lights Automatic first, Return on it does nothing, and the last matching ref takes no key', async () => {
    // "rel": ↓ Automatic, Return keeps the popup, ↓ none, ↓ Automatic, ↓ none, ↑ none.
    expect(await drive([release], 'rel', '↓ Return ↓ ↓ ↓ ↑')).toEqual(['automatic', 'stay', '-', 'automatic', '-', '-']);
    // "e" (origin/feature/audit, origin/release/2026-10): origin/release/2026-10 is never lit.
    expect(await drive([feature, release], 'e', '↓ ↓ ↓ ↓ ↓ ↑ ↑ ↑')).toEqual(['automatic', feature, '-', 'automatic', feature, 'automatic', '-', '-']);
    expect(await drive([feature, release], 'e', '↑ ↑ ↓')).toEqual(['-', '-', 'automatic']);
    expect(await drive([feature, release], 'e', '↓ ↓ Return')).toEqual(['automatic', feature, 'pick']);
    // "a" (main, origin/feature/audit, origin/release/2026-10).
    expect(await drive([main, feature, release], 'a', '↓ ↓ ↓ ↓ ↓ ↑ ↑')).toEqual(['automatic', main, feature, '-', 'automatic', '-', '-']);
    // "zzz": no matching ref, no key lights a row (Automatic still shows), Return only closes.
    expect(await drive([], 'zzz', '↓ ↑ Return')).toEqual(['-', '-', 'close']);
    // The pointer on the last matching ref: Base UI drops that index, so ↓ starts at Automatic and Return only closes.
    expect(await drive([feature, release], 'e', '↓ ↓', release)).toEqual(['automatic', feature]);
    expect(await drive([feature, release], 'e', 'Return', release)).toEqual(['close']);
    // A blank query is no query (trimmed, as filteredBaseRefItems tests it).
    expect(await drive([main, feature, release], '  ', '↑ ↑ Return', 'automatic')).toEqual(['-', release, 'pick']);
  });

  test('with no query the keys walk every row and Return picks Automatic too', async () => {
    // The reference, opened at the selected Automatic: ↑ none, ↑ the last, ↑ ↑, ↓.
    expect(await drive([main, feature, release], '', '↑ ↑ ↑ ↑ ↓', 'automatic')).toEqual(['-', release, feature, main, feature]);
    expect(await drive([main, feature, release], '', 'Return', 'automatic')).toEqual(['pick']);
    expect(await drive([main, feature, release], '', '↓ ↓ ↓ ↓', 'automatic')).toEqual([main, feature, release, '-']);
    // The picker's search uses them: its keys are dbKeys of the rows, ↓/↑ dbMove, and Return acts only when not "stay".
    const picker = (await Bun.file(appPath('diff.contract')).text()).split('\ncomponent DiffBasePicker\n')[1]!.split('\ncomponent ')[0]!;
    expect(picker).toContain('derive keys = dbKeys(data.diffBaseRefs.map((r) => r.id), query)');
    expect(picker).toContain('let next = dbMove(keys, highlight, k == "ArrowDown", query)');
    expect(picker).toContain('let outcome = dbReturn(keys, highlight, query)\n    if outcome != "stay"\n      hidePopover("diff-base-menu")');
    expect(picker).toContain('if outcome == "pick" and pickValue != ""\n        command("diff-base", "", pickValue, 0)');
  });

  test('Return hides pick and close outcomes, keeps queried Automatic open, and leaves rows mounted for reopen', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const picker = contractComponent('diff.contract', 'DiffBasePicker');
    const { dbKeys, dbReturn } = await keyFns();
    const refs = [{ id: main, selected: true, value: 'main' }, { id: feature, selected: false, value: 'origin/feature/audit' }];
    const cases = [
      { query: '', cursor: main, refs, expected: [['hide', 'diff-base-menu'], ['focus', 'diff-base'], ['command', 'diff-base', '', 'main', 0]] },
      { query: '', cursor: 'automatic', refs, expected: [['hide', 'diff-base-menu'], ['focus', 'diff-base'], ['command', 'diff-base', '', '__automatic_base_ref__', 0]] },
      { query: '', cursor: '-', refs, expected: [['hide', 'diff-base-menu'], ['focus', 'diff-base']] },
      { query: 'main', cursor: 'automatic', refs: refs.slice(0, 1), expected: [] },
      { query: 'zzz', cursor: '-', refs: [], expected: [['hide', 'diff-base-menu'], ['focus', 'diff-base']] },
    ];
    for (const row of cases) {
      const state = { ...picker.state(), cursor: row.cursor, cursorAt: 4 }, calls: unknown[][] = [];
      const scope = { data: { diffBaseAutomatic: false, diffBaseRefs: row.refs }, query: row.query, session: 4,
        dbKeys, dbReturn, hidePopover: (id: string) => calls.push(['hide', id]), focus: (id: string) => calls.push(['focus', id]),
        command: (...args: unknown[]) => calls.push(['command', ...args]) };
      picker.call('pick', state, { ...scope, ...picker.derives(state, scope) }, []);
      expect(calls).toEqual(row.expected);
      expect(picker.drawn('testId="diff-base-picker"', state, scope)).toBe(true);
      expect(picker.derives(state, { ...scope, query: '', session: 5 }).highlight).toBe(row.refs.length ? main : '');
    }
    const compare = contractComponent('diff.contract', 'DiffCompare'), state = compare.state(), calls: unknown[][] = [];
    compare.call('open', state, { command: (...args: unknown[]) => calls.push(args) }, []);
    expect(state).toMatchObject({ query: '', opened: 1 });
    expect(calls).toEqual([['diffbase', 'open', '', 0]]);
  });

});

describe('the scope menu', () => {
  test('Latest turn with no turns only closes the menu', async () => {
    const { client, command, previews } = harness();
    await command('diff');
    const asked = previews().length;
    await command('diff-scope', '', 'latest');
    const view = snapshot(client);
    expect(view).toMatchObject({ diffScope: 'branch', diffError: '', diffTurns: [] });
    expect(previews()).toHaveLength(asked);
    // The choice closes the popover itself (popovertargetaction="hide"); the data module keeps no menu state.
    expect(view).not.toHaveProperty('diffMenu');
  });

  // DiffPanel's DropdownMenu: a popover the trigger opens by popovertarget, and by its keys at the first or last row.
  test('the trigger opens it at its first or last row; the Turn row opens its submenu beside it', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const source = await Bun.file(appPath('diff.contract')).text();
    const line = (needle: string) => source.split('\n').find(candidate => candidate.includes(needle)) ?? '';
    expect(source).not.toContain('-187.5rem');
    expect(source).not.toContain('diff-scope-escape');
    expect(line('button id="diff-scope"')).toContain('popovertarget="diff-scope-menu" key=scopeKey');
    const menu = line('column id="diff-scope-menu"');
    for (const part of ['popover="auto"', 'position-area="bottom span-right"', 'margin-top="0.25rem"', 'toggle=toggled', 'focusin=dialogMenuEntered("diff-scope-menu", "diff-scope")']) expect(menu).toContain(part);
    expect(line('KeyMenu(menuId="diff-scope-keys"')).toContain('items=items, keyed=0, gap="0px", modal=true)');
    const turns = line('column id="diff-turns-menu"');
    for (const part of ['popover="auto"', 'position-area="right span-bottom"', 'position-try-fallbacks="flip-inline"', 'margin-top="-0.3125rem"']) expect(turns).toContain(part);
    expect(line('button id="diff-scope-turns"')).toContain('popovertarget=(data.diffTurns.length > 0 ? "diff-turns-menu" : "")');
    expect(line('button id=testId press=choose')).toContain('popovertarget=menuId popovertargetaction="hide"');
    const toolbar = contractComponent('diff.contract', 'DiffToolbar');
    const items = [{ id: 'diff-scope-branch', label: 'Changes' }, { id: 'diff-scope-turns', label: 'Turn' }];
    const kmTriggerKey = (k: string) => ['ArrowDown', 'ArrowUp', 'Enter', ' ', 'Space'].includes(k);
    const kmTriggerTarget = (rows: { id: string }[], k: string) => (k === 'ArrowUp' ? rows.slice(-1) : rows.slice(0, 1)).map(row => row.id).join('');
    for (const [k, target] of [['ArrowDown', 'diff-scope-branch'], ['ArrowUp', 'diff-scope-turns'], ['Enter', 'diff-scope-branch']] as const) {
      const calls: unknown[][] = [];
      const record = (name: string) => (...args: unknown[]) => calls.push([name, ...args]);
      toolbar.call('scopeKey', {}, { scopeItems: items, kmTriggerKey, kmTriggerTarget, preventDefault: record('preventDefault'), stopPropagation: record('stopPropagation'),
        showPopover: record('showPopover'), focus: record('focus') }, [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false }]);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['showPopover', 'diff-scope-menu', { source: 'diff-scope' }], ['focus', target]]);
    }
    const state = toolbar.state();
    toolbar.call('scopeToggled', state, {}, [{ newState: 'open' }]);
    expect(state.scopeOpen).toBe(true);
    toolbar.call('scopeToggled', state, {}, [{ newState: 'closed' }]);
    expect(state.scopeOpen).toBe(false);
    // → on Turn opens the submenu, only when the thread has turns.
    const row = contractComponent('diff.contract', 'DiffTurnsItem');
    for (const [turns, expected] of [[[{ id: 't1' }], [['preventDefault'], ['stopPropagation'], ['showPopover', 'diff-turns-menu', { source: 'diff-scope-turns' }]]], [[], []]] as const) {
      const calls: unknown[][] = [];
      const record = (name: string) => (...args: unknown[]) => calls.push([name, ...args]);
      row.call('key', row.state(), { data: { diffTurns: turns }, preventDefault: record('preventDefault'), stopPropagation: record('stopPropagation'), showPopover: record('showPopover') },
        ['ArrowRight', { metaKey: false, ctrlKey: false, altKey: false }]);
      expect(calls).toEqual(expected);
    }
  });
});

describe('file header counts', () => {
  test("Pierre's counts read deletions first and hide a zero beside a change; DiffStatLabel reads additions first, compact", () => {
    expect(headerStat({ additions: 12345, deletions: 6789 }, false)).toEqual({ statAligned: false, addText: '+12,345', delText: '-6,789' });
    expect(headerStat({ additions: 5, deletions: 1 }, false)).toEqual({ statAligned: false, addText: '+5', delText: '-1' });
    expect(headerStat({ additions: 1, deletions: 0 }, false)).toEqual({ statAligned: false, addText: '+1', delText: '' });
    expect(headerStat({ additions: 0, deletions: 0 }, false)).toEqual({ statAligned: false, addText: '+0', delText: '-0' });
    expect(headerStat({ additions: 2400, deletions: 0 }, true)).toEqual({ statAligned: true, addText: '+2.4k', delText: '-0' });
  });

  test('a large source draws DiffStatLabel on every header, a small one Pierre\'s, and the total is compact', async () => {
    const large = harness(true);
    await large.command('diff');
    const view = snapshot(large.client);
    expect(view).toMatchObject({ diffAdditionsText: '+2.4k', diffDeletionsText: '-1' });
    expect(view.diffItems.filter(item => item.kind === 'file').map(item => [item.path, item.statAligned, item.addText, item.delText])).toEqual([
      ['data/rows.txt', true, '+2.4k', '-0'], ['src/app.ts', true, '+5', '-1']]);
    const small = harness(false);
    await small.command('diff');
    expect(snapshot(small.client).diffItems.find(item => item.kind === 'file')).toMatchObject({ path: 'src/app.ts', statAligned: false, addText: '+5', delText: '-1' });
  });
});

describe('a file\'s Copy path shows while the pointer is over any of its rows (diffs-container:hover)', () => {
  test('a row names its file as the pointer enters it, once; the list forgets it as the pointer leaves', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const row = contractComponent('diff-rows.contract', 'DiffRow'), named: string[] = [];
    const scope = (hoveredPath: string) => ({ item: { kind: 'line', path: 'src/a.ts' }, hoveredPath, hoverPath: (path: string) => named.push(path) });
    row.call('rowHover', row.state(), scope(''), [true]);
    row.call('rowHover', row.state(), scope('src/a.ts'), [true]);
    row.call('rowHover', row.state(), scope('src/a.ts'), [false]);
    expect(named).toEqual(['src/a.ts']);
    const body = contractComponent('diff.contract', 'DiffPanelBody'), state = { hoveredFile: '' };
    body.call('hoverFile', state, {}, ['src/a.ts']);
    body.call('listHover', state, {}, [true]);
    expect(state.hoveredFile).toBe('src/a.ts');
    body.call('listHover', state, {}, [false]);
    expect(state.hoveredFile).toBe('');
  });
  test('the header\'s Copy path is drawn opaque while its file is hovered, the header itself or not', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const row = contractComponent('diff-rows.contract', 'DiffRow'), header = contractComponent('diff-rows.contract', 'DiffFileHeader');
    const item = { kind: 'file', path: 'src/a.ts' }, scope = { item, command: () => {}, scheme: 'red-green' };
    const handed = (hoveredPath: string) => row.callArguments('DiffFileHeader(item=item', {}, { ...scope, hoveredPath }).fileHovered;
    expect([handed('src/a.ts'), handed('src/b.ts'), handed('')]).toEqual([true, false, false]);
    const opacity = (fileHovered: boolean, hovering = false) => header.attribute('testId=`diff-copy-${item.path}`', 'opacity', { hovering, copyFocused: false }, { ...scope, fileHovered });
    expect([opacity(true), opacity(false), opacity(false, true)]).toEqual([1, 0, 1]);
  });
});

describe('⌘↩ in a line comment draft', () => {
  test("while the draft's textarea holds the focus the composer's Send declares no chord", async () => {
    const { client, command, calls } = harness();
    await command('diff');
    await command('diff-view', 'file', 'src/app.ts');
    expect(snapshot(client).composer.sendChords).toContain('Meta+Enter');
    await command('diffreview', 'comment:additions', 'src/app.ts', 7);
    expect(snapshot(client).diffCommentOpen).toBe(true);
    expect(snapshot(client).composer.sendChords).toBe('');
    await command('diffreview', 'blur');
    expect(snapshot(client).composer.sendChords).toContain('Meta+Enter');
    await command('diffreview', 'focus');
    expect(snapshot(client).composer.sendChords).toBe('');
    // Closing the panel gives the chord back while the draft stays.
    client.diffOpen = false;
    expect(snapshot(client).composer.sendChords).toContain('Meta+Enter');
    client.diffOpen = true;
    await command('diffreview', 'save', 'audit note');
    const view = snapshot(client);
    expect(view.diffCommentOpen).toBe(false);
    // The comment became the composer's "app.ts L7" chip.
    expect(String(calls.filter(call => call.op === 'editorInsert').at(-1)?.text)).toStartWith('[app.ts L7](t3-context://v1/review-comment/');
    expect(view.composer.sendChords).toContain('Meta+Enter');
    // A focus report with no draft open never holds the chord.
    await command('diffreview', 'focus');
    expect(snapshot(client).composer.sendChords).toContain('Meta+Enter');
  });

  test('a draft that left the tree without a blur holds no chord: another thread, a diff without its line, its file collapsed', async () => {
    const { client, command, serve } = harness();
    const chords = () => snapshot(client).composer.sendChords;
    await command('diff');
    await command('diff-view', 'file', 'src/app.ts');
    await command('diffreview', 'comment:additions', 'src/app.ts', 7);
    expect(chords()).toBe('');
    // A keyboard thread switch closes the panel; no blur comes (LLP 1008). The Diff reopened on another thread is that
    // thread's own scope (collapseScopeKey), so the draft is released with the viewer it belonged to, and the hold with it.
    client.threadId = 't2';
    client.diffOpen = false;
    await command('diff');
    expect(snapshot(client).diffCommentOpen).toBe(false);
    expect(chords()).toContain('Meta+Enter');
    // Back on the first thread a new draft takes the focus (autofocus reports it).
    client.threadId = 't1';
    await command('diff');
    expect(snapshot(client).diffCommentOpen).toBe(false);
    await command('diffreview', 'comment:additions', 'src/app.ts', 7);
    await command('diffreview', 'focus');
    expect(chords()).toBe('');
    // A refresh whose diff no longer has the draft's file: the card is gone, and so is the hold.
    serve(otherPatch);
    await command('diff-refresh');
    expect(snapshot(client).diffFiles.map(file => file.path)).toEqual(['README.md']);
    expect(chords()).toContain('Meta+Enter');
    // The file back: the draft was released with its file (AnnotatableCodeView), so ⌘↩ stays Send's.
    serve(smallPatch);
    await command('diff-refresh');
    expect([snapshot(client).diffCommentOpen, chords()]).toEqual([false, expect.stringContaining('Meta+Enter')]);
    // A new draft, its file then collapsed: no card is drawn, so ⌘↩ is Send's again.
    await command('diffreview', 'comment:additions', 'src/app.ts', 7);
    await command('diffreview', 'focus');
    expect(chords()).toBe('');
    await command('diff-view', 'file', 'src/app.ts');
    expect(chords()).toContain('Meta+Enter');
  });
});
