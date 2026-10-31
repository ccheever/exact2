// The reply's Markdown media and code, read from the clone's sources.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { messageChips } from '../../src/timeline/message-chips';
import { attachmentUrls, imagePreviewAction, imagePreviewView } from '../../src/timeline/timeline-attachments';
import { markdownDocument, markdownGallery } from '../../src/media/media-views';

const native = { available: true } as unknown as Native;
const TEXT = 'Here is the shot:\n\n![Shot](docs/shot.png)\n\nA logo in a line: ![Logo](assets/logo.png) and a web badge: ![Badge](http://127.0.0.1:16483/badge.png).\n\n![Clip](docs/clip.mp4)\n\n| a | b |\n| - | - |\n| ![Cell](docs/cell.png) | 1 |\n';
function chatClient() {
  const signed: unknown[] = [];
  const client = {
    scopes: ['filesystem:read'],
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, connection: 'connected', origin: 'http://127.0.0.1:16482', generation: 1, draftKey: 'env:t1',
    local: { composerControls: {} },
    projection: { visibleTurnItems: [{ item: { type: 'assistant_message', text: TEXT } }], messages: [], runs: [], thread: { projectId: 'p1' } },
    shell: { projects: [{ id: 'p1', title: 'work', workspaceRoot: '/repo' }] },
    rpc: async (_native: unknown, method: string, payload: { resource: { path: string } }) => { signed.push([method, payload.resource]); return { relativeUrl: `/api/assets/${encodeURIComponent(payload.resource.path)}`, expiresAt: 0 }; },
    restAccess: () => ({ call: async () => ({}) }),
  } as unknown as T3Client;
  return { client, signed };
}

describe('a chat Markdown image is a "Preview <alt>" button that opens the expanded image dialog over its message\'s images', () => {
  test('the press opens the gallery of the document\'s images in order (a video left out, a table cell\'s kept), stepping round both ways', async () => {
    const { client } = chatClient();
    const chips = messageChips({ text: TEXT }, '/repo', [], '["t1","m1"]');
    expect(markdownDocument('["t1","m1"]')?.text).toBe(TEXT);
    expect(markdownGallery(markdownDocument('["t1","m1"]')!).map(item => item.name)).toEqual(['Shot', 'Logo', 'Badge', 'Cell']);
    // An image inside a link's text is the link's (expandableMarkdownImageProps returns early inside an `<a>`).
    expect(markdownGallery({ text: 'See [![Linked](l.png)](https://a.com) and ![Own](o.png)', root: '/repo', scope: {} }).map(item => item.name)).toEqual(['Own']);
    const shot = chips.find(chip => chip.kind === 'media' && chip.label === 'Shot')!;
    imagePreviewAction(client, 'image-md-open', shot.target, `${shot.owner}\nmedia-${shot.owner}-b1\n0`);
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewId: 'media:/repo/docs/shot.png', imagePreviewName: 'Shot', imagePreviewPosition: '(1/4)',
      imagePreviewPrevious: true, imagePreviewNext: true, imagePreviewVideo: false, imagePreviewTip: '/repo/docs/shot.png', imagePreviewReturn: 'media-["t1","m1"]-b1' });
    expect(JSON.parse(imagePreviewView(client).imagePreviewSource)).toMatchObject({ kind: 'image', name: 'Shot', reference: { kind: 'file', path: '/repo/docs/shot.png', relativePath: 'docs/shot.png' } });
    imagePreviewAction(client, 'image-step', '', 'next');
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewName: 'Logo', imagePreviewPosition: '(2/4)' });
    imagePreviewAction(client, 'image-step', '', 'next');
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewId: 'http://127.0.0.1:16483/badge.png', imagePreviewName: 'Badge', imagePreviewPosition: '(3/4)', imagePreviewTip: 'http://127.0.0.1:16483/badge.png' });
    imagePreviewAction(client, 'image-step', '', 'next');
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewId: 'media:/repo/docs/cell.png', imagePreviewName: 'Cell', imagePreviewPosition: '(4/4)' });
    imagePreviewAction(client, 'image-step', '', 'next');
    expect(imagePreviewView(client).imagePreviewName).toBe('Shot');
    imagePreviewAction(client, 'image-step', '', 'previous');
    expect(imagePreviewView(client).imagePreviewName).toBe('Cell');
    // The dialog's URL list holds the web image as written and the host paths signed through the thread.
    const urls = (await attachmentUrls(client, native, 0)).items.map(item => [item.id, item.url]);
    expect(urls).toContainEqual(['http://127.0.0.1:16483/badge.png', 'http://127.0.0.1:16483/badge.png']);
    expect(urls).toContainEqual(['media:/repo/docs/shot.png', 'http://127.0.0.1:16482/api/assets/%2Frepo%2Fdocs%2Fshot.png']);
    expect(new Set(urls.map(([id]) => id)).size).toBe(urls.length);
    imagePreviewAction(client, 'image-close', '', '');
    expect(imagePreviewView(client).imagePreviewId).toBe('');
    // Leaving the thread drops it; an image no document holds opens nothing.
    imagePreviewAction(client, 'image-md-open', shot.target, `${shot.owner}\nmedia-${shot.owner}-b1\n0`);
    (client as { threadId: string }).threadId = 'other';
    expect(imagePreviewView(client).imagePreviewId).toBe('');
    expect(() => imagePreviewAction(client, 'image-md-open', 'media:/repo/none.png', shot.owner)).toThrow('That image is no longer available.');
  });
  test('a repeated image opens at the clicked occurrence, preserves its name and focus, and wraps from there', () => {
    const { client } = chatClient();
    const owner = 'files:/repo/docs/repeated.md';
    const text = '![Shot](shot.png)\n\nInline ![Logo](logo.png), then ![Shot again](shot.png).';
    const chips = messageChips({ text }, '/repo/docs', [], owner);
    const shot = chips.find(chip => chip.kind === 'media' && chip.label === 'Shot')!;
    const opener = `media-${owner}-b1-r5`;
    imagePreviewAction(client, 'image-md-open', shot.target, `${owner}\n${opener}\n2`);
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewName: 'Shot again', imagePreviewPosition: '(3/3)', imagePreviewReturn: opener });
    imagePreviewAction(client, 'image-step', '', 'previous');
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewName: 'Logo', imagePreviewPosition: '(2/3)' });
    imagePreviewAction(client, 'image-step', '', 'next');
    imagePreviewAction(client, 'image-step', '', 'next');
    expect(imagePreviewView(client)).toMatchObject({ imagePreviewName: 'Shot', imagePreviewPosition: '(1/3)' });
    for (const occurrence of ['1', '3', '-1', 'NaN', '']) {
      expect(() => imagePreviewAction(client, 'image-md-open', shot.target, `${owner}\n${opener}\n${occurrence}`)).toThrow('That image is no longer available.');
    }
  });
});
