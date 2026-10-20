// tamber-paths.mjs (stage 11's window, a fix; round 1's rework): one rule
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

test('a machine message loses whole paths only, longest first; authored text is not read', () => {
  const app = '/tmp/x', home = homedir();
  const scrub = messageScrubber(app);
  expect(scrub(`another tamber server writes ${app}`)).toBe('another tamber server writes <app>');
  expect(scrub(`${app}/app.tamber: Permission denied`)).toBe('<app>/app.tamber: Permission denied');
  // A path inside another, or a longer name, is not the app's.
  expect(scrub('/private/tmp/xy/app.contract')).toBe('/private/tmp/xy/app.contract');
  expect(scrub('/tmp/xy is not /tmp/x')).toBe('/tmp/xy is not <app>');
  expect(scrub(`${home}istrator/notes`)).toBe(`${home}istrator/notes`);
  expect(scrub(`${home}/notes`)).toBe('~/notes');
  // Only a refusal's messages and a `down` event's; a commit's is authored.
  const reply = { ok: { commits: [{ message: `moved ${home}/x` }] }, error: { message: `${home}/a`, diagnostics: [{ message: `${home}/b` }] } };
  expect(scrubbedMessages(reply, scrub)).toEqual({ ok: { commits: [{ message: `moved ${home}/x` }] }, error: { message: '~/a', diagnostics: [{ message: '~/b' }] } });
  expect(scrubbedMessages({ event: 'down', message: `${home}/c` }, scrub)).toEqual({ event: 'down', message: '~/c' });
});
