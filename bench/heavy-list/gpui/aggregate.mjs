// G3c's retained process samples. @ref LLP 1149 §4.1.
// bun bench/heavy-list/gpui/aggregate.mjs target/g3c [--verify | --json]
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const [directory = 'target/g3c', mode] = process.argv.slice(2);
assert([undefined, '--verify', '--json'].includes(mode), 'expected --verify or --json');
const arms = ['native', 'gpui', 'ink'];
const samples = Object.fromEntries(arms.map(arm => [arm, []]));
const rejected = [];
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

for (const phase of ['cold', 'fling']) {
  const folder = join(directory, 'series', phase);
  for (const file of readdirSync(folder).sort()) {
    if (!/^(native|gpui|ink)-\d+(-retry)?\.json$/.test(file)) continue;
    const path = join(folder, file);
    const raw = JSON.parse(readFileSync(path, 'utf8'));
    if (!raw.accepted) {
      rejected.push(path);
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
  assert.equal(samples[arm].length, 10, `${arm}: ten accepted launches`);
  assert.equal(samples[arm].filter(sample => sample.fling_seconds).length, 5, `${arm}: five fling/rest samples`);
  const keys = new Set(samples[arm].flatMap(sample => Object.keys(sample)).filter(key => key !== 'path'));
  metrics[arm] = Object.fromEntries([...keys].map(key => [key,
    distribution(samples[arm].filter(sample => key in sample).map(sample => sample[key])),
  ]));
}
const format = (arm, key, digits) => {
  const row = metrics[arm][key];
  return row ? `${row.median.toFixed(digits)} (${row.min.toFixed(digits)}–${row.max.toFixed(digits)})` : '—';
};
const header = '| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal |\n|---|---:|---:|---:|';
const table = rows => [header, ...rows.map(([label, key, digits]) =>
  `| ${label} | ${arms.map(arm => format(arm, key, digits)).join(' | ')} |`)].join('\n');
const resources = table([
  ['Cold to first complete frame, ms; native snapshot / GPUI and Ink presented drawable', 'cold_ms', 2],
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
]);
cadence += '\n| Inputs received / posted per sample | ' + arms.map(arm => {
  const received = metrics[arm].received;
  const posted = metrics[arm].posted;
  assert.equal(posted.min, posted.max, 'one posted denominator per arm');
  return `${received.median}/${posted.median} (${received.min}–${received.max}/${posted.median})`;
}).join(' | ') + ' |';
cadence += '\n| Driver maxGapMs; maximum of four gestures | ' + arms.map(arm => format(arm, 'maxGapMs', 3)).join(' | ') + ' |';

if (mode === '--verify') {
  const record = readFileSync(new URL('./RESULTS.md', import.meta.url), 'utf8').split('## G3c:')[1];
  assert(record, 'missing G3c results');
  for (const output of [resources, cadence]) assert(record.includes(output), `G3c table differs:\n${output}`);
  console.log(`G3c reproduced exactly: two tables, 30 retained launches, 15 fling/rest samples; ${rejected.length} rejected attempt.`);
} else if (mode === '--json') {
  console.log(JSON.stringify({ samples, metrics, rejected }, null, 2));
} else {
  console.log(`${resources}\n\n${cadence}`);
}
