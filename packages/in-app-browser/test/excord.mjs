// LLP 1133.024 §20 stage 4's drive: Excord's link embeds under the agent,
// against the fixture server its app.json names, started as the agent's CLI
// starts it (`driveServer`), whose #announcements messages each carry one.
// Where the page shows over the app (iOS, Android) the title's tap sends
// `openBrowser`, held as a `browser` ticket, answered as each platform answers:
// `dismissed` on iOS (Safari's view reports Done), `opened` on Android (a
// Custom Tab reports nothing after its launch); elsewhere (macOS, the web) the fact says `overApp` false and the
// tap follows the link, which this drive does not tap on the Mac, where the
// adapter opens the default browser.
//
// usage: bun packages/in-app-browser/test/excord.mjs <ios|macos|web|android> [--sim <udid>] [--emulator <serial>]
//   Builds nothing: build Excord for the host first (Android: its APK,
//   `bun scripts/agent-android.mjs build excord --apk`, driven on the Canvas reader).
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { checker } from '../../camera/test/fixture.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv[0] ?? 'ios';
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
if (host === 'android') Object.assign(process.env, { EXACT_ANDROID_READER: '1' }, argv.includes('--emulator') ? { ANDROID_SERIAL: argv[argv.indexOf('--emulator') + 1] } : {});
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { driveServer, appServer } = await import(resolve(root, 'scripts/verify.mjs'));
const { stateOp } = await import(resolve(root, 'scripts/agent-inspect.mjs'));
const { check, failed } = checker();

/** A state, derive or resource by name, as `state <name>` reads it. */
const named = (s, name) => stateOp(s, [name]);
const testIds = async (s) => (await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean);
const over = host === 'ios' || host === 'android';
const storage = `in-app-browser-${Date.now().toString(36)}`;
const served = await driveServer(resolve(root, 'apps/excord'), storage);
// The build reaches the server at this Mac's loopback, which a phone's is not: `adb reverse` carries it there.
const port = host === 'android' ? appServer(resolve(root, 'apps/excord')).port : null;
const adb = (...args) => spawnSync('adb', [...(process.env.ANDROID_SERIAL ? ['-s', process.env.ANDROID_SERIAL] : []), ...args]);
if (port) adb('reverse', `tcp:${port}`, `tcp:${port}`);
try {
  const s = await open({ host, app: 'excord', storage });
  try {
    // Excord opens on the direct messages (`server-@me`); the fixture's channels are its one guild's,
    // listed once the gateway is ready. The fixture answers on the wall clock, so each wait lets it run.
    let guild;
    for (let i = 0; i < 60 && !guild; i++) {
      await s.clock('+500 real');
      await s.clock('settle');
      guild = (await testIds(s)).find((id) => id.startsWith('server-') && id !== 'server-@me');
    }
    check(host, 'the fixture guild is listed', !!guild, (await testIds(s)).filter((id) => id.startsWith('server-')).join(','));
    if (!guild) throw new Error('no guild to open');
    await s.tap(guild);
    await s.clock('settle');
    await s.tap('channel-announcements');
    await s.clock('settle');
    const browser = await named(s, 'browser');
    check(host, `the fact: overApp ${over}`, browser?.overApp === over && browser?.available === true, JSON.stringify(browser));
    const titles = (await testIds(s)).filter((id) => id.startsWith('embed-title-'));
    check(host, 'the channel shows link embeds with titles', titles.length > 0, titles.join(','));
    if (over && titles.length) {
      await s.tap(titles.at(-1));
      await s.clock('+100');
      const pending = (await named(s, 'pending')) ?? [];
      const ticket = pending.find((p) => p.device?.capability === 'browser');
      check(host, 'the title holds a browser ticket naming the page', !!ticket && JSON.stringify(ticket).includes('https://'), JSON.stringify(pending));
      const answer = host === 'ios' ? 'dismissed' : 'opened';
      await s.tap(`@${ticket?.ticket ?? 'browser'}`, { choice: answer });
      await s.clock('settle');
      const browsed = await named(s, 'browsed');
      check(host, `the ticket answers ${answer}`, JSON.stringify(browsed ?? null).includes(`"${answer}"`), JSON.stringify(browsed));
      // A module's lines: the journal on Apple and the web; logcat on Android, where `context.log` writes.
      let logs = '';
      for (let i = 0; i < 10 && !logs.includes('in-app-browser: '); i++) {
        logs += host === 'android' ? String(adb('logcat', '-d', '-s', 'exact').stdout).replace(/\r/g, '') : ((await s.logs()).lines ?? []).join('\n') + '\n';
        if (!logs.includes('in-app-browser: ')) await s.clock('+500 real');
      }
      check(host, 'the journal names the origin, not the path', new RegExp(`in-app-browser: ${answer} https://[a-z.]+$`, 'm').test(logs) && !/in-app-browser: .*https:\/\/[a-z.]+\/\S/.test(logs), logs.split('\n').filter((l) => l.includes('in-app-browser')).join(' | '));
    }
  } finally { await s.close(); }
} finally {
  if (port) adb('reverse', '--remove', `tcp:${port}`);
  await served?.stop();
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
