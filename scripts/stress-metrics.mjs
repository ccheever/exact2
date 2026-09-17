// @ref LLP 1041 §5 — callback/DOM observations, never a physical-display FPS claim.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { arch, cpus, platform, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from './agent.mjs';

export function summarize(values, target) {
  const a = values.filter(x => Number.isFinite(x) && x >= 0).toSorted((a, b) => a - b);
  const rank = q => a.length ? a[Math.max(0, Math.ceil(q * a.length) - 1)] : null;
  return { count: a.length, p50_ms: rank(.5), p95_ms: rank(.95), p99_ms: rank(.99),
    max_ms: a.at(-1) ?? null, over_target: a.filter(x => x > target).length };
}

export function stressOptions(args) {
  const read = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
  const url = new URL(read('--stress-url', ''));
  if (url.protocol !== 'http:' || !['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))
    throw new Error('--stress-url must be a loopback HTTP fixture, not a live app');
  const bounded = (key, fallback, min, max) => {
    const value = Number(read(key, fallback));
    if (!Number.isFinite(value) || value < min || value > max) throw new Error(`${key}: expected ${min}..${max}`);
    return value;
  };
  return { url: url.href, seconds: bounded('--seconds', 10, 1, 120),
    hz: bounded('--target-hz', 120, 1, 240), throttle: bounded('--cpu-throttle', 1, 1, 20),
    taps: args.flatMap((value, i) => value === '--tap' ? [args[i + 1]] : []),
    input: read('--input', 'stress-input'), echo: read('--echo', 'stress-echo') };
}

// Installed after ready and before workload controls. Samples are bounded.
function instrument(inputId, echoId) {
  const find = id => [...document.querySelectorAll('[data-testid]')].find(e => e.dataset.testid === id);
  const field = find(inputId), echo = find(echoId);
  if (!field || !echo) throw new Error('stress input and echo testIds are required');
  const state = { frames: [], inputs: [], seen_inputs: 0, unmatched_inputs: 0, dropped_samples: 0 };
  let previous, active = true, pending, frameId;
  const append = (array, value) => array.length < 30000 ? array.push(value) : state.dropped_samples++;
  const frame = now => {
    if (!active) return;
    if (previous != null) append(state.frames, now - previous);
    previous = now;
    frameId = requestAnimationFrame(frame);
  };
  const changed = () => {
    if (!pending || echo.textContent !== pending.value) return;
    const matched = pending; pending = null;
    matched.dom_ms = performance.now() - matched.start;
    requestAnimationFrame(() => {
      if (active) append(state.inputs, { dom_ms: matched.dom_ms, next_raf_ms: performance.now() - matched.start });
    });
  };
  const onInput = () => {
    if (pending) state.unmatched_inputs++;
    state.seen_inputs++;
    pending = { start: performance.now(), value: field.value };
    queueMicrotask(changed);
  };
  // Capture precedes the host's input listener/commit on the same event.
  field.addEventListener('input', onInput, true);
  const observer = new MutationObserver(changed);
  observer.observe(echo, { subtree: true, characterData: true, childList: true });
  frameId = requestAnimationFrame(frame);
  globalThis.__exactStress = { state, stop() {
    active = false; cancelAnimationFrame(frameId); observer.disconnect();
    field.removeEventListener('input', onInput, true);
    if (pending) state.unmatched_inputs++;
    return { ...state, dom_nodes: document.querySelectorAll('*').length,
      js_heap_bytes: performance.memory?.usedJSHeapSize ?? null,
      note: 'input-event to matching Contract echo DOM and following rAF; excludes OS input delivery, not physical presentation' };
  } };
}

export async function runStressMetrics(args) {
  const options = stressOptions(args);
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  if (!existsSync(chrome)) throw new Error('set CHROME to a Chrome executable');
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-stress-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run',
    '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows',
    '--no-default-browser-check', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const errors = [];
  const boundedError = text => { if (errors.length < 20) errors.push(text); };
  child.on('error', error => cdp.fail(error.message));
  child.on('exit', () => cdp.fail('stress Chrome exited'));
  try {
    const { targetInfos } = await cdp.send('Target.getTargets');
    const { sessionId } = await cdp.send('Target.attachToTarget', {
      targetId: targetInfos.find(x => x.type === 'page').targetId, flatten: true,
    });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async expression => {
      const response = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (response.exceptionDetails) throw new Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
      return response.result.value;
    };
    cdp.listeners.push(event => {
      if (event.method === 'Runtime.exceptionThrown') boundedError(event.params.exceptionDetails.text);
    });
    await call('Runtime.enable'); await call('Page.enable');
    await call('Page.bringToFront');
    await call('Emulation.setDeviceMetricsOverride', { width: 1200, height: 850, deviceScaleFactor: 1, mobile: false });
    await call('Emulation.setCPUThrottlingRate', { rate: options.throttle });
    await call('Page.navigate', { url: options.url });
    const deadline = Date.now() + 30000;
    while (!await evaluate('Boolean(globalThis.exact && document.querySelector("[data-testid]"))')) {
      if (Date.now() > deadline) throw new Error('fixture did not boot within 30s');
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    await evaluate('exact.ready');
    const controls = await evaluate('exact.root.innerText.slice(0, 1600)');
    await evaluate(`(${instrument.toString()})(${JSON.stringify(options.input)}, ${JSON.stringify(options.echo)})`);
    for (const id of options.taps) {
      const point = await evaluate(`(() => {
        const el = [...document.querySelectorAll('[data-testid]')].find(e => e.dataset.testid === ${JSON.stringify(id)});
        if (!el) throw Error('missing workload control ' + ${JSON.stringify(id)});
        el.scrollIntoView({ block: 'center' }); const r = el.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      })()`);
      await call('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...point });
      await call('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...point });
    }
    const start = Date.now();
    while (Date.now() - start < options.seconds * 1000) {
      await evaluate(`(() => {
        const el = [...document.querySelectorAll('[data-testid]')].find(e => e.dataset.testid === ${JSON.stringify(options.input)});
        if (!el) throw Error('stress input disappeared'); el.focus();
      })()`);
      await call('Input.insertText', { text: 'x' });
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    const sample = await evaluate('__exactStress.stop()');
    const interval = 1000 / options.hz;
    const result = { identity: { commit: spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim(),
      working_tree: spawnSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).stdout,
      platform: platform(), arch: arch(), cpu: cpus()[0]?.model, browser: await cdp.send('Browser.getVersion') },
      options, controls_before: controls, controls_after: await evaluate('exact.root.innerText.slice(0, 1600)'),
      observed_ms: Date.now() - start, target_interval_ms: interval,
      frame_callback_gaps: summarize(sample.frames, interval),
      input_to_echo_dom: summarize(sample.inputs.map(x => x.dom_ms), interval),
      input_to_echo_next_raf: summarize(sample.inputs.map(x => x.next_raf_ms), interval),
      sample, errors,
      limitation: 'Headless callback cadence is not display refresh or physical FPS. over_target counts intervals, not missed presentation deadlines. JS heap is not total/RSS/decoded-image memory. Compare idle and loaded runs; queue/execution spans are not instrumented here.' };
    console.log(JSON.stringify(result, null, 2));
    if (errors.length || sample.unmatched_inputs || !sample.inputs.length) process.exitCode = 1;
    return result;
  } finally {
    cdp.fail('stress run finished');
    if (child.exitCode == null) child.kill();
    await new Promise(resolve => {
      if (child.exitCode != null) return resolve();
      const timer = setTimeout(() => { child.kill('SIGKILL'); resolve(); }, 2000);
      child.once('exit', () => { clearTimeout(timer); resolve(); });
    });
    rmSync(profile, { recursive: true, force: true });
  }
}
