// Retained three- or four-arm process samples. @ref LLP 1149 §4.1.
// bun bench/heavy-list/gpui/aggregate.mjs target/g3c [--verify | --json]
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const [directory = 'target/g3c', mode] = process.argv.slice(2);
assert([undefined, '--verify', '--json'].includes(mode), 'expected --verify or --json');
const fourArms = readdirSync(join(directory, 'series', 'fling')).some(file => /^egui-/.test(file));
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

const phases = ['cold', 'fling'];
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
    const sample = { path, cold_ms: raw.coldMs };
    if (phase === 'fling') {
      for (const part of ['fling', 'rest']) {
        const { start, end } = raw[part];
        const seconds = end.time - start.time;
        assert(Math.abs(seconds - 10) < 0.05, `${path}: ${part} duration`);
        sample[`${part}_seconds`] = seconds;
        for (const counter of ['cpu', 'main']) {
          sample[`${part}_${counter}`] = (end[counter] - start[counter]) * 1000 / seconds;
        }
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
    }
    samples[arm].push(sample);
  }
}

const metrics = {};
for (const arm of arms) {
  if (fourArms) samples[arm].sort((a, b) => round(a.path) - round(b.path));
  const flings = samples[arm].filter(sample => sample.fling_seconds).length;
  assert(samples[arm].length > 0 && samples[arm].length <= 10, `${arm}: one to ten accepted launches`);
  assert(flings > 0 && flings <= 5, `${arm}: one to five fling/rest samples`);
  const slots = samples[arm].map(sample => sample.path.split('/').at(-1).replace('-retry', ''));
  assert.equal(new Set(slots).size, slots.length, `${arm}: duplicate accepted slot`);
  const keys = new Set(samples[arm].flatMap(sample => Object.keys(sample)).filter(key => key !== 'path'));
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
const resources = table([
  [fourArms ? 'Cold to first complete frame, ms; native snapshot / painted drawable' : 'Cold to first complete frame, ms; native snapshot / GPUI and Ink presented drawable', 'cold_ms', 2],
  ['Fling process CPU, ms/s', 'fling_cpu', 2],
  ['Fling process CPU per presented frame, ms', 'cpu_per_frame', 3],
  ['Fling main-thread CPU, ms/s', 'fling_main', 2],
  ['Rest process CPU, ms/s', 'rest_cpu', 3],
  ['Rest main-thread CPU, ms/s', 'rest_main', 3],
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

if (mode === '--verify') {
  const record = readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8').split(`## ${report}:`)[1];
  assert(record, `missing ${report} results`);
  for (const output of [resources, cadence]) assert(record.includes(output), `${report} table differs:\n${output}`);
  const retained = Object.values(samples).flat();
  console.log(`${report} reproduced exactly: two tables, ${retained.length} retained launches, ${retained.filter(sample => sample.fling_seconds).length} fling/rest samples; ${rejected.length} rejected attempts.`);
} else if (mode === '--json') {
  console.log(JSON.stringify({ samples, metrics, rejected, ...(fourArms ? { unused } : {}) }, null, 2));
} else {
  console.log(`${resources}\n\n${cadence}`);
}
