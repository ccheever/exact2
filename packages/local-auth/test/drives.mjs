// LLP 1133.010 §9's acceptance drives 1–8 under the agent, with Excord against
// its fixture: the substitute's tickets stand for the person (§7), and the
// presence backend is the agent's own store, kept in the drive's scratch tree.
// Each `open` is a launch; a relaunch keeps the drive's store. Drive 9 (the
// web) is `web.mjs`'s.
//
// usage: bun packages/local-auth/test/drives.mjs [macos|ios|android] [--only 1,2,…]
//   Builds nothing: build Excord for the host first (host/apple/build.mjs
//   excord-apple, `--ios --sim <udid>` for a simulator), which the driver
//   checks is current. Starts Excord's fixture server on 4790 and stops it.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { checker } from './fixture.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['macos', 'ios', 'android'].includes(a)) ?? 'macos';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const { open, faultRequest } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const where = (n) => `${host} drive ${n}`;
const ORIGIN = 'http://127.0.0.1:4790', TOKEN = 'fixture-token', LOCKED = `${TOKEN}: locked, user-presence`;
// A token whose bytes no name shares, so a check that they are nowhere in the state means something.
const SECRET = 'fixture-secret-8';
const run = Date.now().toString(36);
const store = (name) => `la-${run}-${name}`;

