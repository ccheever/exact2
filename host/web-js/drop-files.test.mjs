import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// Files dragged or pasted in (LLP 1069.010, "Files dropped or pasted"): any
// type with no `file_handlers`, a folder as a folder's handle, a drag over a
// textarea landing on the column's `drop`, `dragenter`/`dragleave` around
// it, and a paste's files. The drags are Chrome's own (`Input.dispatchDragEvent`).
test('a column hears a file drag over its textarea, its drop and a paste of files', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/drop-files-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Drop Files Test', app: { id: 'test.drop-files', name: 'Drop Files Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component DropFiles
  state over = false
  state enters = 0
  state got = ""
  state pasted = ""
  action entered(e: DragEvent)
    over = true
    enters = enters + 1
  action left
    over = false
  action dropped(e: DragEvent)
    got = e.files.join(" ")
  action pastedIn(e: ClipboardEvent)
    if e.files.length > 0
      pasted = e.files.join(" ")
      preventDefault()
  view
    column testId="column" width=400 height=400 padding=16 dragenter=entered dragleave=left drop=dropped
      textarea testId="composer" paste=pastedIn
      text "label" testId="label"
`);
    const files = resolve(dir, 'files');
    mkdirSync(resolve(files, 'folder'), { recursive: true });
    writeFileSync(resolve(files, 'notes.xyz'), 'not a declared type');
    writeFileSync(resolve(files, 'folder', 'a.md'), 'a');
    writeFileSync(resolve(files, 'shot.png'), Buffer.from('89504e470d0a1a0a', 'hex'));
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'drop-files-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      if (path.startsWith('/shared/')) return new Response(Bun.file(resolve(root, 'host/web', path.slice('/shared/'.length))));
      const file = resolve(dist, path === '/' ? 'index.html' : path.slice(1));
      return existsSync(file) ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
    const slots = async () => (await browser.state()).slots;
    // Over the column, then into its textarea: one enter, no leave between.
    await browser.tap('column', { dragenter: [resolve(files, 'notes.xyz')] });
    await browser.tap('composer', { dragenter: [resolve(files, 'notes.xyz')] });
    expect(await slots()).toMatchObject({ over: true, enters: 1 });
    await browser.tap('column', { dragleave: true });
    expect((await slots()).over).toBe(false);
    // Dropped on the textarea: the column's, a file of no declared type and a folder; no path text, and the drag ends.
    await browser.tap('composer', { drop: [resolve(files, 'notes.xyz'), resolve(files, 'folder')] });
    await browser.clock('settle');
    expect(await slots()).toMatchObject({ over: false, enters: 2, got: 'doc:/1/notes.xyz doc:/2/folder' });
    expect(await browser.carrier.evaluate('document.querySelector("[data-testid=composer]").value')).toBe('');
    // A paste of an image's data: one file, the field's own paste prevented; a paste of text alone is the field's.
    await browser.type('composer', { clipboard: 'paste', text: '', image: resolve(files, 'shot.png') });
    expect((await slots()).pasted).toBe('doc:/3/image.png');
    await browser.type('composer', { clipboard: 'paste', text: 'hello' });
    expect(await browser.carrier.evaluate('document.querySelector("[data-testid=composer]").value')).toBe('hello');
    expect((await slots()).pasted).toBe('doc:/3/image.png');
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 60000);
