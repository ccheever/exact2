// The reply's Markdown media and code, read from the clone's sources.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { messageChips } from './r4-timeline-chips';
import { attachmentUrls, imagePreviewAction, imagePreviewView } from './timeline-attachments';
import { markdownDocument, markdownGallery } from './media-views';
import { LINE_SCAN_LIMIT, scanWork, tokenizeLine } from './r12-render-textmate';
import { highlightSlice, resetHighlightSlicing, shikiTokens, startHighlightTurn } from './r12-render-highlight';
import { highlight } from './timeline-highlight';

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

const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
/** A component's text, from its line to the next top-level declaration. */
function component(file: string, name: string): string {
  const text = source(file), start = text.indexOf(`component ${name}\n`);
  if (start < 0) throw new Error(`no component ${name} in ${file}`);
  const rest = text.slice(start + 1), end = rest.search(/\n(component|shape|fn|keyframes|use) /);
  return text.slice(start, end < 0 ? undefined : start + 1 + end);
}

const native = { available: true } as unknown as Native;
const TEXT = 'Here is the shot:\n\n![Shot](docs/shot.png)\n\nA logo in a line: ![Logo](assets/logo.png) and a web badge: ![Badge](http://127.0.0.1:16483/badge.png).\n\n![Clip](docs/clip.mp4)\n\n| a | b |\n| - | - |\n| ![Cell](docs/cell.png) | 1 |\n';
function chatClient() {
  const signed: unknown[] = [];
  const client = {
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
  test('Files and an attachment name their own documents', () => {
    expect(source('r4-surfaces-files.ts')).toContain("messageChips({ text }, folder, [], `files:${absolute}`, scope)");
    expect(source('r5-panels-attach.ts')).toContain("messageChips({ text }, '', [], `attachment:${meta.id}`, { thread: false })");
  });
  test('the settled image is the button; the video, the loading slot and a failure keep MediaActions', () => {
    const body = component('media-markdown.contract', 'MarkdownMediaBody');
    expect(body).toContain('MediaPreviewActions(source=source, tip=tip, label=`Preview ${alt.trim() != "" ? alt.trim() : "image"}`, open=open, local=local, testId="markdown-image-actions", nodeId=nodeId)');
    expect(body).toContain('if kind == "video" or (brokenUrl != "" and brokenUrl == url)\n      MediaActions(source=source, tip=tip, local=local, elementId="", testId=(kind == "video" ? "markdown-video-actions" : "markdown-image-actions"))');
    expect(component('media-markdown.contract', 'MarkdownMedia')).toContain('open=local("image-md-open", target, `${owner}\\n${nodeId}\\n${galleryIds.map((id) => `media-${owner}-${id}`).indexOf(nodeId)}`), nodeId=nodeId)');
    const markdown = source('markdown.contract');
    expect(markdown).toContain('standalone=true, link="", linkLabel="", favicon=false, owner=chip.owner, nodeId=`media-${chip.owner}-${block.id}`)');
    expect(markdown).toContain('standalone=(run.icon == "standalone"), link=run.link, linkLabel=(run.label != "" ? run.label : run.text), favicon=(webLink(run.link) and linkLeads(runs, run.id, run.link)), owner=chip.owner, nodeId=`media-${chip.owner}-${blockId}-${run.id}`)');
    const button = component('media-actions.contract', 'MediaPreviewActions');
    expect(button).toMatch(/action pointerMenu\n\s+hover\(false\)\n\s+focus\(nodeId\)\n\s+local\("media-menu", source, "pointer"\)/);
    // Its tooltip is the hover layer's "code" tip at the button's frame, after Base UI's 600 ms (hover-layer.contract).
    expect(button).toContain('hoverTipAt(hoverTipAtFrame(nodeId, "code", tip, [], "top", "center", frame(nodeId),');
    expect(source('hover-layer.contract')).toContain('kind == "tooltip" or kind == "code" ? 600 : 0');
    expect(source('app-window.contract')).toContain('else if hoverTip.kind == "code"\n              HoverCode(tip=hoverTip)');
    expect(component('hover-layer.contract', 'HoverCode')).toContain('font-family="ui-monospace" font-size="0.6875rem"');
    expect(component('timeline-attachments.contract', 'ImagePreviewDialog')).toMatch(/action closePreview\n\s+local\("image-close", "", ""\)/);
    expect(component('timeline-attachments.contract', 'ImagePreviewDialog')).not.toContain('focus(returnFocus)');
    expect(source('app-overlays.contract')).toContain('DialogFinalFocus(open=(data.imagePreviewId != ""), owner="image-preview")');
    for (const part of ['button id=nodeId press=press', 'contextmenu=pointerMenu', 'key=menuKey', 'tabindex=0', 'aria-label=label', 'cursor="zoom-in"', 'hatch="t3-media" data-media-kind="actions"'])
      expect(button).toContain(part);
    expect(button).toContain('k == "ContextMenu" or (e.shiftKey and not e.metaKey and not e.ctrlKey and not e.altKey and k == "F10")');
  });
});

