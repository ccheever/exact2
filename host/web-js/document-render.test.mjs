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
    const html = path => readFileSync(resolve(dir, 'dist', path, 'index.html'), 'utf8');
    check(path => {
      const page = html(path);
      return [/<title>([^<]*)<\/title>/.exec(page)?.[1], /<meta name="description" content="([^"]*)">/.exec(page)?.[1]];
    }, html);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('JavaScript-rendered viewport uses default media facts without a reader’s device', async () => {
  await renderedPages(`routes nav
  home "/" render=build

shape Viewport
  width: number
  height: number
  prefersReducedMotion: bool
  prefersReducedTransparency: bool
  prefersContrast: string
  prefersColorScheme: string
  pointer: string
  hover: string
  colorGamut: string
  dynamicRange: string
  displayMode: string

component MediaFacts
  resource viewport = exactViewport() as shape Viewport
  view
    main
      head title=\`\${viewport.width}x\${viewport.height}/\${viewport.prefersReducedMotion}/\${viewport.prefersReducedTransparency}/\${viewport.prefersContrast}/\${viewport.prefersColorScheme}/\${viewport.pointer}/\${viewport.hover}/\${viewport.colorGamut}/\${viewport.dynamicRange}/\${viewport.displayMode}\`
      text "Viewport facts"
`, page => {
    expect(page('')[0]).toBe('390x844/false/false/no-preference/light/fine/hover/srgb/standard/standalone');
  });
}, 30000);

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

test('JavaScript-rendered navigation keeps split facts without browser geometry', async () => {
  await renderedPages(`routes nav
  home "/" render=build activate=interaction
    detail "/detail" render=build activate=interaction

component SplitDocument
  action open
    nav = push(nav, path("detail"))
  action back
    nav = back(nav)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" position="relative"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0
          if e.name == "home"
            list
              button "Open" press=open
          if e.name == "detail"
            button "Back" id="back" press=back
`, (_, html) => {
    for (const path of ['', 'detail']) {
      const page = html(path);
      expect(page).toContain('data-exact-list');
      expect(page).toContain('data-exact-router-press="true"');
      expect(page).not.toMatch(/\sdata-exact-split="/);
    }
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
