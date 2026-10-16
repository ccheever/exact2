import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test('a second navigation root keeps its split list live while its detail supplies the head', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Agent Test', app: { id: 'test.heads', name: 'Head Agent Test' }, host: { web: {} } }));
    // The earlier covered copy must lose testId lookup to the visible split list's control.
    const source = readFileSync(new URL('./conformance/heads-split.contract', import.meta.url), 'utf8').replace('      main navigationKey=', `      main navigationKey="1002" navigationBack="unused" height=24
        column navigationKey="1001"
          button press=toggle testId="list-commit"
            text "Covered control"
        column navigationKey="1002"
          text "Another navigation root"
      main navigationKey=`);
    writeFileSync(resolve(dir, 'app.contract'), source);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'head-agent-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [1000, 900] });
    expect(await browser.carrier.evaluate('document.querySelectorAll("[navigationBack]").length')).toBe(2);
    await browser.tap('open-detail');
    await browser.tap('commit');
    let commits = 1;
    for (const width of [1000, 420, 1000]) {
      await browser.resize(width, 900);
      await browser.clock('settle');
      const wide = width >= 900, copies = (await browser.tree()).nodes.filter(n => n.props.testId === 'list-commit');
      expect(copies).toHaveLength(2);
      const [covered, list] = copies, observed = await browser.op({ op: 'check' });
      expect({ covered: !!covered.inactive, list: !!list.inactive, checked: observed.items.some(item => item.id === list.id && item.control === 'button') })
        .toEqual({ covered: true, list: !wide, checked: wide });
      if (wide) {
        expect((await browser.tree('list-commit', { shallow: true })).nodes[0].id).toBe(list.id);
        await browser.tap('list-commit');
        commits++;
      }
      const state = await browser.state();
      expect(state.slots).toMatchObject({ commits, named: true });
      expect(state.head).toMatchObject({ title: 'Ada', description: 'Directory description', canonical: '/directory' });
      expect(await browser.carrier.evaluate('document.title')).toBe('Ada');
    }
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

test('a JavaScript-rendered direct detail selects its retained href at boot', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/split-document-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Split Document Agent Test', app: { id: 'test.split-document', name: 'Split Document Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  home "/" render=build
    alpha "/a" render=build

component SplitDocumentAgent
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" position="relative" testId="navigation"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0 testId=\`route-\${e.name}\`
          if e.name == "home"
            list
              link href="/a" testId="row-alpha"
                text "Alpha"
          if e.name == "alpha"
            text "Alpha detail"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'split-document-agent-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path === '/a' ? 'a/index.html' : path.slice(1))));
    } });
    browser = await open({ host: 'web', url: new URL('a', server.url).href, size: [1280, 844] });
    const state = await browser.state();
    expect(state.navigation).toMatchObject({ url: '/a', route: '1', stack: ['0', '1'] });
    expect(await browser.carrier.evaluate(`(() => ({
      list: document.querySelector('[data-testid="route-home"]').getAttribute('data-exact-split'),
      detail: document.querySelector('[data-testid="route-alpha"]').getAttribute('data-exact-split'),
      selected: document.querySelector('[data-testid="row-alpha"]').hasAttribute('data-exact-split-row'),
    }))()`)).toEqual({ list: 'list', detail: 'detail', selected: true });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
