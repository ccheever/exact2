// Lane r5-panels: the details card's pull request rows, the Pull request (P)
// surface, sent attachment previews and the persisted Files preferences, against
// the reference's rules (ThreadDetailsPrRows / ThreadDetailsPrRow, useOpenPrLink,
// rightPanelStore openPullRequest / openAttachment, AttachmentFilePreview,
// FilePreviewPanel's localStorage keys). Native paths run against a recording fake.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { panelState, surfaceLocal, panelView, availability } from './r4-surfaces-panel';
import { filesLocal } from './r4-surfaces-files';
import { prRowsView, prTarget, rowAction, checksRollup, statusTooltip, providerOfUrl, prSurfaceId, threadPrTarget, forgetRowDetail, sidebarPrBadge } from './r5-panels-pr';
import { previewKind, previewDelimiter, findAttachment } from './r5-panels-attach';
import { adoptFilesPrefs, filesPrefs } from './r5-panels-prefs';
import { surfaces } from './shell';

type Call = { method: string; payload: Record<string, unknown> };
const URL72 = 'https://github.com/ccheever/exact2/pull/72';
const link = (number: number, extra: Record<string, unknown> = {}) => ({ host: 'github.com', repository: 'ccheever/exact2', number, url: `https://github.com/ccheever/exact2/pull/${number}`,
  source: 'manual', linkedAt: `2026-10-0${number % 9}T10:00:00.000Z`, snapshot: null, watch: null, ...extra });
const github = { provider: 'github', canonicalKey: 'github.com/ccheever/exact2', displayName: 'ccheever/exact2', owner: 'ccheever', name: 'exact2', locator: { remoteUrl: 'git@github.com:ccheever/exact2.git' } };
function fakeClient(over: Record<string, unknown> = {}) {
  const calls: Call[] = [];
  const replies: Record<string, (payload: Record<string, unknown>) => unknown> = {};
  const request = async (method: string, payload: Record<string, unknown>) => { calls.push({ method, payload }); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); };
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, revision: 0, generation: 1, diffOpen: false, diffText: '', diffError: '', diffLoading: false, origin: 'http://127.0.0.1:14861',
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    config: { environment: { capabilities: { pullRequests: true, threadPullRequests: true, pullRequestChecks: true } } },
    local: { clientSettings: { wordWrap: true } },
    projection: { visibleTurnItems: [{ item: { type: 'user_message', attachments: [
      { type: 'file', id: 'att-md', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 64 },
      { type: 'file', id: 'att-csv', name: 'table.csv', mimeType: 'text/csv', sizeBytes: 12 },
      { type: 'file', id: 'att-bin', name: 'blob.bin', mimeType: 'application/octet-stream', sizeBytes: 2048 },
      { type: 'image', id: 'img-1', name: 'shot.png', mimeType: 'image/png', sizeBytes: 9 },
    ] } }] },
    shell: { projects: [{ id: 'p1', title: 'p5-demo', workspaceRoot: '/repo' }, { id: 'p2', title: 'Single checkout two', workspaceRoot: '/w', repositoryIdentity: github }],
      threads: [{ id: 't1', projectId: 'p1', branch: 'main', pullRequests: [link(72)] as unknown[] }] },
    rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => request(method, payload),
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>) => request(method, payload),
      call: async (call: Record<string, unknown>) => { calls.push({ method: String(call.op), payload: call }); const reply = replies[String(call.op)]; return reply ? reply(call) : {}; },
    }),
    ...over,
  } as unknown as T3Client;
  return { client, calls, replies };
}
const native = { available: true } as unknown as Native;