// The fixture, for every drive: a relaunch identifies against the same world.
const fixture = spawn('bun', [resolve(root, 'apps/excord/fixture/server.ts'), '--world', 'transcript', '--reset-on-identify'], { stdio: ['ignore', 'pipe', 'inherit'] });
await new Promise((ok, fail) => {
  fixture.stdout.once('data', ok);
  fixture.once('exit', (code) => fail(new Error(`the fixture server exited (${code}); is 4790 taken?`)));
});
const control = (name, body) => fetch(`${ORIGIN}/control/${name}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }).then((r) => r.json());
const fixtureState = () => fetch(`${ORIGIN}/control/state`).then((r) => r.json());

/** A launch on the drive's store, Face ID's permission as `permission`. */
async function launch(name, { permission = 'granted', failFetch } = {}) {
  const s = await open({ host, app: 'excord', storage: store(name), failFetch });
  await s.prefer({ 'permission.local-auth': permission });
  return s;
}
const state = (s) => s.state();
const testIds = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
const holds = async (s) => ((await state(s)).pending ?? []).filter((p) => p.device);
const list = async (s) => ((await state(s)).store ?? []).filter((n) => n.startsWith(TOKEN));
/** Answer the held request of `capability`, once it is held. */
async function answer(s, capability, choice) {
  await s.clock('settle');
  return s.answer('tap', `@${capability}`, choice);
}
/** Wait, on the wall clock the gateway runs on, until `want` holds of the state. */
async function until(s, want, ms = 15000) {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = await state(s))) return got;
  }
  return got;
}
// Signed in: the channels loaded over an open gateway socket.
const signedIn = (st) => st.resources?.session?.ready === true && st.resources?.categories?.length > 0
  && (st.derives?.gw ?? st.resources?.gateway)?.state === 'ready' && (st.streams ?? []).some((x) => x.name === 'gateway' && x.readyState === 1);
/** Turn the lock on from the account menu: the check, then the move. */
async function lockOn(s) {
  await s.clock('settle');
  await s.tap('account');
  await s.tap('lock-switch');
  await answer(s, 'presence', 'allow');
  await s.clock('settle');
}
/** The app's home: this Mac's, or on a simulator (EXACT_SIM) the app's data container. */
let container;
function home() {
  if (host !== 'ios') return homedir();
  container ??= spawnSync('xcrun', ['simctl', 'get_app_container', process.env.EXACT_SIM ?? 'booted', 'com.exact.excord', 'data'], { encoding: 'utf8' }).stdout.trim();
  return container;
}
/** The drive store's files: whether a plain copy and a locked item of the token are on disk. */
function copies(name) {
  const tree = [resolve(home(), 'Library/Caches/exact/com.exact.excord/agent', store(name))].find(existsSync);
  if (!tree) return null;
  const files = (dir) => (existsSync(dir) ? readdirSync(dir) : []);
  return { plain: files(resolve(tree, 'secrets')).length > 0, locked: files(resolve(tree, 'kv/exact.presence.agent')).length > 0 };
}
// Anything of the account, or the sign-in form.
const ACCOUNT = /^(channel-|dm-|message-|signed-out|transcript|token$|sign-in)/;

const drives = {
  async 1() {
    const s = await launch('a', { permission: 'prompt' });
    try {
      await s.clock('settle');
      await s.tap('account');
      await s.tap('lock-switch');
      const asked = await holds(s);
      check(where(1), 'turning the lock on asks Face ID\'s permission first', asked[0]?.device?.capability === 'permission', JSON.stringify(asked));
      await answer(s, 'permission', 'allow');
      await answer(s, 'presence', 'allow');
      await s.clock('settle');
      const st = await state(s);
      check(where(1), 'the switch is on once protect resolves', st.resources?.session?.lockOn === true, JSON.stringify(st.resources?.session));
      check(where(1), 'the store lists the token locked', JSON.stringify(await list(s)) === JSON.stringify([LOCKED]), JSON.stringify(st.store));
      check(where(1), 'only the locked copy is on disk', JSON.stringify(copies('a')) === '{"plain":false,"locked":true}', JSON.stringify(copies('a')));
    } finally { await s.close(); }
  },
  async 2() {
    const s = await launch('a');
    try {
      const first = await testIds(s);
      check(where(2), 'the first frame shows nothing of the account', ![...first].some((t) => ACCOUNT.test(t)), [...first].join(' '));
      await s.clock('data');
      const landed = await testIds(s), st = await state(s);
      check(where(2), 'the first data answer shows the lock screen, not the account or sign-in', landed.has('lock-screen') && ![...landed].some((t) => ACCOUNT.test(t)), [...landed].join(' '));
      check(where(2), 'no gateway socket while locked', !(st.streams ?? []).length, JSON.stringify(st.streams));
      await s.clock('settle');
      const asked = await holds(s);
      check(where(2), 'one ticket, a presence check from the app\'s own code', asked.length === 1 && asked[0].device.capability === 'presence' && asked[0].device.args.origin === 'app' && asked[0].device.args.request === 'store.unlock', JSON.stringify(asked));
      await answer(s, 'presence', 'allow');
      const loaded = await until(s, signedIn);
      check(where(2), 'the channels load', signedIn(loaded), JSON.stringify(loaded.resources?.session));
      const after = await fixtureState();
      check(where(2), 'the gateway identifies once, with the token', after.identify_token === TOKEN && loaded.resources?.gateway?.attempt === 1, JSON.stringify({ attempt: loaded.resources?.gateway?.attempt, token: after.identify_token }));
    } finally { await s.close(); }
  },
  async 3() {
    const s = await launch('a');
    try {
      await answer(s, 'presence', 'cancel');
      await s.clock('settle');
      const ids = await testIds(s);
      check(where(3), 'a cancel leaves the lock screen with Unlock', ids.has('lock-screen') && ids.has('unlock'), [...ids].join(' '));
      await s.tap('unlock');
      await answer(s, 'presence', 'allow');
      const loaded = await until(s, signedIn);
      check(where(3), 'Unlock asks again, and the channels load', signedIn(loaded), JSON.stringify(loaded.resources?.session));
    } finally { await s.close(); }
  },
  async 4() {
    // The gateway rotates the token while it is locked: kept locked.
    let s = await launch('a');
    try {
      await answer(s, 'presence', 'allow');
      await until(s, signedIn);
      await control('rotate-token', { token: `${TOKEN}-2` });
      await until(s, () => false, 1500);
      check(where(4), 'a rotated token is kept locked, never plain', JSON.stringify(await list(s)) === JSON.stringify([LOCKED]) && copies('a').plain === false, JSON.stringify({ list: await list(s), disk: copies('a') }));
    } finally { await s.close(); }
    s = await launch('a');
    try {
      await answer(s, 'presence', 'allow');
      await until(s, signedIn);
      check(where(4), 'the relaunch identifies with the rotated token', (await fixtureState()).identify_token === `${TOKEN}-2`, (await fixtureState()).identify_token);
    } finally { await s.close(); }
    // From plain, the lock turned on while a READY rotates the token mid-move.
    await control('rotate-token', { token: TOKEN });
    s = await launch('b');
    try {
      await until(s, signedIn);
      await s.op(faultRequest('fail', ['fetch', 'exact-presence:hold.protect.record']));
      await s.clock('settle');
      await s.tap('account');
      await s.tap('lock-switch');
      await answer(s, 'presence', 'allow');
      await s.clock('settle');
      await control('rotate-token', { token: `${TOKEN}-3` });
      await until(s, () => false, 1000);
      const held = await list(s);
      check(where(4), 'the fault holds protect after its record: the token is still plain', JSON.stringify(held) === JSON.stringify([`${TOKEN}: plain`]), JSON.stringify(held));
      await s.op(faultRequest('pass', ['fetch', 'exact-presence:hold.protect.record']));
      await until(s, (st) => st.resources?.session?.lockOn === true, 5000);
      check(where(4), 'the write waited behind the move and was kept locked', JSON.stringify(await list(s)) === JSON.stringify([LOCKED]) && JSON.stringify(copies('b')) === '{"plain":false,"locked":true}', JSON.stringify({ held, after: await list(s), disk: copies('b') }));
    } finally { await s.close(); }
    s = await launch('b');
    try {
      await answer(s, 'presence', 'allow');
      await until(s, signedIn);
      check(where(4), 'a relaunch identifies with the token rotated mid-move', (await fixtureState()).identify_token === `${TOKEN}-3`, (await fixtureState()).identify_token);
    } finally { await s.close(); }
    await control('rotate-token', { token: TOKEN });
  },
  async 5() {
    await control('rotate-token', { token: TOKEN });
    const s = await launch('e');
    try {
      await until(s, signedIn);
      await lockOn(s);
      const st = await until(s, signedIn);
      const socket = st.streams?.find((x) => x.name === 'gateway')?.ticket;
      await s.prefer({ 'visibility-state': 'hidden' });
      await s.prefer({ 'visibility-state': 'visible' });
      await s.clock('+5');
      const covered = await testIds(s);
      check(where(5), 'a return shows the lock screen', covered.has('lock-screen'), [...covered].join(' '));
      await s.tap('unlock');
      await answer(s, 'presence', 'allow');
      await s.clock('settle');
      const back = await until(s, signedIn, 3000), ids = await testIds(s);
      check(where(5), 'the check passed, the lock screen goes', !ids.has('lock-screen'), [...ids].join(' '));
      const same = back.streams?.find((x) => x.name === 'gateway');
      check(where(5), 'the same socket, never closed', same?.ticket === socket && same?.readyState === 1, JSON.stringify({ socket, now: back.streams }));
    } finally { await s.close(); }
  },
  async 6() {
    const s = await launch('e');
    try {
      await answer(s, 'presence', 'invalidated');
      await s.clock('settle');
      const ids = await testIds(s), st = await state(s);
      check(where(6), 'the platform dropped the token: the sign-in screen', ids.has('signed-out') && !ids.has('lock-screen'), [...ids].join(' '));
      check(where(6), 'the store lists it absent', JSON.stringify(await list(s)) === JSON.stringify([`${TOKEN}: absent`]), JSON.stringify(st.store));
    } finally { await s.close(); }
  },
  async 7() {
    let s = await launch('c');
    try {
      await until(s, signedIn);
      await lockOn(s);
      await s.tap('account');
      await s.tap('lock-switch');
      const asked = await holds(s);
      await answer(s, 'presence', 'allow');
      await s.clock('settle');
      check(where(7), 'turning the lock off asks, then keeps the token plain', asked[0]?.device?.args?.request === 'store.unprotect' && JSON.stringify(await list(s)) === JSON.stringify([`${TOKEN}: plain`]), JSON.stringify({ asked, list: await list(s) }));
    } finally { await s.close(); }
    s = await launch('c');
    try {
      const st = await until(s, signedIn);
      check(where(7), 'a relaunch shows the transcript with no ticket', signedIn(st) && !(await holds(s)).length, JSON.stringify(await holds(s)));
    } finally { await s.close(); }
  },
  async 8() {
    // Each move stopped after a step, as a crash would stop it; the relaunch reconciles.
    for (const [point, after] of [['protect.record', 'pending'], ['protect.write', LOCKED], ['unprotect.record', LOCKED], ['unprotect.write', LOCKED]]) {
      const name = `m-${point}`;
      let s = await launch(name);
      try {
        await until(s, signedIn);
        await control('rotate-token', { token: SECRET });
        await until(s, () => false, 1000);
        if (point.startsWith('unprotect')) await lockOn(s);
        await s.op(faultRequest('fail', ['fetch', `exact-presence:${point}`, 'times', '1']));
        await s.tap('account');
        await s.tap('lock-switch');
        await answer(s, 'presence', 'allow');
        await s.clock('settle');
      } finally { await s.close(); }
      // A pending move waits for the drive (`hold.move`), so the relaunch reads what reconciliation left.
      s = await launch(name, { failFetch: 'exact-presence:hold.move' });
      try {
        await s.clock('data');
        const st = await state(s), shown = await list(s);
        const want = after === 'pending' ? `${TOKEN}: pending` : after;
        check(where(8), `after ${point}: ${want}, never both copies`, JSON.stringify(shown) === JSON.stringify([want]) && !(copies(name).plain && copies(name).locked), JSON.stringify({ shown, disk: copies(name) }));
        check(where(8), `after ${point}: the switch shows on`, st.resources?.session?.lockOn === true, JSON.stringify(st.resources?.session));
        if (point === 'protect.record') {
          const ids = await testIds(s);
          const seen = JSON.stringify(st) + JSON.stringify(await s.logs());
          check(where(8), 'a pending launch shows nothing of the account, and its token is in no state or journal line', ![...ids].some((t) => ACCOUNT.test(t)) && !seen.includes(SECRET), [...ids].join(' '));
          // The device credential withdrawn: the move cannot be made, and the copy is deleted.
          await s.prefer({ extension: 'local-auth.localAuthStatus={"available":true,"biometry":"faceID","deviceCredential":false,"presenceSecrets":true}' });
          await s.op(faultRequest('pass', ['fetch', 'exact-presence:hold.move']));
          await s.clock('+10');
          check(where(8), 'no way to lock it: the plain copy is deleted, the name absent', JSON.stringify(await list(s)) === JSON.stringify([`${TOKEN}: absent`]) && JSON.stringify(copies(name)) === '{"plain":false,"locked":false}', JSON.stringify({ list: await list(s), disk: copies(name) }));
        }
        if (point === 'unprotect.record') {
          await answer(s, 'presence', 'allow');
          await s.clock('settle');
        }
      } finally { await s.close(); }
      if (point === 'unprotect.record') {
        s = await launch(name);
        try {
          await s.clock('settle');
          check(where(8), 'an unlock never removes the lock: still locked, with a ticket', JSON.stringify(await list(s)) === JSON.stringify([LOCKED]) && (await holds(s)).length === 1, JSON.stringify({ list: await list(s), holds: await holds(s) }));
          const seen = JSON.stringify(await state(s)) + JSON.stringify(await s.logs());
          check(where(8), 'the token is in no state or journal line', !seen.includes(SECRET), 'found');
        } finally { await s.close(); }
      }
      await control('rotate-token', { token: TOKEN });
    }
    // A locked write the platform refuses: the previous value stays readable until its retry lands.
    for (const retried of [false, true]) {
      const name = `set-${retried}`;
      let s = await launch(name);
      try {
        await until(s, signedIn);
        await lockOn(s);
        await s.op(faultRequest('fail', ['fetch', 'exact-presence:set', 'times', '1']));
        await control('rotate-token', { token: `${TOKEN}-4` });
        await until(s, () => false, retried ? 2500 : 750);
        check(where(8), `a refused locked write keeps the name locked (${retried ? 'retried' : 'not yet'})`, JSON.stringify(await list(s)) === JSON.stringify([LOCKED]), JSON.stringify(await list(s)));
      } finally { await s.close(); }
      s = await launch(name);
      try {
        await answer(s, 'presence', 'allow');
        await until(s, () => false, 1500);
        const token = (await fixtureState()).identify_token;
        check(where(8), retried ? 'once the retry landed, unlock answers the new token' : 'before the retry, unlock answers the previous token', token === (retried ? `${TOKEN}-4` : TOKEN), token);
      } finally { await s.close(); }
      await control('rotate-token', { token: TOKEN });
    }
  },
};

try {
  for (const [n, drive] of Object.entries(drives)) {
    if (only && !only.includes(Number(n))) continue;
    try { await drive(); } catch (e) { check(where(n), 'ran', false, e.stack ?? e.message); }
  }
} finally {
  fixture.kill();
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
