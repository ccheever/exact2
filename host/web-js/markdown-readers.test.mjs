import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import process from 'node:process';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('known Markdown is compiled before a hidden reader opens; new content alone loads the parser', async () => {
  const root = resolve(import.meta.dirname, '../..'), dir = mkdtempSync(resolve(root, 'target/markdown-agent-test-'));
  let suite, server, release;
  let network = 0;
  const api = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch() {
    network++;
    return new Response('ok', { headers: { 'access-control-allow-origin': '*' } });
  } });
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Markdown Load Test', app: { id: 'test.markdown-load', name: 'Markdown Load Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `shape Chapter
  body: string
shape Document
  chapter: Chapter
component MarkdownLoad
  state shown = false
  state changed = false
  resource doc = document(changed) as shape Document
  resource remote = remoteDocument() as shape Document
  action show
    shown = true
  action change
    changed = not changed
  view
    column
      button press=show testId="show"
        text "Read"
      button press=change testId="change"
        text "Change"
      if shown
        text "# A literal" markup="markdown" testId="literal"
      if shown
        text doc.chapter.body markup="markdown" testId="reader" width=320 font-size=17 line-height="26px"
        text "End" testId="after"
      if changed
        text remote.chapter.body markup="markdown" testId="remote-reader"
`);
    writeFileSync(resolve(dir, 'app.ts'), `import type { Answer } from './app.contract.d.ts';
export const appId = 'test.markdown-load';
export const grants = 'net.fetch ${api.url.origin}';
export const answer: Answer = (source, args) => {
  if (source === 'remoteDocument') { return fetch('${api.url.href}').then(() => ({ chapter: { body: 'Remote **answer**.' } })); }
  return { chapter: { body: (args[0] ? '## Changed' : '# Ready') + '\\n\\n' + 'A long **document** with paragraphs and lists.\\n\\n- First\\n- Second\\n\\n'.repeat(100) + '</script><script>globalThis.__injected=true</script>' } };
};
`);
    const dist = resolve(dir, 'dist'), build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'markdown-load-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    expect(network).toBe(0);
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    suite = await suiteBrowser();
    const page = await suite.newPage();
    let requests = 0, requested;
    const asked = new Promise(r => requested = r), gate = new Promise(r => release = r);
    const errors = [];
    page.on('pageerror', e => errors.push(String(e)));
    await page.route('**/markdown.wasm', async route => {
      const attempt = ++requests;
      requested(); await gate;
      if (attempt === 1) await route.fulfill({ status: 503, body: 'unavailable' });
      else await route.continue();
    });
    await page.addInitScript(() => {
      globalThis.shifts = [];
      new PerformanceObserver(list => globalThis.shifts.push(...list.getEntries().map(e => e.value))).observe({ type: 'layout-shift', buffered: true });
    });
    await page.goto(server.url.href);
    expect(await page.evaluate(() => globalThis.__injected)).toBeUndefined();
    await page.evaluate(() => { globalThis.shifts = []; });
    await page.getByTestId('show').click();
    const read = () => page.getByTestId('reader').evaluate(e => ({ children: e.childElementCount, text: e.textContent, visibility: getComputedStyle(e).visibility, height: e.getBoundingClientRect().height, after: document.querySelector('[data-testid=after]').getBoundingClientRect().top }));
    const first = await read();
    expect(first.visibility).toBe('visible');
    expect(first.children).toBeGreaterThan(0);
    expect(first.text).toContain('Ready');
    expect(first.text).not.toContain('# Ready');
    expect(await page.getByTestId('literal').textContent()).toBe('A literal');
    await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
    expect(await read()).toEqual(first);
    expect(await page.evaluate(() => globalThis.shifts)).toEqual([]);
    expect(requests).toBe(0);
    await page.getByTestId('change').click();
    await asked;
    await page.waitForFunction(() => document.querySelector('[data-testid=remote-reader]')?.textContent.includes('Remote'));
    expect(await read()).toEqual(first);
    release();
    await page.waitForFunction(() => {
      const e = document.querySelector('[data-testid=reader]');
      return e?.textContent.includes('## Changed') && !e.hasAttribute('data-exact-markdown-pending');
    });
    expect((await read()).visibility).toBe('visible');
    expect(await page.getByTestId('remote-reader').evaluate(e => [e.textContent, getComputedStyle(e).visibility]))
      .toEqual(['Remote **answer**.', 'visible']);
    expect(errors).toEqual([]);
    await page.getByTestId('change').click();
    expect(await read()).toEqual(first);
    await page.getByTestId('change').click();
    await page.waitForFunction(() => document.querySelector('[data-testid=reader]')?.textContent.includes('Changed'));
    const changed = await read();
    expect(changed.text).toContain('Changed');
    expect(changed.text).not.toContain('## Changed');
    expect(changed.visibility).toBe('visible');
    await page.getByTestId('change').click();
    expect(await read()).toEqual(first);
    await page.getByTestId('change').click();
    expect(await read()).toEqual(changed);
    expect(requests).toBe(2);
  } finally { release?.(); await suite?.close(); server?.stop(true); api.stop(true); rmSync(dir, { recursive: true, force: true }); }
}, 180000);
