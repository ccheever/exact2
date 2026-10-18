// The reply's Markdown media and code, read from the clone's sources.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { messageChips } from '../../src/timeline/message-chips';
import { attachmentUrls, imagePreviewAction, imagePreviewView } from '../../src/timeline/timeline-attachments';
import { markdownDocument, markdownGallery } from '../../src/media/media-views';
import { LINE_SCAN_LIMIT, scanWork, tokenizeLine } from '../../src/highlight/textmate';
import { highlightSlice, resetHighlightSlicing, shikiTokens, startHighlightTurn } from '../../src/highlight/shiki-highlight';
import { highlight } from '../../src/highlight/timeline-highlight';

const UNIT = 'template <typename T> std::vector<T> twice(const T& x) { return {x, x}; } ';
const templateLine = (length: number) => UNIT.repeat(Math.ceil(length / UNIT.length)).slice(0, length);

describe('KD-code-cpp-perf: Shiki\'s 500 ms line limit, counted as regex work', () => {
  test('a 2,960-character template-dense cpp line finishes well inside the limit, every character in its scopes', () => {
    tokenizeLine('int x;', 'cpp', null);
    const before = scanWork();
    const result = tokenizeLine(templateLine(2960), 'cpp', null);
    // Before the memo kept its searches across a capture's retokenizing: 1.2e10 units (6.2 s in Hermes); now 4.0e8 (0.25 s).
    expect(scanWork() - before).toBeLessThan(LINE_SCAN_LIMIT * 0.6);
    expect(result.stopped).toBe(false);
    expect(result.pieces.every(piece => piece.scopes.length > 0)).toBe(true);
  });
  test('sliced, the line spreads over turns and paints as it does unsliced', () => {
    const text = `${templateLine(2960)}\nint main() { return 0; }`;
    resetHighlightSlicing();
    const whole = shikiTokens(text, 'cpp');
    resetHighlightSlicing();
    startHighlightTurn();
    let tokens = shikiTokens(text, 'cpp'), turns = 1;
    while (!tokens && turns < 500) { startHighlightTurn(); highlightSlice(); tokens = shikiTokens(text, 'cpp'); turns++; }
    resetHighlightSlicing();
    expect(turns).toBeGreaterThan(5);
    expect(turns).toBeLessThan(500);
    expect(tokens).toEqual(whole);
  });
  test('a 5,920-character line stops at the limit: what came before keeps its scopes, the rest is plain, the next line starts where it stopped', () => {
    const stopped = tokenizeLine(templateLine(5920), 'cpp', null);
    expect(stopped.stopped).toBe(true);
    const rest = stopped.pieces[stopped.pieces.length - 1]!;
    expect(rest.scopes).toEqual([]);
    expect(rest.text.length).toBeGreaterThan(1000);
    expect(rest.text.length).toBeLessThan(5920);
    expect(stopped.pieces.slice(0, -1).every(piece => piece.scopes.length > 0)).toBe(true);
    const next = tokenizeLine('int main() { return 0; }', 'cpp', stopped.state);
    expect(next.stopped).toBe(false);
    expect(next.pieces.some(piece => piece.scopes.some(scope => scope.startsWith('keyword') || scope.startsWith('storage')))).toBe(true);
    // The app path paints the rest plain (synColor class '').
    resetHighlightSlicing();
    const tokens = highlight(`${templateLine(5920)}\nint main() { return 0; }`, 'cpp');
    const firstLine = tokens.map(token => token.text).join('').indexOf('\n');
    let at = 0, plainTail = 0;
    for (const token of tokens) { if (at >= firstLine - rest.text.length && at < firstLine) plainTail += token.cls === '' ? token.text.length : 0; at += token.text.length; }
    expect(plainTail).toBeGreaterThanOrEqual(rest.text.length - UNIT.length);
  });
});

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
