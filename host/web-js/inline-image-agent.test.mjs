// The JS target's agent tree for an image among a paragraph's words (LLP 1011.002), in a browser. Its own file (not
// document-agent.test.mjs, which the async lane leaves out: scripts/async.mjs LEFT_OUT) so the lane runs it.
import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';
// @ref LLP 1011.002 — an image in a paragraph's text, between its words, is an `Image` in the agent's tree, as the
// runner's tree types it (the native and wasm hosts; conformance/inline-image.contract), not one of the paragraph's
// text runs.
test('an image in a paragraph\'s text is an Image in the agent tree', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/inline-image-agent-test-'));
  let browser, server;
  try {
    const png = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAACCAYAAAB/qH1jAAAAEklEQVR4nGN44ODwHxkzoAsAAEE1EvnzbXFzAAAAAElFTkSuQmCC';
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Inline Image Agent Test', app: { id: 'test.inline-image', name: 'Inline Image Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component InlineImageAgent
  view
    column testId="root"
      text testId="paragraph"
        text "A "
        image "${png}" width=16 height=8 aria-label="a bar" testId="inline"
        text " b"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'inline-image-agent-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname, file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href });
    const nodes = (await browser.tree()).nodes ?? await browser.tree();
    const by = id => nodes.find(n => n.props?.testId === id);
    expect({ type: by('inline')?.type, label: by('inline')?.props.accessibilityLabel, source: by('inline')?.props.imageSource }).toEqual({ type: 'Image', label: 'a bar', source: png });
    // No text of its own, as the runner's tree has none (conform reads a missing text and "" alike; this does not).
    expect(by('inline')?.props.text).toBeUndefined();
    // The paragraph and its words stay text.
    expect(by('paragraph')?.type).toBe('Text');
    expect(nodes.filter(n => n.type === 'Text' && /^(A | b)$/.test(n.props?.text ?? '')).length).toBe(2);
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 60000);
