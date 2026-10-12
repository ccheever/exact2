// LLP 1115.000 D4.6: a page of the JS target keeps the manifest's `lang`,
// which screen-reader voices, hyphenation and `:lang()` read: the shell the
// development build writes (host/web-js/build.mjs), and a page the JavaScript
// runtime renders over it (host/web-js/render.mjs). A real build of an app
// that declares a language, outside the repository (`EXACT_APP_DIR`), then a
// real render of its one page.
import { test, expect, afterAll } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { renderer } from '../../web-js/render.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-lang-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

test('the JS build\'s shell and a rendered page keep the manifest\'s lang', async () => {
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Olá', lang: 'pt-BR', app: { id: 'com.example.ola', name: 'Olá' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "Olá, mundo" testId="hello"\n');
  const dist = resolve(dir, 'dist');
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'ola', '--out', dist, '--render', 'js'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  expect(build.status, build.stderr).toBe(0);
  expect(readFileSync(resolve(dist, 'index.html'), 'utf8')).toStartWith('<!doctype html>\n<html lang="pt-BR">\n');
  expect(JSON.parse(readFileSync(resolve(dist, 'manifest.json'), 'utf8')).lang).toBe('pt-BR');
  const page = await renderer(dist)('/');
  expect(page.html).toStartWith('<!doctype html>\n<html lang="pt-BR" dir="ltr">\n');
  expect(page.html).toContain('Olá, mundo');
}, 120000);
