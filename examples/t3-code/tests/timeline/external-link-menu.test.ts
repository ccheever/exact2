// A reply's web link menu (externalLinkContextMenu.test.ts's cases, ported) and its hookup: the window's `chat-link-menu`
// popover shows the rows `chatLinkMenuView` computes before the right-click (the window's `linkMenuRows` picks them: the
// page's, a pull request link's, else the thread's), and a row's press is `chatlocal:link-pick`, with no native menu.
import { beforeEach, describe, expect, mock, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import { resetRemoteEditorsForTests } from '../../src/connections/remote-open';
import { toasts } from '../../src/shell/toast';
import { changeChatLink, chatLinkThreadAction } from '../../src/pull-requests/pages-pr-links';
import { activeRef } from '../../src/terminal/terminal-drawer-view';
import { chatExternalLinkPick, chatLinkMenuView, externalLinkContextMenuItems, resolveExternalWebLinkHost, resolveExternalWebLinkHref, showExternalLinkContextMenu,
  type ExternalLinkContextMenuAction } from '../../src/timeline/external-link-menu';
import { notePullRequestLinks, resetPullRequestLinksForTests } from '../../src/timeline/pull-link-registry';
import { contractComponent, contractFns, value } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';
import { readFileSync } from 'node:fs';

function harness(selection: ExternalLinkContextMenuAction | null) {
  return {
    showContextMenu: mock(async (_items: unknown) => selection),
    openInPreview: mock(async (_href: string) => undefined),
    openExternal: mock(async (_href: string) => undefined),
    copyLink: mock(async (_href: string) => undefined as unknown),
    updateThreadLink: mock(async (_href: string, _linked: boolean) => undefined),
    reportFailure: mock((_operation: string, _cause: unknown) => undefined),
  };
}

describe('external chat link context menu', () => {
  test('offers both open actions and Copy Link', async () => {
    const h = harness(null);
    await showExternalLinkContextMenu({ href: 'https://example.com/docs?topic=menus#copy', ...h });
    expect(h.showContextMenu.mock.calls[0]![0]).toEqual([
      { id: 'open-in-preview', label: 'Open in integrated browser' },
      { id: 'open-external', label: 'Open in system browser' },
      { id: 'copy-link', label: 'Copy Link' },
    ]);
    expect(h.openInPreview).not.toHaveBeenCalled();
    expect(h.openExternal).not.toHaveBeenCalled();
    expect(h.copyLink).not.toHaveBeenCalled();
  });

  test("still offers the link's own actions where the integrated browser cannot be opened", async () => {
    const h = harness(null);
    await showExternalLinkContextMenu({ href: 'https://github.com/pingdotgg/t3code/pull/6169', canOpenInPreview: false, ...h });
    expect(h.showContextMenu.mock.calls[0]![0]).toEqual([
      { id: 'open-external', label: 'Open in system browser' },
      { id: 'copy-link', label: 'Copy Link' },
    ]);
  });

  test('copies the exact destination without opening it', async () => {
    const h = harness('copy-link');
    const href = 'https://example.com/docs?topic=menus#copy';
    await showExternalLinkContextMenu({ href, ...h });
    expect(h.copyLink).toHaveBeenCalledWith(href);
    expect(h.openInPreview).not.toHaveBeenCalled();
    expect(h.openExternal).not.toHaveBeenCalled();
  });

  for (const [action, label, linked] of [['link-to-thread', 'Link to thread', true], ['unlink-from-thread', 'Unlink from thread', false]] as const) {
    test(`offers and runs the ${action} action first`, async () => {
      const h = harness(action);
      const href = 'https://github.com/pingdotgg/t3code/pull/42';
      await showExternalLinkContextMenu({ href, threadLinkAction: action, ...h });
      expect((h.showContextMenu.mock.calls[0]![0] as Obj[])[0]).toEqual({ id: action, label });
      expect(h.updateThreadLink).toHaveBeenCalledWith(href, linked);
    });
  }

  for (const [selection, callback] of [['open-in-preview', 'openInPreview'], ['open-external', 'openExternal']] as const) {
    test(`preserves the ${selection} action`, async () => {
      const h = harness(selection);
      await showExternalLinkContextMenu({ href: 'https://example.com/docs', ...h });
      expect(h[callback]).toHaveBeenCalledWith('https://example.com/docs');
      expect(h.copyLink).not.toHaveBeenCalled();
    });
  }

  test('reports the selected action when it fails, and the menu when it cannot show', async () => {
    const failing = harness('copy-link');
    const cause = new Error('clipboard denied');
    failing.copyLink.mockImplementation(async () => { throw cause; });
    await showExternalLinkContextMenu({ href: 'https://example.com/docs', ...failing });
    expect(failing.reportFailure).toHaveBeenCalledWith('copy-link', cause);
    const hidden = harness(null);
    hidden.showContextMenu.mockImplementation(async () => { throw cause; });
    await showExternalLinkContextMenu({ href: 'https://example.com/docs', ...hidden });
    expect(hidden.reportFailure).toHaveBeenCalledWith('show-link-context-menu', cause);
    expect(hidden.openExternal).not.toHaveBeenCalled();
    const thread = harness('link-to-thread');
    thread.updateThreadLink.mockImplementation(async () => { throw cause; });
    await showExternalLinkContextMenu({ href: 'https://github.com/a/b/pull/42', threadLinkAction: 'link-to-thread', ...thread });
    expect(thread.reportFailure).toHaveBeenCalledWith('link-pull-request-to-thread', cause);
  });

  test('resolves an external web link host and href as the reference does', () => {
    const hosts: [string | undefined, string | null][] = [['https://example.com', 'example.com'], ['http://localhost:3000/path', 'localhost'],
      ['//cdn.example.com/clip.mp4?signature=abc#t=2', 'cdn.example.com'], ['//', null], ['#details', null], ['mailto:hello@example.com', null],
      ['file:///tmp/example.txt', null], ['javascript:void(0)', null], ['not a URL', null], [undefined, null]];
    for (const [href, host] of hosts) expect(resolveExternalWebLinkHost(href)).toBe(host);
    expect(resolveExternalWebLinkHref('//cdn.example.com/a')).toBe('https://cdn.example.com/a');
    expect(externalLinkContextMenuItems({ canOpenInPreview: true }).map(item => item.id)).toEqual(['open-in-preview', 'open-external', 'copy-link']);
  });
});

const files: Files = { fs: { mkdir: async () => undefined, readFile: async () => { throw new Error('none'); }, atomicWriteFile: async () => undefined } } as unknown as Files;

/** The rows the window's popover shows for a right-click on `href` (app-window.contract `linkMenu`, then `linkMenuRows`), as labels. */
const windowSource = readFileSync(appPath('src/shell/app-window.contract'), 'utf8');
function shownRows(client: T3Client, native: Native, storage: Files | undefined, href: string, page = false): string[] {
  const window = contractComponent('src/shell/app-window.contract', 'T3Window'), state: Obj = { linkMenuHref: '', linkMenuPage: false };
  window.call('linkMenu', state, { pageCover: page }, [href]);
  const rows = value(/^  derive linkMenuRows = (.+)$/m.exec(windowSource)![1]!, { ...state, data: { linkMenu: chatLinkMenuView(client, native, storage) } });
  return (rows as Obj[]).map(row => String(row.label));
}

describe('the reply link menu hookup', () => {
  test('the popover shows the reference rows over the open thread, and a row\'s pick acts: Copy Link copies the href', async () => {
    resetRemoteEditorsForTests();
    resetPullRequestLinksForTests();
    const owner = new T3Client();
    owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'p'; owner.threadId = 't';
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, later: async (input: unknown) => { calls.push(obj(input)); return { ok: true, generation: 0, value: {} }; } };
    expect(shownRows(owner, native, files, 'https://example.test')).toEqual(['Open in integrated browser', 'Open in system browser', 'Copy Link']);
    expect(shownRows(owner, native, files, 'https://example.test', true)).toEqual(['Open in system browser', 'Copy Link']);
    await owner.command('chatlocal:link-pick', '', 'copy-link\nhttps://example.test/docs', 0, native, files);
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['https://example.test/docs']);
    calls.length = 0;
    // A link with no web host has no menu of its own (the shell's takes its right-click), and the page offers no Browser.
    await owner.command('chatlocal:link-pick', '', 'copy-link\nmailto:a@b.c', 0, native, files);
    await owner.command('chatlocal:link-pick', 'page', 'open-in-preview\nhttps://example.test', 0, native, files);
    expect(calls.filter(call => call.op !== 'devicePresentation')).toEqual([]); // no copy, no browser, no native menu
  });
});

