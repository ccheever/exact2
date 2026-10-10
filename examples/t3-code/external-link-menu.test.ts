// shell-context-menu: a reply's web link menu (externalLinkContextMenu.test.ts's cases, ported) and its hookup: the
// Markdown link nodes carry `contextmenu`, the root action names `chatlocal:link-menu`, and that op reaches the native
// `contextMenu` op with the reference's items.
import { describe, expect, mock, test } from 'bun:test';
import { obj, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { T3Client } from './client';
import { resetRemoteEditorsForTests } from './remote-open';
import { toasts } from './toast';
import { changeChatLink, chatLinkThreadAction } from './pages-pr-links';
import { activeRef } from './terminal-drawer-view';
import { chatExternalLinkMenu, externalLinkContextMenuItems, resolveExternalWebLinkHost, resolveExternalWebLinkHref, showExternalLinkContextMenu,
  type ExternalLinkContextMenuAction } from './external-link-menu';

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
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

describe('the reply link menu hookup', () => {
  test("every Markdown web link node and the pull request link carry the menu; the root names the op", async () => {
    const markdown = await source('markdown.contract');
    const lines = markdown.split('\n');
    const links = lines.filter(line => line.includes('press=linkOpen("link", run.href)'));
    expect(links.length).toBe(4);
    // The link-start run's text sits in a row with its globe, which carries the menu for both (a right-click bubbles to it).
    expect(links.filter(line => line.includes('contextmenu=linkMenu(run.href)')).length).toBe(3);
    expect(markdown).toContain('row contextmenu=linkMenu(run.href) flex-shrink=0 align-items="flex-start"');
    // Only a web link has the menu (ChatMarkdown: `if (!href || !faviconHost) return;`): each handler sits under a branch
    // that asserts webLink(run.href), so a mailto, irc, xmpp or fragment link leaves its click to the shell's menu.
    const indent = (line: string) => line.length - line.trimStart().length;
    const branches = (at: number) => {
      const chain: string[] = [];
      for (let i = at - 1, depth = indent(lines[at]!); i >= 0 && depth > 0; i--) {
        if (lines[i]!.trim() === '' || indent(lines[i]!) >= depth) continue;
        depth = indent(lines[i]!); chain.push(lines[i]!.trim());
        // An `else` stands for the negation of the `when` just above it.
        if (lines[i]!.trim() === 'else') { const when = lines.slice(0, i).reverse().find(line => indent(line) === depth)!; chain.push(`not ${when.trim()}`); }
      }
      return chain;
    };
    const handlers = lines.flatMap((line, at) => line.includes('contextmenu=linkMenu(run.href)') ? [at] : []);
    expect(handlers.length).toBe(4);
    for (const at of handlers) {
      const chain = branches(at);
      expect(chain.some(branch => /^when (.* and )?webLink\(run\.href\)$/.test(branch))).toBe(true);
      expect(chain.some(branch => branch.startsWith('not when webLink') || branch.startsWith('else'))).toBe(false);
    }
    // FlowRuns: a link-start run of any other link is plain link text, with no favicon and no handler.
    const plain = lines.findIndex(line => line.includes('text run.text href=(run.kind == "link" or run.kind == "link-start" ? run.href : "") white-space="pre"'));
    expect(plain).toBeGreaterThan(0);
    expect(lines[plain]).not.toContain('contextmenu=');
    expect(branches(plain).some(branch => branch.endsWith('and not (run.kind == "link-start" and webLink(run.href))'))).toBe(true);
    expect(markdown).toContain('contextmenu=local("link-menu", "", run.href) hover=hover role="link"');
    expect(markdown.match(/ {2}inject\n {4}linkOpen: action\n {4}linkMenu: action\n/g)?.length).toBe(2);
    const window = await source('app-window.contract');
    expect(window).toMatch(/provide[\s\S]*?\n {4}linkOpen\n {4}linkMenu\n/);
    expect(window).toMatch(/action linkMenu\(href: string\)\n {4}chatLocal\("link-menu", "", href\)/);
  });

  test('chatlocal:link-menu opens the reference items over the open thread, and Copy Link copies the href', async () => {
    resetRemoteEditorsForTests();
    const owner = new T3Client();
    owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'p'; owner.threadId = 't';
    const calls: Obj[] = [];
    let pick: string | null = null;
    const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = obj(input); calls.push(request);
      return { ok: true, generation: 0, value: request.op === 'contextMenu' ? { clicked: pick } : {} };
    } };
    await owner.command('chatlocal:link-menu', '', 'https://example.test', 0, native, files);
    const menus = () => calls.filter(call => call.op === 'contextMenu').map(call => (call.items as Obj[]).map(item => item.label));
    expect(menus()).toEqual([['Open in integrated browser', 'Open in system browser', 'Copy Link']]);
    pick = 'copy-link';
    await owner.command('chatlocal:link-menu', '', 'https://example.test/docs', 0, native, files);
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['https://example.test/docs']);
    calls.length = 0;
    await owner.command('chatlocal:link-menu', '', 'mailto:a@b.c', 0, native, files);
    expect(menus()).toEqual([]);
  });
});

