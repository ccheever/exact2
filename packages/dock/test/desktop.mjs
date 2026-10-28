// LLP 1133.025's drives on a Mac, on the desktop fixture (test/fixture):
// an app `exact new` made outside the repo, depending on @exact/login-item,
// @exact/global-shortcuts and @exact/dock, built and driven under the agent.
//
//  1. The status item: `state.statusItems` lists it with its menu, the tree
//     marks the button presented elsewhere, and AppKit shows an
//     `NSStatusItem`; its tap opens the menu, whose row presses (`focus()`).
//  2. The Dock menu: `state.dockMenu` lists New Note; the button stays in
//     File too.
//  3. The menu item hidden by the page goes, shown comes back.
//  4. The substitutes: the login item registers hidden; a chord is listed,
//     pressed by `tap @ref`, moved; attention is kept for the agent.
//  5. `--login hidden`: no window, the status item placed, the page hidden,
//     `launchedAtLogin`; the tray's Open shows the window.
//  6. `--bind login-item`: SMAppService's registration of this fixture
//     bundle, and its unregistration (the item is never left registered).
//  7. `--bind global-shortcuts`: Carbon registers the chord, and a check of
//     another answers without keeping it.
//  8. An accessory app (`host.macos.activationPolicy`): no window at launch,
//     the status item placed, the window shown by Open.
//
// usage: bun packages/dock/test/desktop.mjs [macos|web|linux] [--only 1,2,…] [--keep] [--app <dir>]
//   --app reuses a fixture an earlier --keep kept, rebuilt with this checkout's sources.
//   On web and Linux the drive is the absent one: the modules answer
//   NotSupportedError and the status and Dock buttons are ordinary buttons.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { checker } from '../../camera/test/fixture.mjs';
import { texts } from '../../photos/test/fixture.mjs';
import { root, run } from '../../contacts/test/fixture.mjs';
import { build, launch } from '../../tracking-consent/test/fixture.mjs';
import { cpSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename } from 'node:path';

const argv = process.argv.slice(2);
const host = argv.find((a) => ['macos', 'web', 'linux'].includes(a)) ?? 'macos';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const runs = (n) => !only || only.includes(n);
const { check, failed } = checker();

