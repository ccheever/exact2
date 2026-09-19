#!/usr/bin/env bun
/**
 * metrics — startup and speed numbers from one captured-source run.
 * Builds start with a private cache. Diagnostic, never blocking (rules/RULES.md §Loop shape:
 * run everything, block on almost nothing).
 *
 *   bun scripts/metrics.mjs            table
 *   bun scripts/metrics.mjs --json     one JSON object
 *   bun scripts/metrics.mjs --app <name> measure that resolved app
 *   bun scripts/metrics.mjs --scaling  runner workloads (300/3000/10000 rows), no browser
 *   bun scripts/metrics.mjs --list-memory  fresh-process eager-list heap/RSS baseline (25/1000/25000)
 *   bun scripts/metrics.mjs --list-memory --collections --repeats 3 --json
 *       paired eager/virtualized rows, actual kernel geometry, twenty full traversals
 *   bun scripts/metrics.mjs --stress-url http://127.0.0.1:PORT --seconds 10 --target-hz 120
 *       sample a local fixture; repeat --tap <testId> to start workload controls
 *   bun scripts/metrics.mjs --interaction <testId> first browser action to measure
 *   bun scripts/metrics.mjs --rebuild  also time an app edit → wasm rebuild (the cold path)
 *   bun scripts/metrics.mjs --long     also the macOS host: an initial build, a touch-one-line
 *                                       rebuild, and the app's boot phases (minutes, not seconds)
 *
 * Budgets are read from rules/RULES.md so they cannot drift from the prose.
 */
import { spawnSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { arch, cpus, platform, release, tmpdir, totalmem } from 'node:os';
import { gzipSync } from 'node:zlib';
import { dirname, resolve } from 'node:path';
import { publicFileCards, readStaticFile, webContentType } from '../host/web/serve.mjs';
import { appleArtifacts, assertAppleIdentity } from '../host/apple/build.mjs';
import { developmentBuildEnv, resolveApp, withAppFixture } from './app.mjs';
import { Cdp } from './agent.mjs';

const t0 = Date.now();
const ROOT = resolve(new URL('..', import.meta.url).pathname);
// Explicitly sample an already-running local stress fixture. This mode records
// its live source state and does not claim the private-capture build guarantee.
if (process.argv.includes('--stress-url')) {
  const { runStressMetrics } = await import('./stress-metrics.mjs');
  await runStressMetrics(process.argv.slice(2));
  process.exit(process.exitCode ?? 0);
}
const appName = process.argv.includes('--app') ? process.argv[process.argv.indexOf('--app') + 1] : undefined;
const app = resolveApp(appName);
// The child uses the captured scripts and inputs; only this invocation's
// resolved root bypasses capture. Foreign inherited markers cannot do so.
if (!process.argv.includes('--scaling') && !process.argv.includes('--list-memory') && process.env.EXACT_DIAGNOSTIC_ROOT !== ROOT) {
  const code = await withAppFixture(app, async ({ exactRoot, env }) => {
    // The captured source excludes node_modules. Resolve the pinned toolchain
    // inside this private checkout, rather than borrowing the live workspace.
    const installed = spawnSync(process.execPath, ['install', '--frozen-lockfile'],
      { cwd: exactRoot, env, encoding: 'utf8' });
    if (installed.error || installed.status !== 0) throw new Error(`diagnostic bun install --frozen-lockfile: ${installed.error?.message ?? installed.stderr}`);
    const child = spawn(process.execPath, [resolve(exactRoot, 'scripts/metrics.mjs'), ...process.argv.slice(2)],
      { cwd: exactRoot, env, stdio: 'inherit' });
    return await new Promise((done, fail) => { child.once('error', fail); child.once('exit', (code) => done(code ?? 1)); });
  });
  process.exit(code);
}
const json = process.argv.includes('--json');
const rebuild = process.argv.includes('--rebuild');
const long = process.argv.includes('--long');
const rules = readFileSync(resolve(ROOT, 'rules/RULES.md'), 'utf8');
const budget = (label) => rules.match(new RegExp(`\\|\\s*${label}[^|]*\\|\\s*([^|\\n]+)`, 'i'))?.[1].trim() ?? '?';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const out = { identity: { commit: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' }).stdout.trim(),
  platform: platform(), release: release(), arch: arch(), cpu: cpus()[0]?.model,
  source_diff_sha256: sha256(spawnSync('git', ['diff', 'HEAD', '--', '*.rs', '*.mjs', '*.js', 'Cargo.*'], { cwd: ROOT }).stdout),
  rustc: spawnSync('rustc', ['--version'], { encoding: 'utf8' }).stdout.trim() } };
if (process.env.EXACT_DIAGNOSTIC_ROOT === ROOT) {
  const source = JSON.parse(process.env.EXACT_DIAGNOSTIC_SOURCE);
  out.source_capture_s = (t0 - source.started) / 1000;
  // The private Git commit already contains every captured working edit.
  // Its empty diff cannot describe the original source; the snapshot does.
  delete out.identity.source_diff_sha256;
  out.identity = { ...out.identity, commit: source.sources.find(s => s.roles.includes('exact2'))?.commit,
    app: source.app, source_snapshot: source.snapshot, sources: source.sources };
}
// Opt-in large workloads run in the existing metrics binary. No browser or
// rebuild is needed to compare runner algorithms on one fixed machine.
if (process.argv.includes('--list-memory') && process.argv.includes('--collections')) {
  if (process.argv.includes('--scaling')) throw new Error('choose collections or scaling');
  const repeats = process.argv.includes('--repeats') ? Number(process.argv[process.argv.indexOf('--repeats') + 1]) : 3;
  if (!Number.isInteger(repeats) || repeats < 1 || repeats > 10) throw new Error('--repeats must be 1..10');
  const env = { ...developmentBuildEnv(), EXACT_APP_DIR: resolve(ROOT, 'apps/caltrain') };
  // Include untracked modules: git diff alone omits a new implementation until staged.
  const sourceDigest = () => {
    const listed = spawnSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', '*.rs', '*.mjs', '*.js', '*.json', '*.contract', 'Cargo.*'], { cwd: ROOT, encoding: 'utf8' });
    if (listed.status !== 0) throw new Error(listed.stderr);
    const digest = createHash('sha256');
    for (const path of [...new Set(listed.stdout.split('\0').filter(Boolean))].sort()) {
      const file = resolve(ROOT, path);
      if (existsSync(file)) digest.update(path).update('\0').update(readFileSync(file)).update('\0');
    }
    return digest.digest('hex');
  };
  const compilerProcesses = () => {
    const result = spawnSync('ps', ['-axo', 'comm='], { encoding: 'utf8' });
    return result.status === 0 ? result.stdout.trim().split('\n').map(s => s.trim())
      .filter(s => /(^|\/)(cargo|rustc|swiftc|swift-frontend|clang|clang\+\+|cc1|ld)$/.test(s)) : null;
  };
  out.identity.physical_memory_bytes = totalmem();
  out.identity.logical_cpus = cpus().length;
  out.identity.source_files_sha256_before_build = sourceDigest();
  const built = spawnSync('cargo', ['build', '--locked', '-q', '--release', '-p', 'caltrain-web', '--bin', 'metrics'], { cwd: ROOT, encoding: 'utf8', env });
  if (built.status !== 0) { console.error(built.error?.message ?? built.stderr); process.exit(built.status ?? 1); }
  const binary = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'release/metrics');
  out.identity.binary_sha256 = sha256(readFileSync(binary));
  out.identity.source_files_sha256_after_build = sourceDigest();
  out.identity.source_changed_during_build = out.identity.source_files_sha256_before_build !== out.identity.source_files_sha256_after_build;
  const sample = (count, mode, repeat) => new Promise((done, fail) => {
    console.error(`collection metrics: ${count} rows, ${mode}, repeat ${repeat + 1}/${repeats}`);
    const child = spawn(binary, ['--collection-memory', String(count), mode, '--hold'], { cwd: ROOT, env, stdio: ['pipe', 'pipe', 'pipe'] });
    let stdout = '', stderr = '', failure;
    const phases = [];
    const timer = setTimeout(() => { failure = new Error(`collection ${count}/${mode}: exceeded 60 s`); child.kill(); }, 60000);
    child.once('error', error => { clearTimeout(timer); fail(error); });
    child.stdin.on('error', error => { failure ??= error; child.kill(); });
    child.stderr.on('data', bytes => { stderr = (stderr + bytes).slice(-8192); });
    child.stdout.on('data', bytes => {
      stdout += bytes;
      while (!failure && stdout.includes('\n')) {
        const split = stdout.indexOf('\n'); const line = stdout.slice(0, split); stdout = stdout.slice(split + 1);
        try {
          const phase = JSON.parse(line);
          const rss = ['darwin', 'linux'].includes(platform())
            ? spawnSync('ps', ['-o', 'rss=', '-p', String(child.pid)], { encoding: 'utf8', timeout: 5000 }) : null;
          const kib = Number(rss?.stdout?.trim());
          phase.process_rss_bytes = rss?.status === 0 && kib > 0 ? kib * 1024 : null;
          phase.compiler_processes_at_sample = compilerProcesses();
          phases.push(phase); child.stdin.write('\n');
        } catch (error) { failure = error; child.kill(); }
      }
    });
    child.once('close', code => {
      clearTimeout(timer);
      if (failure || code !== 0 || phases.at(-1)?.phase !== 'runner_dropped') fail(failure ?? new Error(`collection ${count}/${mode} exited ${code}: ${stderr}`));
      else done({ count, mode, repeat, phases });
    });
  });
  out.collection_memory = [];
  for (const count of [25, 1000, 25000]) for (let repeat = 0; repeat < repeats; repeat++) {
    // Alternate ordering across repetitions; every cell still gets a fresh process.
    for (const mode of repeat % 2 ? ['virtualized', 'eager'] : ['eager', 'virtualized']) out.collection_memory.push(await sample(count, mode, repeat));
  }
  out.identity.source_files_sha256_after_measurements = sourceDigest();
  out.identity.source_changed_during_measurements = out.identity.source_files_sha256_after_build !== out.identity.source_files_sha256_after_measurements;
  out.collection_memory_note = 'Same release binary; fresh process per N/mode/repeat; fixed 390x800 nested scrollport inside 390x844 kernel viewport; distinct numeric records, one text root and one owned state slot per row. Virtualized feedback uses actual kernel wrapper heights and no pins; eager scrolling requires no runner calls or new layout (reported zero work, not host frame cost). Twenty traversals means top-bottom-top in viewport-sized steps; a fitting 25-row document has no scroll distance. Action samples include the authored action plus any post-layout collection feedback needed to settle. Per-phase raw synchronous runner-call, kernel-layout and driver elapsed times are milliseconds, not OS thread CPU counters or physical presentation. Tracked heap is System requested bytes since the post-compile/pre-data baseline, including O(N) records/key-height metadata and runtime transients; encoded input, diagnostic buffers, allocator slack and internal realloc transients are excluded. RSS is ps process resident memory and includes diagnostic buffers; it is not Apple physical footprint. Local-slot counts are live authored row roots times the verified one owned slot in the template. Native host views, decoded raster memory and first pixel are unmeasured. Compiler process samples are boundary observations, not proof of an otherwise idle machine. Source hashes describe the live workspace (including untracked source), not a hermetic capture; binary SHA identifies the measured executable.';
  if (json) console.log(JSON.stringify(out));
  else {
    console.log(`Collection runner metrics — ${JSON.stringify(out.identity)}`);
    for (const cell of out.collection_memory) {
      const phase = name => cell.phases.find(p => p.phase === name);
      const settled = phase('twenty_traversals');
      console.log(`  ${cell.count} ${cell.mode} #${cell.repeat + 1}: ${settled.live_row_instances} rows / ${settled.live_kernel_nodes} nodes; heap ${settled.retained_heap_delta_bytes} B; RSS ${settled.process_rss_bytes ?? 'unmeasured'} B; input p50 ${phase('input_echo').runner_call_ms.p50}; body p50 ${phase('all_row_bodies').runner_call_ms.p50} ms`);
    }
    console.log(out.collection_memory_note);
  }
  process.exit(0);
}
if (process.argv.includes('--list-memory')) {
  if (process.argv.includes('--scaling')) throw new Error('choose --list-memory or --scaling');
  const env = { ...developmentBuildEnv(), EXACT_APP_DIR: resolve(ROOT, 'apps/caltrain') };
  const built = spawnSync('cargo', ['build', '-q', '--release', '-p', 'caltrain-web', '--bin', 'metrics'],
    { cwd: ROOT, encoding: 'utf8', env });
  if (built.status !== 0) { console.error(built.error?.message ?? built.stderr); process.exit(built.status ?? 1); }
  const binary = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'release/metrics');
  out.identity.binary_sha256 = sha256(readFileSync(binary));
  const sample = (count, windowed) => new Promise((done, fail) => {
    const child = spawn(binary, ['--list-memory', String(count), '--hold', ...(windowed ? ['--windowed'] : [])], { cwd: ROOT, env, stdio: ['pipe', 'pipe', 'pipe'] });
    let stdout = '', stderr = '', result, failure;
    const timer = setTimeout(() => { failure = new Error(`list memory: ${count} rows exceeded 60 s`); child.kill(); }, 60000);
    child.once('error', error => { clearTimeout(timer); fail(error); });
    child.stdin.on('error', error => { failure ??= error; child.kill(); });
    child.stderr.on('data', bytes => { stderr = (stderr + bytes).slice(-8192); });
    child.stdout.on('data', bytes => {
      stdout += bytes;
      if (result || failure || !stdout.includes('\n')) return;
      try {
        result = JSON.parse(stdout.slice(0, stdout.indexOf('\n')));
        // Both Darwin and Linux ps report this column in KiB. This is RSS,
        // not Apple's phys_footprint and not a decoded-image measurement.
        const rss = ['darwin', 'linux'].includes(platform())
          ? spawnSync('ps', ['-o', 'rss=', '-p', String(child.pid)], { encoding: 'utf8', timeout: 5000 }) : null;
        const kib = Number(rss?.stdout?.trim());
        result.process_rss_bytes = rss?.status === 0 && kib > 0 ? kib * 1024 : null;
        result.rss_note = result.process_rss_bytes == null ? 'unmeasured: ps RSS unavailable' : 'ps RSS while the runner is alive; process total, not phys_footprint';
        child.stdin.end('\n');
      } catch (error) { failure = error; child.kill(); }
    });
    child.once('close', code => {
      clearTimeout(timer);
      if (failure || code !== 0 || !result) fail(failure ?? new Error(`list memory: ${count} rows exited ${code}: ${stderr}`));
      else done(result);
    });
  });
  out.list_memory = [];
  for (const count of [25, 1000, 25000]) for (const windowed of [false, true]) out.list_memory.push(await sample(count, windowed));
  out.list_memory_note = 'One fresh process per size/mode; one text leaf per row, monospace layout. Windowed rows add a fixed-height wrapper; measured at an 844px scrollport with one viewport of overscan each side. Windowed retained heap/RSS includes twenty complete down/up traversals; initial_retained_heap_bytes is before traversal; first_traversal_retained_heap_bytes is after one traversal at the same middle position. Live nodes are sampled in the middle. Input data and key/index metadata remain O(N). Heap is net System allocator requested bytes after compilation (data + decoded plan + runner + kernel); peak excludes allocator-internal realloc transients. Encoded input, allocator slack, stacks and host allocations are excluded from heap, included where resident in RSS. First pixel, native views and decoded image bytes are unmeasured. Memory tracking adds allocator overhead to these construction timings; this is not a frame-rate benchmark.';
  if (json) console.log(JSON.stringify(out));
  else {
    console.log(`List memory comparison — ${JSON.stringify(out.identity)}`);
    for (const r of out.list_memory) console.log(`  ${r.rows} rows / ${r.mode}: ${r.live_kernel_nodes} nodes; data ${r.data_heap_bytes} B; plan ${r.decoded_plan_heap_bytes} B; retained heap ${r.retained_heap_delta_bytes} B; peak ${r.peak_heap_delta_bytes} B; RSS ${r.process_rss_bytes ?? 'unmeasured'} B; boot ${r.runner_boot_ms} ms; layout ${r.layout_ms} ms`);
    console.log(out.list_memory_note);
  }
  process.exit(0);
}
if (process.argv.includes('--scaling')) {
  const run = spawnSync('cargo', ['run', '-q', '--release', '-p', 'caltrain-web', '--bin', 'metrics', '--', '--scaling'],
    { cwd: ROOT, encoding: 'utf8', env: { ...developmentBuildEnv(), EXACT_APP_DIR: resolve(ROOT, 'apps/caltrain') } });
  if (run.status !== 0) { console.error(run.stderr); process.exit(run.status ?? 1); }
  Object.assign(out, JSON.parse(run.stdout.trim().split('\n').pop()));
  out.scaling_fixture = { app: 'caltrain', workload: 'fixed synthetic runner rows; independent of --app and EXACT_APP_DIR' };
  out.identity.binary_sha256 = sha256(readFileSync(resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'release/metrics')));
  if (json) console.log(JSON.stringify(out));
  else {
    console.log(`Caltrain fixed synthetic runner scaling — ${JSON.stringify(out.identity)}`);
    for (const r of out.scaling) console.log(`  ${r.rows} rows / ${r.action}: runner ${r.runner_update_ms.p50}/${r.runner_update_ms.p95} ms p50/p95; layout ${r.layout_ms.p50}; web+runner ${r.web_runner_and_batch_ms.p50}; requests ${r.source_requests.p50}; touched ${r.touched.p50}`);
    console.log(out.scaling_note);
  }
  process.exit(0);
}
// The live dev-loop session reuses one Chrome profile (a fresh profile's
// first launch can stall for seconds); the --dump-dom render below gets a
// fresh one each time (with a reused profile it waits out its whole
// timeout). Every server here sends no-store, so nothing is cached.
const profile = resolve(ROOT, 'target/exact-chrome-profile', app.id.replace(/[^a-zA-Z0-9._-]/g, '_'));
mkdirSync(profile, { recursive: true });
const step = (name, f) => { const t = Date.now(); const v = f(); out[`_${name}_s`] = (Date.now() - t) / 1000; return v; };

