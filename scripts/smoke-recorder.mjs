// The recorder, driven (LLP 1133.009 §11): the microphone through
// @exact/media-capture and @exact/recording, each implementation its own
// substitute under the agent. `smoke.mjs` runs this for `--app recorder`: on
// the web, macOS and iOS, one session records on the agent's clock, its
// waveform draws the level's bars, and the takes it files read the same on
// every host (each label carries the take's average level, so the
// implementations' synthetic signals cannot drift); on `host`, two sessions
// of the sample host record from the one capture, each its own take, and one
// records on after the other is destroyed.
import { readFileSync, rmSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { decodePng } from './png.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// The synthetic signal's takes, the same on every host (pinned from the web).
export const TAKES = ['Take 1 · 3.0 s · level 52%', 'Take 2 · 1.5 s · level 27%'];

// Coloured waveform bars inside the waveform's box, in a screenshot: any pixel far from grey.
async function bars(s, path) {
  const shot = await s.screenshot(path);
  const png = decodePng(readFileSync(path));
  const box = (await s.layout()).nodes.find((n) => n.testId === 'waveform');
  if (!box) return 0;
  const scale = png.width / shot.w;
  let coloured = 0;
  for (let y = Math.round(box.y * scale); y < Math.round((box.y + box.h) * scale); y += 2) {
    for (let x = Math.round(box.x * scale); x < Math.round((box.x + box.w) * scale); x += 2) {
      const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
      if (Math.max(r, g, b) - Math.min(r, g, b) > 80) coloured += 1;
    }
  }
  return coloured;
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
const granted = (s) => s.prefer({ 'permission.microphone': 'granted' });

export async function recorderSmoke({ host, open, check: record_ }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record_(ok, `${host} recorder: ${what}`); };
  const t0 = Date.now();
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-recorder-'));
  const s = await open({ host, storage: `recorder-${randomUUID().slice(0, 8)}`, env: { EXACT_AGENT_STORAGE_FRESH: '1' } });
  try {
    await granted(s);
    await s.clock('settle');
    let st = await s.state();
    check(st.resources.mic?.available === true && st.resources.mic?.state === 'granted', `the microphone is available and granted: ${JSON.stringify(st.resources.mic)}`);
    const idle = await bars(s, resolve(tmp, 'idle.png'));
    check(idle === 0, `no bars before recording (${idle} coloured pixels)`);
    await record(s, 3000);
    st = await s.state();
    check(st.resources.rec?.state === 'recording' && st.resources.rec?.duration === 3000, `recording for 3 s of the agent's clock: ${JSON.stringify(st.resources.rec)}`);
    check(st.resources.level?.levels?.length === 60, `the level covers every interval of the jump: ${st.resources.level?.levels?.length} levels`);
    const live = await bars(s, resolve(tmp, 'live.png'));
    check(live > 200, `the waveform draws the level's bars (${live} coloured pixels)`);
    await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(TAKES.slice(0, 1)), `the first take reads the same on every host: ${JSON.stringify(await labels(s))}`);
    const extensions = (await s.state()).extensions;
    check(extensions?.['media-capture']?.agent === true && extensions?.recording?.agent === true, `each implementation is its own substitute: ${JSON.stringify(extensions)}`);
    await record(s, 1500);
    await stop(s);
    check(JSON.stringify(await labels(s)) === JSON.stringify(TAKES), `both takes read the same on every host: ${JSON.stringify(await labels(s))}`);
    await s.clock('settle');
    const media = (await s.state()).media ?? [];
    check(media.some((m) => Math.abs((m.state?.duration ?? 0) - 3) < 0.05) && media.some((m) => Math.abs((m.state?.duration ?? 0) - 1.5) < 0.05), `each take's file plays at its length: ${JSON.stringify(media.map((m) => m.state?.duration))}`);
  } finally {
    await s.close?.();
    rmSync(tmp, { recursive: true, force: true });
  }
  console.log(`${host} recorder: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (LLP 1133.009)`);
}

// Two sessions of the sample host: each its own tracks and recorders over the
// process's one capture; one records on after the other is destroyed (D3, D6).
export async function recorderSessions({ host, open }) {
  const failures = [];
  const check = (ok, what) => { if (!ok) failures.push(what); };
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-host-'));
  const control = resolve(dir, 'control');
  writeFileSync(control, '');
  const s = await open({ host, session: 'a', storage: `recorder-${randomUUID().slice(0, 8)}`, env: { EXACT_HOST_CONTROL: control, EXACT_AGENT_STORAGE_FRESH: '1' } });
  try {
    check(s.sessions?.join(',') === 'a,b', `the host announced sessions ${JSON.stringify(s.sessions)}, not a,b`);
    await granted(s); await s.clock('settle'); s.session = 'b'; await granted(s); await s.clock('settle');
    s.session = 'a'; await record(s, 3000);
    s.session = 'b';
    check((await s.state()).resources.rec?.state !== 'recording', 'b is idle while a records: its own recorders');
    await record(s, 1500);
    s.session = 'a'; await stop(s);
    check((await labels(s)).length >= 1, `a filed its take: ${JSON.stringify(await labels(s))}`);
    s.session = 'b';
    check((await s.state()).resources.rec?.state === 'recording', 'b is still recording after a stopped');
    writeFileSync(control, 'destroy a\n');
    await sleep(800);
    await s.clock('+500');
    check((await s.state()).resources.rec?.state === 'recording', 'b records on after a was destroyed');
    await stop(s);
    check((await s.state()).resources.rec?.state !== 'recording', `b stopped its own take: ${JSON.stringify((await s.state()).resources.rec)}`);
  } finally {
    await s.close?.();
    rmSync(dir, { recursive: true, force: true });
  }
  return failures;
}