describe('details card pull request rows (ThreadDetailsPrRows)', () => {
  test('a link with no snapshot is "#72" behind the muted pull request glyph, opening beside the thread', async () => {
    const { client } = fakeClient();
    const view = await prRowsView(client, native, { status: { refName: 'main' }, threadId: 't1', now: 1, projectId: 'p1' });
    expect(view.show).toBe(true);
    expect(view.rows[0]).toMatchObject({ label: '#72', icon: 'git-pull-request-arrow', tinted: false, tooltip: 'Pull request #72', aria: URL72, split: false });
    // useOpenChangeRequestLink: the project whose repository is the link's own reads it.
    expect(JSON.parse(view.rows[0]!.target)).toEqual({ projectId: 'p2', host: 'github.com', repository: 'ccheever/exact2', number: 72, url: URL72 });
    expect(view.moreLabel).toBe('');
  });
  test('a snapshot names the pull request and tints its glyph; other links fold behind "Show N more"', async () => {
    const { client } = fakeClient();
    const thread = (client.shell.threads as Record<string, unknown>[])[0]!;
    thread.pullRequests = [link(72, { snapshot: { title: 'Fix the panel', state: 'open', isDraft: true, updatedAt: '2026-10-04T00:00:00.000Z' } }),
      link(73, { snapshot: { title: 'Merged one', state: 'merged', isDraft: false, updatedAt: '2026-10-03T00:00:00.000Z' } })];
    const view = await prRowsView(client, native, { status: null, threadId: 't1', now: 1, projectId: 'p1' });
    expect(view.rows[0]).toMatchObject({ label: '#72: Fix the panel', state: 'draft', tinted: true, icon: 'git-pull-request-arrow', tooltip: 'PR #72 - Draft: Fix the panel' });
    expect(view.rest).toEqual([]);
    expect(view).toMatchObject({ moreLabel: 'Show 1 more', expanded: false });
    await surfaceLocal(client, native, 'r5-pr-more', '', '');
    const open = await prRowsView(client, native, { status: null, threadId: 't1', now: 1, projectId: 'p1' });
    expect(open.rest.map(row => [row.label, row.state, row.icon])).toEqual([['#73: Merged one', 'merged', 'git-merge']]);
    expect(open).toMatchObject({ moreLabel: 'Show less', expanded: true });
  });
  test('without links the branch\'s own pull request shows, only while the status reports that branch', async () => {
    const { client } = fakeClient();
    (client.shell.threads as Record<string, unknown>[])[0]!.pullRequests = [];
    const pr = { number: 9, title: 'Branch work', url: 'https://gitlab.com/a/b/-/merge_requests/9', state: 'open' };
    const view = await prRowsView(client, native, { status: { refName: 'main', pr, sourceControlProvider: { kind: 'gitlab' } }, threadId: 't1', now: 1, projectId: 'p1' });
    expect(view.rows[0]).toMatchObject({ label: '#9: Branch work', tooltip: 'MR #9 - Open: Branch work', target: '' });
    expect((await prRowsView(client, native, { status: { refName: 'other', pr }, threadId: 't1', now: 1, projectId: 'p1' })).show).toBe(false);
  });
  test('with the host\'s detail the row splits: state glyph, the checks, and Ready on a draft', async () => {
    const { client, replies, calls } = fakeClient();
    (client.shell.projects as Record<string, unknown>[])[0]!.repositoryIdentity = github;
    replies['pullRequests.detail'] = () => ({ number: 72, title: 'Fix the panel', state: 'open', isDraft: true, mergeability: 'mergeable',
      capabilities: { actions: ['ready', 'merge'], mergeMethods: ['merge'] }, viewerPermissions: { actions: ['ready', 'merge'] }, mergeCapabilities: { merge: true }, checks: [] });
    replies['pullRequests.checks'] = () => ({ checks: [{ name: 'ci', status: 'pending' }, { name: 'lint', status: 'success' }] });
    const view = await prRowsView(client, native, { status: null, threadId: 't1', now: 1000, projectId: 'p1' });
    expect(view.rows[0]).toMatchObject({ split: true, state: 'draft', icon: 'git-pull-request-draft', checks: '', action: 'ready', actionLabel: 'Ready' });
    expect(calls.find(call => call.method === 'pullRequests.detail')!.payload).toEqual({ projectId: 'p1', host: 'github.com', repository: 'ccheever/exact2', number: 72, allowStale: false });
    // A second read inside ten minutes reuses the answer.
    await prRowsView(client, native, { status: null, threadId: 't1', now: 2000, projectId: 'p1' });
    expect(calls.filter(call => call.method === 'pullRequests.detail').length).toBe(1);
    forgetRowDetail(client);
  });
  test('resolveThreadPanelPullRequestAction and the checks rollup', () => {
    const base = { state: 'open', capabilities: { actions: ['merge', 'ready'], mergeMethods: ['squash'] }, viewerPermissions: { actions: ['merge', 'ready'] }, mergeCapabilities: { squash: true } };
    expect(rowAction({ ...base, mergeability: 'conflicting' })).toBe('resolve');
    expect(rowAction({ ...base, isDraft: true })).toBe('ready');
    expect(rowAction({ ...base, checks: [{ status: 'failure' }] })).toBe('fix');
    expect(rowAction({ ...base, checks: [{ status: 'pending' }] })).toBe('');
    expect(rowAction({ ...base, checks: [{ status: 'success' }] })).toBe('merge');
    expect(rowAction({ ...base, state: 'merged' })).toBe('');
    expect(checksRollup([])).toBe('');
    expect(checksRollup([{ status: 'success' }, { status: 'pending' }])).toBe('pending');
    expect(checksRollup([{ status: 'skipped' }])).toBe('');
    expect(statusTooltip({ number: 4, url: '', title: 'T', state: 'closed', isDraft: false, provider: providerOfUrl('https://bitbucket.org/a/b/pull-requests/4') })).toBe('PR #4 - Closed: T');
  });
  test('a link no project here can read stays an ordinary link', () => {
    const { client } = fakeClient();
    expect(prTarget(client, 'https://example.com/not/a/pr')).toBeNull();
    const unread = fakeClient({ config: { environment: { capabilities: { threadPullRequests: true } } } }).client;
    expect(prTarget(unread, URL72)).toBeNull();
  });
});