// 1. Native pipeline numbers (a release bin; warm cache builds in ~1 s).
step('native', () => {
  const source = resolve(app.dir, 'web/src/bin/metrics.rs');
  if (!existsSync(source)) {
    Object.assign(out, Object.fromEntries(['compile_ms', 'bake_ms', 'decode_ms', 'plan_bytes', 'baked_bytes', 'boot_ms', 'nodes', 'text_nodes', 'layout_ms', 'update_ms', 'inherit_ms', 'inherit_touched', 'tick_ms', 'web_boot_ms', 'web_first_batch_bytes', 'web_update_ms', 'web_update_batch_bytes'].map((key) => [key, NaN])));
    out.native_note = `${app.name} has no web/src/bin/metrics.rs`; // the browser/dev rows below still measure the resolved app
    return;
  }
  const r = spawnSync('cargo', ['run', '-q', '--release', '-p', app.crate('web'), '--bin', 'metrics'], { cwd: app.workspace, encoding: 'utf8', env: developmentBuildEnv() });
  if (r.status !== 0) { console.error(r.stderr); process.exit(1); }
  Object.assign(out, JSON.parse(r.stdout.trim().split('\n').pop()));
});

// 2. The wasm, raw and gzipped — always rebuilt (a warm build is ~1.5 s),
// so every number below is for the code as it is now.
step('wasm', () => {
  const dist = resolve(ROOT, 'host/web/dist');
  const b = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.crate('web')], { cwd: ROOT, stdio: ['ignore', 'ignore', 'inherit'] });
  if (b.status !== 0) process.exit(b.status ?? 1);
  out.web_artifacts = publicFileCards(dist);
  out.web_artifact_id = sha256(JSON.stringify(out.web_artifacts));
  const wasm = readFileSync(resolve(dist, 'app.wasm'));
  out.wasm_bytes = wasm.length;
  out.wasm_gzip_bytes = gzipSync(wasm, { level: 9 }).length;
  const glue = readFileSync(resolve(dist, 'glue.js'));
  out.glue_bytes = glue.length;
  if (existsSync(resolve(dist, 'gpu_bg.wasm'))) {
    const g = readFileSync(resolve(dist, 'gpu_bg.wasm'));
    out.gpu_wasm_bytes = g.length;
    out.gpu_wasm_gzip_bytes = gzipSync(g, { level: 9 }).length;
    out.gpu_glue_bytes = readFileSync(resolve(dist, 'gpu.js')).length;
  }
});

