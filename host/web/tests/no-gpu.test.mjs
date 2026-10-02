import {afterAll, expect, test} from 'bun:test';
import {chmodSync, existsSync, mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {open} from '../../../scripts/agent.mjs';

const root = resolve(new URL('../../..', import.meta.url).pathname);
const defaultDist = resolve(root, 'host/web/dist');
const marker = dist => {
  try {
    const receipt = JSON.parse(readFileSync(resolve(dist, '.exact-build.json'), 'utf8'));
    return receipt.target ?? (existsSync(resolve(dist, 'app.wasm')) ? 'wasm' : null);
  }
  catch { return null; }
};
const distFor = target => {
  const configured = process.env[`EXACT_NO_GPU_${target.toUpperCase()}_DIST`];
  if (configured) return resolve(configured);
  return marker(defaultDist) === target ? defaultDist : null;
};

const originalChrome = process.env.CHROME;
let wrapperDir = null;
if (process.env.EXACT_NO_GPU_CHROME) process.env.CHROME = process.env.EXACT_NO_GPU_CHROME;
else if (originalChrome && existsSync(originalChrome)) {
  wrapperDir = mkdtempSync(resolve(tmpdir(), 'exact-no-gpu-chrome-'));
  const wrapper = resolve(wrapperDir, 'chrome');
  const quoted = `'${originalChrome.replaceAll("'", "'\\''")}'`;
  await Bun.write(wrapper, `#!/bin/sh\nexec ${quoted} --disable-gpu "$@"\n`);
  chmodSync(wrapper, 0o700);
  process.env.CHROME = wrapper;
}
afterAll(() => {
  if (originalChrome === undefined) delete process.env.CHROME; else process.env.CHROME = originalChrome;
  if (wrapperDir) rmSync(wrapperDir, {recursive:true, force:true});
});

const browserMissing = !process.env.CHROME || !existsSync(process.env.CHROME);
const timed = async (target, operation, run, timings) => {
  const started = performance.now();
  const reply = await run();
  const ms = performance.now() - started;
  timings.push(`${operation} ${ms.toFixed(1)} ms`);
  expect(ms, `${target} ${operation} must answer in under 2 s`).toBeLessThan(2000);
  return reply;
};

for (const target of ['js', 'wasm']) {
  const dist = distFor(target);
  const unavailable = browserMissing ? 'Chrome is missing' : !dist ? `${target} dist is missing` : marker(dist) !== target ? `${dist} is not a ${target} dist` : null;
  if (unavailable) console.warn(`SKIP no-GPU ${target}: ${unavailable}`);
  const check = unavailable ? test.skip : test.serial;

  check(`Caltrain stays responsive without a GPU on the ${target} target`, async () => {
    const session = await open({host:'web', app:'caltrain', webDist:dist});
    const timings = [];
    const screenshot = resolve(tmpdir(), `exact-no-gpu-${target}-${process.pid}.png`);
    try {
      await timed(target, 'tap change-station', () => session.tap('change-station'), timings);
      for (let round = 1; round <= 3; round++) {
        await timed(target, `round ${round} clock +1000`, () => session.clock('+1000'), timings);
        await timed(target, `round ${round} clock settle`, () => session.clock('settle'), timings);
        await timed(target, `round ${round} screenshot`, () => session.screenshot(screenshot), timings);
        expect(existsSync(screenshot)).toBe(true);
      }
      const recovery = await session.carrier.evaluate('exact.gpu?.recovery ?? null');
      expect(recovery?.status).toBe('no device');
      expect(recovery?.code).toBe('no-adapter');
      const pendingReply = await session.carrier.evaluate(`(async () => {
        const settled = exact.gpu.settled;
        exact.gpu.settled = async () => [{name:'GPU recovery fixture'}];
        try { return await exact.agentSettled({op:'state'}); }
        finally { exact.gpu.settled = settled; }
      })()`);
      expect(pendingReply.error).toContain('GPU is not settled');
      expect(pendingReply.pending).toEqual(['GPU recovery fixture']);
      console.info(`no-GPU ${target}: ${timings.join(' · ')}`);
    } finally {
      await session.close();
      rmSync(screenshot, {force:true});
    }
  }, 60000);
}