/** The fixture app: `exact new`, the fixture's files, the three packages. */
function makeFixture({ accessory = false } = {}) {
  const here = resolve(import.meta.dir, 'fixture');
  const dir = mkdtempSync(join(tmpdir(), 'exact-desktop-')), app = join(dir, accessory ? 'menubarfixture' : 'desktopfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const manifestPath = join(app, 'app.json'), appJson = JSON.parse(readFileSync(manifestPath, 'utf8'));
  if (accessory) { appJson.host ??= {}; appJson.host.macos = { ...(appJson.host.macos ?? {}), activationPolicy: 'accessory' }; }
  writeFileSync(manifestPath, JSON.stringify(appJson, null, 2));
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', appJson.app.id));
  const pkg = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  for (const name of ['login-item', 'global-shortcuts', 'dock']) (pkg.dependencies ??= {})[`@exact/${name}`] = `file:${resolve(root, 'packages', name)}`;
  writeFileSync(join(app, 'package.json'), JSON.stringify(pkg, null, 2));
  run('bun', ['install'], app);
  return { app, name: basename(app), id: appJson.app.id, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

const logs = async (s) => (await s.logs({ all: true })).lines.join('\n');
const visible = (st) => (st.documents?.windows ?? []).map((w) => w.visible);

async function drives(fixture) {
  if (runs(1) || runs(2) || runs(3)) {
    const s = await launch(fixture, host);
    try {
      await s.clock('settle');
      const st = await s.state();
      const items = st.statusItems ?? [], dock = st.dockMenu ?? [];
      if (host === 'macos') {
        check(host, '1: state.statusItems lists the tray with its menu',
          items.length === 1 && items[0].label === 'Desktop Fixture' && items[0].menu?.length === 4, JSON.stringify(items));
        const tree = await s.tree(), tray = tree.nodes.find((n) => n.props?.testId === 'tray');
        check(host, '1: the tree marks the tray presented elsewhere', tray?.presented === 'status', JSON.stringify(tray));
        const opened = await s.tap('tray');
        check(host, '1: a tap opens the NSStatusItem menu', opened?.native === 'NSStatusItem' && opened?.menu?.includes('Open Desktop Fixture') && !opened.menu.some((r) => r.startsWith('Quit ')), JSON.stringify(opened));
        await s.tap('tray-open');
        await s.clock('settle');
        check(host, '1: the menu row presses', (await texts(s)).opened?.startsWith('Opened: 1'), (await texts(s)).opened);
        check(host, '2: state.dockMenu lists New Note', dock.length === 1 && dock[0].label === 'New Note', JSON.stringify(dock));
        await s.tap('toggle-tray');
        await s.clock('settle');
        check(host, '3: the hidden button removes the item', (await s.state()).statusItems?.length === 0, JSON.stringify((await s.state()).statusItems));
        await s.tap('toggle-tray');
        await s.clock('settle');
        check(host, '3: shown, it comes back', (await s.state()).statusItems?.length === 1, '');
      } else {
        // Absent (D14): ordinary buttons in the page.
        const tree = await s.tree(), tray = tree.nodes.find((n) => n.props?.testId === 'tray');
        check(host, 'absent: the status button is an ordinary button', tray && tray.presented === undefined, JSON.stringify(tray));
        await s.tap('at-login');
        await s.clock('settle');
        check(host, 'absent: registerLoginItem refuses NotSupportedError', /registerLoginItem[^\n]*NotSupportedError/.test(await logs(s)),
          (await logs(s)).split('\n').filter((l) => /login/i.test(l)).slice(-3).join('\n'));
        check(host, 'absent: no shortcut listed', !((await s.state()).globalShortcuts?.length), JSON.stringify((await s.state()).globalShortcuts));
      }
    } finally { await s.close(); }
  }
  if (host !== 'macos') return;
  if (runs(4)) {
    const s = await launch(fixture, host);
    try {
      await s.clock('settle');
      await s.tap('at-login');
      await s.clock('settle');
      check(host, '4: the login item registers hidden (substitute)', (await texts(s)).login === 'Login item: enabled (hidden)', (await texts(s)).login);
      await s.tap('not-at-login');
      await s.clock('settle');
      const listed = (await s.state()).globalShortcuts ?? [];
      check(host, '4: the chord is listed', listed.length === 1 && listed[0].keys === 'Control+Alt+Meta+K', JSON.stringify(listed));
      if (listed[0]) {
        await s.tap(`@${listed[0].ref}`);
        await s.clock('settle');
        check(host, '4: tap @ref presses it', (await texts(s)).presses === 'Presses: 1 snap', (await texts(s)).presses);
      }
      await s.tap('rebind');
      await s.clock('settle');
      const moved = (await s.state()).globalShortcuts ?? [];
      check(host, '4: new keys move the registration', moved.length === 1 && moved[0].keys === 'Control+Alt+Meta+J', JSON.stringify(moved));
      await s.tap('attend');
      await s.clock('settle');
      check(host, '4: attention is journaled', /requestAttention/.test(await logs(s)), '');
    } finally { await s.close(); }
  }
  if (runs(5)) {
    // What `agent.mjs --login hidden` does: the host's reading, then the runner's entry.
    const s = await launch(fixture, host, { env: { EXACT_LOGIN_LAUNCH: 'hidden' } });
    try {
      await s.clock('settle');
      await s.op({ op: 'launchEntry', launch: 'login', entry: 'hidden' });
      await s.clock('settle');
      const st = await s.state();
      check(host, '5: no window shown', visible(st).length > 0 && visible(st).every((v) => v === false), JSON.stringify(st.documents?.windows));
      check(host, '5: the status item is placed', st.statusItems?.length === 1, JSON.stringify(st.statusItems));
      check(host, '5: the page is hidden, not in the background', st.device?.visibilityState === 'hidden' && st.device?.background === false, JSON.stringify({ visibilityState: st.device?.visibilityState, background: st.device?.background }));
      check(host, '5: launchedAtLogin', (await texts(s)).login?.includes('launched at login'), (await texts(s)).login);
      await s.tap('tray');
      await s.tap('tray-open');
      await s.clock('settle');
      check(host, '5: Open shows the window', visible(await s.state()).some((v) => v === true), JSON.stringify((await s.state()).documents?.windows));
    } finally { await s.close(); }
  }
  if (runs(6)) {
    const s = await launch(fixture, host, { bind: 'login-item' });
    try {
      await s.clock('settle');
      await s.tap('at-login');
      await s.clock('+2000 real');
      const on = (await texts(s)).login;
      check(host, '6: SMAppService registers the fixture', /enabled|requires-approval/.test(on), `${on}\n${await logs(s)}`);
      await s.tap('not-at-login');
      await s.clock('+2000 real');
      check(host, '6: and unregisters it', (await texts(s)).login.startsWith('Login item: disabled'), (await texts(s)).login);
    } finally { await s.close(); }
  }
  if (runs(7)) {
    const s = await launch(fixture, host, { bind: 'global-shortcuts' });
    try {
      await s.clock('+1000 real');
      check(host, '7: Carbon registers the chord', /registered/.test((await texts(s)).snap) && !/not registered/.test((await texts(s)).snap), (await texts(s)).snap);
      await s.tap('check');
      await s.clock('+1000 real');
      check(host, '7: a check of another answers', (await texts(s)).checked === 'Check: true ', (await texts(s)).checked);
    } finally { await s.close(); }
  }
}

const kept = argv.includes('--app') ? resolve(argv[argv.indexOf('--app') + 1]) : null;
const fixture = kept ? { app: kept, name: basename(kept), remove: () => {} } : makeFixture();
try {
  build(fixture, host);
  await drives(fixture);
} finally {
  if (!argv.includes('--keep')) fixture.remove(); else console.log(`kept ${fixture.app}`);
}
if (host === 'macos' && runs(8)) {
  const menuBar = makeFixture({ accessory: true });
  try {
    build(menuBar, host);
    const s = await launch(menuBar, host);
    try {
      await s.clock('settle');
      const st = await s.state();
      check(host, '8: an accessory app opens no window', visible(st).length > 0 && visible(st).every((v) => v === false), JSON.stringify(st.documents?.windows));
      check(host, '8: its status item is placed', st.statusItems?.length === 1, JSON.stringify(st.statusItems));
      const opened = await s.tap('tray');
      check(host, '8: its menu ends with Quit', opened?.menu?.at(-1)?.startsWith('Quit '), JSON.stringify(opened));
      await s.tap('tray-open');
      await s.clock('settle');
      check(host, '8: Open shows the window', visible(await s.state()).some((v) => v === true), JSON.stringify((await s.state()).documents?.windows));
    } finally { await s.close(); }
  } finally {
    if (!argv.includes('--keep')) menuBar.remove(); else console.log(`kept ${menuBar.app}`);
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