// 3. Boot modules (the fifth check's count).
step('boot', () => {
  const r = spawnSync(process.execPath, [resolve(ROOT, 'scripts/boot.mjs'), '--json'], { cwd: ROOT, encoding: 'utf8' });
  out.boot = JSON.parse(r.stdout);
  out.boot_modules = out.boot.modules;
  out.boot_ok = r.status === 0;
});

// 4. A real browser. Instrumentation is installed by CDP only for this
// diagnostic run: shipped HTML/glue/wasm stay byte-identical to the build.
// Paint Timing is navigation-relative; the old rAF stamp is not a paint.
{
  const t = Date.now();
  const dist = resolve(ROOT, 'host/web/dist');
  const served = new Map();
  const server = createServer((req, res) => {
    const found = readStaticFile(dist, req.url.split('?')[0]);
    if (!found) { res.writeHead(404); res.end(); return; }
    const body = found.body;
    served.set(found.route, { path: found.route, bytes: body.length, sha256: sha256(body) });
    res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
    res.end(body);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  let child, fresh;
  try {
    if (!existsSync(chrome)) throw new Error('no Chrome at $CHROME');
    fresh = mkdtempSync(resolve(tmpdir(), 'exact-metrics-'));
    child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${fresh}`,
      '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run',
      '--no-default-browser-check', 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    child.on('exit', () => cdp.fail('metrics Chrome exited'));
    const { targetInfos } = await cdp.send('Target.getTargets');
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find((x) => x.type === 'page').targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
      return result.result.value;
    };
    out.browser_version = await cdp.send('Browser.getVersion');
    await call('Page.enable');
    await call('Page.bringToFront');
    await call('Performance.enable');
    await call('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: false });
    await call('Page.addScriptToEvaluateOnNewDocument', { source: `(${function instrument() {
      const m = globalThis.__exactMetrics = { longtasks: [], wasm_instantiations: [] };
      new PerformanceObserver((entries) => {
        for (const e of entries.getEntries()) m.longtasks.push({ start_ms: e.startTime, duration_ms: e.duration });
      }).observe({ type: 'longtask', buffered: true });
      const instantiate = WebAssembly.instantiateStreaming;
      WebAssembly.instantiateStreaming = async function(...args) {
        const start = performance.now();
        const value = await instantiate.apply(this, args);
        m.wasm_instantiations.push({ start_ms: start, duration_ms: performance.now() - start });
        return value;
      };
      new MutationObserver(() => {
        const root = document.getElementById('exact-root');
        if (m.dom_navigation_ms == null && root?.dataset.bootMs != null) {
          m.dom_navigation_ms = performance.now();
          m.content_text_characters = root.innerText.trim().length;
          m.content_dom_ms = m.content_text_characters ? m.dom_navigation_ms : null;
        }
      }).observe(document, { subtree: true, childList: true, attributes: true });
    }.toString()})()` });
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/?agent=1` });
    await evaluate(`new Promise((resolve, reject) => {
      const start = performance.now();
      const check = () => {
        const root = document.getElementById('exact-root');
        if (root?.dataset.error) return reject(new Error(root.dataset.error));
        if (root?.dataset.bootMs) return requestAnimationFrame(() => requestAnimationFrame(resolve));
        if (performance.now() - start > 10000) return reject(new Error('no first DOM frame'));
        setTimeout(check, 10);
      }; check();
    })`);
    const startup = await evaluate(`({ ...__exactMetrics,
      script_to_dom_ms: Number(exact.root.dataset.bootMs),
      script_to_raf_ms: Number(exact.root.dataset.frameCallbackMs),
      paints: performance.getEntriesByType('paint').map(e => ({ name: e.name, start_ms: e.startTime })),
      resources: performance.getEntriesByType('resource').map(e => ({ path: new URL(e.name).pathname,
        start_ms: e.startTime, duration_ms: e.duration, bytes: e.decodedBodySize, initiator: e.initiatorType })) })`);
    const paint = startup.paints.find(p => p.name === 'first-paint')?.start_ms;
    for (const resource of startup.resources) {
      resource.phase = resource.start_ms <= startup.dom_navigation_ms ? 'before first DOM commit'
        : paint != null && resource.start_ms > paint ? 'after observed first paint'
        : 'after DOM; paint order unconfirmed';
    }
    startup.initial_js_and_wasm_bytes = startup.resources
      .filter(r => r.start_ms <= startup.dom_navigation_ms && /\.(?:js|wasm)$/.test(r.path))
      .reduce((bytes, r) => bytes + r.bytes, 0);
    startup.data_executor = 'this web build has no TypeScript executor; Rust data is linked in app.wasm';
    out.browser_startup = startup;
    out.browser_dom_ms = startup.script_to_dom_ms;
    out.browser_paint_ms = startup.paints.find((p) => p.name === 'first-paint')?.start_ms ?? NaN;
    out.browser_contentful_paint_ms = startup.paints.find((p) => p.name === 'first-contentful-paint')?.start_ms ?? NaN;
    out.browser_performance = (await call('Performance.getMetrics')).metrics;
    // The sample's first useful action. Other apps name a testId explicitly;
    // no random button is activated merely to produce a timing number.
    const interaction = process.argv.includes('--interaction') ? process.argv[process.argv.indexOf('--interaction') + 1]
      : app.name === 'caltrain' ? 'change-station' : null;
    if (interaction) {
      const point = await evaluate(`(() => {
        const el = [...document.querySelectorAll('[data-testid]')].find(e => e.dataset.testid === ${JSON.stringify(interaction)});
        if (!el) throw new Error('missing interaction target');
        const r = el.getBoundingClientRect();
        __exactMetrics.interaction = { target: ${JSON.stringify(interaction)} };
        document.addEventListener('click', () => {
          __exactMetrics.interaction.input_ms = performance.now();
          const observer = new MutationObserver(() => {
            __exactMetrics.interaction.changed_dom_ms = performance.now(); observer.disconnect();
          }); observer.observe(exact.root, { subtree: true, childList: true, attributes: true, characterData: true });
        }, { capture: true, once: true });
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      })()`);
      await call('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...point });
      await call('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...point });
      out.browser_first_interaction = await evaluate(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve({
        ...__exactMetrics.interaction, frame_opportunity_ms: performance.now(),
        note: 'real CDP click; DOM change observed, frame opportunity is not confirmed presentation'
      }))))`);
    } else out.browser_interaction_note = 'unmeasured: name --interaction <testId> for this app';
    out.browser_served = [...served.values()];
    out.browser_startup_note = 'headless Chrome, empty profile, localhost/no-store; instrumented single sample; content is nonempty DOM text, not application-specific readiness; paints are navigation-relative';
  } catch (error) {
    out.browser_note = String(error);
    out.browser_dom_ms ??= NaN;
  } finally {
    if (child) {
      const exited = new Promise(resolve => child.once('exit', resolve));
      try { process.kill(-child.pid, 'SIGKILL'); } catch {}
      await Promise.race([exited, new Promise(resolve => setTimeout(resolve, 2000))]);
    }
    if (fresh) rmSync(fresh, { recursive: true, force: true });
    server.close();
  }
  out._browser_s = (Date.now() - t) / 1000;
}

// 5. The dev loop: edit app.contract → the page shows it, through the
// resident driver (host/web/dev.mjs; no cargo build in the loop). The budget
// row "Dev restart, request to present" measured end to end: file saved →
// first frame of the new plan in the DOM.
{
  const t = Date.now();
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const port = 20000 + Math.floor(Math.random() * 20000);
  const dev = spawn(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', app.name, '--port', String(port)], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  let compilerPid = null, diagnostic = '', devExited = false;
  dev.stderr.on('data', chunk => { diagnostic = (diagnostic + chunk).slice(-8000); });
  dev.on('exit', () => { devExited = true; });
  const lines = [];
  let waiters = [];
  let buf = '';
  dev.stdout.on('data', (d) => { buf += d; const parts = buf.split('\n'); buf = parts.pop(); for (const l of parts) { lines.push(l); compilerPid ??= /^compiler pid (\d+)/.exec(l)?.[1]; waiters = waiters.filter((w) => !w(l)); } });
  const until = (re, ms, start = lines.length) => new Promise((ok) => {
    const previous = lines.slice(start).map(l => re.exec(l)).find(Boolean);
    if (previous || devExited) return ok(previous ?? null);
    const finish = value => { clearTimeout(timer); dev.off('exit', exited); waiters = waiters.filter(item => item !== w); ok(value); };
    const w = l => { const m = re.exec(l); if (!m) return false; finish(m); return true; };
    const exited = () => finish(null);
    const timer = setTimeout(exited, ms);
    dev.once('exit', exited); waiters.push(w);
  });
  const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
  const source = resolve(app.dir, 'app.contract');
  const original = readFileSync(source, 'utf8');
  let page = null;
  try {
    const ready = await until(/^(?:plan ready|module generation ready)/, 120000, 0); // a cold build of the dev bin can take a while; warm is ~1 s
    if (ready && existsSync(chrome)) {
      page = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
        '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run',
        '--no-default-browser-check', 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
      const cdp = new Cdp(page.stdio[3], page.stdio[4]);
      page.on('exit', () => cdp.fail('dev metrics Chrome exited'));
      const { targetInfos } = await cdp.send('Target.getTargets');
      const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find(x => x.type === 'page').targetId, flatten: true });
      const call = (method, params) => cdp.send(method, params, sessionId);
      const evaluate = async expression => {
        const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
        return r.result.value;
      };
      const connected = until(/^page connected/, 20000);
      await call('Page.enable');
      await call('Page.navigate', { url: `http://127.0.0.1:${port}/` });
      if (!await connected) throw new Error('the page did not subscribe to the dev stream');
      await evaluate(`new Promise((resolve, reject) => {
        const start = performance.now();
        const check = () => {
          if (globalThis.exact?.generation > 0) return resolve(true);
          if (performance.now() - start > 20000) return reject(new Error('the page did not finish its initial boot'));
          setTimeout(check, 10);
        }; check();
      })`);
      const visible = await evaluate('exact.root.innerText');
      const literals = [...original.matchAll(/\btext ("(?:[^"\\]|\\.)*")/g)];
      const match = literals.find(m => { try { const value = JSON.parse(m[1]); return value.trim() && visible.includes(value); } catch { return false; } });
      if (!match) throw new Error('no visible literal text in app.contract to edit; no edit timing claimed');
      const samples = [];
      for (let index = 0; index < 20; index++) {
        const marker = `exact-metrics-${Date.now()}-${index}`;
        const replacement = match[0].replace(match[1], JSON.stringify(`${JSON.parse(match[1])} ${marker}`));
        const edited = original.slice(0, match.index) + replacement + original.slice(match.index + match[0].length);
        // Observe before saving; the timestamp includes filesystem notification,
        // producer work, publication and the browser's actual changed content.
        await evaluate(`globalThis.__exactReloadMetric = new Promise((resolve, reject) => {
          const observer = new MutationObserver(() => {
            if (!exact.root.innerText.includes(${JSON.stringify(marker)})) return;
            const dom = Date.now(); observer.disconnect();
            requestAnimationFrame(() => { clearTimeout(timer); resolve({ dom, frame: Date.now() }); });
          });
          const timer = setTimeout(() => { observer.disconnect(); reject(Error('edited text did not arrive')); }, 10000);
          observer.observe(document.body, { subtree: true, childList: true, characterData: true });
        }); void 0;`);
        const planReady = until(/^(?:edit → plan ready|module generation ready in) (\d+) ms/, 10000);
        const saved = Date.now();
        writeFileSync(source, edited);
        const result = await evaluate('__exactReloadMetric');
        const ready = await planReady;
        samples.push({ dom_ms: result.dom - saved, frame_opportunity_ms: result.frame - saved,
          producer_and_publish_ms: ready ? Number(ready[1]) : null });
      }
      const percentile = (key, fraction) => {
        const values = samples.map(sample => sample[key]).filter(Number.isFinite).sort((a, b) => a - b);
        if (!values.length) return NaN;
        return fraction === .5 && values.length % 2 === 0
          ? (values[values.length / 2 - 1] + values[values.length / 2]) / 2
          : values[Math.ceil(values.length * fraction) - 1];
      };
      out.reload_samples = samples;
      out.reload_ms = percentile('dom_ms', .5);
      out.reload_p95_ms = percentile('dom_ms', .95);
      out.reload_frame_opportunity_ms = percentile('frame_opportunity_ms', .5);
      out.reload_plan_ms = percentile('producer_and_publish_ms', .5);
      out.reload_verified = true;
      out.reload_note = '20 distinct Contract edits; save-to-visible-DOM p50/p95, next frame opportunity reported separately; includes module producer for TypeScript apps';
    } else {
      out.reload_ms = NaN;
      out.reload_note = ready ? 'no Chrome at $CHROME' : 'dev driver did not come up: ' + (diagnostic || lines.slice(-2).join(' | '));
    }
  } catch (error) { out.reload_ms = NaN; out.reload_note = String(error); } finally {
    writeFileSync(source, original);
    await sleep(300);
    if (page) { try { process.kill(-page.pid, 'SIGKILL'); } catch {} }
    // Both groups, explicitly: the driver's and its resident compiler's.
    try { process.kill(-dev.pid, 'SIGTERM'); } catch {}
    await sleep(200);
    try { process.kill(-dev.pid, 'SIGKILL'); } catch {}
    if (compilerPid) { try { process.kill(-Number(compilerPid), 'SIGKILL'); } catch {} }
  }
  out._reload_s = (Date.now() - t) / 1000;
}

// 6. Optional: the dev loop without the resident driver — touch app.contract, rebuild the wasm.
if (rebuild) {
  step('rebuild', () => {
    const source = resolve(app.dir, 'app.contract');
    const now = new Date();
    utimesSync(source, now, now);
    const t = Date.now();
    const r = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.crate('web')], { cwd: ROOT, stdio: 'ignore' });
    out.rebuild_ms = r.status === 0 ? Date.now() - t : NaN;
  });
}