describe('the sidebar pull request badge (resolveThreadPullRequestBadge)', () => {
  test('one link, several unrelated links, a stack, and tombstones', () => {
    expect(sidebarPrBadge({ pullRequests: [link(72)] })).toEqual({ badge: '72', stacked: false, badgeIcon: 'git-pull-request-arrow', badgeState: '' });
    expect(sidebarPrBadge({ pullRequests: [link(72, { snapshot: { state: 'merged', title: 'x' } })] })).toEqual({ badge: '72', stacked: false, badgeIcon: 'git-merge', badgeState: 'merged' });
    expect(sidebarPrBadge({ pullRequests: [link(72), link(73)] })).toEqual({ badge: '+2', stacked: false, badgeIcon: 'git-pull-request-arrow', badgeState: 'open' });
    const layer = (number: number, head: string, base: string) => link(number, { snapshot: { state: 'open', isDraft: false, headBranch: head, baseBranch: base, title: 't' } });
    expect(sidebarPrBadge({ pullRequests: [layer(1, 'a', 'main'), layer(2, 'b', 'a')] })).toEqual({ badge: '2', stacked: true, badgeIcon: 'layers', badgeState: 'open' });
    expect(sidebarPrBadge({ pullRequests: [link(72, { source: 'stack-dismissed' })] }).badge).toBe('');
    expect(sidebarPrBadge({ pullRequests: [link(1, { snapshot: { state: 'closed' } }), link(2, { snapshot: { state: 'merged' } })] }).badgeState).toBe('closed');
  });
});

