// Lane r10-device: the Files surface's rendered HTML (FilePreviewPanel's eye toggle and
// WorkspaceBrowserPreview), the Duo / fold stand tooltips' text, and regular expression literals and
// ternaries in HTML scripts against runs recorded from Shiki 4.2 (pierre-light) on the HEAD source.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { filesLocal, filesView } from './r4-surfaces-files';
import { adoptFilesPrefs, filesPrefs } from './r5-panels-prefs';
import { contentRevision, htmlResource, htmlToggleLabel, isHtmlPath, revisedUrl } from './r10-device-files-html';
import { highlight, type Token } from './timeline-highlight';
import { r9Folding } from './r9-device-duo';

const native = { available: true } as unknown as Native;
type Call = { method: string; payload: Record<string, unknown> };
function fakeClient(threadId = 't1') {
  const calls: Call[] = [];
  const files: Record<string, string> = { 'site/index.html': '<h1>Hi</h1>', 'notes.md': '# n' };
  const client = {
    environmentId: 'env', threadId, projectId: 'p1', ready: true, origin: 'http://127.0.0.1:9', presentation: {},
    config: {}, local: { clientSettings: { wordWrap: true } },
    shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: threadId ? [{ id: threadId, projectId: 'p1' }] : [] },
    rpc: async (_native: Native, method: string, payload: Record<string, unknown>) => { calls.push({ method, payload }); return { relativeUrl: '/api/assets/signed/abc?sig=1' }; },
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>) => {
        calls.push({ method, payload });
        if (method === 'projects.listEntries') return { entries: [{ path: 'site', kind: 'directory' }, { path: 'notes.md', kind: 'file' }] };
        if (method === 'projects.readFile') { const path = String(payload.relativePath); return { contents: files[path] ?? '', byteLength: (files[path] ?? '').length, truncated: false }; }
        return {};
      },
    }),
  } as unknown as T3Client;
  return { client, calls, files };
}
const surface = (path: string) => ({ id: `file:${path}`, kind: 'file', path, reveal: 0 }) as never;

describe('Files: rendered HTML (FilePreviewPanel renderBrowserFile)', () => {
  test('an .html file opens rendered from a signed workspace-file URL, with the eye toggle', async () => {
    const { client, calls } = fakeClient();
    const view = await filesView(client, native, surface('site/index.html'), 1000);
    expect(view.preview).toBe('html');
    expect(view.rendered).toBe(true);
    expect(view.canRender).toBe(true);
    expect(view.renderLabel).toBe('Show HTML source');
    expect(view.renderIcon).toBe('code');
    expect(view.url).toBe(`http://127.0.0.1:9/api/assets/signed/abc?sig=1&workspace-revision=${contentRevision('<h1>Hi</h1>')}`);
    expect(view.crumbsOffset).toBe(-1);
    const mint = calls.find(call => call.method === 'assets.createUrl');
    expect(mint?.payload).toEqual({ resource: { _tag: 'workspace-file', threadId: 't1', path: '/repo/site/index.html' } });
    // Show HTML source: the numbered source, and the preference sticks for the next page.
    await filesLocal(client, native, 'render', 'site/index.html', '');
    const source = await filesView(client, native, surface('site/index.html'), 2000);
    expect(source.preview).toBe('code');
    expect(source.rendered).toBe(false);
    expect(source.renderLabel).toBe('Show rendered page');
    expect(source.renderIcon).toBe('eye');
    expect(source.url).toBe('');
    expect(source.lines.length).toBe(1);
    expect(filesPrefs(client).renderBrowserFile).toBe(false);
    expect(calls.filter(call => call.method === 'assets.createUrl').length).toBe(1);
  });

  test('the preference persists and defaults to rendered; Markdown and tables keep their own', () => {
    const next = {} as { files?: unknown };
    adoptFilesPrefs(next, { files: { renderBrowserFile: false } });
    expect((next.files as { renderBrowserFile?: boolean }).renderBrowserFile).toBe(false);
    adoptFilesPrefs(next, {});
    expect((next.files as { renderBrowserFile?: boolean }).renderBrowserFile).toBeUndefined();
  });

  test('a draft names its workspace (draft-workspace-file); a refused mint shows the reference text', async () => {
    expect(htmlResource('', '/repo', '/repo/a.html', 'a.html')).toEqual({ _tag: 'draft-workspace-file', cwd: '/repo', path: 'a.html' });
    const { client } = fakeClient();
    (client as unknown as { rpc: () => Promise<never> }).rpc = async () => { throw new Error('nope'); };
    const view = await filesView(client, native, surface('site/index.html'), 1000);
    expect(view.preview).toBe('error');
    expect(view.previewError).toBe('Unable to load file preview.');
  });

  test('helpers', () => {
    expect(['a.html', 'b.HTM', 'c.html?x=1', 'd.pdf', 'e.htmlx'].map(isHtmlPath)).toEqual([true, true, true, false, false]);
    expect(htmlToggleLabel(true)).toBe('Show HTML source');
    expect(htmlToggleLabel(false)).toBe('Show rendered page');
    expect(revisedUrl('http://h/a?s=1', 'r1')).toBe('http://h/a?s=1&workspace-revision=r1');
    expect(revisedUrl('http://h/a', 'r 1')).toBe('http://h/a?workspace-revision=r%201');
    expect(contentRevision('a')).not.toBe(contentRevision('b'));
  });
});