// 6. The macOS app's startup, when it has been built (`bun host/apple/build.mjs`;
// --long builds it): exec → main (dyld), NSApplication, the window, the runner
// with layout and text measurement, the batch applied, the first paint.
const macBin = appleArtifacts(app).binary;
const macHostBinary = appleArtifacts(app, { host: true }).binary;
const macReceipt = resolve(dirname(macBin), 'receipt.json');
const macBuiltApp = () => {
  try { return JSON.parse(readFileSync(macReceipt, 'utf8')).app?.id ?? null; }
  catch { return null; }
};
const macRun = () => {
  assertAppleIdentity(app, macBin);
  return spawnSync(macBin, [], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_ASSETS: app.dir, EXACT_SMOKE: '1' }, timeout: 20000 });
};
const macParse = (o) => {
  delete out.macos_note;
  out.macos_boot_ms = Number(/^boot ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  const s = /startup: exec→main ([\d.?]+) ms; main→NSApplication ([\d.]+) ms; →window ([\d.]+) ms/.exec(o);
  if (s) { out.macos_exec_ms = Number(s[1]); out.macos_nsapp_ms = Number(s[2]); out.macos_window_ms = Number(s[3]); }
  const p = /runner\+layout ([\d.]+) ms of which (\d+) text measurements \((\d+) cached\) ([\d.]+) ms in CoreText; apply ([\d.]+) ms/.exec(o);
  if (p) { out.macos_runner_ms = Number(p[1]); out.macos_measurements = Number(p[2]); out.macos_measure_hits = Number(p[3]); out.macos_measure_ms = Number(p[4]); out.macos_apply_ms = Number(p[5]); }
  out.macos_paint_ms = Number(/^painted ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  out.macos_gpu_ms = Number(/^gpu: module loaded in ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  out.macos_web_loaded = /^web: module loaded/m.test(o);
  out.macos_views = Number(/; (\d+) views/.exec(o)?.[1] ?? NaN);
  const stampOf = (line, label) => Number(new RegExp(`${label.replace(/[.()]/g, '\\$&')} ([\\d.]+)`).exec(line)?.[1] ?? NaN);
  const st = /^stamps: (.*)$/m.exec(o)?.[1] ?? '';
  out.macos_finish_launching_ms = stampOf(st, 'didFinishLaunching');
  out.macos_first_frame_ms = stampOf(st, 'first frame applied');
};
// The platform floor: an empty AppKit app with the same stamps, built once.
const floorBin = resolve(ROOT, 'target/exact-floor');
const floorRun = () => {
  if (!existsSync(floorBin)) spawnSync('swiftc', ['-O', '-o', floorBin, resolve(ROOT, 'host/apple/macos/floor.swift')], { stdio: 'ignore' });
  if (!existsSync(floorBin)) return;
  spawnSync(floorBin, [], { encoding: 'utf8', timeout: 10000 }); // warm up
  const line = /^floor: (.*)$/m.exec(spawnSync(floorBin, [], { encoding: 'utf8', timeout: 10000 }).stdout ?? '')?.[1] ?? '';
  const stampOf = (label) => Number(new RegExp(`${label.replace(/[.()]/g, '\\$&')} ([\\d.]+)`).exec(line)?.[1] ?? NaN);
  out.floor_nsapp_ms = stampOf('NSApplication.shared');
  out.floor_window_ms = stampOf('NSWindow') - stampOf('NSScrollView');
  out.floor_finish_launching_ms = stampOf('didFinishLaunching') - stampOf('activate');
  out.floor_draw_ms = stampOf('first draw');
};
step('macos-boot', () => {
  if (!existsSync(macBin)) { out.macos_boot_ms = NaN; out.macos_note = 'not built (bun host/apple/build.mjs)'; return; }
  const built = macBuiltApp();
  if (built !== app.id) { out.macos_boot_ms = NaN; out.macos_note = `binary receipt is for ${built ?? 'an unknown app'}, not ${app.id} (bun host/apple/build.mjs ${app.crate('apple')})`; return; }
  macRun(); // the first launch of a fresh binary is a cold outlier: warm up, report the second
  macParse(macRun().stdout ?? '');
  floorRun();
});

// 7. Long: the macOS host's builds — a warm build (cargo release staticlib +
// swift), then the budget row "touch one line, rebuild that crate" for the
// host crate — and the startup again on the fresh binary.
if (long) {
  step('macos', () => {
    const build = () => {
      const t = Date.now();
      const r = spawnSync(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), app.crate('apple')], { cwd: ROOT, encoding: 'utf8' });
      const m = /cargo ([\d.]+) s, swift ([\d.]+) s/.exec(r.stdout ?? '');
      return { ok: r.status === 0, total_s: (Date.now() - t) / 1000, cargo_s: m ? Number(m[1]) : NaN, swift_s: m ? Number(m[2]) : NaN };
    };
    const warm = build();
    out.macos_build_s = warm.ok ? warm.total_s : NaN;
    out.macos_build_cargo_s = warm.cargo_s;
    out.macos_build_swift_s = warm.swift_s;
    const src = resolve(ROOT, 'host/apple/src/host.rs');
    const now = new Date();
    utimesSync(src, now, now);
    const touched = build();
    out.macos_touch_s = touched.ok ? touched.total_s : NaN;
    if (touched.ok && macBuiltApp() === app.id) {
      macRun();
      macParse(macRun().stdout ?? '');
      floorRun();
    }
  });
  // The linked delta (LLP 1031 D7): the sample host — a native app linking
  // the archive and ExactKit and nothing else — against the empty AppKit app
  // `floor.swift`, installed bytes and gzip apart; each optional artifact
  // (the GPU module, the web arm) reported beside it, never folded in.
  step('macos-link-delta', () => {
    const r = spawnSync(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), app.crate('apple'), '--host'], { cwd: ROOT, encoding: 'utf8' });
    if (r.status !== 0 || !existsSync(macHostBinary) || !existsSync(floorBin)) { out.link_delta_bytes = NaN; return; }
    assertAppleIdentity(app, macHostBinary);
    const size = (f) => statSync(f).size;
    const gz = (f) => gzipSync(readFileSync(f), { level: 9 }).length;
    const binDir = dirname(macHostBinary);
    out.floor_bytes = size(floorBin); out.floor_gzip_bytes = gz(floorBin);
    out.host_bytes = size(macHostBinary); out.host_gzip_bytes = gz(macHostBinary);
    out.macos_bytes = size(macBin); out.macos_gzip_bytes = gz(macBin);
    out.link_delta_bytes = out.host_bytes - out.floor_bytes;
    out.link_delta_gzip_bytes = out.host_gzip_bytes - out.floor_gzip_bytes;
    for (const [key, name] of [['gpu_module', 'libexact_gpu.dylib'], ['web_module', 'libexact_web.dylib']]) {
      const f = resolve(binDir, name);
      out[`${key}_bytes`] = existsSync(f) ? size(f) : NaN;
      out[`${key}_gzip_bytes`] = existsSync(f) ? gz(f) : NaN;
    }
  });
}

