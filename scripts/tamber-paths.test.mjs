// tamber-paths.mjs (tamber/1.15; round 1's rework): one rule
// names a file, and machine-written messages lose this machine's
// directories only where a whole path stands. Every directory here is made
// for the test and removed after.
import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { messageScrubber, namedPaths, pathName, scrubbedMessages } from './tamber-paths.mjs';

test('a file is named from the app, from its package, or as outside: never by its path', () => {
  const root = mkdtempSync(join(tmpdir(), 'tamber-paths-'));
  try {
    const app = join(root, 'app'), linked = join(root, 'libs/linked'), loose = join(root, 'loose');
    for (const d of [join(app, 'node_modules/@acme'), linked, loose]) mkdirSync(d, { recursive: true });
    writeFileSync(join(linked, 'package.json'), '{"name":"@acme/linked"}');
    writeFileSync(join(linked, 'index.contract'), 'x');
    writeFileSync(join(loose, 'a.contract'), 'x');
    symlinkSync(linked, join(app, 'node_modules/@acme/linked'));
    writeFileSync(join(app, 'app.contract'), 'x');
    expect(pathName(join(app, 'app.contract'), app)).toBe('app.contract');
    // The app reached through its real path (macOS's /tmp is /private/tmp).
    expect(pathName(join(realpathSync(app), 'app.contract'), app)).toBe('app.contract');
    // Through the link inside the app: the app's name for it.
    expect(pathName(join(app, 'node_modules/@acme/linked/index.contract'), app)).toBe('node_modules/@acme/linked/index.contract');
    // The package's own path, outside the app: its package's name.
    expect(pathName(join(realpathSync(linked), 'index.contract'), app)).toBe('@acme/linked/index.contract');
    expect(pathName(join(loose, 'a.contract'), app)).toBe('<outside>/a.contract');
    // Relative names, and strings that are no path, are kept.
    expect(pathName('app.tamber', app)).toBe('app.tamber');
    expect(pathName('exact:app', app)).toBe('exact:app');
    // Only `file` fields are named.
    const named = namedPaths({ file: join(app, 'app.contract'), value: join(app, 'assets/i.svg'), chain: [{ file: join(realpathSync(linked), 'index.contract') }] }, app);
    expect(named).toEqual({ file: 'app.contract', value: join(app, 'assets/i.svg'), chain: [{ file: '@acme/linked/index.contract' }] });
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('a machine message names each path in it by the rule, keeps its line and column, and leaves authored text', () => {
  const root = mkdtempSync(join(tmpdir(), 'tamber-paths-msg-'));
  try {
    const app = join(root, 'app'), home = homedir();
    mkdirSync(join(app, 'assets'), { recursive: true });
    writeFileSync(join(app, 'app.contract'), 'x');
    const scrub = messageScrubber(app);
    expect(scrub(`another tamber server writes ${app}`)).toBe('another tamber server writes <app>');
    expect(scrub(`another tamber server writes ${realpathSync(app)}.`)).toBe('another tamber server writes <app>.');
    expect(scrub(`${app}/app.contract:3:7 [syntax] expected a view`)).toBe('app.contract:3:7 [syntax] expected a view');
    expect(scrub(`${app}/missing.tamber: No such file`)).toBe('missing.tamber: No such file');
    expect(scrub(`reads \`${join(root, 'loose.contract')}\` (gone)`)).toBe('reads `<outside>/loose.contract` (gone)');
    // A path inside another, or a longer name, is not the app's; one that names nothing there is left.
    expect(scrub(`${app}y/app.contract`)).toBe(`${app}y/app.contract`);
    expect(scrub(`${home}istrator/notes`)).toBe(`${home}istrator/notes`);
    expect(scrub('POST /__tamber/subscribe')).toBe('POST /__tamber/subscribe');
    // Only a refusal's messages and a `down` event's; a commit's is authored.
    const reply = { ok: { commits: [{ message: `moved ${app}/app.contract` }] }, error: { message: `${app}/app.contract`, diagnostics: [{ message: `${app}/assets` }] } };
    expect(scrubbedMessages(reply, scrub)).toEqual({ ok: { commits: [{ message: `moved ${app}/app.contract` }] }, error: { message: 'app.contract', diagnostics: [{ message: 'assets' }] } });
    expect(scrubbedMessages({ event: 'down', message: `writes ${app}` }, scrub)).toEqual({ event: 'down', message: 'writes <app>' });
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('a machine message names a package by its package, however it is installed', () => {
  const root = mkdtempSync(join(tmpdir(), 'tamber-paths-pkg-'));
  try {
    const app = join(root, 'repo/app');
    const pkg = (dir, name) => { mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, 'package.json'), JSON.stringify({ name })); writeFileSync(join(dir, 'index.contract'), 'x'); };
    mkdirSync(app, { recursive: true });
    // A linked package outside the app.
    const linked = join(root, 'libs/linked');
    pkg(linked, '@acme/linked');
    // A dependency nested in another's node_modules, outside the app (where a link leads).
    pkg(join(linked, 'node_modules/inner'), 'inner');
    // A node_modules installed above the app (a monorepo's root).
    pkg(join(root, 'repo/node_modules/@acme/up'), '@acme/up');
    // Bun's per-file links: the package's files link into a store, where the package.json is.
    const store = join(root, 'store/@acme+bun@1.0.0/node_modules/@acme/bun');
    pkg(store, '@acme/bun');
    mkdirSync(join(app, 'node_modules/@acme/bun'), { recursive: true });
    symlinkSync(join(store, 'index.contract'), join(app, 'node_modules/@acme/bun/index.contract'));
    const scrub = messageScrubber(app);
    expect(scrub(`${realpathSync(linked)}/index.contract:3:1 [syntax] expected a view`)).toBe('@acme/linked/index.contract:3:1 [syntax] expected a view');
    expect(scrub(`${realpathSync(linked)}/missing.tamber: No such file`)).toBe('@acme/linked/missing.tamber: No such file');
    expect(scrub(`${realpathSync(linked)}/node_modules/inner/index.contract: unreadable`)).toBe('inner/index.contract: unreadable');
    expect(scrub(`${root}/repo/node_modules/@acme/up/index.contract:1:1 refused`)).toBe('@acme/up/index.contract:1:1 refused');
    expect(scrub(`${realpathSync(store)}/index.contract: refused`)).toBe('@acme/bun/index.contract: refused');
    // Through the app's own node_modules: the app's name for it.
    expect(scrub(`${app}/node_modules/@acme/bun/index.contract: refused`)).toBe('node_modules/@acme/bun/index.contract: refused');
    for (const m of [root, realpathSync(root)]) expect(scrub(`${realpathSync(store)}/index.contract`).includes(m)).toBe(false);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