describe('Files breadcrumbs: a mounting preview settles at the end (relaunch), a pick in an open one short', () => {
  test('restored file at mount: end; a file picked afterwards for the first time: 36pt short; hidden then shown: end', async () => {
    const { client, files } = fakeClient();
    files['docs/guide/intro.md'] = '# intro'; files['docs/guide/other.md'] = '# other';
    (client as unknown as { presentation: object }).presentation = { anchors: { crumbs: [0, 203], 'crumbs-clip': [12, 162] } };
    // Mount (a relaunch restoring the panel on this file): uncached, yet it settles at the end.
    expect((await filesView(client, native, surface('docs/guide/intro.md'), 1000)).crumbsOffset).toBe(-1);
    // A different file picked while the preview shows: its first read settles short (scrollLeft 5).
    const { ensureFile } = await import('./r4-surfaces-files');
    await ensureFile(client, native, 'docs/guide/other.md', true);
    expect((await filesView(client, native, surface('docs/guide/other.md'), 2000)).crumbsOffset).toBe(5);
    // The preview hidden (another surface) and shown again with a new uncached file: a mount, so the end.
    const { crumbsHidden } = await import('./r10-device-crumbs');
    crumbsHidden(client);
    files['docs/guide/third.md'] = '# third';
    await ensureFile(client, native, 'docs/guide/third.md', true);
    expect((await filesView(client, native, surface('docs/guide/third.md'), 3000)).crumbsOffset).toBe(-1);
    // Another thread's panel is a mount too.
    files['docs/guide/fourth.md'] = '# fourth';
    await ensureFile(client, native, 'docs/guide/fourth.md', true);
    (client as unknown as { threadId: string }).threadId = 't2';
    expect((await filesView(client, native, surface('docs/guide/fourth.md'), 4000)).crumbsOffset).toBe(-1);
  });
});

