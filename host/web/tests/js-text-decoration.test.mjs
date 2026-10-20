// CSS Text Decoration 3 §2.1: a box's lines are drawn through its inline
// descendants' text in the box's colour, and are not inherited. The browser
// does that only when the JS target keeps nested text nested: the decorated
// box's element holds its descendants' elements, and they have no line of
// their own (native hosts propagate the lines themselves, so a sibling
// layout here would be the one host that disagrees). A real build of a
// fixture app outside the repository (`EXACT_APP_DIR`), then a real render.
import { test, expect, afterAll } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { renderer } from '../../web-js/render.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-decoration-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

test('a decorated text holds its nested text, which draws no line of its own', async () => {
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Lines', app: { id: 'com.example.lines', name: 'Lines' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), [
    'component App',
    '  view',
    '    text text-decoration="line-through" testId="struck"',
    '      text "a "',
    '      text "b" testId="b"',
    '      text',
    '        text "deep" testId="deep"',
    '',
  ].join('\n'));
  const dist = resolve(dir, 'dist');
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'lines', '--out', dist, '--render', 'js'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  expect(build.status, build.stderr).toBe(0);
  const { html } = await renderer(dist)('/');
  // The decorated element holds every run below it, and only it has a line.
  const struck = html.match(/<div class="(c\d+)"[^>]*data-testid="struck"[^>]*>(.*?)<\/div>/);
  expect(struck, html).not.toBeNull();
  expect(html).toContain(`.${struck[1]}{text-decoration-line:line-through;`);
  expect(struck[2].replace(/ data-site="\d+"/g, '')).toBe('<span>a </span><span data-testid="b">b</span><span><span data-testid="deep">deep</span></span>');
}, 120000);
