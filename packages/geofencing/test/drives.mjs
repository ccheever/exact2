// LLP 1133.021 §13's geofencing drives 10 to 14 and 16 under the agent, on
// the geofencing fixture (`apps/geofence-fixture`), where an arrival posts
// a notification (`@exact/notifications`), its permission granted by the
// drive: a wake's post never raises a ticket. Drive 15 (a wake past its budget
// keeps its transition) is the runner's (`contract/cli/tests/it/
// module_geofencing.rs`), since the fixture's handler has no request to
// hold open.
//
// 10. Turn it on: one `geolocation-background` ticket on macOS and iOS,
//     `allow` → the fence listed at Palo Alto, radius 200, `inside` none.
//     Denied → no fence, and the explanation.
// 11. Arrive, warm: `tap @<ref>` → a `geofenceenter` wake with origin
//     `wake`, the notification listed, the wake completed, `inside` true;
//     again → `geofenceleave`, no notification.
// 12. Arrive, cold: `--wake` the entry with its event → the headless boot
//     raises it and the notification is listed.
// 13. Arrive, late: `"deliver": "launch"` with an old timestamp →
//     delivered after the first pixel with that timestamp.
// 14. Refused: background location denied → the explanation; a twenty-first
//     fence fails `QuotaExceededError`.
// 16. Absent: on the web and Linux, `available` false and no switch.
//
// usage: bun packages/geofencing/test/drives.mjs <web|macos|ios|linux> [--browser chromium|firefox|webkit] [--sim <udid>]
//   (the fixture built for that host first: host/web/build.mjs geofence-fixture,
//   host/apple/build.mjs [--ios] geofence-fixture-apple, cargo build --profile host-dev -p geofence-fixture-linux)
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv[0] ?? 'macos';
const flag = (name) => argv.includes(name) ? argv[argv.indexOf(name) + 1] : null;
const browser = flag('--browser'), sim = flag('--sim');
if (sim) process.env.EXACT_SIM = sim;
process.env.EXACT_IDENTITY ??= '-';
const { open, textOf } = await import(resolve(root, 'scripts/agent.mjs'));
let failures = 0;
const check = (name, ok, detail) => { if (!ok) failures++; console.log(`${ok ? 'PASS' : 'FAIL'} ${host}${browser ? `/${browser}` : ''} ${name}${ok ? '' : `: ${detail}`}`); };

const launch = async (n) => open({ host, app: 'geofence-fixture', storage: `geofence-drive-${n}`, ...(browser ? { browser: browser === 'chromium' ? 'chrome' : browser } : {}) });
const state = async (s) => s.op({ op: 'state' });
const text = async (s, id) => {
  const nodes = (await s.tree()).nodes, node = nodes.find((n) => n.props?.testId === id);
  return node ? textOf(nodes, node) ?? '' : null;
};
const lines = async (s) => (await s.logs()).lines ?? [];
const settle = (s) => s.clock('settle');
const permissionTickets = (st) => (st.pending ?? []).filter((p) => p.device?.capability === 'permission').map((p) => p.device.args?.capability);

async function session(n, body) {
  const s = await launch(n);
  try { await settle(s); await body(s); }
  catch (e) { check(`${n}: ran`, false, e.message); }
  finally { await s.close?.(); }
}