// ChatMarkdown's linkedThreadPullRequestFor / resolveThreadPullRequest / updateThreadPullRequestLink over the open thread
// (pages-pr-links.ts chatLinkThreadAction and changeChatLink), driven through `chatlocal:link-menu`'s handler.
const PR7 = 'https://github.com/lane/sandbox/pull/7';
const sandbox = { id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox', displayName: 'lane/sandbox', owner: 'lane', name: 'sandbox' } };
function linking(over: Obj = {}, refusal = '') {
  const dispatched: Obj[] = [], menus: string[][] = [];
  let pick = '';
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
      call: async (request: Obj) => {
        if (request.op !== 'contextMenu') return {};
        menus.push((request.items as Obj[]).map(item => String(item.label)));
        return { clicked: pick };
      },
    }),
    ...over,
  } as unknown as T3Client;
  const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
  return { client, native, dispatched, menus, choose: (id: string) => { pick = id; } };
}

describe("a reply's pull request link over the open thread", () => {
  test('Unlink where the open thread links it, Link where it could, nothing otherwise', () => {
    expect(chatLinkThreadAction(linking({ threadId: 't2' }).client, PR7)).toBe('unlink-from-thread');
    expect(chatLinkThreadAction(linking({ threadId: 't1' }).client, PR7)).toBe('link-to-thread');
    // Not a pull request, a pull request on a host no project is on, no thread open, a server that cannot link.
    expect(chatLinkThreadAction(linking().client, 'https://example.com/docs')).toBeUndefined();
    expect(chatLinkThreadAction(linking().client, 'https://gitlab.com/other/repo/-/merge_requests/7')).toBeUndefined();
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

  test('the menu leads with the thread action and its pick links or unlinks the open thread', async () => {
    const linked = linking({ threadId: 't2' });
    linked.choose('unlink-from-thread');
    await chatExternalLinkMenu(linked.client, linked.native, files, PR7);
    expect(linked.menus).toEqual([['Unlink from thread', 'Open in integrated browser', 'Open in system browser', 'Copy Link']]);
    expect(linked.dispatched).toEqual([{ type: 'thread.pull-request.unlink', commandId: 'command-1', threadId: 't2', host: 'github.com', repository: 'lane/sandbox', number: 7 }]);
    const other = linking({ threadId: 't1' });
    other.choose('link-to-thread');
    await chatExternalLinkMenu(other.client, other.native, files, PR7);
    expect(other.menus[0]![0]).toBe('Link to thread');
    expect(other.dispatched).toEqual([{ type: 'thread.pull-request.link', commandId: 'command-1', threadId: 't1', host: 'github.com', repository: 'lane/sandbox', number: 7, url: PR7, source: 'manual' }]);
    // Without the data module's storage (no dispatch path) the menu has no thread action.
    const bare = linking({ threadId: 't2' });
    await chatExternalLinkMenu(bare.client, bare.native, undefined, PR7);
    expect(bare.menus).toEqual([['Open in integrated browser', 'Open in system browser', 'Copy Link']]);
  });

  test("a new thread's draft offers the Browser, whose pick gives the draft its thread id first", async () => {
    const draft = linking({ threadId: '' });
    draft.choose('open-in-preview');
    await chatExternalLinkMenu(draft.client, draft.native, files, 'https://example.com/docs');
    expect(draft.menus).toEqual([['Open in integrated browser', 'Open in system browser', 'Copy Link']]);
    expect(activeRef(draft.client)).toEqual({ environmentId: 'env', threadId: 'command-1' });
    // No project (nothing open) has no thread to open beside.
    const none = linking({ threadId: '', projectId: '' });
    await chatExternalLinkMenu(none.client, none.native, files, 'https://example.com/docs');
    expect(none.menus).toEqual([['Open in system browser', 'Copy Link']]);
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
