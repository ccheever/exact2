import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webCompiler } from './module.mjs';

async function renderedPages(source, check) {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-render-test-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Render Test', app: { id: 'test.heads', name: 'Head Render Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), source);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'head-render-test', '--out', resolve(dir, 'dist'), '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    check(path => {
      const html = readFileSync(resolve(dir, 'dist', path, 'index.html'), 'utf8');
      return [/<title>([^<]*)<\/title>/.exec(html)?.[1], /<meta name="description" content="([^"]*)">/.exec(html)?.[1]];
    });
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('JavaScript-rendered pages exclude a covered route’s deeper head', async () => {
  await renderedPages(`routes nav
  tab home "/" render=build
    detail "/detail" render=build
    plain "/plain" render=build

component Heads
  view
    column
      head title="Root" description="Root description"
      main navigationKey=\`\${top(nav).id}\` navigationBack="back"
        for e in stack(nav) key=e.id
          column navigationKey=\`\${e.id}\`
            if e.name == "home"
              column
                head title="Home" description="Home description"
            if e.name == "detail"
              head title="Detail" description="Detail description"
            text e.name
`, page => {
    expect(page('')).toEqual(['Home', 'Home description']);
    expect(page('detail')).toEqual(['Detail', 'Detail description']);
    expect(page('plain')).toEqual(['Root', 'Root description']);
  });
}, 30000);

test('JavaScript-rendered pages exclude the other retained tabs’ heads', async () => {
  const source = readFileSync(new URL('./conformance/heads-tabs.contract', import.meta.url), 'utf8')
    .replace('tab home "/"', 'tab home "/" render=build')
    .replace('tab saved "/saved"', 'tab saved "/saved" render=build');
  await renderedPages(source, page => {
    expect(page('')).toEqual(['Home', 'Base description']);
    expect(page('saved')).toEqual(['Saved', 'Saved description']);
  });
}, 30000);