describe('the Pull request (P) surface', () => {
  test('the chooser enables it with a panel target and opens "#72" as a tab keyed by its reference', async () => {
    const { client, replies } = fakeClient();
    expect(availability(client).pullRequest).toBe(true);
    expect(surfaces(client).find(row => row.id === 'pull-request')).toMatchObject({ available: true, shortcut: 'P' });
    const target = threadPrTarget(client)!;
    expect(target).toEqual({ projectId: 'p1', host: 'github.com', repository: 'ccheever/exact2', number: 72, url: URL72 });
    await surfaceLocal(client, native, 'open', '', 'pull-request');
    const state = panelState(client);
    expect(state.active).toBe(prSurfaceId(target));
    expect(state.active).toBe('pull-request:p1:github.com:ccheever%2Fexact2:72');
    replies['projects.listEntries'] = () => ({ entries: [] });
    const view = await panelView(client, null, 1);
    expect(view.tabs[0]).toMatchObject({ kind: 'pull-request', title: '#72', icon: 'git-pull-request-arrow', tone: '' });
    expect(view.pr).toEqual({ selected: JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'ccheever/exact2', number: 72 }), number: 72, label: '#72' });
    // The details row opens the same kind of surface through the link's project; the tab is its peer.
    await surfaceLocal(client, native, 'r5-pr-open', '', URL72);
    expect(state.surfaces.map(entry => entry.id)).toEqual(['pull-request:p1:github.com:ccheever%2Fexact2:72', 'pull-request:p2:github.com:ccheever%2Fexact2:72']);
    await surfaceLocal(client, native, 'close', state.active, '');
    expect(state.surfaces.length).toBe(1);
  });
  test('a pull request tab wears its linked snapshot\'s lifecycle', async () => {
    const { client } = fakeClient();
    (client.shell.threads as Record<string, unknown>[])[0]!.pullRequests = [link(72, { snapshot: { title: 'T', state: 'merged', isDraft: false, syncedAt: '2026-10-04T00:00:00.000Z' } })];
    await surfaceLocal(client, native, 'open', '', 'P');
    const view = await panelView(client, null, 1);
    expect(view.tabs[0]).toMatchObject({ title: '#72', icon: 'git-merge', tone: 'merged' });
  });
  test('without the environment\'s pull request reads the row stays unavailable with the reference hint', () => {
    const { client } = fakeClient({ config: { environment: { capabilities: { threadPullRequests: true } } } });
    expect(surfaces(client).find(row => row.id === 'pull-request')).toMatchObject({ available: false, reason: 'No pull request on this branch yet.' });
  });
});

