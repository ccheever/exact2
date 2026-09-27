// The recorder, driven (LLP 1067.000): one native object with a view and
// functions, on every host through the agent's operations. `smoke.mjs` runs
// this for `--app recorder`: on the web, macOS and iOS, one session records
// on the agent's clock, its waveform draws from the object while TypeScript
// sees only coarse state, and the takes it saves read the same on every
// host (the labels carry the average level, so the page module and the
// Swift module cannot drift); on `host`, two sessions of the sample host keep
// their own recorders, and one outlives the other's destruction.
import { readFileSync, rmSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { decodePng } from './png.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// The synthetic input's takes, the same on every host (pinned from the web).
export const TAKES = ['Take 1 · 3.0 s · level 52%', 'Take 2 · 1.5 s · level 51%'];

// Red waveform bars inside the waveform's box, in a screenshot.
async function bars(s, path) {
  const shot = await s.screenshot(path);
  const png = decodePng(readFileSync(path));
  const box = (await s.layout()).nodes.find((n) => n.testId === 'waveform');
  if (!box) return 0;
  const scale = png.width / shot.w;
  let red = 0;
  for (let y = Math.round(box.y * scale); y < Math.round((box.y + box.h) * scale); y += 2) {
    for (let x = Math.round(box.x * scale); x < Math.round((box.x + box.w) * scale); x += 2) {
      const i = (y * png.width + x) * 4;
      if (png.data[i] > 180 && png.data[i + 1] < 110 && png.data[i + 2] < 110) red += 1;
    }
  }
  return red;
}

async function record(s, ms) {
  await s.tap('toggle');
  await s.clock('settle');
  await s.clock(`+${ms}`);
}

async function stop(s) {
  await s.tap('toggle');
  await s.clock('settle');
}

const labels = async (s) => ((await s.state()).resources.library?.takes ?? []).map((t) => t.label);

export async function recorderSmoke({ host, open, check: record_ }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record_(ok, `${host} recorder: ${what}`); };
  const t0 = Date.now();
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-recorder-'));
  const s = await open({ host });
  try {
    await s.clock('settle');
    let st = await s.state();
    check(st.resources.status?.available === true && st.resources.status?.recording === false, `the app module is available and idle: ${JSON.stringify(st.resources.status)}`);
    const idle = await bars(s, resolve(tmp, 'idle.png'));
    check(idle === 0, `no bars before recording (${idle} red pixels)`);
    await record(s, 3000);
    st = await s.state();
    check(st.resources.status?.recording === true, `recording after the tap: ${JSON.stringify(st.resources.status)}`);
    const live = await bars(s, resolve(tmp, 'live.png'));
    check(live > 200, `the waveform draws the live take from the object (${live} red pixels)`);
    // Three seconds of levels at 20 a second never cross the source: it was
    // asked for its status at boot and once when recording began. Apple
    // answers the status with native.call (no request at all); the web has
    // no synchronous call, so its source falls back to native.later.
    const asks = (await s.logs()).lines.filter((l) => /request \d+ \(status\): POST exact-native:/.test(l)).length;
    check(host === 'web' ? asks >= 1 && asks <= 2 : asks === 0, `the status is coarse, and answered by ${host === 'web' ? 'native.later' : 'native.call'} (${asks} status requests)`);
    await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(TAKES.slice(0, 1)), `the first take reads the same on every host: ${JSON.stringify(await labels(s))}`);
    await record(s, 1500);
    await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(TAKES), `both takes read the same on every host: ${JSON.stringify(await labels(s))}`);
  } finally {
    await s.close?.();
    rmSync(tmp, { recursive: true, force: true });
  }
  console.log(`${host} recorder: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (LLP 1067.000)`);
}

// Two sessions of the sample host: each its own module instance and clock;
// one keeps recording after the other is destroyed (LLP 1067.000 Q6).
export async function recorderSessions({ host, open }) {
  const failures = [];
  const check = (ok, what) => { if (!ok) failures.push(what); };
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-host-'));
  const control = resolve(dir, 'control');
  writeFileSync(control, '');
  const s = await open({ host, session: 'a', env: { EXACT_HOST_CONTROL: control } });
  try {
    check(s.sessions?.join(',') === 'a,b', `the host announced sessions ${JSON.stringify(s.sessions)}, not a,b`);
    await s.clock('settle'); s.session = 'b'; await s.clock('settle');
    s.session = 'a'; await record(s, 3000);
    s.session = 'b';
    check((await s.state()).resources.status?.recording === false, 'b is idle while a records: its own instance');
    await record(s, 1500);
    s.session = 'a'; await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(TAKES.slice(0, 1)), `a saved its own take: ${JSON.stringify(await labels(s))}`);
    s.session = 'b';
    check((await s.state()).resources.status?.recording === true, 'b is still recording after a stopped');
    await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(['Take 1 · 1.5 s · level 51%']), `b saved its own take on its own clock: ${JSON.stringify(await labels(s))}`);
    writeFileSync(control, 'destroy a\n');
    await sleep(800);
    await record(s, 1500);
    await stop(s);
    check((await labels(s)).length === 2, `b records after a was destroyed: ${JSON.stringify(await labels(s))}`);
  } finally {
    await s.close?.();
    rmSync(dir, { recursive: true, force: true });
  }
  return failures;
}