if (host === 'web' || host === 'linux') {
  // 16: no cell here.
  await session(16, async (s) => {
    check('16: available false', (await text(s, 'status'))?.startsWith('status: false'), await text(s, 'status'));
    check('16: no switch', (await text(s, 'absent')) !== null && (await text(s, 'arrive-switch')) === null, await text(s, 'absent'));
  });
} else {
  // 10: one ticket for background location; allow lists the fence.
  await session(10, async (s) => {
    await s.tap('arrive-switch');
    await settle(s);
    const asked = permissionTickets(await state(s));
    check('10: one ticket, for geolocation-background', asked.length === 1 && asked[0] === 'geolocation-background', JSON.stringify((await state(s)).pending));
    await s.answer('tap', '@permission', 'allow');
    await settle(s);
    const fences = (await state(s)).geofences ?? [];
    const pa = fences.find((f) => f.name === 'station:pa');
    check('10: the fence is listed at Palo Alto, radius 200, inside none', pa && pa.latitude === 37.4434 && pa.longitude === -122.1649 && pa.radius === 200 && pa.inside == null, JSON.stringify(fences));
    check('10: the switch reads on', (await text(s, 'arrive-switch')) === 'On', await text(s, 'arrive-switch'));
  });
  await session('10-denied', async (s) => {
    await s.tap('arrive-switch');
    await settle(s);
    await s.answer('tap', '@permission', 'deny');
    await settle(s);
    check('10: denied, no fence', ((await state(s)).geofences ?? []).length === 0, JSON.stringify((await state(s)).geofences));
    check('10: denied, the switch explains', (await text(s, 'explain')) !== null, await text(s, 'status'));
  });
  // 11: a warm arrival and a leave.
  let printed = null;
  await session(11, async (s) => {
    await s.prefer({ 'permission.geolocation-background': 'granted', 'permission.notifications': 'granted' });
    await s.tap('arrive-switch');
    await settle(s);
    const fence = ((await state(s)).geofences ?? [])[0];
    printed = fence;
    const tapped = await s.answer('tap', `@${fence?.ref}`);
    check('11: the tap raises geofenceenter as a wake', JSON.stringify(tapped).includes('geofenceenter'), JSON.stringify(tapped));
    await settle(s);
    const after = await state(s), journal = await lines(s);
    check('11: the arrival is heard with its name and time', /^entered station:pa at \d+/.test(await text(s, 'heard')), await text(s, 'heard'));
    check('11: the notification is listed', JSON.stringify(after.notifications ?? []).includes('station:pa'), JSON.stringify(after.notifications));
    check('11: the wake completed', journal.some((l) => /wake: completed/.test(l)), journal.filter((l) => /wake/.test(l)).join(' | '));
    check('11: inside is true', after.geofences?.[0]?.inside === true, JSON.stringify(after.geofences));
    await s.answer('tap', `@${fence?.ref}`);
    await settle(s);
    check('11: the next tap leaves', (await text(s, 'heard')) === 'left station:pa', await text(s, 'heard'));
    check('11: a leave posts nothing', ((await state(s)).notifications ?? []).length === (after.notifications ?? []).length, JSON.stringify((await state(s)).notifications));
  });
  // 12: a cold wake from the entry as state printed it.
  if (printed) await session(12, async (s) => {
    await s.prefer({ 'permission.geolocation-background': 'granted', 'permission.notifications': 'granted' });
    const entry = { ...printed, event: 'geofenceenter', inside: false };
    const r = await s.op({ op: 'launchEntry', launch: 'wake', entry: JSON.stringify(entry) });
    check('12: the cold wake raises geofenceenter', JSON.stringify(r).includes('geofenceenter'), JSON.stringify(r));
    await settle(s);
    check('12: the boot was cold', (await state(s)).wake?.cold === true, JSON.stringify((await state(s)).wake));
    check('12: the notification is listed', JSON.stringify((await state(s)).notifications ?? []).includes('station:pa'), JSON.stringify((await state(s)).notifications));
  });
  // 13: a transition kept for a launch, delivered with its own time.
  if (printed) await session(13, async (s) => {
    const old = Date.now() - 3600000;
    const entry = { ...printed, event: 'geofenceenter', inside: false, deliver: 'launch', timestamp: old };
    await s.op({ op: 'launchEntry', launch: 'wake', entry: JSON.stringify(entry) });
    await settle(s);
    check('13: delivered with its original timestamp', (await text(s, 'heard')) === `entered station:pa at ${old}`, await text(s, 'heard'));
    check('13: not as a cold wake', (await state(s)).wake?.cold === false, JSON.stringify((await state(s)).wake));
  });
  // 14: denied explains; the twenty-first fence is over the quota.
  await session(14, async (s) => {
    await s.prefer({ 'permission.geolocation-background': 'denied' });
    await settle(s);
    check('14: denied, the switch explains', (await text(s, 'explain')) !== null, await text(s, 'status'));
    await s.prefer({ 'permission.geolocation-background': 'granted' });
    for (let i = 0; i < 21; i++) { await s.tap('add-one'); await settle(s); }
    const journal = await lines(s);
    check('14: twenty fences listed', ((await state(s)).geofences ?? []).length === 20, ((await state(s)).geofences ?? []).length);
    check('14: the twenty-first fails QuotaExceededError', journal.some((l) => /addGeofence: refused: QuotaExceededError/.test(l)), journal.slice(-6).join(' | '));
  });
}
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