describe('a media node\'s tooltip is drawn (MediaActions\' TooltipPopup, variant code), with the full path', () => {
  test('MediaActions shows MediaTip on hover, no AppKit help tag, and opening its menu closes the tip', () => {
    const actions = component('media-actions.contract', 'MediaActions');
    expect(actions).toContain('title="" hover=hover contextmenu=pointerMenu key=menuKey aria-label=tip hatch="t3-media" data-media-kind="actions"');
    expect(actions).not.toContain('title=tip');
    expect(actions).toContain('MediaTip(label=tip, shown=over)');
    expect(actions).toMatch(/action pointerMenu\n\s+over = false\n\s+local\("media-menu", source, "pointer"\)/);
    const tip = component('media-actions.contract', 'MediaTip');
    for (const part of ['bottom="100%"', 'max-width="30rem"', 'font-family="ui-monospace" font-size="0.6875rem"', 'overflow-wrap="anywhere"', 'transition=(shown ? "opacity 150ms ease 600ms" : "opacity 0ms")', 'pointer-events="none"'])
      expect(tip).toContain(part);
  });
  test('the expanded dialog\'s media carries the same tooltip', () => {
    expect(source('app-overlays.contract')).toContain('tip=data.imagePreviewTip');
    expect(component('timeline-attachments.contract', 'ImagePreviewDialog')).toContain('MediaActions(source=source, tip=(tip != "" ? tip : name)');
  });
});

describe('the code block, table and welcome copy buttons morph back from the check', () => {
  test('no keyframes; each button draws MorphCopyCheck on while its copy is the last and not yet released', () => {
    const markdown = source('markdown.contract'), welcome = source('pages-welcome.contract');
    expect(markdown).not.toMatch(/mdCheck|mdCopy/);
    expect(welcome).not.toMatch(/wcShown|wcHidden/);
    expect(component('markdown.contract', 'CopyCodeButton')).toContain('MorphCopyCheck(on=(copied > 0 and copied != codeCopyDone), size=12');
    expect(component('markdown.contract', 'TableCopyButton')).toContain('MorphCopyCheck(on=(copied > 0 and copied != codeCopyDone), size=12');
    expect(component('pages-welcome.contract', 'WelcomeCommand')).toContain('MorphCopyCheck(on=(copied > 0 and copied != welcomeCopyDone), size=14');
    // MorphCopyCheck moves `d` on the spring both ways (shell-morph.contract).
    expect(component('shell-morph.contract', 'MorphCopyCheck')).toContain('transition=(still ? "none" : "d -exact-spring(420, 30, 1)")');
  });
  test('the root releases each copy after the reference\'s time, keyed on its nonce, and provides it', () => {
    const app = source('app.contract');
    expect(app).toMatch(/task codeCopyHold when data\.codeCopyNonce > 0 key=data\.codeCopyNonce\n\s+after\(1200, codeCopyRelease\)/);
    expect(app).toMatch(/task welcomeCopyHold when welcome\.commandCopied > 0 key=welcome\.commandCopied\n\s+after\(1500, welcomeCopyRelease\)/);
    expect(app).toMatch(/provide\n\s+codeCopyDone\n\s+welcomeCopyDone/);
    expect(source('timeline-presentation.ts')).toContain('codeCopyNonce: view.codeCopy.threadId === client.threadId ? view.codeCopy.nonce : 0');
  });
});
