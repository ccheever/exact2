// Retained three- or four-arm process samples. @ref LLP 1149 §4.1.
// bun bench/heavy-list/gpui/aggregate.mjs target/g3c [--verify | --json]
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const [directory = 'target/g3c', mode, bootOutput] = process.argv.slice(2);
assert([undefined, '--verify', '--json', '--boot', '--sample'].includes(mode), 'expected --verify, --json, --boot or --sample');
assert(bootOutput === undefined || mode === '--boot' && ['--table', '--verify'].includes(bootOutput), 'only --boot accepts --table or --verify');
const hookStates = new Set();
const hookSamples = [];
function hookState(raw, path, retained = raw.accepted !== false) {
  assert(mode === '--sample' || !raw.fling?.start?.forcedInteractiveDiagnostic,
    `${path}: forced QoS is diagnostic only; use --sample, never a scoreboard table`);
  const enabled = raw.gpuHookEnabled ?? true; // Pre-switch probes always enabled the hook.
  assert.equal(typeof enabled, 'boolean', `${path}: gpuHookEnabled must be boolean`);
  hookSamples.push({ path, gpuHookEnabled: retained ? enabled : raw.gpuHookEnabled ?? null,
    requestedGpuHookEnabled: raw.requestedGpuHookEnabled ?? null });
  if (retained) {
    hookStates.add(enabled);
    assert.equal(hookStates.size, 1, `${path}: mixed GPU hook states; aggregate separate configurations`);
  }
  if (!enabled) for (const gpu of Object.values(raw.gpu ?? {})) {
    for (const key of ['committed', 'completed', 'invalid', 'seconds'])
      assert.equal(gpu[key], 0, `${path}: hook off but observed GPU ${key}`);
    assert(!gpu.queueClass && !gpu.bufferClass, `${path}: hook off but observed GPU class`);
  }
  return enabled;
}
const hookMetadata = () => ({ gpuHookEnabled: [...hookStates][0] ?? null, hookSamples });
const hookLabel = () => `GPU hook: ${[...hookStates][0] ? 'on' : 'off'} (gpuHookEnabled=${[...hookStates][0]}).`;
function gpuMetrics(raw, phase, enabled, seconds, frames, path) {
  const gpu = raw.gpu?.[phase];
  // Older cadence probes have no command-buffer instrument.
  if (!enabled || !gpu) return {};
  assert.equal(gpu.committed, gpu.completed, `${path}: outstanding command buffers`);
  assert.equal(gpu.invalid, 0, `${path}: invalid GPU timestamps`);
  if (raw.arm !== 'native') assert(gpu.committed > 0, `${path}: no observed GPU work`);
  return { buffers: gpu.completed, ...(gpu.completed ? {
    gpu_ms_s: gpu.seconds * 1000 / seconds,
    ...(frames ? { gpu_ms_frame: gpu.seconds * 1000 / frames } : {}),
  } : {}) };
}
function workCounters(start, end, seconds, path) {
  const result = {};
  for (const counter of ['instructions', 'cycles']) {
    if (!(counter in start) && !(counter in end)) continue;
    assert(Number.isSafeInteger(start[counter]) && Number.isSafeInteger(end[counter])
      && end[counter] >= start[counter], `${path}: invalid ${counter}`);
    result[`${counter}_million_s`] = (end[counter] - start[counter]) / seconds / 1e6;
  }
  if ('cycles_million_s' in result && end.cpu > start.cpu)
    result.effective_ghz = (end.cycles - start.cycles) / (end.cpu - start.cpu) / 1e9;
  if ('instructions_million_s' in result && end.cycles > start.cycles)
    result.ipc = (end.instructions - start.instructions) / (end.cycles - start.cycles);
  return result;
}
function verifyPowerReport() {
  const runPath = join(directory, 'run.json');
  if (!existsSync(runPath)) return false;
  const run = JSON.parse(readFileSync(runPath, 'utf8'));
  if (!run.power) return false;
  assert.equal(run.exit, 0, 'power driver failed');
  assert(['fling', 'bar'].includes(run.phase), 'power workload');
  assert.equal(run.hook, '0', 'power series uses the default hook-off configuration');
  const power = JSON.parse(readFileSync(join(directory, 'power-summary.json'), 'utf8'));
  assert.equal(power.quiet, run.quiet, 'power qualification differs from its series');
  assert.equal(power.powerHookEnabled, false);
  const targets = ['native', 'gpui', 'ink', 'egui'];
  const phases = run.phase === 'bar' ? ['bar'] : ['fling', 'rest'];
  for (const arm of targets) for (const phase of phases) {
    const rows = power.samples[arm].filter(row => row.phase === phase);
    assert.equal(rows.length, 1, `${arm}/${phase}: one power run`);
    const row = rows[0], path = join(directory, 'series', row.path);
    const raw = JSON.parse(readFileSync(path, 'utf8'));
    assert(raw.accepted && raw.pid === row.pid && raw.gpuHookEnabled === false, `${path}: power PID/hook`);
    assert(row.samples.length >= 5 && row.samples.every(sample => sample.seconds > 0), `${path}: power intervals`);
    const { start, end } = raw[phase];
    assert(Math.abs(end.time - start.time - 10) < 0.05, `${path}: power phase duration`);
    workCounters(start, end, end.time - start.time, path);
    for (const key of Object.keys(row.metrics)) {
      const seconds = row.samples.reduce((total, sample) => total + sample.seconds, 0);
      const mean = row.samples.reduce((total, sample) => total + sample[key] * sample.seconds, 0) / seconds;
      assert(Math.abs(mean - row.metrics[key]) < 1e-9, `${path}: weighted ${key}`);
      const metric = power.metrics[arm][`${phase}_${key}`];
      assert([metric.median, metric.min, metric.max].every(value => Math.abs(value - mean) < 1e-9), `${path}: one-run ${key}`);
    }
  }
  const lines = ['| Metric | Native | GPUI | Ink | egui |', '|---|---:|---:|---:|---:|'];
  for (const phase of phases) for (const [key, label, digits] of [
    ['system_cpu_w', 'system CPU, W', 3], ['system_gpu_w', 'system GPU, W', 3],
    ['process_energy_impact_per_s', 'process energy impact/s', 2],
    ['system_gpu_mhz', 'system GPU clock, MHz', 1], ['system_gpu_active_pct', 'system GPU active, %', 2],
  ]) {
    const cells = targets.map(arm => {
      const row = power.metrics[arm][`${phase}_${key}`];
      assert([row.median, row.min, row.max].every(Number.isFinite));
      return `${row.median.toFixed(digits)} (${row.min.toFixed(digits)}–${row.max.toFixed(digits)})`;
    });
    lines.push(`| ${phase[0].toUpperCase() + phase.slice(1)} interior: ${label} | ${cells.join(' | ')} |`);
  }
  const table = lines.join('\n');
  assert.equal(readFileSync(join(directory, 'power-table.md'), 'utf8').trim(), table);
  assert(readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8').includes(table), `power table differs:\n${table}`);
  console.log(`${hookLabel()} Separate power table reproduced from its retained PID joins; ${run.quiet ? 'quiet' : 'excluded'} series. CPU timing rows are not published.`);
  return true;
}
if (mode === '--sample') {
  const raw = JSON.parse(readFileSync(directory, 'utf8'));
  assert.equal(raw.accepted, true, `${directory}: rejected sample`);
  const enabled = hookState(raw, directory);
  console.log(`${hookLabel()} Instrument check only; not a scoreboard table.\n`);
  console.log('| Phase | Seconds | Process CPU, ms | Main CPU, ms | Instructions | Cycles | Effective GHz | IPC |');
  console.log('|---|---:|---:|---:|---:|---:|---:|---:|---:|');
  for (const phase of [raw.workload, ...(raw.rest ? ['rest'] : [])]) {
    const { start, end } = raw[phase];
    const seconds = end.time - start.time;
    const work = workCounters(start, end, seconds, directory);
    gpuMetrics(raw, phase, enabled, seconds, 0, directory);
    console.log(`| ${phase} | ${seconds.toFixed(6)} | ${((end.cpu-start.cpu)*1000).toFixed(3)} | ${((end.main-start.main)*1000).toFixed(3)} | ${end.instructions-start.instructions} | ${end.cycles-start.cycles} | ${work.effective_ghz?.toFixed(3) ?? '—'} | ${work.ipc?.toFixed(3) ?? '—'} |`);
  }
  process.exit(0);
}
if (mode === '--boot') {
  const samples = [];
  for (const file of readdirSync(directory).filter(f => /^(metal|wgpu)-\d+\.run\.json$/.test(f)).sort()) {
    const stem = file.replace('.run.json', '');
    const read = suffix => JSON.parse(readFileSync(join(directory, stem + suffix), 'utf8'));
    const run = read('.run.json');
    const raw = existsSync(join(directory, stem + '.json')) ? read('.json') : {};
    const enabled = hookState(raw, stem, run.exit === 0);
    if (!existsSync(join(directory, stem + '.json.trace.json'))) {
      assert(run.timeout || run.exit !== 0, `${stem}: successful launch without trace`);
      samples.push({sample: stem, arm: stem.split('-')[0], exit: run.exit ?? 'timeout', quiet: false, phaseMs: null,
        archiveOpenMs: null, compileMs: null, firstPixelMs: null, firstCpuMs: null, firstInkMs: null,
        firstCpuCompleteMs: null, firstInkCompleteMs: null});
      continue;
    }
    if (run.cacheMode === 'miss') {
      assert.equal(run.cachePresentAtLaunch, false, `${stem}: cache not removed`);
    }
    if (run.cacheMode === 'retained' && run.exit === 0) {
      assert.equal(raw.cpuSubmitted, false, `${stem}: retained archive missed`);
    }
    const trace = read('.json.trace.json');
    assert.equal(trace.dropped, 0, `${stem}: dropped trace spans`);
    const events = trace.traceEvents;
    const window = existsSync(join(directory, stem + '.json.window')) ? read('.json.window') : {};
    const compile = events.find(e => e.name === 'worker compile pipelines');
    const firstShown = Math.min(...events.filter(e => e.name === 'presented' && e.ts > 0).map(e => e.ts));
    if (run.cacheMode === 'miss' && run.exit === 0) {
      assert(compile, `${stem}: miss without a compiler span`);
      if (!raw.cpuSubmitted) assert(compile.ts + compile.dur <= firstShown,
        `${stem}: ink presented before the miss finished compiling`);
    }
    const arm = stem.split('-')[0];
    const names = arm === 'metal'
      ? ['boot Metal constructor', 'boot Metal attach layer', 'boot samplers buffers white textures']
      : ['boot wgpu constructor', 'boot wgpu attach layer'];
    const spans = events.filter(e => names.includes(e.name)).map(e => [e.ts, e.ts + e.dur]).sort((a,b) => a[0]-b[0]);
    assert(spans.length >= 2, `${stem}: missing startup spans`);
    // Inclusive spans overlap in the old constructor. Count their union once.
    let elapsed = 0, start = spans[0][0], end = spans[0][1];
    for (const [a,b] of spans.slice(1)) {
      if (a <= end) end = Math.max(end,b);
      else { elapsed += end-start; start=a; end=b; }
    }
    elapsed += end-start;
    const phase = name => events.filter(e => e.name === name).reduce((n,e) => n + e.dur / 1000, 0);
    const value = name => events.find(e => e.name === name)?.args?.value ?? null;
    const constructor = events.find(e => names[0] === e.name);
    const compiledOnBoot = events.filter(e => e.name === 'worker compile pipelines' && e.tid === constructor.tid).length;
    assert.equal(compiledOnBoot, 0, `${stem}: compilation on boot thread`);
    samples.push({sample: stem, arm, gpuHookEnabled: enabled, exit: run.exit, quiet: run.before.quiet && run.after.quiet,
      phaseMs: elapsed/1000, archiveOpenMs: phase('boot archive open'),
      compileMs: phase('worker compile pipelines'),
      firstPixelMs: raw.firstDrawableMs ?? null, firstCpuMs: raw.firstCpuMs ?? null,
      firstInkMs: raw.firstInkMs ?? null, cpuSubmitted: raw.cpuSubmitted ?? null,
      // Older probes did not establish completeness for each painter. Keep it missing.
      firstCpuCompleteMs: raw.firstCpuCompleteMs ?? null,
      firstInkCompleteMs: raw.firstInkCompleteMs ?? null,
      thermal: [raw.thermalStart ?? null, raw.thermalEnd ?? null],
      activeAtStart: raw.activeAtStart ?? null, windowActive: window.active ?? null, keyWindow: window.keyWindow ?? null,
      activationSuppressed: raw.activationSuppressed ?? null, compiledOnBoot,
      deviceMs: value('boot device milliseconds'), shadersMs: value('boot shaders milliseconds'),
      cpuWhileCompiling: Boolean(raw.cpuSubmitted && compile && compile.ts <= firstShown && firstShown < compile.ts + compile.dur),
      spans: Object.fromEntries(events.filter(e => e.name.startsWith('boot ')).map(e => [e.name, e.dur / 1000])),
    });
  }
  const medians = {};
  for (const arm of ['metal', 'wgpu']) {
    const rows = samples.filter(s => s.arm === arm);
    if (!rows.length) continue;
    medians[arm] = {count: rows.length, quiet: rows.filter(s => s.quiet).length,
      displayed: rows.filter(s => s.firstPixelMs !== null).length,
      cpuCompleteSamples: rows.filter(s => s.firstCpuCompleteMs !== null).length,
      inkCompleteSamples: rows.filter(s => s.firstInkCompleteMs !== null).length,
      phaseSamples: rows.filter(s => s.phaseMs !== null).length,
      activated: rows.filter(s => s.activeAtStart || s.windowActive || s.keyWindow).length,
      cpuWhileCompiling: rows.filter(s => s.cpuWhileCompiling).length};
    for (const key of ['phaseMs', 'archiveOpenMs', 'compileMs', 'firstPixelMs', 'firstCpuMs', 'firstInkMs', 'firstCpuCompleteMs', 'firstInkCompleteMs']) {
      const values = rows.map(s => s[key]).filter(v => v !== null).sort((a,b) => a-b);
      medians[arm][key] = values.length ? (values[Math.floor((values.length-1)/2)] + values[Math.floor(values.length/2)])/2 : null;
    }
  }
  if (bootOutput) {
    const targets = Object.keys(medians);
    const rows = [
      ['Launches', 'count'], ['Displayed', 'displayed'], ['Complete Ink screens', 'inkCompleteSamples'],
      ['Device/layer phase, ms', 'phaseMs'], ['First pixel, ms', 'firstPixelMs'],
      ['First CPU pixel, ms', 'firstCpuMs'],
      ['Complete CPU screen, ms', 'firstCpuCompleteMs'], ['Complete Ink screen, ms', 'firstInkCompleteMs'],
      ['Worker compilation/serialization, ms', 'compileMs'], ['Active or key windows', 'activated'],
    ];
    const table = `| Metric | ${targets.join(' | ')} |\n|---|${targets.map(() => '---:|').join('')}\n`
      + rows.map(([label, key]) => `| ${label} | ${targets.map(arm => {
        const value = medians[arm][key];
        if (value === null) return '—';
        if (!key.endsWith('Ms')) return String(value);
        const values = samples.filter(s => s.arm === arm && s[key] !== null).map(s => s[key]);
        return `${value.toFixed(3)} (${Math.min(...values).toFixed(3)}–${Math.max(...values).toFixed(3)})`;
      }).join(' | ')} |`).join('\n');
    if (bootOutput === '--verify') {
      const record = readFileSync(new URL('../../../host/ink/MEASUREMENTS.md', import.meta.url), 'utf8');
      assert(record.includes(table), `boot table differs:\n${table}`);
      console.log(`${hookLabel()} Boot table reproduced exactly: ${samples.length} retained attempts.`);
    } else console.log(`${hookLabel()}\n\n${table}`);
  } else console.log(JSON.stringify({...hookMetadata(), instrument: 'probe CACurrentMediaTime startup span union; MTLDrawable.presentedTime first pixels and per-painter complete scenes', medians, samples}, null, 2));
  process.exit(0);
}
const phaseFiles = phase => existsSync(join(directory, 'series', phase)) ? readdirSync(join(directory, 'series', phase)) : [];
const hasSamples = phase => phaseFiles(phase).some(file => /^(native|gpui|ink|egui)-\d+(-retry)?\.json$/.test(file));
const fourArms = ['fling', 'cold', 'bar', 'caret'].some(phase => phaseFiles(phase).some(file => /^egui-/.test(file)));
const arms = ['native', 'gpui', 'ink', ...(fourArms ? ['egui'] : [])];
const report = fourArms ? 'G4' : 'G3c';
const samples = Object.fromEntries(arms.map(arm => [arm, []]));
const rejected = [];
const unused = [];
const round = path => Number(path.match(/-(\d+)(?:-retry)?\.json$/)[1]);
const sum = values => values.reduce((a, b) => a + b, 0);
const inside = (value, [start, end]) => start <= value && value <= end;
const gaps = times => times.slice(1).map((time, i) => (time - times[i]) * 1000);
const mib = bytes => bytes / 2 ** 20;
function quantile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  assert(sorted.length, 'empty cadence');
  return sorted[Math.min(Math.floor(p * sorted.length), sorted.length - 1)];
}
function distribution(values) {
  const sorted = values.sort((a, b) => a - b);
  assert(sorted.length && sorted.every(Number.isFinite), 'missing or nonfinite metric');
  const middle = Math.floor(sorted.length / 2);
  return {
    median: sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    min: sorted[0], max: sorted.at(-1),
  };
}