// Which link node a run draws (markdown.contract FlowRuns): a web link's carries the `chat-link-menu` popover with its
// `contextmenu`; a mailto link's and a fragment's carry neither, so their right-click is the desktop shell's.
describe('the Markdown link nodes', () => {
  const flow = contractComponent('src/timeline/markdown.contract', 'FlowRuns'), fns = contractFns('src/timeline/markdown.contract');
  const run = (kind: string, href: string) => ({ id: 'r1', kind, href, mono: false, label: 'docs', text: 'docs', link: '', findParts: [] });
  const drawn = (marker: string, kind: string, href: string) => flow.drawn(marker, {}, { ...fns, run: run(kind, href), chips: [] });
  const webStart = 'contextmenu=linkMenu(run.href) contextPopover="chat-link-menu" aria-label=run.label', plainStart = 'link href=run.href press=open(run.href) aria-label=run.label';
  const webWord = 'contextPopover="chat-link-menu" tabindex=0', fragmentWord = 'box press=open(run.href) tabindex=-1';
  test('a web link has the menu on its first and later words; a mailto link and a fragment leave it to the shell', () => {
    expect([drawn(webStart, 'link-start', 'https://example.com'), drawn(plainStart, 'link-start', 'https://example.com')]).toEqual([true, false]);
    for (const href of ['mailto:a@b.c', '#notes']) expect([drawn(webStart, 'link-start', href), drawn(plainStart, 'link-start', href)]).toEqual([false, true]);
    expect([drawn(webWord, 'link', 'https://example.com'), drawn(fragmentWord, 'link', 'https://example.com')]).toEqual([true, false]);
    expect([drawn(webWord, 'link', '#notes'), drawn(fragmentWord, 'link', '#notes')]).toEqual([false, true]);
    expect([drawn(webWord, 'link', 'mailto:a@b.c'), drawn(fragmentWord, 'link', 'mailto:a@b.c')]).toEqual([false, false]); // its words are text
  });
});

