// Contract-independent HTML/MCP metadata and guest requests; all inputs are synthetic.
import { expect, test } from 'bun:test';
import { compactDynamicToolOutput, htmlRenderFromToolItem, mcpAppFromToolItem } from '../../src/timeline/inline-tool-output';
import { htmlRenderFrameHeight, htmlRenderOutlineBoxes, htmlRenderTheme, htmlRenderThemeFragment, readHtmlRenderContentHeight, readHtmlRenderImageRequest, readHtmlRenderKeyDown, readHtmlRenderLinkRequest, readHtmlRenderReference } from '../../src/timeline/html-render';
import { readMcpAppReference, mcpAppFileName, clampMcpAppHeight } from '../../src/timeline/mcp-app';
import { STANDARD } from '../../src/appearance/settings-appearance-editor';

const html = { attachmentId: 'page-1', title: ' Page ', height: 500, heights: [[320, 900], [728, 500], [1000, 400]], outline: 't0,0,182,10,2' };
const app = { attachmentId: 'app-1', server: 'weather', tool: 'forecast', resourceUri: 'ui://forecast/index.html' };

test('provider envelopes retain render metadata and reject failed or foreign tool results', () => {
  for (const output of [{ htmlRender: html }, { structuredContent: { htmlRender: html } }, { content: [{ type: 'text', text: JSON.stringify({ htmlRender: html }) }] }]) {
    expect(htmlRenderFromToolItem({ toolName: 'mcp__t3-code__html_render', output })?.title).toBe('Page');
  }
  expect(htmlRenderFromToolItem({ toolName: 'other.html_render', output: { htmlRender: html } })).toBeUndefined();
  expect(htmlRenderFromToolItem({ toolName: 'html_render', output: { isError: true, htmlRender: html } })).toBeUndefined();
  expect(mcpAppFromToolItem({ toolName: 'weather.forecast', output: { t3McpApp: app } })).toEqual(app);
  expect(mcpAppFromToolItem({ toolName: 'other.forecast', output: { t3McpApp: app } })).toBeUndefined();
});

test('untrusted result envelopes have byte, nesting and block budgets', () => {
  expect(compactDynamicToolOutput(' '.repeat(16_385))).toBeUndefined();
  expect(compactDynamicToolOutput({ content: Array(33).fill({ text: JSON.stringify({ htmlRender: html }) }) })).toBeUndefined();
  const nested = { content: { content: { content: { content: { content: { htmlRender: html } } } } } };
  expect(compactDynamicToolOutput(nested)).toBeUndefined();
  expect(compactDynamicToolOutput({ error: 'refused', htmlRender: html })).toEqual({ isError: true, htmlRender: { ...html, title: 'Page' } });
});

test('measured frame and skeleton fit the nearby layout widths and live page height', () => {
  const ref = readHtmlRenderReference(html)!;
  expect(htmlRenderFrameHeight(ref, 520)).toBe(900);
  expect(htmlRenderFrameHeight(ref, 860)).toBe(500);
  expect(htmlRenderFrameHeight(ref, 860, 750)).toBe(750);
  expect(htmlRenderFrameHeight({ ...ref, height: 300 }, 520, 800)).toBe(300);
  expect(htmlRenderOutlineBoxes(ref, 364)[0]).toMatchObject({ kind: 'text', width: 364, lines: 2 });
  expect(readHtmlRenderReference({ ...html, heights: [[-1, 20]] })?.outline).toBeUndefined();
  expect(readHtmlRenderReference({ ...html, height: Infinity })).toBeUndefined();
});

test('MCP references drop invalid origin policy, constrain permissions and keep resource identity', () => {
  const ref = readMcpAppReference({ ...app, csp: { connectDomains: ['https://api.example.test', "https://bad.test 'unsafe-eval'", 'https://api.test/path'] }, permissions: { camera: {}, unknown: {}, microphone: false } });
  expect(ref?.csp).toEqual({ connectDomains: ['https://api.example.test'] });
  expect(ref?.permissions).toEqual({ camera: {} });
  expect(readMcpAppReference({ ...app, resourceUri: 'https://example.test/app' })).toBeUndefined();
  expect(mcpAppFileName({ ...app, tool: '../hello world' })).toBe('..-hello-world.html');
  expect(clampMcpAppHeight(NaN)).toBe(320);
  expect(clampMcpAppHeight(1e6)).toBe(2000);
});

test('guest notifications are decoded by method and URL scheme before any action', () => {
  const message = (method: string, params: unknown) => ({ jsonrpc: '2.0', id: 1, method, params });
  expect(readHtmlRenderLinkRequest(message('ui/open-link', { url: 'https://example.test', metaKey: true }))).toMatchObject({ url: 'https://example.test', metaKey: true, ctrlKey: false });
  expect(readHtmlRenderLinkRequest(message('ui/open-link', { url: 'javascript:alert(1)' }))).toBeUndefined();
  expect(readHtmlRenderImageRequest(message('t3/notifications/open-image', { src: 'data:image/png;base64,AA==', name: ' image ' }))).toEqual({ src: 'data:image/png;base64,AA==', name: 'image' });
  expect(readHtmlRenderImageRequest(message('t3/notifications/open-image', { src: 'file:///private/image' }))).toBeUndefined();
  expect(readHtmlRenderContentHeight(message('ui/notifications/size-changed', { height: 620 }))).toBe(620);
  expect(readHtmlRenderContentHeight(message('ui/notifications/size-changed', { height: -1 }))).toBeUndefined();
  expect(readHtmlRenderKeyDown(message('t3/notifications/keydown', { key: 'p', code: 'KeyP', metaKey: true }))).toMatchObject({ key: 'p', metaKey: true, repeat: false });
});

test('initial document theme includes active hex colors and decodes without URL mutation', () => {
  const theme = htmlRenderTheme({ ...STANDARD.dark, canvas: '#123456', accent: '#abcdef' }, 'dark');
  expect(theme.variables['--background']).toBe('#123456');
  expect(theme.variables['--accent']).toBe('#abcdef');
  const encoded = htmlRenderThemeFragment(theme);
  expect(JSON.parse(decodeURIComponent(encoded.slice('#t3-theme='.length)))).toEqual(theme);
});