if (hasSamples('bar') || hasSamples('caret')) {
  staticReport();
  process.exit(0);
}

function staticReport() {
  const observations = readFileSync(join(directory, 'series', 'quiet.jsonl'), 'utf8')
    .trim().split('\n').map(JSON.parse);
  const retained = {}, excluded = [];
  const header = '| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |\n|---|---:|---:|---:|---:|';
  const targets = ['native', 'gpui', 'ink', 'egui'];
  const summary = {};
  const phases = ['fling', 'bar', 'caret'].filter(hasSamples);
  for (const phase of phases) {
    retained[phase] = Object.fromEntries(targets.map(arm => [arm, []]));
    const folder = join(directory, 'series', phase);
    for (const file of readdirSync(folder).sort()) {
      if (/^(native|gpui|ink|egui)-\d+(-retry)?\.log$/.test(file)
          && !existsSync(join(folder, file.replace(/\.log$/, '.json')))) {
        excluded.push({ path: join(folder, file), reason: 'launch ended without a result' });
      }
      if (!/^(native|gpui|ink|egui)-\d+(-retry)?\.json$/.test(file)) continue;
      const path = join(folder, file), raw = JSON.parse(readFileSync(path, 'utf8'));
      const enabled = hookState(raw, path);
      if (!raw.accepted || raw.thermalStart !== raw.thermalEnd) {
        excluded.push({ path, reason: raw.reason ?? 'focus or thermal state' });
        continue;
      }
      assert.equal(raw.workload, phase, `${path}: workload`);
      const { start, end, peak, displayLink } = raw[phase];
      const seconds = end.time - start.time;
      assert(Math.abs(seconds - 10) < 0.05, `${path}: duration ${seconds}`);
      const before = observations.findLastIndex(row => row.time <= start.time);
      const after = observations.findIndex(row => row.time >= end.time);
      const observed = before >= 0 && after >= 0 ? observations.slice(before, after + 1) : [];
      const covered = observed.length >= 2 && observed.every((row, i) => i === 0 || row.time - observed[i - 1].time < 6);
      if (!covered || observed.some(row => !row.quiet)) {
        excluded.push({ path, reason: covered ? 'loaded' : 'unmonitored' });
        continue;
      }
      const arm = raw.arm;
      assert(targets.includes(arm), `${path}: arm`);
      assert.equal(raw.size[0], 800, `${path}: content width`);
      assert.equal(raw.size[1], arm === 'native' ? 800 : 768, `${path}: content height`);
      assert.equal(raw.scale, 2, `${path}: scale`);
      const bounds = [start.time, end.time];
      const frames = (arm === 'native' ? displayLink.map(row => row.timestamp) : raw.presented)
        .filter(time => inside(time, bounds)).length;
      const sample = { path, gpuHookEnabled: enabled, seconds, frames, fps: frames / seconds,
        cpu: (end.cpu - start.cpu) * 1000 / seconds,
        main: (end.main - start.main) * 1000 / seconds,
        ...workCounters(start, end, seconds, path),
        peak_mib: mib(peak), end_mib: mib(end.footprint),
        ...gpuMetrics(raw, phase, enabled, seconds, frames, path),
      };
      if (phase === 'fling') {
        const driver = readFileSync(path.replace(/\.json$/, '.log'), 'utf8')
          .split('\n').filter(line => line.startsWith('{"posted":')).map(JSON.parse);
        assert.equal(driver.length, 4, `${path}: four flings`);
        sample.posted = sum(driver.map(row => row.posted));
        sample.received = raw.fling.wheelEvents;
        sample.maxGapMs = Math.max(...driver.map(row => row.maxGapMs));
        sample.rest_cpu = (raw.rest.end.cpu - raw.rest.start.cpu) * 1000 / (raw.rest.end.time - raw.rest.start.time);
        sample.rest_main = (raw.rest.end.main - raw.rest.start.main) * 1000 / (raw.rest.end.time - raw.rest.start.time);
      } else {
        assert.equal(raw[phase].wheelEvents, 0, `${path}: unexpected input`);
        const offset = arm === 'native' ? raw.nativeOffsetEnd : raw[`${arm}OffsetEnd`].withinItem;
        assert.equal(offset, 0, `${path}: scrolled`);
        if (arm !== 'native') assert.equal(raw[`${arm}OffsetEnd`].item, 0, `${path}: not row zero`);
      }
      retained[phase][arm].push(sample);
    }
    summary[phase] = {};
    for (const arm of targets) {
      const samples = retained[phase][arm];
      const slots = samples.map(row => round(row.path));
      assert(slots.every(slot => slot >= 1 && slot <= 5) && new Set(slots).size === slots.length, `${phase}/${arm}: slots`);
      const keys = new Set(samples.flatMap(sample => Object.keys(sample)).filter(key => !['path', 'gpuHookEnabled'].includes(key)));
      summary[phase][arm] = Object.fromEntries([...keys].map(key => [key,
        distribution(samples.filter(sample => key in sample).map(sample => sample[key])),
      ]));
    }
  }
  const cell = (phase, arm, key, digits) => {
    const metric = summary[phase][arm][key];
    return metric ? `${metric.median.toFixed(digits)} (${metric.min.toFixed(digits)}–${metric.max.toFixed(digits)})` : '—';
  };
  const tables = ['bar', 'caret', 'fling'].filter(phase => phases.includes(phase)).map(phase => {
    const rows = [
      ['Process CPU, ms/s', 'cpu', 3], ['Main-thread CPU, ms/s', 'main', 3],
      ...(['instructions', 'cycles'].filter(key => targets.some(arm => summary[phase][arm][`${key}_million_s`]))
        .map(key => [`Process ${key}, million/s`, `${key}_million_s`, 3])),
      ...(['effective_ghz', 'ipc'].filter(key => targets.some(arm => summary[phase][arm][key]))
        .map(key => [key === 'ipc' ? 'Process instructions/cycle' : 'Process effective GHz', key, 3])),
      ['Frames/s; native display link / painted drawables', 'fps', 2],
      ['Peak footprint, MiB', 'peak_mib', 2], ['End footprint, MiB', 'end_mib', 2],
      ['GPU busy time, ms/s; process command buffers', 'gpu_ms_s', 3],
      ['GPU ms/frame; native cadence / painted drawables', 'gpu_ms_frame', 4],
      ['Completed command buffers per sample', 'buffers', 0],
    ];
    return `${phase}\n\n${header}\n| Quiet samples | ${targets.map(arm => retained[phase][arm].length).join(' | ')} |\n`
      + rows.map(([label, key, digits]) => `| ${label} | ${targets.map(arm => cell(phase, arm, key, digits)).join(' | ')} |`).join('\n');
  });
  if (mode === '--json') console.log(JSON.stringify({ ...hookMetadata(), samples: retained, metrics: summary, excluded }, null, 2));
  else if (mode === '--verify') {
    if (verifyPowerReport()) return;
    const record = readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8').split('## G5:')[1];
    assert(record, 'missing G5 results');
    for (const table of tables) assert(record.includes(table), `G5 table differs:\n${table}`);
    console.log(`${hookLabel()} G5 reproduced exactly: ${tables.length === 3 ? 'three' : tables.length} table${tables.length === 1 ? '' : 's'}; ${excluded.length} excluded samples.`);
  } else console.log(hookLabel() + '\n\n' + tables.join('\n\n'));
}