describe('A restored reveal whose view answer was abandoned asks again', () => {
  test('a folder listing that never answered is asked again after two seconds; the tree stops loading', async () => {
    const { client, files } = fakeClient();
    files['docs/guide/intro.md'] = '# intro';
    let drop = true;
    const access = (client as unknown as { restAccess: () => { request: (m: string, p: Record<string, unknown>) => Promise<unknown> } }).restAccess();
    (client as unknown as { restAccess: () => unknown }).restAccess = () => ({
      request: (method: string, payload: Record<string, unknown>) => {
        if (method === 'projects.listEntries' && payload.directoryPath === 'docs/guide' && drop) { drop = false; return new Promise(() => {}); }
        if (method === 'projects.listEntries' && payload.directoryPath === 'docs') return Promise.resolve({ entries: [{ path: 'docs/guide', kind: 'directory' }] });
        if (method === 'projects.listEntries' && payload.directoryPath === 'docs/guide') return Promise.resolve({ entries: [{ path: 'docs/guide/intro.md', kind: 'file' }] });
        return access.request(method, payload);
      },
    });
    const first = filesView(client, native, surface('docs/guide/intro.md'), 1000);
    void first; // abandoned: one of its replies never comes
    await new Promise(resolve => setTimeout(resolve, 10));
    const { filesState } = await import('./r4-surfaces-files');
    expect(filesState(client).dirs.has('docs/guide')).toBe(false);
    expect((await filesView(client, native, surface('docs/guide/intro.md'), 1500)).loading).toBe(true);
    const healed = await filesView(client, native, surface('docs/guide/intro.md'), 3600);
    expect(healed.loading).toBe(false);
    expect(filesState(client).dirs.get('docs/guide')).toEqual([{ path: 'docs/guide/intro.md', kind: 'file', ignored: false }]);
  });
});

describe('Duo and fold tooltips (DeviceDuoControls / DeviceAndroidFoldControls TooltipPopup)', () => {
  test('each stand names its pose; Book adds "/ bookshelf"; Fold / Unfold device', () => {
    const duo = r9Folding({ duo: { supported: true, hingeAngle: 180, hingePose: 'open' } }, 'ios', true, true);
    expect(duo.groups.flatMap(group => group.stands.map(stand => stand.tip))).toEqual(['Closed', 'Book / bookshelf', 'Open', 'Laptop', 'Tent']);
    const fold = r9Folding({ status: 'streaming', fold: { supported: true, posture: 'opened' } }, 'android', false, true);
    expect(fold.groups[0]!.stands.map(stand => stand.tip)).toEqual(['Fold device', 'Unfold device']);
  });
});

// Shiki 4.2 (html grammar, pierre-light) runs, as `text:colour`, mapped to the app's classes.
const COLOR: Record<string, string> = { str: '#199f43', regex: '#17a5af', punct: '#636363', flag: '#d5a910', tag: '#d5512f', kw: '#d32a61', esc: '#16a994', var: '#d47628', op: '#08c0ef', fn: '#693acf', decl: '#a631be', num: '#1ca1c7', '': '#0a0a0a' };
function colours(tokens: Token[]): string {
  const out: string[] = [];
  for (const token of tokens) for (const character of token.text) if (!/\s/.test(character)) out.push(`${character}${COLOR[token.cls] ?? token.cls}`);
  return out.join(' ');
}
function shiki(runs: [string, string][]): string {
  const out: string[] = [];
  for (const [text, colour] of runs) for (const character of text) if (!/\s/.test(character)) out.push(`${character}${colour.toLowerCase()}`);
  return out.join(' ');
}

