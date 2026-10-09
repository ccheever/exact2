// diff-panel-parity (desktop audit 2026-10-09, PA-6 PA-7 PA-8 PA-12): the Changes header's comparison
// target and its picker, the scope menu's turn rows, file header counts, and ⌘↩ in a line comment
// draft, driven through T3Client.command as the Contract drives them, against a scripted server.
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot } from './presentation';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';
import { baseRefView, buildBaseRefChoices, emptyBaseRefPicker, filterBaseRefChoices, loadBaseRefs, normalizeBaseRef, type BaseRefPicker } from './diff-base-ref';
import { headerStat } from './diff';

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
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  const previews = () => calls.filter(call => call.method === 'review.getDiffPreview' && !obj(obj(call.payload).file).path).map(call => obj(call.payload));
  const listRefs = () => calls.filter(call => call.method === 'vcs.listRefs').map(call => obj(call.payload));
  const serve = (next: string) => { patch = next; };
  return { client, command, previews, listRefs, calls, serve };
}

describe('PA-6: the comparison target', () => {
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
  // lane diff-panel-parity, base Automatic): its static Automatic row keeps keyboard index 0 under a query,
  // while Base UI counts only the matching refs (the user's rule of 2026-10-09: match it).
  const keyFns = async () => {
    const text = await Bun.file(new URL('./diff.contract', import.meta.url)).text();
    const names = ['dbStep', 'dbKeys', 'dbMove', 'dbReturn'];
    const decls = names.map(name => {
      const found = new RegExp(`^fn ${name}\\(([^)]*)\\): [^=]+ = (.+)$`, 'm').exec(text);
      if (!found) throw new Error(`diff.contract: no fn ${name}`);
      const params = found[1].split(',').map(param => param.split(':')[0]!.trim());
      const body = found[2]!.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
      return `function ${name}(${params.join(', ')}) { return ${body}; }`;
    });
    const stdlib = 'const length = x => x.length, indexOf = (x, y) => x.indexOf(y), slice = (x, a, b) => x.slice(a, b), join = (x, s) => x.join(s), concat = (x, y) => x.concat(y), trim = s => s.trim();';
    return new Function(`${stdlib}\n${decls.join('\n')}\nreturn { ${names.join(', ')} };`)() as {
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
    const picker = (await Bun.file(new URL('./diff.contract', import.meta.url)).text()).split('\ncomponent DiffBasePicker\n')[1]!.split('\ncomponent ')[0]!;
    expect(picker).toContain('derive keys = dbKeys(map(data.diffBaseRefs, (r) => r.id), query)');
    expect(picker).toContain('let next = dbMove(keys, highlight, k == "ArrowDown", query)');
    expect(picker).toContain('let outcome = dbReturn(keys, highlight, query)\n    if outcome != "stay"\n      pickedAt = session');
    expect(picker).toContain('if outcome == "pick" and pickValue != ""\n        command("diff-base", "", pickValue, 0)');
  });
});

describe('PA-7: the scope menu', () => {
  test('Latest turn with no turns only closes the menu', async () => {
    const { client, command, previews } = harness();
    await command('diff');
    await command('diff-view', 'menu', 'scope');
    const asked = previews().length;
    await command('diff-scope', '', 'latest');
    const view = snapshot(client);
    expect(view).toMatchObject({ diffMenu: '', diffScope: 'branch', diffError: '', diffTurns: [] });
    expect(previews()).toHaveLength(asked);
  });
});

describe('PA-8: file header counts', () => {
  test("Pierre's counts read deletions first and hide a zero beside a change; DiffStatLabel reads additions first, compact", () => {
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

describe('PA-12: ⌘↩ in a line comment draft', () => {
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
    // A keyboard thread switch closes the panel and keeps the draft; no blur comes (LLP 1008). The Diff reopened on
    // another thread draws the same file and line under the same scope, but the focus was this thread's.
    client.threadId = 't2';
    client.diffOpen = false;
    await command('diff');
    expect(snapshot(client).diffCommentOpen).toBe(true);
    expect(chords()).toContain('Meta+Enter');
    // Back on the first thread the card mounts again with the focus (autofocus reports it).
    client.threadId = 't1';
    await command('diff');
    await command('diffreview', 'focus');
    expect(chords()).toBe('');
    // A refresh whose diff no longer has the draft's file: the card is gone, and so is the hold.
    serve(otherPatch);
    await command('diff-refresh');
    expect(snapshot(client).diffFiles.map(file => file.path)).toEqual(['README.md']);
    expect(chords()).toContain('Meta+Enter');
    // The file back, then collapsed: no card is drawn, so ⌘↩ is Send's again.
    serve(smallPatch);
    await command('diff-refresh');
    expect(chords()).toBe('');
    await command('diff-view', 'file', 'src/app.ts');
    expect(chords()).toContain('Meta+Enter');
  });
});