const phases = ['cold', 'fling'].filter(phase => existsSync(join(directory, 'series', phase)));
if (existsSync(join(directory, 'series', 'cold2'))) phases.push('cold2');
for (const phase of phases) {
  const folder = join(directory, 'series', phase);
  for (const file of readdirSync(folder).sort()) {
    // A readiness timeout exits before the probe can write its result JSON.
    if (/^(native|gpui|ink|egui)-\d+(-retry)?\.log$/.test(file)
        && !existsSync(join(folder, file.replace(/\.log$/, '.json')))) {
      const path = join(folder, file);
      assert(readFileSync(path, 'utf8').includes('probe: first complete frame timed out'),
        `${path}: unclassified launch without result JSON`);
      rejected.push(path);
    }
    if (!/^(native|gpui|ink|egui)-\d+(-retry)?\.json$/.test(file)) continue;
    const path = join(folder, file);
    const raw = JSON.parse(readFileSync(path, 'utf8'));
    const enabled = hookState(raw, path);
    if (!raw.accepted) {
      rejected.push(path);
      continue;
    }
    // G4's resumed cold series retained rounds 11–12 beyond the ten-launch column.
    if (fourArms && round(path) > 10) {
      assert.notEqual(phase, 'fling', `${path}: extra fling round`);
      unused.push(path);
      continue;
    }
    const arm = file.split('-')[0];
    const sample = { path, gpuHookEnabled: enabled, cold_ms: raw.coldMs };
    if (phase === 'fling') {
      for (const part of ['fling', 'rest']) {
        const { start, end } = raw[part];
        const seconds = end.time - start.time;
        assert(Math.abs(seconds - 10) < 0.05, `${path}: ${part} duration`);
        sample[`${part}_seconds`] = seconds;
        for (const counter of ['cpu', 'main']) {
          sample[`${part}_${counter}`] = (end[counter] - start[counter]) * 1000 / seconds;
        }
        for (const [key, value] of Object.entries(workCounters(start, end, seconds, path)))
          sample[`${part}_${key}`] = value;
      }
      sample.peak_mib = mib(raw.fling.peak);
      sample.end_fling_mib = mib(raw.fling.end.footprint);
      sample.end_rest_mib = mib(raw.rest.end.footprint);
      const driver = readFileSync(path.replace(/\.json$/, '.log'), 'utf8')
        .split('\n').filter(line => line.startsWith('{"posted":')).map(JSON.parse);
      assert.equal(driver.length, 4, `${path}: four gesture reports`);
      sample.posted = sum(driver.map(row => row.posted));
      sample.received = raw.fling.wheelEvents;
      sample.received_fraction = sample.received / sample.posted;
      sample.maxGapMs = Math.max(...driver.map(row => row.maxGapMs));
      sample.emptyCallbacks = sum(driver.map(row => row.emptyCallbacks));
      const windows = raw.inputWindows;
      assert.equal(windows.length, 4, `${path}: four input windows`);
      sample.gesture_seconds = sum(windows.map(([start, end]) => end - start));
      let intervals;
      if (arm === 'native') {
        // Native has callback timestamps, not presented drawables.
        intervals = gaps(raw.fling.displayLink.map(row => row.timestamp));
      } else {
        const times = raw.presented.filter(time => time > 0);
        assert(times.every((time, i) => i === 0 || time >= times[i - 1]), `${path}: unordered presents`);
        intervals = windows.flatMap(window => gaps(times.filter(time => inside(time, window))));
        sample.gesture_frames = sum(windows.map(window => times.filter(time => inside(time, window)).length));
        sample.gesture_fps = sample.gesture_frames / sample.gesture_seconds;
        sample.fling_frames = times.filter(time => inside(time, [raw.fling.start.time, raw.fling.end.time])).length;
        sample.fling_fps = sample.fling_frames / sample.fling_seconds;
        sample.cpu_per_frame = (raw.fling.end.cpu - raw.fling.start.cpu) * 1000 / sample.fling_frames;
      }
      sample.intervals = intervals.length;
      for (const p of [50, 95, 99]) sample[`p${p}`] = quantile(intervals, p / 100);
      sample.late = intervals.filter(ms => ms > 12.5).length;
      sample.late_per_s = sample.late / (arm === 'native' ? sample.fling_seconds : sample.gesture_seconds);
      Object.assign(sample, gpuMetrics(raw, 'fling', enabled, sample.fling_seconds, sample.fling_frames, path));
    }
    samples[arm].push(sample);
  }
}