// ChatMarkdown's linkedThreadPullRequestFor / resolveThreadPullRequest / updateThreadPullRequestLink over the open thread
// (pages-pr-links.ts chatLinkThreadAction and changeChatLink), driven through `chatlocal:link-menu`'s handler.
const PR7 = 'https://github.com/lane/sandbox/pull/7';
const sandbox = { id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox', displayName: 'lane/sandbox', owner: 'lane', name: 'sandbox' } };
function linking(over: Obj = {}, refusal = '') {
  const dispatched: Obj[] = [];
  const client = {
    environmentId: 'env', origin: 'http://127.0.0.1:16360', connection: 'connected', statusMessage: '', scopes: [], threadId: 't1', projectId: 'p1', ready: true, revision: 0, generation: 1, error: '',
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    local: { drafts: {} as Record<string, string>, composerControls: { contexts: {} } },
    config: { environment: { label: 'Lane A', capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} },
    shell: { projects: [sandbox], threads: [
      { id: 't1', projectId: 'p1', title: 'Write the changelog', archivedAt: null, pullRequests: [] },
      { id: 't2', projectId: 'p1', title: 'Release notes', archivedAt: null, pullRequests: [{ host: 'github.com', repository: 'lane/sandbox', number: 7, url: PR7, source: 'manual' }] },
    ] as Obj[] },
    async dispatch(_native: unknown, _storage: unknown, payload: Obj) { if (refusal) throw new ClientError(refusal); dispatched.push(payload); return {}; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `command-${index + 1}`),
      call: async () => ({}),
    }),
    ...over,
  } as unknown as T3Client;
  const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
  // The rows shown for a right-click on `href` (`page`: on the pull request page), and a row's pick.
  return { client, native, dispatched, rows: (href: string, storage: Files | undefined = files, page = false) => shownRows(client, native, storage, href, page),
    pick: (id: string, href: string, storage: Files | undefined = files, page = false) => chatExternalLinkPick(client, native, storage, `${id}\n${href}`, page) };
}