describe('HTML scripts: regular expression literals and ternaries as Shiki colours them', () => {
  test('a literal and its parts; division is not a literal', () => {
    const code = '<script>var re = /a[b/]+c\\/d/gi; x = a / b / c;</script>';
    expect(colours(highlight(code, 'html'))).toBe(shiki([
      ['<', '#636363'], ['script', '#D5512F'], ['>', '#636363'], ['var', '#A631BE'], ['re', '#D47628'], ['=', '#08C0EF'], ['/', '#199F43'], ['a', '#17A5AF'], ['[', '#636363'], ['b/', '#D5A910'], [']', '#636363'], ['+', '#D5A910'], ['c', '#17A5AF'], ['\\/', '#16A994'], ['d', '#17A5AF'], ['/', '#199F43'], ['gi', '#D32A61'], [';', '#636363'],
      ['x', '#D47628'], ['=', '#08C0EF'], ['a', '#D47628'], ['/', '#08C0EF'], ['b', '#D47628'], ['/', '#08C0EF'], ['c', '#D47628'], [';', '#636363'], ['</', '#636363'], ['script', '#D5512F'], ['>', '#636363'],
    ]));
  });
  test('groups, classes, anchors, escapes and backreferences', () => {
    const code = '<script>y = /(x|y)?\\d{2,3}[^a-z]$\\.\\b/m.test(s)</script>';
    expect(colours(highlight(code, 'html'))).toBe(shiki([
      ['<', '#636363'], ['script', '#D5512F'], ['>', '#636363'], ['y', '#D47628'], ['=', '#08C0EF'], ['/', '#199F43'], ['(', '#636363'], ['x', '#17A5AF'], ['|', '#636363'], ['y', '#17A5AF'], [')', '#636363'], ['?', '#D5A910'], ['\\d', '#D5512F'], ['{2,3}', '#D5A910'], ['[^', '#636363'], ['a-z', '#D5A910'], [']', '#636363'], ['$', '#D32A61'], ['\\.', '#16A994'], ['\\b', '#D32A61'], ['/', '#199F43'], ['m', '#D32A61'], ['.', '#636363'], ['test', '#693ACF'], ['(', '#636363'], ['s', '#D47628'], [')</', '#636363'], ['script', '#D5512F'], ['>', '#636363'],
    ]));
    const named = '<script>a = /(?<n>q)\\1\\k<n>.*?[\\d\\]]+?/u</script>';
    expect(colours(highlight(named, 'html'))).toBe(shiki([
      ['<', '#636363'], ['script', '#D5512F'], ['>', '#636363'], ['a', '#D47628'], ['=', '#08C0EF'], ['/', '#199F43'], ['(?<', '#636363'], ['n', '#D47628'], ['>', '#636363'], ['q', '#17A5AF'], [')', '#636363'], ['\\1\\k<', '#D32A61'], ['n', '#D47628'], ['>', '#D32A61'], ['.', '#D5512F'], ['*?', '#D5A910'], ['[', '#636363'], ['\\d', '#D5512F'], ['\\]', '#16A994'], [']', '#636363'], ['+?', '#D5A910'], ['/', '#199F43'], ['u', '#D32A61'], ['</', '#636363'], ['script', '#D5512F'], ['>', '#636363'],
    ]));
  });
  test('a closing tag inside a literal stays in it; a JSX-looking group does not start JSX', () => {
    const tokens = highlight('<script>var r = /</script>/</script><p>x</p>', 'html');
    expect(colours(tokens)).toBe(shiki([
      ['<', '#636363'], ['script', '#D5512F'], ['>', '#636363'], ['var', '#A631BE'], ['r', '#D47628'], ['=', '#08C0EF'], ['/<', '#08C0EF'], ['/', '#199F43'], ['script>', '#17A5AF'], ['/', '#199F43'], ['</', '#08C0EF'], ['script', '#D47628'], ['>', '#08C0EF'], ['<', '#636363'], ['p', '#D5512F'], ['>x</', '#636363'], ['p', '#D5512F'], ['>', '#636363'],
    ]));
    expect(colours(highlight('<script>m = s.match(/(?<year>\\d{4})/u);</script>', 'html'))).toContain('y#d47628 e#d47628 a#d47628 r#d47628 >#636363 \\#d5512f');
  });
  test("a ternary's colon is a keyword, also around a literal; an object key's is punctuation", () => {
    const tokens = highlight('<script>const z = c ? /yes/ : {a: 1};</script>', 'html');
    expect(tokens.find(token => token.text.trim() === ':' && token.cls === 'kw')).toBeTruthy();
    expect(colours(highlight('x = c ? {b: 1} : 2', 'typescript'))).toBe('x#d47628 =#08c0ef c#d47628 ?#d32a61 {#636363 b#d47628 :#636363 1#1ca1c7 }#636363 :#d32a61 2#1ca1c7');
  });
});

