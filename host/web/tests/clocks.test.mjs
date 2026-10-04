// Synced animations in a real browser (LLP 1055.002): navigation.js's
// `animationClocks` sets each joined animation's `startTime` once, so two
// pulses that mounted apart share one phase; the CSS play state still
// pauses and resumes them, and a resumed one rejoins the phase.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const check = existsSync(chrome) ? test : test.skip;

const page = `<style>@keyframes pulse { from { opacity: 0.4 } to { opacity: 1 } }</style>
<div id="exact-root"></div>
<script type="module">
  import { animationClocks } from './navigation.js';
  const root = document.getElementById('exact-root'), clocks = animationClocks(root);
  window.add = (id, animation, clock = 'Pending') => {
    const el = document.createElement('div');
    el.id = id;
    el.style.cssText = 'animation:' + animation + ';' + (clock ? '--exact-animation-clock:' + clock + ';' : '');
    root.append(el);
    clocks.sync();
  };
  window.sync = () => clocks.sync();
  window.anim = id => document.getElementById(id).getAnimations()[0];
  window.ready = true;
</script>`;

check('pulses that mount apart share a phase, and a resumed one rejoins it', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-clocks-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    // Local time on the 1600 ms cycle (800 ms, alternate).
    // Several read in one evaluation, so a loaded machine cannot move the
    // page's clock between them.
    const phases = (...ids) => evaluate(`${JSON.stringify(ids)}.map(id => { const a = anim(id); return [a.currentTime % 1600, a.playState]; })`);
    const phase = async id => (await phases(id))[0];

    await evaluate(`add('lock', 'pulse 800ms ease-in-out infinite alternate')`);
    await Bun.sleep(300);
    await evaluate(`add('engine', 'pulse 800ms ease-in-out infinite alternate')`);
    await evaluate(`add('free', 'pulse 800ms ease-in-out infinite alternate', '')`);
    const [[lock], [engine], [free]] = await phases('lock', 'engine', 'free');
    expect(Math.abs(lock - engine)).toBeLessThan(1);
    expect(Math.abs(lock - free)).toBeGreaterThan(200); // no clock: CSS's own start
    // A lone pulse on an idle clock starts at its first keyframe.
    await evaluate(`add('alone', 'pulse 800ms infinite alternate', 'Other')`);
    expect((await phase('alone'))[0]).toBeLessThan(100);

    // The CSS play state still applies after `startTime` was set.
    await evaluate(`document.getElementById('engine').style.animationPlayState = 'paused'; sync()`);
    expect((await phase('engine'))[1]).toBe('paused');
    await Bun.sleep(250);
    await evaluate(`document.getElementById('engine').style.animationPlayState = 'running'; sync()`);
    const [[l2], [e2, state]] = await phases('lock', 'engine');
    expect(state).toBe('running');
    expect(Math.abs(l2 - e2)).toBeLessThan(1);

    // A lone member keeps its clock busy while paused: a joiner then, and
    // the member's own resume, both take the phase it started on.
    await evaluate(`add('solo', 'pulse 800ms infinite alternate', 'Solo')`);
    const origin = await evaluate(`anim('solo').startTime`);
    await Bun.sleep(200);
    await evaluate(`document.getElementById('solo').style.animationPlayState = 'paused'; sync()`);
    await Bun.sleep(300);
    await evaluate(`add('late', 'pulse 800ms infinite alternate', 'Solo')`);
    await evaluate(`document.getElementById('solo').style.animationPlayState = 'running'; sync()`);
    const [late, solo] = await evaluate(`[anim('late').startTime, anim('solo').startTime]`);
    expect(Math.abs((late - origin) % 1600)).toBeLessThan(1);
    expect(Math.abs((solo - origin) % 1600)).toBeLessThan(1);

    // A running pulse moved to another clock joins that one: a later
    // joiner there shares its start.
    await evaluate(`add('mover', 'pulse 800ms infinite alternate', 'Before')`);
    await Bun.sleep(300);
    await evaluate(`document.getElementById('mover').style.setProperty('--exact-animation-clock', 'After'); sync()`);
    await Bun.sleep(300);
    await evaluate(`add('after', 'pulse 800ms infinite alternate', 'After')`);
    const [mover, after] = await evaluate(`[anim('mover').startTime, anim('after').startTime]`);
    expect(Math.abs((after - mover) % 1600)).toBeLessThan(1);

    // A finite one joining late ends on a cycle boundary of the clock.
    await evaluate(`add('three', 'pulse 800ms 3 alternate')`);
    const ends = await evaluate(`(() => { const a = anim('three'), l = anim('lock'); return [(a.startTime - l.startTime) % 1600, a.effect.getComputedTiming().endTime]; })()`);
    expect(Math.abs(ends[0])).toBeLessThan(1);
    expect(ends[1]).toBeCloseTo(2400, 6);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 30_000);