const metrics = {};
for (const arm of arms) {
  if (fourArms) samples[arm].sort((a, b) => round(a.path) - round(b.path));
  const flings = samples[arm].filter(sample => sample.fling_seconds).length;
  assert(samples[arm].length > 0 && samples[arm].length <= 10, `${arm}: one to ten accepted launches`);
  assert(flings <= 5 && (flings > 0 || !hasSamples('fling')), `${arm}: one to five fling/rest samples, or a cold-only series`);
  const slots = samples[arm].map(sample => sample.path.split('/').at(-1).replace('-retry', ''));
  assert.equal(new Set(slots).size, slots.length, `${arm}: duplicate accepted slot`);
  const keys = new Set(samples[arm].flatMap(sample => Object.keys(sample)).filter(key => !['path', 'gpuHookEnabled'].includes(key)));
  metrics[arm] = Object.fromEntries([...keys].map(key => [key,
    distribution(samples[arm].filter(sample => key in sample).map(sample => sample[key])),
  ]));
}
const format = (arm, key, digits) => {
  const row = metrics[arm][key];
  return row ? `${row.median.toFixed(digits)} (${row.min.toFixed(digits)}–${row.max.toFixed(digits)})` : '—';
};
const header = '| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal |'
  + (fourArms ? ' egui 0.37.0 |' : '') + '\n|---|' + arms.map(() => '---:|').join('');
