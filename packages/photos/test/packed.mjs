// The seam is public (LLP 1133.001 D8, LLP 1133.019 stage 4): @exact/photos
// packed as a registry would carry it (`bun pm pack`), installed into an app
// `exact new` made outside the repo, builds unchanged, and its pictures are
// drawn. The fixture package serving its own `img` scheme from outside the
// tree is seams.mjs's.
//
// - The web: the build takes the package's web implementation, which
//   answers absence: `available` false, and a read fails.
// - macOS, under the agent: the read is granted by its ticket, the agent's
//   library (the packed package's own fixture photos) is listed, and each
//   picture is admitted and drawn.
//
// usage: bun packages/photos/test/packed.mjs [web] [macos]
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { checker, makeFixture, root, run, texts, until } from './fixture.mjs';

const hosts = process.argv.slice(2).length ? process.argv.slice(2) : ['web', ...(process.platform === 'darwin' ? ['macos'] : [])];
const { app, dir } = makeFixture({ packed: true }), name = app.split('/').pop();
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1', EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

const installed = join(app, 'node_modules/@exact/photos');
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
check('install', 'the app depends on the tarball, and the package is a copy, not a link', manifest.dependencies['@exact/photos'].endsWith('.tgz') && existsSync(join(installed, 'exact.json')) && !run('ls', ['-ld', installed], app).startsWith('l'), manifest.dependencies['@exact/photos']);
check('install', 'the tarball carries the agent library', [1, 2, 3, 4, 5, 6].every((n) => existsSync(join(installed, 'fixture', `agent-${n}.jpg`))), 'fixture/ missing from the packed package');

for (const host of hosts) {
  if (host === 'web') {
    run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
    check('web', 'the app builds unchanged', existsSync(join(app, 'target/web-dist/index.html')), 'no target/web-dist/index.html');
    const s = await open({ host: 'web', app: name });
    try {
      const status = await until(s, 'status', (t) => /^(true|false) /.test(t ?? ''));
      check('web', 'the web implementation answers absence', status === 'false unknown none unknown', status);
      await s.tap('read');
      await s.clock('settle');
      check('web', 'and lists nothing', (await texts(s)).count === '0', (await texts(s)).count);
    } finally {
      await s.close();
    }
    continue;
  }
  run('bun', [resolve(root, 'host/apple/build.mjs'), `${name}-apple`], root);
  const s = await open({ host, app: name });
  try {
    await s.prefer({ 'permission.photos': 'prompt' });
    await s.tap('read');
    await s.clock('+100');
    const { pending } = await s.state();
    check(host, 'the read waits on a photos ticket', JSON.stringify(pending).includes('photos.photoAssets'), JSON.stringify(pending));
    await s.tap('@photos.photoAssets', { choice: 'allow' });
    await s.clock('settle');
    const count = await until(s, 'count', (t) => t === '6');
    check(host, "the packed package's agent library: six photos", count === '6', count);
    const pictures = await until(s, 'pictures', (t) => t?.startsWith('6 '), 20000);
    check(host, 'each picture is drawn, none fails', pictures === '6 0 ', pictures);
    const counted = (await s.state()).extensions?.photos?.pictures;
    check(host, 'each admitted by the runner', counted?.admitted >= 6, JSON.stringify(counted));
    await s.screenshot(join(dir, 'packed.png'));
  } finally {
    await s.close();
  }
}
rmSync(dir, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