describe("a reply's pull request link over the open thread", () => {
  test('Unlink where the open thread links it, Link where it could, nothing otherwise', () => {
    expect(chatLinkThreadAction(linking({ threadId: 't2' }).client, PR7)).toBe('unlink-from-thread');
    expect(chatLinkThreadAction(linking({ threadId: 't1' }).client, PR7)).toBe('link-to-thread');
    // Multiple-link servers use the parsed URL, including repositories absent from the project list.
    expect(chatLinkThreadAction(linking().client, 'https://gitlab.com/other/repo/-/merge_requests/7')).toBe('link-to-thread');
    // Not a pull request, no thread open, a server that cannot link.
    expect(chatLinkThreadAction(linking().client, 'https://example.com/docs')).toBeUndefined();
    expect(chatLinkThreadAction(linking({ threadId: '' }).client, PR7)).toBeUndefined();
    const unsupported = { config: { environment: { label: 'Lane A', capabilities: { pullRequests: true } }, settings: {} } };
    expect(chatLinkThreadAction(linking({ threadId: 't2', ...unsupported }).client, PR7)).toBeUndefined();
    // A one-link server: the thread's legacy link decides Unlink; Link needs the project's own repository.
    const single = { config: { environment: { label: 'Lane A', capabilities: { pullRequests: true, threadPullRequestLinking: true } }, settings: {} } };
    const legacy = linking({ threadId: 't1', ...single });
    (legacy.client.shell.threads[0] as Obj).linkedPullRequest = { host: 'github.com', repository: 'lane/sandbox', number: 7, url: PR7 };
    expect(chatLinkThreadAction(legacy.client, PR7)).toBe('unlink-from-thread');
    expect(chatLinkThreadAction(linking({ threadId: 't1', ...single }).client, PR7)).toBe('link-to-thread');
    expect(chatLinkThreadAction(linking({ threadId: 't1', ...single }).client, 'https://github.com/lane/elsewhere/pull/7')).toBeUndefined();
  });

  // The pull request links in the Markdown on screen, which a chip list notes as it is made (message-chips.ts).
  const mergeRequest = 'https://gitlab.com/other/repo/-/merge_requests/7';
  beforeEach(() => { resetPullRequestLinksForTests(); notePullRequestLinks(`See [the release](${PR7}) and ${mergeRequest}.`); });

  test('the menu leads with the thread action and its pick links or unlinks the open thread', async () => {
    const linked = linking({ threadId: 't2' });
    expect(linked.rows(PR7)).toEqual(['Unlink from thread', 'Open in integrated browser', 'Open in system browser', 'Copy Link']);
    await linked.pick('unlink-from-thread', PR7);
    expect(linked.dispatched).toEqual([{ type: 'thread.pull-request.unlink', commandId: 'command-1', threadId: 't2', host: 'github.com', repository: 'lane/sandbox', number: 7 }]);
    const other = linking({ threadId: 't1' });
    expect(other.rows(PR7)[0]).toBe('Link to thread');
    await other.pick('unlink-from-thread', PR7); // a row the menu does not offer
    await other.pick('link-to-thread', PR7);
    expect(other.dispatched).toEqual([{ type: 'thread.pull-request.link', commandId: 'command-1', threadId: 't1', host: 'github.com', repository: 'lane/sandbox', number: 7, url: PR7, source: 'manual' }]);
    const foreign = linking({ threadId: 't1' });
    expect(foreign.rows(mergeRequest)[0]).toBe('Link to thread');
    await foreign.pick('link-to-thread', mergeRequest);
    expect(foreign.dispatched).toEqual([{ type: 'thread.pull-request.link', commandId: 'command-1', threadId: 't1', host: 'gitlab.com', repository: 'other/repo', number: 7, url: mergeRequest, source: 'manual' }]);
    // A link node's href spelled as `new URL` spells it (host lowercased) finds the same rows.
    notePullRequestLinks('See https://GitHub.com/lane/sandbox/pull/7 too.');
    expect(other.rows('https://GitHub.com/lane/sandbox/pull/7')[0]).toBe('Link to thread');
    expect(other.rows('https://github.com/lane/sandbox/pull/7')[0]).toBe('Link to thread');
    // A pull request link not on screen yet has the plain rows until a chip list notes it.
    expect(foreign.rows('https://github.com/lane/sandbox/pull/8')[0]).toBe('Open in integrated browser');
    // Without the data module's storage (no dispatch path) the menu has no thread action.
    const bare = linking({ threadId: 't2' });
    expect(shownRows(bare.client, bare.native, undefined, PR7)).toEqual(['Open in integrated browser', 'Open in system browser', 'Copy Link']);
    await chatExternalLinkPick(bare.client, bare.native, undefined, `unlink-from-thread\n${PR7}`);
    expect(bare.dispatched).toEqual([]);
  });

  test("a new thread's draft offers the Browser, whose pick gives the draft its thread id first", async () => {
    const draft = linking({ threadId: '' });
    expect(draft.rows('https://example.com/docs')).toEqual(['Open in integrated browser', 'Open in system browser', 'Copy Link']);
    await draft.pick('open-in-preview', 'https://example.com/docs');
    expect(activeRef(draft.client)).toEqual({ environmentId: 'env', threadId: 'command-1' });
    // No project (nothing open) has no thread to open beside.
    const none = linking({ threadId: '', projectId: '' });
    expect(none.rows('https://example.com/docs')).toEqual(['Open in system browser', 'Copy Link']);
  });

  test('on the pull request page there is no thread: no thread action and no integrated browser', async () => {
    const page = linking({ threadId: 't2' });
    const draft = linking({ threadId: '' });
    expect([page.rows(PR7, files, true), draft.rows(PR7, files, true)]).toEqual([['Open in system browser', 'Copy Link'], ['Open in system browser', 'Copy Link']]);
    await page.pick('unlink-from-thread', PR7, files, true);
    await draft.pick('open-in-preview', PR7, files, true);
    expect(page.dispatched).toEqual([]);
    expect(activeRef(draft.client)).toBeNull();
  });

  test("an unlink the thread does not hold does nothing; a refusal is the chat's toast", async () => {
    const idle = linking({ threadId: 't1' });
    await changeChatLink(idle.client, idle.native, files, PR7, false);
    expect(idle.dispatched).toEqual([]);
    expect(toasts(idle.client)).toEqual([]);
    for (const [threadId, linked, title] of [['t1', true, 'Unable to link pull request'], ['t2', false, 'Unable to unlink pull request']] as const) {
      const refused = linking({ threadId }, 'Thread is archived.');
      await changeChatLink(refused.client, refused.native, files, PR7, linked);
      expect(toasts(refused.client)).toEqual([expect.objectContaining({ kind: 'error', title, description: 'Thread is archived.' })]);
      expect(refused.client.error).toBe('');
    }
  });
});