describe('sent attachment preview (AttachmentFilePreview)', () => {
  test('filePreviewKind and the delimiter follow the MIME type, then the name', () => {
    expect(previewKind({ name: 'notes.md', mimeType: 'text/markdown' })).toBe('markdown');
    expect(previewKind({ name: 'a.ts', mimeType: 'application/octet-stream' })).toBe('text');
    expect(previewKind({ name: 'Makefile' })).toBe('text');
    expect(previewKind({ name: 'shot.PNG' })).toBe('image');
    expect(previewKind({ name: 'blob.bin', mimeType: 'application/octet-stream' })).toBe('unsupported');
    expect(previewDelimiter({ name: 'x.tsv' })).toBe('\t');
    expect(previewDelimiter({ name: 'x.csv', mimeType: 'application/json' })).toBeNull();
  });
  test('a chip opens its own tab, replacing the standalone explorer; the text renders as Markdown first', async () => {
    const { client, replies, calls } = fakeClient();
    const state = panelState(client);
    state.surfaces = [{ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }];
    replies['assets.createUrl'] = payload => ({ relativeUrl: `/api/assets/${(payload.resource as { disposition: string }).disposition}/sig`, expiresAt: 0 });
    replies.attachmentText = () => ({ ok: true, text: '# Notes\n\n- one\n', truncated: false });
    await surfaceLocal(client, native, 'r5-attachment', 'att-md', 'owner');
    expect(state.surfaces.map(entry => entry.id)).toEqual(['attachment:att-md']);
    let view = await panelView(client, native, 1000);
    expect(view.tabs[0]).toMatchObject({ kind: 'attachment', title: 'notes.md', fileToken: 'markdown' });
    expect(view.attachment).toMatchObject({ name: 'notes.md', size: '1 KB', preview: 'markdown', canRender: true, rendered: true, renderLabel: 'Show markdown source', renderIcon: 'code',
      showWrap: false, canCopy: true, copyLabel: 'Copy contents', canSave: true });
    expect(view.attachment.markdown.blocks.length).toBeGreaterThan(0);
    expect(calls.find(call => call.method === 'attachmentText')!.payload.url).toBe('http://127.0.0.1:14861/api/assets/inline/sig');
    await surfaceLocal(client, native, 'r5-att-render', 'att-md', '');
    view = await panelView(client, native, 1000);
    expect(view.attachment).toMatchObject({ preview: 'code', renderLabel: 'Show rendered markdown', renderIcon: 'eye', showWrap: true, wrap: true });
    expect(view.attachment.lines.map(line => line.number)).toEqual(['1', '2', '3', '4']);
    await surfaceLocal(client, native, 'r5-att-copy', 'att-md', '');
    expect(calls.find(call => call.method === 'copyText')!.payload.text).toBe('# Notes\n\n- one\n');
    view = await panelView(client, native, 5000);
    expect(view.attachment).toMatchObject({ copied: true, copyLabel: 'Copied' });
    view = await panelView(client, native, 7100);
    expect(view.attachment.copied).toBe(false);
    replies.attachmentSave = call => ({ ok: true, saved: true, name: call.name });
    await surfaceLocal(client, native, 'r5-att-save', 'att-md', '');
    const save = calls.find(call => call.method === 'attachmentSave')!;
    expect(save.payload).toMatchObject({ url: 'http://127.0.0.1:14861/api/assets/attachment/sig', name: 'notes.md' });
  });
  test('a CSV renders as a table; a file with no preview offers Save; an unknown id fails plainly', async () => {
    const { client, replies } = fakeClient();
    replies['assets.createUrl'] = () => ({ relativeUrl: '/api/assets/x', expiresAt: 0 });
    replies.attachmentText = () => ({ ok: true, text: 'a,b\n1,2\n', truncated: false });
    await surfaceLocal(client, native, 'r5-attachment', 'att-csv', '');
    let view = await panelView(client, native, 1);
    expect(view.attachment).toMatchObject({ preview: 'table', renderLabel: 'Show source', renderIcon: 'code' });
    expect(view.attachment.table[0]!.cells.map(cell => cell.text)).toEqual(['a', 'b']);
    await surfaceLocal(client, native, 'r5-attachment', 'att-bin', '');
    view = await panelView(client, native, 1);
    expect(view.attachment).toMatchObject({ preview: 'none', canCopy: false, canSave: true, noPreview: 'Save it to open in an app that supports bin files.' });
    expect(findAttachment(client, 'img-1')).toBeNull(); // images open the expanded image dialog
    await expect(surfaceLocal(client, native, 'r5-attachment', 'gone', '')).rejects.toThrow('That attachment is no longer available.');
  });
  test('a read failure shows the message and Try again reads afresh', async () => {
    const { client, replies } = fakeClient();
    replies['assets.createUrl'] = () => ({ relativeUrl: '/api/assets/x', expiresAt: 0 });
    replies.attachmentText = () => ({ ok: false, message: 'This file is not UTF-8 text. Open it in another app to view its contents.' });
    await surfaceLocal(client, native, 'r5-attachment', 'att-md', '');
    let view = await panelView(client, native, 1);
    expect(view.attachment).toMatchObject({ preview: 'error', error: 'This file is not UTF-8 text. Open it in another app to view its contents.', canCopy: false });
    replies.attachmentText = () => ({ ok: true, text: 'ok', truncated: true });
    await surfaceLocal(client, native, 'r5-att-retry', 'att-md', '');
    view = await panelView(client, native, 1);
    expect(view.attachment).toMatchObject({ preview: 'markdown', copyLabel: 'Copy preview', truncatedNote: 'Preview limited to the first 1 MB of a 64 byte file. Save the file to read it in full.' });
  });
});

describe('Files preferences persist (FilePreviewPanel localStorage keys)', () => {
  test('defaults: explorer open, Markdown as source, tables rendered; toggles land in the saved record', async () => {
    const { client } = fakeClient();
    expect(filesPrefs(client)).toEqual({ explorer: true, renderMarkdown: false, renderTable: true });
    await filesLocal(client, native, 'explorer', '', '');
    await filesLocal(client, native, 'render', 'README.md', '');
    await filesLocal(client, native, 'render', 'data.csv', '');
    await filesLocal(client, native, 'wrap', '', '');
    const saved = JSON.parse(JSON.stringify({ version: 1, ...(client.local as object) }));
    expect(saved.files).toEqual({ explorer: false, renderMarkdown: true, renderTable: false });
    expect(saved.clientSettings.wordWrap).toBe(false);
    const next = {};
    adoptFilesPrefs(next, saved);
    expect(next).toEqual({ files: { explorer: false, renderMarkdown: true, renderTable: false } });
    const fresh = {};
    adoptFilesPrefs(fresh, { files: { explorer: 'yes' } });
    expect(fresh).toEqual({ files: { explorer: true, renderMarkdown: false, renderTable: true } });
  });
});