describe('Right panels persist across launches (rightPanelStore byThreadKey)', () => {
  test('files, file tabs and the pull requests list are kept (lane r11-device keeps the rest: r11-device.test.ts)', async () => {
    const { savedPanel, adoptRightPanels, syncRightPanels, restoreRightPanel } = await import('./r10-device-panels');
    const s = (id: string, kind: string, path = '', line = 0) => ({ id, kind, path, line });
    expect(savedPanel({ visible: true, active: 'file:docs/a.md', surfaces: [s('terminal:1', 'terminal'), s('file:docs/a.md', 'file', 'docs/a.md', 3), s('device:x', 'device')] }))
      .toEqual({ visible: true, active: 'file:docs/a.md', surfaces: [{ id: 'file:docs/a.md', kind: 'file', path: 'docs/a.md', line: 3 }] });
    // An active surface that was not kept: the first survivor while open, none while closed; nothing kept is no panel.
    expect(savedPanel({ visible: true, active: 'terminal:1', surfaces: [s('terminal:1', 'terminal'), s('files', 'files')] })?.active).toBe('files');
    expect(savedPanel({ visible: false, active: 'terminal:1', surfaces: [s('terminal:1', 'terminal'), s('files', 'files')] })?.active).toBe('');
    expect(savedPanel({ visible: true, active: 'terminal:1', surfaces: [s('terminal:1', 'terminal')] })).toBeNull();
    // A malformed record is dropped on load.
    const next = {} as { rightPanels?: Record<string, unknown> };
    adoptRightPanels(next, { rightPanels: { 'env:t1': { visible: true, active: 'file:x', surfaces: [{ id: 'file:x', kind: 'file', path: 'x' }, { id: 'file:y', kind: 'file', path: 'z' }, 7] }, 'env:t2': 'nope' } });
    expect(next.rightPanels).toEqual({ 'env:t1': { visible: true, active: 'file:x', surfaces: [{ id: 'file:x', kind: 'file', path: 'x', line: 0 }] } });
    // Sync writes only on change; restore applies once to an untouched panel after the preferences were read.
    const owner = { local: {} as object, preferencesLoaded: true };
    const panels = new Map([['env:t1', { surfaces: [{ id: 'file:docs/a.md', kind: 'file' as const, path: 'docs/a.md', line: 0, reveal: 2 }], active: 'file:docs/a.md', visible: true, userRevision: 3 }]]);
    expect(syncRightPanels(owner, panels)).toBe(true);
    expect(syncRightPanels(owner, panels)).toBe(false);
    const fresh = { surfaces: [], active: '', visible: false, userRevision: 0 };
    const relaunched = { local: JSON.parse(JSON.stringify(owner.local)), preferencesLoaded: false, ready: true };
    expect(restoreRightPanel(relaunched, 'env:t1', fresh)).toBe(false);
    relaunched.preferencesLoaded = true;
    expect(restoreRightPanel(relaunched, 'env:t1', fresh)).toBe(true);
    expect(fresh).toEqual({ surfaces: [{ id: 'file:docs/a.md', kind: 'file', path: 'docs/a.md', line: 0, reveal: 0 }], active: 'file:docs/a.md', visible: true, userRevision: 0 });
    expect(restoreRightPanel(relaunched, 'env:t1', { surfaces: [], active: '', visible: false, userRevision: 0 })).toBe(false);
    // An untouched panel (before its restore) keeps the saved one; closing every tab forgets it.
    expect(syncRightPanels(owner, new Map([['env:t1', { surfaces: [], active: '', visible: false, userRevision: 0 }]]))).toBe(false);
    panels.get('env:t1')!.surfaces = [];
    expect(syncRightPanels(owner, panels)).toBe(true);
    expect((owner.local as { rightPanels?: object }).rightPanels).toEqual({});
  });
});
