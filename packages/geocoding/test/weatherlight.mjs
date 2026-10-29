// LLP 1133.021 §13's geocoding drives 6 to 8 on Weatherlight, under the
// agent: a followed position named by `reverseGeocode`'s preferred answer.
//
// 6. Named: the Paris place preferred → the city reads "Paris", the region "France".
// 7. Unnamed: `[]`, then `{"error":"NetworkError"}` → "Current location",
//    and the journal names the failure.
// 8. Absent (the web, Linux): `geocodingStatus().available` false, no send,
//    "Current location".
// 9. Real, with --bind: the platform's geocoder names the preferred position
//    over Paris (macOS, iOS; needs the network).
//
// usage: bun packages/geocoding/test/weatherlight.mjs <macos|ios|web|linux> [--bind] [--browser chromium|firefox|webkit] [--sim <udid>]
//   (Weatherlight built for that host first)
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv[0] ?? 'macos';
const flag = (name) => argv.includes(name) ? argv[argv.indexOf(name) + 1] : null;
const browser = flag('--browser'), sim = flag('--sim'), bind = argv.includes('--bind');
if (sim) process.env.EXACT_SIM = sim;
process.env.EXACT_IDENTITY ??= '-';
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
let failures = 0;
const check = (name, ok, detail) => { if (!ok) failures++; console.log(`${ok ? 'PASS' : 'FAIL'} ${host}${browser ? `/${browser}` : ''} ${name}${ok ? '' : `: ${detail}`}`); };

const PARIS = '[{"latitude":48.8566,"longitude":2.3522,"city":"Paris","countryCode":"FR","country":"France"}]';
const HERE = 'geolocation.watchPosition={"coords":{"latitude":48.8566,"longitude":2.3522,"accuracy":50}}';

/** A launch that follows the preferred position with `geocoded` preferred for the place, then the slots and the journal. */
async function follow(n, geocoded) {
  const s = await open({ host, app: 'weatherlight', storage: `geocoding-drive-${n}`, ...(bind ? { bind: 'geocoding' } : {}), ...(browser ? { browser: browser === 'chromium' ? 'chrome' : browser } : {}) });
  try {
    await s.clock('settle');
    await s.prefer({ 'permission.geolocation': 'granted', extension: HERE });
    if (geocoded) await s.prefer({ extension: `geocoding.reverseGeocode=${geocoded}` });
    await s.tap('use-location');
    await s.clock('settle');
    const st = await s.op({ op: 'state' });
    return { slots: st.slots ?? {}, lines: (await s.logs()).lines ?? [] };
  } finally { await s.close?.(); }
}

const run = async (name, body) => { try { await body(); } catch (e) { check(`${name}: ran`, false, e.message); } };

if (bind) {
  await run('9', async () => {
    const r = await follow(9, null);
    check('9: the platform names Paris', r.slots.city === 'Paris', `${r.slots.city} ${r.lines.filter((l) => /geocod/i.test(l)).join(' | ')}`);
  });
} else if (host === 'web' || host === 'linux') {
  await run('8', async () => {
    const r = await follow(8, null);
    check('8: following', r.slots.following === true, JSON.stringify(r.slots));
    check('8: the label stays "Current location"', r.slots.city === 'Current location', r.slots.city);
    check('8: no reverseGeocode was sent', !r.lines.some((l) => /reverseGeocode/.test(l)), r.lines.filter((l) => /geocod/i.test(l)).join(' | '));
  });
} else {
  await run('6', async () => {
    const r = await follow(6, PARIS);
    check('6: the city reads Paris', r.slots.city === 'Paris', r.slots.city);
    check('6: the region reads France', r.slots.region === 'France', r.slots.region);
  });
  await run('7', async () => {
    const none = await follow('7-none', '[]');
    check('7: no place, "Current location"', none.slots.city === 'Current location', none.slots.city);
    const failed = await follow('7-error', '{"error":"NetworkError"}');
    check('7: a failure, "Current location"', failed.slots.city === 'Current location', failed.slots.city);
    check('7: the journal names the failure', failed.lines.some((l) => /NetworkError/.test(l)), failed.lines.slice(-8).join(' | '));
  });
}
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