// Keep the wall-clock observation whole: it is what a user sees. AppKit's
// empty-window floor is a separate experiment, not a subtraction that can
// make a slow launch look fast. The portion Exact can trade against its cold
// start budget is the directly stamped runner/layout + presenter application.
out.macos_total_ms = out.macos_exec_ms + out.macos_paint_ms;
out.macos_framework_ms = out.macos_runner_ms + out.macos_apply_ms;
out.total_s = (Date.now() - t0) / 1000 + (out.source_capture_s ?? 0);

if (json) { console.log(JSON.stringify(out)); process.exit(0); }

const ms = (v) => (Number.isFinite(v) ? `${v.toFixed(v < 10 ? 2 : 1)} ms` : 'n/a');
const kib = (v) => Number.isFinite(v) ? `${(v / 1024).toFixed(0)} KiB` : 'n/a';
const bytes = (v, suffix = '') => Number.isFinite(v) ? `${v.toLocaleString()} B${suffix}` : out.native_note ?? 'n/a';
const grade = (v, label) => {
  const stated = budget(label);
  const ceiling = Number(/^([\d.]+)\s*ms\b/i.exec(stated)?.[1] ?? NaN);
  if (!Number.isFinite(v) || !Number.isFinite(ceiling)) return `budget ${stated}`;
  return `budget ${stated}; ${v <= ceiling ? 'within' : 'OVER'}`;
};
const rows = [
  ['compile app.contract → plan', ms(out.compile_ms), bytes(out.plan_bytes)],
  ['bake (one runner boot at build)', ms(out.bake_ms), bytes(out.baked_bytes, ' baked')],
  ['decode + validate the plan', ms(out.decode_ms), ''],
  ['runner boot → first frame', ms(out.boot_ms), Number.isFinite(out.nodes) ? `${out.nodes} nodes, ${out.text_nodes} text` : ''],
  ['layout 390×844 (Taffy)', ms(out.layout_ms), ''],
  ['update: screen swap (press)', ms(out.update_ms), `budget ${budget('Dev restart')}`],
  ['update: inherited row on the root', ms(out.inherit_ms), Number.isFinite(out.inherit_touched) ? `${out.inherit_touched} nodes re-derived (text-color; LLP 1035.000 D2)` : ''],
  ['tick: advance 1 s', ms(out.tick_ms), ''],
  ['web host first batch', ms(out.web_boot_ms), `${kib(out.web_first_batch_bytes)} JSON`],
  ['web host update batch', ms(out.web_update_ms), `${kib(out.web_update_batch_bytes)} JSON`],
  ['wasm (web profile + wasm-opt)', kib(out.wasm_bytes), `${kib(out.wasm_gzip_bytes)} gzip; glue ${kib(out.glue_bytes)}`],
  ['GPU module (web, on demand)', Number.isFinite(out.gpu_wasm_bytes) ? kib(out.gpu_wasm_bytes) : 'n/a', Number.isFinite(out.gpu_wasm_bytes) ? `${kib(out.gpu_wasm_gzip_bytes)} gzip; glue ${kib(out.gpu_glue_bytes)}; observed load order in JSON` : ''],
  ['browser: script → DOM', ms(out.browser_dom_ms), `budget ${budget('Cold start')}`],
  ['browser: navigation → first paint', ms(out.browser_paint_ms), 'Paint Timing, single instrumented sample'],
  ['browser: → contentful paint', ms(out.browser_contentful_paint_ms), 'browser content, not application readiness'],
  ['browser: click → changed DOM', ms(out.browser_first_interaction?.changed_dom_ms - out.browser_first_interaction?.input_ms), out.browser_first_interaction?.target ?? out.browser_interaction_note ?? out.browser_note ?? 'unmeasured'],
  ['boot modules before first pixel', `${out.boot_modules}`, `${out.boot.javascript_bytes} B source JS; ${out.boot_ok ? 'allowed paths' : 'VIOLATION'}; not a content/work proof`],
  ['edit → DOM (resident dev loop)', ms(out.reload_ms), Number.isFinite(out.reload_ms) ? `p50; p95 ${ms(out.reload_p95_ms)}; next frame ${ms(out.reload_frame_opportunity_ms)}; budget ${budget('Dev restart')}` : out.reload_note ?? ''],
  ['macOS: exec → first paint (raw)', ms(out.macos_total_ms), Number.isFinite(out.macos_paint_ms) ? `${out.macos_views} views; empty AppKit main → draw ${ms(out.floor_draw_ms)}` : out.macos_note ?? ''],
];
if (Number.isFinite(out.macos_paint_ms)) rows.push(
  ['  Exact: runner → NSViews', ms(out.macos_framework_ms), grade(out.macos_framework_ms, 'Cold start')],
  ['  ours: runner + layout', ms(out.macos_runner_ms), `${out.macos_measurements} text measurements (${out.macos_measure_hits} cached), ${ms(out.macos_measure_ms)} in CoreText`],
  ['  ours: batch → NSViews', ms(out.macos_apply_ms), ''],
  ['  AppKit: exec → main', ms(out.macos_exec_ms), 'dyld, the Swift runtime'],
  ['  AppKit: NSApplication', ms(out.macos_nsapp_ms), `floor ${ms(out.floor_nsapp_ms)} (waits on the window server)`],
  ['  AppKit: NSWindow', ms(out.macos_window_ms), `floor ${ms(out.floor_window_ms)} (NSThemeFrame, a dlopen)`],
  ['  AppKit: run → didFinishLaunching', ms(out.macos_finish_launching_ms - out.macos_first_frame_ms), `floor ${ms(out.floor_finish_launching_ms)} (Dock registration ≈ 65 ms of it)`],
  ['  main → first paint', ms(out.macos_paint_ms), `floor ${ms(out.floor_draw_ms)}: an empty window on this machine`],
  ['  GPU module (dlopen + device)', ms(out.macos_gpu_ms), 'after first paint, first canvas'],
  ['  web arm (dlopen)', out.macos_web_loaded ? 'loaded' : 'not loaded', out.macos_web_loaded ? 'VIOLATION: first screen has no iframe' : 'first iframe commit only'],
);
if (rebuild) rows.push(['edit → wasm rebuilt (no driver)', ms(out.rebuild_ms), out.rebuild_note ?? 'the cold path: cargo build of the app crate']);
if (long) {
  const s = (v) => (Number.isFinite(v) ? `${v.toFixed(1)} s` : 'n/a');
  rows.push(['macOS: initial captured build', s(out.macos_build_s), `cargo ${s(out.macos_build_cargo_s)} · swift ${s(out.macos_build_swift_s)}; budget ${budget('Full build')}`]);
  const mib = (v) => (Number.isFinite(v) ? `${(v / 1048576).toFixed(2)} MB` : 'n/a');
  rows.push(
    ['macOS: link delta (sample host − floor)', mib(out.link_delta_bytes), Number.isFinite(out.link_delta_bytes) ? `${mib(out.link_delta_gzip_bytes)} gzip; host ${mib(out.host_bytes)}, floor ${mib(out.floor_bytes)}; the archive + ExactKit, nothing optional (LLP 1031 D7)` : 'not measured (the sample host or the floor did not build)'],
    ['  optional: GPU module (dlopen)', mib(out.gpu_module_bytes), Number.isFinite(out.gpu_module_bytes) ? `${mib(out.gpu_module_gzip_bytes)} gzip; paid at the first canvas` : 'no GPU crate'],
    ['  optional: web arm (dlopen)', mib(out.web_module_bytes), Number.isFinite(out.web_module_bytes) ? `${mib(out.web_module_gzip_bytes)} gzip; paid at the first iframe` : 'n/a'],
  );
  rows.push(['macOS: touch one line, rebuild', s(out.macos_touch_s), `host/apple/src/host.rs; budget ${budget('Touch one line')}`]);
}
console.log(`web artifact sha256 ${out.web_artifact_id}; hardware ${out.identity.cpu}; commit ${out.identity.commit}`);
console.log(`exact2 metrics (${app.id}, captured source) — ${new Date().toISOString().slice(0, 19)}Z, private build cache, p50 where repeated`);
for (const [k, v, note] of rows) console.log(`  ${k.padEnd(34)} ${v.padStart(11)}   ${note}`);
console.log(`  ${'total'.padEnd(34)} ${`${out.total_s.toFixed(1)} s`.padStart(11)}   capture ${(out.source_capture_s ?? 0).toFixed(1)} s · native ${out._native_s.toFixed(1)} s · wasm ${out._wasm_s.toFixed(1)} s · browser ${out._browser_s.toFixed(1)} s · dev loop ${out._reload_s.toFixed(1)} s · macOS boot ${out['_macos-boot_s'].toFixed(1)} s${rebuild ? ` · rebuild ${out._rebuild_s.toFixed(1)} s` : ''}${long ? ` · macOS ${out._macos_s.toFixed(1)} s` : ''}`);