const table = rows => [header, ...rows.map(([label, key, digits]) =>
  `| ${label} | ${arms.map(arm => format(arm, key, digits)).join(' | ')} |`)].join('\n');
if (!hasSamples('fling')) {
  const cold = table([['Cold to first complete frame, ms; native snapshot / painted drawable', 'cold_ms', 2]]);
  if (mode === '--json') console.log(JSON.stringify({ ...hookMetadata(), samples, metrics, rejected, unused }, null, 2));
  else if (mode === '--verify') {
    const record = readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8');
    assert(record.includes(cold), `cold-only table differs:\n${cold}`);
    console.log(`${hookLabel()} Cold-only table reproduced exactly: ${Object.values(samples).flat().length} retained launches; ${rejected.length} rejected attempt${rejected.length === 1 ? '' : 's'}.`);
  } else console.log(`${hookLabel()}\n\n${cold}`);
  process.exit(0);
}
const resources = table([
  [fourArms ? 'Cold to first complete frame, ms; native snapshot / painted drawable' : 'Cold to first complete frame, ms; native snapshot / GPUI and Ink presented drawable', 'cold_ms', 2],
  ['Fling process CPU, ms/s', 'fling_cpu', 2],
  ['Fling process CPU per presented frame, ms', 'cpu_per_frame', 3],
  ['Fling main-thread CPU, ms/s', 'fling_main', 2],
  ...(['instructions', 'cycles'].filter(key => arms.some(arm => metrics[arm][`fling_${key}_million_s`]))
    .map(key => [`Fling process ${key}, million/s`, `fling_${key}_million_s`, 3])),
  ...(['effective_ghz', 'ipc'].filter(key => arms.some(arm => metrics[arm][`fling_${key}`]))
    .map(key => [key === 'ipc' ? 'Fling process instructions/cycle' : 'Fling process effective GHz', `fling_${key}`, 3])),
  ['Rest process CPU, ms/s', 'rest_cpu', 3],
  ['Rest main-thread CPU, ms/s', 'rest_main', 3],
  ...(['instructions', 'cycles'].filter(key => arms.some(arm => metrics[arm][`rest_${key}_million_s`]))
    .map(key => [`Rest process ${key}, million/s`, `rest_${key}_million_s`, 3])),
  ...(['effective_ghz', 'ipc'].filter(key => arms.some(arm => metrics[arm][`rest_${key}`]))
    .map(key => [key === 'ipc' ? 'Rest process instructions/cycle' : 'Rest process effective GHz', `rest_${key}`, 3])),
  ['Peak footprint during fling, MiB', 'peak_mib', 2],
  ['End-of-fling footprint, MiB', 'end_fling_mib', 2],
  ['End-of-rest footprint, MiB', 'end_rest_mib', 2],
]);
let cadence = table([
  ['Fling interval p50, ms', 'p50', 2],
  ['Fling interval p95, ms', 'p95', 2],
  ['Fling interval p99, ms', 'p99', 2],
  ['Late intervals/s', 'late_per_s', 3],
  ['Gesture-window duration, s', 'gesture_seconds', 3],
  ['Presented frames/s inside gesture windows', 'gesture_fps', 2],
  ...(fourArms ? [['Presented frames/s over the full fling phase', 'fling_fps', 2]] : []),
]);
cadence += '\n| Inputs received / posted per sample | ' + arms.map(arm => {
  const received = metrics[arm].received;
  const posted = metrics[arm].posted;
  assert.equal(posted.min, posted.max, 'one posted denominator per arm');
  return `${received.median}/${posted.median} (${received.min}–${received.max}/${posted.median})`;
}).join(' | ') + ' |';
cadence += '\n| Driver maxGapMs; maximum of four gestures | ' + arms.map(arm => format(arm, 'maxGapMs', 3)).join(' | ') + ' |';

