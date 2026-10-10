// shell-context-menu: a reply's web link menu (externalLinkContextMenu.test.ts's cases, ported) and its hookup: the
// Markdown link nodes carry `contextmenu`, the root action names `chatlocal:link-menu`, and that op reaches the native
// `contextMenu` op with the reference's items.
import { describe, expect, mock, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { T3Client } from './client';
import { resetRemoteEditorsForTests } from './remote-open';
import { externalLinkContextMenuItems, resolveExternalWebLinkHost, resolveExternalWebLinkHref, showExternalLinkContextMenu,
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
    const links = markdown.split('\n').filter(line => line.includes('press=linkOpen("link", run.href)'));
    expect(links.length).toBe(4);
    // The link-start run's text sits in a row with its globe, which carries the menu for both (a right-click bubbles to it).
    expect(links.filter(line => line.includes('contextmenu=linkMenu(run.href)')).length).toBe(3);
    expect(markdown).toContain('row contextmenu=linkMenu(run.href) flex-shrink=0 align-items="flex-start"');
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