if (fourArms) cadence += '\n| Driver empty callbacks per sample | ' + arms.map(arm => format(arm, 'emptyCallbacks', 0)).join(' | ') + ' |';
const gpuTable = table([
  ['Fling GPU busy time, ms/s; process command buffers', 'gpu_ms_s', 3],
  ['Fling GPU ms/presented frame', 'gpu_ms_frame', 4],
  ['Fling completed command buffers per sample', 'buffers', 0],
]);

if (mode === '--verify') {
  if (verifyPowerReport()) process.exit(0);
    const record = readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8').replace(/<br>G4: [^|]+/g, ' ');
  assert(record, `missing ${report} results`);
  for (const output of [resources, cadence]) assert(record.includes(output), `${report} table differs:\n${output}`);
  const retained = Object.values(samples).flat();
  console.log(`${hookLabel()} ${report} reproduced exactly: two tables, ${retained.length} retained launches, ${retained.filter(sample => sample.fling_seconds).length} fling/rest samples; ${rejected.length} rejected attempts.`);
} else if (mode === '--json') {
  console.log(JSON.stringify({ ...hookMetadata(), samples, metrics, rejected, ...(fourArms ? { unused } : {}) }, null, 2));
} else {
  console.log(`${hookLabel()}\n\n${resources}\n\n${cadence}\n\n${gpuTable}`);
}
