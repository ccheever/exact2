#!/usr/bin/env bun
// Release Exact/Compose/platform Views comparison. Builds finish before measurement.
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cpus, loadavg, platform } from 'node:os';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const delay = ms => new Promise(done => setTimeout(done, ms));
const quote = value => `'${String(value).replaceAll("'", "'\\''")}'`;
const save = (path, value) => { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, value); };
const json = (path, value) => save(path, JSON.stringify(value, null, 2) + '\n');
const sources = {
  startup: 'https://developer.android.com/topic/performance/issues/launch-time',
  compilation: 'https://developer.android.com/reference/androidx/benchmark/macro/CompilationMode',
  art: 'https://source.android.com/docs/core/runtime/jit-compiler',
  memory: 'https://developer.android.com/topic/performance/memory/guide/tools-overview',
  memory_dump: 'https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/master/core/java/android/app/ActivityThread.java',
  frame_metrics: 'https://developer.android.com/reference/android/view/FrameMetrics',
  choreographer: 'https://developer.android.com/reference/android/view/Choreographer.FrameCallback',
  async_handler: 'https://developer.android.com/reference/android/os/Handler#createAsync(android.os.Looper)',
  compose_dispatcher: 'https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/ui/ui/src/androidMain/kotlin/androidx/compose/ui/platform/AndroidUiDispatcher.android.kt',
};

/** Nearest-rank percentiles, with raw samples retained elsewhere in the report. */
export function distribution(values) {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  const at = q => sorted.length ? sorted[Math.max(0, Math.ceil(sorted.length * q) - 1)] : null;
  return { samples: sorted.length, p50: at(.5), p95: at(.95), p99: at(.99), min: sorted[0] ?? null, max: sorted.at(-1) ?? null };
}

/** Logcat messages stay small; reject truncated, duplicate or missing raw chunks. */
export function parseFrameLog(log, renderer, frameSamples, scrollSamples, options = {}) {
  if (log.includes('FATAL EXCEPTION') || log.includes('CompareError:')) {
    throw new Error(`${renderer} comparison process reported a fatal/benchmark error, including after completion`);
  }
  const records = prefix => log.split('\n').map(line => line.match(new RegExp(`${prefix}:(\\{.*\\})`))?.[1]).filter(Boolean).map(JSON.parse);
  const results = records('CompareResult'), markers = records('CompareComplete'), chunks = records('CompareSamples');
  const scrollWorkload = options.heavyList ? 'heavy-scroll-1000' : 'scroll-1000';
  const expected = [...(options.scrollOnly || options.heavyList ? [] : ['counter', 'paint-1', 'paint-100', 'paint-1000', 'transform-1000']), ...(scrollSamples ? [scrollWorkload] : [])];
  if (markers.length !== 1 || markers[0].renderer !== renderer || markers[0].workloads !== expected.length || results.length !== expected.length) {
    throw new Error(`${renderer} comparison has missing/duplicate completion or workloads`);
  }
  for (const workload of expected) {
    const matches = results.filter(row => row.workload === workload && row.renderer === renderer);
    const count = workload === scrollWorkload ? scrollSamples : frameSamples;
    if (matches.length !== 1 || matches[0].actions !== count || matches[0].frame_metric_reports_dropped !== 0) {
      throw new Error(`${renderer}/${workload} invalid action count, duplicate result or lost FrameMetrics reports`);
    }
    if (matches[0].main_thread_cpu.samples !== count || matches[0].main_thread_cpu.p50_ms <= 0) {
      throw new Error(`${renderer}/${workload} missing positive main-thread CPU samples`);
    }
  }
  const series = [];
  if (scrollSamples) {
    const rows = chunks.filter(row => row.renderer === renderer && row.workload === scrollWorkload).sort((a, b) => a.start - b.start);
    const columns = rows[0]?.columns;
    let start = 0;
    for (const chunk of rows) {
      if (chunk.start !== start || JSON.stringify(chunk.columns) !== JSON.stringify(columns) || chunk.rows.some(row => row.length !== columns.length)) {
        throw new Error(`${renderer} scroll series has missing, duplicate or incompatible chunks`);
      }
      series.push(...chunk.rows);
      start += chunk.rows.length;
    }
    const scroll = results.find(row => row.workload === scrollWorkload);
    if (options.heavyList && (scroll.list_mode !== 'heavy-retained-1000' || scroll.retained_rows !== 1000 || scroll.scroll_speed_dp_per_second <= 0 ||
      (options.scrollSpeed !== undefined && scroll.scroll_speed_dp_per_second !== options.scrollSpeed))) {
      throw new Error(`${renderer} heavy-list mode, retained rows or configured velocity mismatch`);
    }
    if (options.heavyList && !['requested_vsync_ns', 'measured_vsync_ns', 'request_submitted_ns', 'requested_px', 'observed_draw_px'].every(name => columns?.includes(name))) {
      throw new Error(`${renderer} heavy-list raw series lacks request/draw timing or offsets`);
    }
    const column = name => columns?.indexOf(name) ?? -1;
    const durationColumns = [['window_frame', 'window_ns'], ['ui_phases', 'ui_ns'], ['layout_measure', 'layout_ns'],
      ['display_list_recording', 'display_list_ns'], ['gpu', 'gpu_ns'], ['frame_deadline', 'deadline_ns']];
    const windowIndex = column('window_ns'), drawIndex = column('completed_draw_ns') >= 0 ? column('completed_draw_ns') : column('action_to_completed_draw_cpu_ns');
    if (windowIndex < 0 || drawIndex < 0 || durationColumns.some(([, name]) => column(name) < 0)) {
      throw new Error(`${renderer} continuous scroll lacks raw observation masks`);
    }
    const observed = series.map(row => ({ window: row[windowIndex] >= 0, draw: row[drawIndex] >= 0 }));
    const metricCount = observed.filter(row => row.window).length, drawnCount = observed.filter(row => row.draw).length;
    const missingDrawMetrics = observed.filter(row => row.draw && !row.window).length;
    const metricsWithoutDraw = observed.filter(row => row.window && !row.draw).length;
    const eligibleCount = series.filter(row => row[windowIndex] >= 0 && row[column('deadline_ns')] >= 0 && row[column('first_draw')] === 0).length;
    const deadlineMisses = series.filter(row => row[windowIndex] >= 0 && row[column('deadline_ns')] >= 0 && row[column('first_draw')] === 0 && row[windowIndex] > row[column('deadline_ns')]).length;
    const deadlineFraction = eligibleCount ? deadlineMisses / eligibleCount : null;
    if ((deadlineFraction === null ? scroll.frame_deadline_miss_fraction !== null : Math.abs(scroll.frame_deadline_miss_fraction - deadlineFraction) > 1e-12) ||
      start !== scrollSamples || scroll.invalid_scroll_readbacks !== 0 || scroll.drawn_requests !== drawnCount ||
      scroll.undrawn_requests !== scrollSamples - drawnCount || scroll.frame_metrics_received !== metricCount ||
      scroll.drawn_requests_without_frame_metrics !== missingDrawMetrics || (!options.heavyList && missingDrawMetrics !== 0) ||
      scroll.frame_deadline_eligible !== eligibleCount || scroll.frame_deadline_misses !== deadlineMisses) {
      throw new Error(`${renderer} continuous scroll summary disagrees with raw observation masks`);
    }
    for (const [metric, name] of durationColumns) {
      const index = column(name), values = series.map(row => row[index]);
      if (values.some(value => value < -1) || scroll[metric].samples !== values.filter(value => value >= 0).length ||
        series.some(row => row[windowIndex] < 0 && row[index] !== -1)) {
        throw new Error(`${renderer} ${metric} summary or missing sentinel disagrees with raw observations`);
      }
    }
    if (series.some(row => row[windowIndex] < 0 && row[column('first_draw')] !== -1)) {
      throw new Error(`${renderer} absent Window metrics have fabricated first-draw observations`);
    }
    if (scroll.window_metrics_coverage !== undefined) {
      if (Math.abs(scroll.window_metrics_coverage - metricCount / scrollSamples) > 1e-12 || scroll.window_metrics_missing_value !== -1 ||
        scroll.requests_without_frame_metrics !== scrollSamples - metricCount || scroll.drawn_requests_with_frame_metrics !== drawnCount - missingDrawMetrics ||
        scroll.frame_metrics_without_completed_draw !== metricsWithoutDraw) {
        throw new Error(`${renderer} Window metrics coverage disagrees with raw observations`);
      }
    } else if (missingDrawMetrics > 0) {
      throw new Error(`${renderer} unavailable Window observations lack explicit coverage metadata`);
    }
    return { ...markers[0], results, scroll_series: { columns, rows: series, missing_value: -1, duration_unit: 'nanoseconds' } };
  }
  if (chunks.length) throw new Error('unexpected raw scroll series while scrolling is disabled');
  return { ...markers[0], results };
}

function memorySummary(readings) {
  const summary = {};
  for (const key of Object.keys(readings[0]).filter(key => key.endsWith('_kib'))) summary[key] = distribution(readings.map(row => row[key]));
  summary.native_allocated_kib = distribution(readings.map(row => row.native_heap.allocated_kib));
  summary.java_allocated_kib = distribution(readings.map(row => row.java_heap.allocated_kib));
  return summary;
}

function androidDuration(value) {
  const pieces = [...value.matchAll(/(\d+)(h|m(?!s)|s|ms)/g)];
  if (!pieces.length) return null;
  const units = { h: 3_600_000, m: 60_000, s: 1000, ms: 1 };
  return pieces.reduce((sum, piece) => sum + Number(piece[1]) * units[piece[2]], 0);
}

/** Only accept this activity's platform startup markers and this launch token. */
export function parseStartup(log, am, product, token) {
  const lines = log.split('\n');
  const displayed = lines.find(line => line.includes(`Displayed ${product.appId}/`));
  const fully = lines.find(line => line.includes(`Fully drawn ${product.appId}/`));
  const marker = lines.map(line => line.match(/Startup:(\{.*\})/)?.[1]).filter(Boolean)
    .map(line => JSON.parse(line)).find(row => row.token === token);
  const duration = line => line ? androidDuration(line.match(/:\s*\+([^ ]+)/)?.[1] ?? '') : null;
  const field = name => Number(am.match(new RegExp(`^${name}: (\\d+)$`, 'm'))?.[1]) || null;
  return { marker: marker ?? null, ttid_ms: duration(displayed), ttfd_ms: duration(fully),
    am_total_ms: field('TotalTime'), am_wait_ms: field('WaitTime'),
    platform_displayed_line: displayed ?? null, platform_fully_drawn_line: fully ?? null };
}

/** Android's resident summary categories and heap allocation columns are distinct. */
export function parseMemory(log) {
  const block = log.split('App Summary')[1]?.split('Objects')[0] ?? '';
  const number = label => {
    const match = block.match(new RegExp(`${label.replaceAll(' ', '\\s+')}\\s*:\\s*(\\d+)`));
    return match ? Number(match[1]) : null;
  };
  const heap = label => {
    const line = log.split('\n').find(line => new RegExp(`^\\s*${label}\\s+\\d`).test(line));
    const match = line?.match(new RegExp(`^\\s*${label}\\s+([\\d ]+)\\s*$`));
    if (!match) return null;
    const values = match[1].trim().split(/\s+/).map(Number);
    // -a adds Pss Clean, Shared Dirty and Shared Clean columns to the table.
    if (values.length !== 8 && values.length !== 11) return null;
    const [pss, privateDirty, privateClean, swapPss, rss, size, allocated, free] = values.length === 8 ? values
      : [values[0], values[3], values[5], values[6], values[7], values[8], values[9], values[10]];
    return { pss_kib: pss, private_dirty_kib: privateDirty, private_clean_kib: privateClean,
      swap_pss_kib: swapPss, rss_kib: rss, size_kib: size, allocated_kib: allocated, free_kib: free };
  };
  const result = { total_pss_kib: number('TOTAL PSS'), total_rss_kib: number('TOTAL RSS'),
    total_swap_pss_kib: number('TOTAL SWAP PSS'), java_resident_kib: number('Java Heap'),
    native_resident_kib: number('Native Heap'), code_resident_kib: number('Code'),
    stack_resident_kib: number('Stack'), graphics_resident_kib: number('Graphics'),
    private_other_resident_kib: number('Private Other'), system_pss_kib: number('System'),
    native_heap: heap('Native Heap'), java_heap: heap('Dalvik Heap') };
  if (result.total_pss_kib == null || result.total_rss_kib == null || !result.native_heap || !result.java_heap) {
    throw new Error('unrecognized dumpsys meminfo format; raw report retained');
  }
  return result;
}

function installedCode(shell, product, compilation, emulator) {
  const apkPaths = shell('pm', 'path', product.appId).split('\n').filter(line => line.startsWith('package:')).map(line => line.slice(8));
  const directory = dirname(apkPaths[0]);
  if (!directory.startsWith('/data/app/')) throw new Error('package code path is outside /data/app');
  let reader = shell, access = 'shell';
  const read = (...args) => {
    try { return reader(...args); }
    catch (error) {
      // Read this package's emulator artifacts without restarting adbd.
      if (access !== 'shell' || !emulator || !error.message.includes('Permission denied') ||
        !shell('su', '0', 'id').includes('uid=0(root)')) throw error;
      reader = (...values) => shell('su', '0', ...values);
      access = 'emulator-root-read-only';
      return reader(...args);
    }
  };
  const artState = shell('pm', 'art', 'dump', product.appId);
  const locations = [...artState.matchAll(/\[location is ([^\]]+)\]/g)].map(match => match[1]);
  const listings = [read('find', directory, '-type', 'f', '-exec', 'stat', '-c', '%s %n', '{}', '+')];
  for (const location of new Set(locations)) {
    if (location.startsWith(directory + '/')) continue;
    const encodedApks = apkPaths.map(path => path.slice(1).replaceAll('/', '@') + '@');
    if (!location.startsWith('/data/dalvik-cache/') || !encodedApks.some(prefix => basename(location).startsWith(prefix))) {
      throw new Error(`ART artifact is outside this package's known code paths: ${location}`);
    }
    // ART may put the package's odex, vdex and image in dalvik-cache rather
    // than /data/app. Select only the exact encoded APK's artifact stem.
    const stem = basename(location).replace(/\.(dex|odex)$/, '');
    const listing = read('find', dirname(location), '-maxdepth', '1', '-type', 'f', '-name', stem + '.*',
      '-exec', 'stat', '-c', '%s %n', '{}', '+');
    if (!listing.split('\n').some(line => line.endsWith(' ' + location))) throw new Error(`ART artifact missing: ${location}`);
    listings.push(listing);
  }
  const listing = listings.join('\n');
  const files = listing.split('\n').filter(Boolean).map(line => {
    const match = line.match(/^(\d+) (.+)$/);
    if (!match) throw new Error(`unrecognized installed file stat: ${line}`);
    return { bytes: Number(match[1]), path: match[2] };
  });
  const uniqueFiles = [...new Map(files.map(row => [row.path, row])).values()];
  const external = uniqueFiles.filter(row => !row.path.startsWith(directory + '/'));
  const du = read('du', '-k', '-s', directory, ...external.map(row => row.path));
  const allocatedKiB = du.split('\n').map(line => Number(line.split(/\s+/)[0]));
  if (allocatedKiB.some(value => !Number.isSafeInteger(value))) throw new Error('unrecognized installed filesystem block count');
  return { compilation, directory, access, files: uniqueFiles, art_state: artState, art_locations: locations,
    package_directory_logical_bytes: uniqueFiles.filter(row => row.path.startsWith(directory + '/')).reduce((sum, row) => sum + row.bytes, 0),
    external_art_logical_bytes: external.reduce((sum, row) => sum + row.bytes, 0), raw_stat: listing, raw_du: du,
    logical_bytes: uniqueFiles.reduce((sum, row) => sum + row.bytes, 0), filesystem_bytes: allocatedKiB.reduce((sum, value) => sum + value, 0) * 1024 };
}

function summarize(report) {
  const memoryGroups = new Map();
  for (const sample of report.memory) {
    const key = `${sample.rows}/${sample.renderer}`;
    if (!memoryGroups.has(key)) memoryGroups.set(key, []);
    memoryGroups.get(key).push(sample);
  }
  report.memory_round_summary = [...memoryGroups.values()].map(rounds => ({
    renderer: rounds[0].renderer, rows: rounds[0].rows, compilation: 'speed', processes: rounds.length,
    summary_scope: 'Distribution across fresh-process medians; each process median uses five correlated diagnostic readings',
    process_medians_kib: Object.fromEntries(Object.keys(rounds[0].summary).map(key =>
      [key, distribution(rounds.map(row => row.summary[key].p50))])),
    per_process: rounds.map(row => ({ round: row.round ?? 0, summary: row.summary })),
  }));
  const groups = new Map();
  for (const sample of report.startup) {
    const key = `${sample.compilation}/${sample.rows}/${sample.renderer}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(sample);
  }
  report.startup_summary = [...groups.values()].map(rows => ({
    renderer: rows[0].renderer, compilation: rows[0].compilation, rows: rows[0].rows,
    ttid_ms: distribution(rows.map(row => row.ttid_ms)), ttfd_ms: distribution(rows.map(row => row.ttfd_ms)),
    am_total_ms: distribution(rows.map(row => row.am_total_ms)),
    on_create_to_ready_draw_ms: distribution(rows.map(row =>
      (row.marker.first_ready_draw_elapsed_ns - row.marker.on_create_elapsed_ns) / 1e6)),
  }));
  const pairs = new Map();
  for (const sample of report.startup) {
    const key = `${sample.compilation}/${sample.rows}/${sample.pair}`;
    if (!pairs.has(key)) pairs.set(key, {});
    pairs.get(key)[sample.renderer] = sample;
  }
  for (const [renderer, baseline] of [['exact', 'compose'], ['exact', 'views'], ['compose', 'views']]) {
    if (!report.products[renderer] || !report.products[baseline]) continue;
    const suffix = `${renderer}_minus_${baseline}`;
    const differences = new Map();
    for (const pair of pairs.values()) {
      const left = pair[renderer], right = pair[baseline];
      if (!left || !right) continue;
      const key = `${left.compilation}/${left.rows}`;
      if (!differences.has(key)) differences.set(key, []);
      differences.get(key).push({ compilation: left.compilation, rows: left.rows,
        ttid: left.ttid_ms - right.ttid_ms, ttfd: left.ttfd_ms - right.ttfd_ms });
    }
    report[`startup_paired_${suffix}`] = [...differences.values()].map(rows => ({
      compilation: rows[0].compilation, rows: rows[0].rows,
      ttid_ms: distribution(rows.map(row => row.ttid)), ttfd_ms: distribution(rows.map(row => row.ttfd)),
    }));
    report[`memory_${suffix}`] = [100, 1000].flatMap(rows => {
      const left = report.memory.filter(row => row.rows === rows && row.renderer === renderer);
      const pairs = left.map(row => [row, report.memory.find(other => other.rows === rows && other.renderer === baseline && (other.round ?? 0) === (row.round ?? 0))])
        .filter(([, right]) => right);
      if (!pairs.length) return [];
      const deltas = Object.fromEntries(Object.keys(pairs[0][0].summary).map(key => [key,
        distribution(pairs.map(([left, right]) => left.summary[key].p50 == null || right.summary[key].p50 == null
          ? null : left.summary[key].p50 - right.summary[key].p50))]));
      return [{ rows, compilation: 'speed', pairs: pairs.length,
        summary_scope: 'Paired difference between fresh-process medians; five diagnostic readings per process are correlated',
        median_delta_kib: Object.fromEntries(Object.entries(deltas).map(([key, value]) => [key, value.p50])),
        paired_process_median_delta_kib: deltas }];
    });
    const left = report.products[renderer].footprint, right = report.products[baseline].footprint;
    if (left && right) report[`apk_${suffix}_bytes`] = Object.fromEntries([
      'apk_bytes', 'payload_compressed_bytes', 'payload_uncompressed_bytes', 'zip_signing_alignment_bytes',
    ].map(key => [key, left[key] - right[key]]));
  }
  const workloads = new Map();
  for (const round of report.frames) for (const row of round.results) {
    const key = `${round.renderer}/${row.workload}/${row.scroll_speed_dp_per_second ?? 0}`;
    if (!workloads.has(key)) workloads.set(key, []);
    workloads.get(key).push({ renderer: round.renderer, ...row });
  }
  report.frame_round_summary = [...workloads.values()].map(rounds => {
    const metrics = Object.keys(rounds[0]).filter(key => rounds[0][key] && typeof rounds[0][key] === 'object'
      && Number.isFinite(rounds[0][key].p50_ms));
    return { renderer: rounds[0].renderer, workload: rounds[0].workload, scroll_speed_dp_per_second: rounds[0].scroll_speed_dp_per_second ?? null, rounds: rounds.length,
      summary_scope: 'Median of per-round p50, p95 and p99; not pooled raw-sample percentiles',
      metrics: Object.fromEntries(metrics.map(key => [key, {
        median_round_p50_ms: distribution(rounds.map(row => row[key]?.p50_ms)).p50,
        median_round_p95_ms: distribution(rounds.map(row => row[key]?.p95_ms)).p50,
        median_round_p99_ms: distribution(rounds.map(row => row[key]?.p99_ms)).p50,
        per_round: rounds.map(row => row[key]),
      }])), per_round_counts: rounds.map(row => Object.fromEntries([
        'actions', 'frame_metric_reports_dropped', 'frame_deadline_misses', 'frame_deadline_eligible',
        'drawn_requests', 'undrawn_requests', 'frame_metrics_received', 'estimated_skipped_vsync_intervals',
        'scroll_turnarounds', 'scroll_readback_mismatches',
      ].filter(key => row[key] !== undefined).map(key => [key, row[key]]))) };
  });
  report.post_workload_memory_summary = [...new Set(report.frames.map(row => row.renderer))].flatMap(renderer => {
    const rounds = report.frames.filter(row => row.renderer === renderer && row.settled_memory);
    return rounds.length ? [{ renderer, processes: rounds.length,
      scope: 'Settled foreground memory after all preceding update workloads and optional continuous scroll in the same process; five correlated readings per process',
      process_medians_kib: Object.fromEntries(Object.keys(rounds[0].settled_memory.summary).map(key =>
        [key, distribution(rounds.map(row => row.settled_memory.summary[key].p50))])) }] : [];
  });
}

/** Measure only already-built, non-debuggable release APKs. No build work occurs here. */
export async function compareAndroid(app, products, options = {}) {
  const env = { ...process.env, ...options.env };
  const sdk = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT;
  if (!sdk) throw new Error('Set ANDROID_HOME for the Android comparison');
  const adbPath = resolve(sdk, 'platform-tools', process.platform === 'win32' ? 'adb.exe' : 'adb');
  const serial = options.serial ?? 'emulator-5554';
  const adb = (...args) => {
    const output = spawnSync(adbPath, ['-s', serial, ...args], { env, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
    if (output.status !== 0) throw new Error(`adb ${args.slice(0, 3).join(' ')}: ${output.stderr?.trim() ?? output.error?.message}`);
    return output.stdout.trim();
  };
  // adb shell reconstructs command text; quote every argument before crossing it.
  const shell = (...args) => adb('shell', args.map(quote).join(' '));
  const destination = options.out ?? resolve(dirname(products.exact.apk), options.smoke ? 'android-comparison-smoke.json' : 'android-comparison.json');
  const evidence = resolve(dirname(destination), basename(destination, '.json') + '-evidence');
  const targets = Object.entries(products);
  const orderedTargets = round => {
    const shift = round % targets.length;
    return [...targets.slice(shift), ...targets.slice(0, shift)];
  };
  const samples = options.samples ?? (options.smoke ? 1 : 20);
  const heavyList = options.heavyList === true;
  const scrollOnly = heavyList || options.scrollOnly === true;
  const scrollSpeeds = options.scrollSpeeds ?? (heavyList ? [240, 1200, 4800] : [0]);
  const startupEnabled = options.startup ?? !heavyList;
  const memoryEnabled = !options.smoke && (options.memory ?? !heavyList);
  const verifySamples = options.verifySamples ?? (options.smoke || heavyList ? 0 : 10);
  const memoryRounds = options.memoryRounds ?? 3;
  const frameSamples = options.frameSamples ?? (options.smoke ? 20 : 120);
  const scrollSamples = options.scrollSamples ?? (heavyList ? 900 : 0);
  if (!Array.isArray(scrollSpeeds) || !scrollSpeeds.length || new Set(scrollSpeeds).size !== scrollSpeeds.length ||
    scrollSpeeds.some(speed => !Number.isFinite(speed) || speed < 0 || speed > 20_000 || (heavyList && speed === 0))) {
    throw new Error("scrollSpeeds must be distinct velocities in 0..20000 dp/s; heavy-list velocities must be positive");
  }
  if (scrollOnly && scrollSamples === 0) throw new Error("scroll-only/heavy-list requires measured scroll samples");
  if (!Number.isSafeInteger(samples) || samples < (options.smoke ? 1 : 2) || !Number.isSafeInteger(verifySamples) || verifySamples < 0) {
    throw new Error('startup sample counts must be integers (samples >= 2, or smoke sample >= 1; verifySamples >= 0)');
  }
  if (!Number.isSafeInteger(memoryRounds) || memoryRounds < 1 || !Number.isSafeInteger(frameSamples) || frameSamples < 1 ||
    !Number.isSafeInteger(scrollSamples) || scrollSamples < 0 || scrollSamples > 6000) {
    throw new Error('memoryRounds/frameSamples must be positive integers; scrollSamples must be an integer in 0..6000 (zero disables continuous scrolling)');
  }
  const fontScale = shell('settings', 'get', 'system', 'font_scale');
  if (Number(fontScale) !== 1) throw new Error(`comparison fixture requires font_scale=1; current value is ${fontScale} (Compose uses sp, Exact/Views use density-scaled px)`);
  const packageHelp = shell('pm', 'help');
  const artHelp = packageHelp.split(/\n\s*art SUB_COMMAND\b/)[1] ?? '';
  if (!/\bclear-app-profiles PACKAGE_NAME\b/.test(artHelp) || !/\bdump \[PACKAGE_NAME\]/.test(artHelp)) {
    throw new Error('comparison requires ART Service pm art clear-app-profiles and dump commands; validated on API 36, independent of the core minSdk 29');
  }
  save(resolve(evidence, 'tooling-pm-help.txt'), packageHelp + '\n');
  for (const [renderer, product] of targets) {
    if (!product.release || product.release.debuggable || !product.appId || !product.activity) throw new Error(`${renderer} is not a declared release APK product`);
    if (options.install !== false) console.log(`${renderer}: ${adb('install', '--no-incremental', '-r', product.apk)}`);
    const installed = shell('dumpsys', 'package', product.appId);
    if (/flags=\[[^\]]*\bDEBUGGABLE\b/i.test(installed)) throw new Error(`${renderer} package is debuggable`);
    save(resolve(evidence, `${renderer}-package.txt`), installed + '\n');
  }
  const property = key => shell('getprop', key);
  const initial = {
    generated_at: new Date().toISOString(), fixture: app.name, smoke: options.smoke ?? false,
    run_configuration: { startup_samples: samples, verify_samples: verifySamples, memory_rounds: memoryRounds,
      frame_samples: frameSamples, scroll_samples: scrollSamples, frame_rounds: options.smoke ? 1 : (options.frameRounds ?? 3),
      startup_enabled: startupEnabled, memory_enabled: memoryEnabled,
      list_mode: heavyList ? "heavy-retained-1000" : "simple-retained", scroll_only: scrollOnly, scroll_speeds_dp_per_second: scrollSpeeds,
      frames_enabled: options.frames !== false, post_frame_memory: !options.smoke && options.postFrameMemory !== false,
      installation_mode: options.install === false ? 'preinstalled' : 'adb --no-incremental -r' },
    build_versions: options.versions ?? products.exact.versions ?? null,
    device: { serial,
      model: property('ro.product.model'), manufacturer: property('ro.product.manufacturer'),
      android_api: Number(property('ro.build.version.sdk')), abi: property('ro.product.cpu.abi'),
      emulator: property('ro.kernel.qemu') === '1', gpu: property('ro.hardware.egl'),
      display: shell('wm', 'size'), density: shell('wm', 'density'), font_scale: fontScale,
      surfaceflinger_gles: shell('dumpsys', 'SurfaceFlinger').split('\n').find(line => line.includes('GLES:'))?.trim() ?? null,
      animations: Object.fromEntries(['window_animation_scale', 'transition_animation_scale', 'animator_duration_scale']
        .map(key => [key, shell('settings', 'get', 'global', key)])) },
    host: { platform: platform(), cpu: cpus()[0]?.model, logical_cpus: cpus().length, load_at_start: loadavg() },
    methodology: {
      prerequisites: 'Fixture font_scale=1: Compose uses sp and Exact/Views use density-scaled px. Comparison tooling requires ART Service pm art clear-app-profiles and dump commands; core minSdk does not guarantee these tools',
      release: 'Non-debuggable release APKs; matched build optimization and ABI are recorded per product',
      installation: options.install === false ? 'Preinstalled artifacts; caller owns install-mode parity' : 'All APKs use adb install --no-incremental -r, avoiding automatic .idsig-driven incremental install for only one renderer',
      startup: 'Cold app process: force-stop all reference apps before each launch; launcher MAIN intent; OS file/page cache remains warm; one discarded prime launch per renderer/configuration; renderer order rotates every matched round',
      ttid: 'Android ActivityTaskManager Displayed marker; launch intent to first destination-window display',
      ttfd: 'Android ActivityTaskManager Fully drawn marker used as TTI proxy; scene ready and complete root drawing before reportFullyDrawn, with no network or delayed data. Interactive fixture behavior is validated separately, not timed by this marker. Exact selects its requested row count through an authored initial press before the first kernel layout and presenter publication; references initialize the requested rows directly',
      am_total: 'am start -W TotalTime retained separately as cross-check; WaitTime includes shell wait overhead',
      compilation_speed: 'cmd package compile -f -m speed: controlled full AOT, removes much JIT variability, not representative fresh-install compilation',
      compilation_verify: 'clear local profiles then cmd package compile -f -m verify: no AOT app methods; JIT remains enabled, each launch uses a new process',
      memory: `Idle foreground process after complete eager scene and 5 seconds settling; ${memoryRounds} fresh-process rounds per row count, renderer order rotates. Each process supplies 5 correlated dumpsys meminfo -a readings 500 ms apart; summary distribution uses process medians. Android diagnostic dumping requests Java GC: native allocator totals and the supplied resident snapshot precede that request, Java heap totals follow it; later resident samples reflect preceding diagnostic GC. No additional harness GC. Heap allocator occupancy is distinct from PSS/RSS and includes platform allocations`,
      memory_post_workloads: 'After each fresh-process frame round, five correlated readings after 5 seconds settling; includes retained allocations from preceding counter, paint, transform and optional scroll workloads in that same process, not an isolated scroll allocation experiment',
      memory_units: 'KiB (1024 bytes); PSS proportionally shares mappings, RSS fully counts resident shared mappings; summary Java/Native categories describe resident memory, heap allocated columns describe allocator occupancy',
      installed_code: 'Logical file bytes and filesystem allocated blocks for this package code directory plus its ART-reported external dalvik-cache artifacts: APK, extracted libraries, oat/dex/vdex/art. External files are selected only by the exact encoded APK artifact stem; excludes private app data, framework/shared files and Play download size',
      frames: `${options.smoke ? 1 : (options.frameRounds ?? 3)} fresh-process rounds per velocity, renderer order rotates; shared completed-window-frame harness; 30 warmups, ${scrollOnly ? "no isolated update actions" : `${frameSamples} measured isolated actions per update workload`}. Summaries are medians of per-round p50/p95/p99, not pooled raw-sample percentiles`,
      action_injection: 'Controller callback invocation: Exact View.callOnClick and direct reference actions; excludes pointer recognition and platform click sound/accessibility feedback',
      scrolling: scrollSamples ? `Separate continuous programmatic scroll phase: 1000 eager retained rows, 30 warmup callbacks then ${scrollSamples} measured callbacks. ${heavyList ? "Rich 200dp cards with wrapped ASCII text, letter avatar, metadata and badges" : "Simple text rows"}. Velocities ${scrollSpeeds.join(",")} dp/s (zero preserves fixed 64dp steps) posted between frames through a common async Handler; positive speed computes absolute triangular position from requested next nominal VSYNC timestamp, catching up after callback gaps instead of slowing the configured trajectory; no wait for completed draw, no touch/fling or virtualization. Scroll range and viewport must match all renderers within 1 pixel. Main-thread CPU uses consecutive callback threadCpuTime deltas and includes common harness overhead; window/UI/display-list/GPU phases come from matching FrameMetrics. Raw bounded Logcat chunks are streamed by an owned adb process. Deadline misses mean TOTAL_DURATION>DEADLINE among observed eligible reports excluding first draw. Heavy-list unavailable Window observations remain -1 with explicit coverage; dropped observer reports, invalid observation masks and invalid readbacks fail validation. Observer-loss counts cover callbacks during the measured interval and async drain, including unmatched reports and possible earlier observer loss. Estimated skipped VSYNC intervals use nominal display refresh cadence (warmup median fallback) and are not compositor dropped frames` : 'Disabled; isolated update actions do not measure continuous scrolling',
      emulator: 'Emulator GPU and host scheduling make these comparative fixture results, not physical-device FPS or production startup guarantees',
    },
    sources, products: Object.fromEntries(targets.map(([renderer, product]) => [renderer, {
      app_id: product.appId, activity: product.activity, apk: product.apk,
      apk_sha256: createHash('sha256').update(readFileSync(product.apk)).digest('hex'),
      release: product.release, footprint: product.footprint, variant: product.variant ?? renderer,
      prototype: product.prototype ?? null, library: product.library ?? null, receipt: product.receipt ?? null,
    }])), startup: [], memory: [], frames: [],
  };
  const report = options.resume ? JSON.parse(readFileSync(destination, 'utf8')) : initial;
  if (options.resume) {
    if (JSON.stringify(report.run_configuration) !== JSON.stringify(initial.run_configuration)) throw new Error('cannot resume comparison with a different/missing run configuration');
    for (const [renderer] of targets) if (report.products[renderer]?.apk_sha256 !== initial.products[renderer].apk_sha256) {
      throw new Error(`cannot resume ${renderer} comparison: APK hash changed`);
    }
    (report.resumptions ??= []).push({ at: new Date().toISOString(), host_load: loadavg(), reason: options.resumeReason ?? 'Resume preserved measurements' });
  }
  const checkpoint = () => { summarize(report); json(destination, report); };
  checkpoint();
  const log = () => adb('logcat', '-d', '-b', 'all', '-v', 'raw', '-s', 'ActivityTaskManager:I', 'ActivityManager:I', 'ExactCompare:I', 'AndroidRuntime:E', '*:S');
  const captureLog = () => {
    // Own this one adb child; continuously drain the pipe so a final raw-series
    // burst can exceed the device's ring buffer without changing global settings.
    const child = spawn(adbPath, ['-s', serial, 'logcat', '-b', 'all', '-v', 'raw', '-s',
      'ActivityTaskManager:I', 'ActivityManager:I', 'ExactCompare:I', 'AndroidRuntime:E', '*:S'], { env, stdio: ['ignore', 'pipe', 'pipe'] });
    let text = '', stderr = '', failure = null, ended = false;
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => {
      text += chunk;
      if (text.length > 32 * 1024 * 1024) { failure = new Error('streamed comparison log exceeds 32MiB'); child.kill('SIGTERM'); }
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', error => { failure = error; });
    const exited = new Promise(done => child.on('close', () => { ended = true; done(); }));
    return {
      read() { if (failure) throw failure; if (ended) throw new Error(`owned adb logcat exited early: ${stderr}`); return text; },
      async close() {
        if (!ended) child.kill('SIGTERM');
        await Promise.race([exited, delay(5000)]);
        if (!ended) { child.kill('SIGKILL'); await exited; }
        return text;
      },
    };
  };
  const memoryReadings = async (renderer, product, name) => {
    await delay(5000);
    const readings = [];
    for (let i = 0; i < 5; i++) {
      const raw = shell('dumpsys', 'meminfo', '-a', product.appId);
      save(resolve(evidence, `${name}-${renderer}-${i}.txt`), raw + '\n');
      readings.push(parseMemory(raw));
      if (i < 4) await delay(500);
    }
    return { summary: memorySummary(readings), readings, correlated_readings: 5, settle_ms: 5000 };
  };
  const stopAll = () => { for (const [, product] of targets) shell('am', 'force-stop', product.appId); };
  const launch = async (renderer, product, rows, name) => {
    stopAll();
    shell('input', 'keyevent', 'KEYCODE_HOME');
    await delay(500);
    adb('logcat', '-c');
    const token = `${name}-${renderer}-${rows}-${Date.now()}`;
    const am = shell('am', 'start', '-W', '-n', `${product.appId}/${product.activity}`,
      '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER',
      '--ez', 'exact_startup', 'true', '--ei', 'exact_startup_rows', rows, '--es', 'exact_startup_token', token);
    const until = Date.now() + 10_000;
    let text = '', parsed;
    do {
      await delay(100);
      text = log();
      parsed = parseStartup(text, am, product, token);
      if (parsed.marker && parsed.ttid_ms != null && parsed.ttfd_ms != null) break;
      if (text.includes('FATAL EXCEPTION')) break;
    } while (Date.now() < until);
    save(resolve(evidence, `${name}-${renderer}-${rows}-startup.txt`), am + '\n\n' + text + '\n');
    if (!parsed.marker || parsed.ttid_ms == null || parsed.ttfd_ms == null) {
      throw new Error(`${renderer} startup missing scene-ready/Displayed/Fully drawn markers; see ${evidence}`);
    }
    if (parsed.marker.renderer !== renderer || parsed.marker.rows !== rows || parsed.ttfd_ms < parsed.ttid_ms) {
      throw new Error(`${renderer} startup has mismatched scene or TTFD < TTID; see ${evidence}`);
    }
    return { renderer, rows, token, ...parsed };
  };
  const compile = mode => {
    stopAll();
    for (const [renderer, product] of targets) {
      let profiles = '';
      if (mode === 'verify') profiles = shell('pm', 'art', 'clear-app-profiles', product.appId);
      const result = shell('cmd', 'package', 'compile', '-f', '-m', mode, product.appId);
      if (!result.includes('Success')) throw new Error(`${renderer} compilation ${mode}: ${result}`);
      save(resolve(evidence, `${renderer}-compile-${mode}.txt`), profiles + '\n' + result + '\n');
      save(resolve(evidence, `${renderer}-art-state-${mode}.txt`), shell('pm', 'art', 'dump', product.appId) + '\n');
    }
  };
  const measureStartups = async (compilation, rows, count) => {
    if (targets.every(([renderer]) => report.startup.filter(row => row.compilation === compilation && row.rows === rows && row.renderer === renderer).length >= count)) return;
    for (const [renderer, product] of targets) await launch(renderer, product, rows, `${compilation}-prime`);
    for (let pair = 0; pair < count; pair++) {
      const order = orderedTargets(pair);
      for (const [renderer, product] of order) {
        if (report.startup.some(row => row.compilation === compilation && row.rows === rows && row.renderer === renderer && row.pair === pair)) continue;
        report.startup.push({ compilation, pair, ...await launch(renderer, product, rows, `${compilation}-${pair}`) });
      }
      console.log(`Startup ${compilation}/${rows}: round ${pair + 1}/${count}`);
      checkpoint();
    }
  };
  compile('speed');
  if (startupEnabled) for (const rows of [100, 1000]) await measureStartups('speed', rows, samples);
  for (const rows of memoryEnabled ? [100, 1000] : []) {
    for (let round = 0; round < memoryRounds; round++) for (const [renderer, product] of orderedTargets(round)) {
      if (report.memory.some(row => row.renderer === renderer && row.rows === rows && (row.round ?? 0) === round)) continue;
      const startup = await launch(renderer, product, rows, `memory-${round}`);
      const memory = await memoryReadings(renderer, product, `memory-${round}-${rows}`);
      report.memory.push({ renderer, rows, round, compilation: 'speed', startup_marker: startup.marker, ...memory });
      console.log(`Memory ${rows}: round ${round + 1}/${memoryRounds} ${renderer}`);
      checkpoint();
    }
  }
  const collectStorage = mode => {
    for (const [renderer, product] of targets) {
      let code;
      try { code = installedCode(shell, product, mode, report.device.emulator); }
      catch (error) { code = { error: error.message, compilation: mode }; }
      (report.products[renderer].installed_code_modes ??= {})[mode] = code;
      if (mode === 'speed') report.products[renderer].installed_code = code;
      save(resolve(evidence, `${renderer}-installed-code-${mode}.txt`), JSON.stringify(code, null, 2) + '\n');
    }
    checkpoint();
  };
  collectStorage('speed');
  if (verifySamples && startupEnabled) {
    compile('verify');
    collectStorage('verify');
    await measureStartups('verify', 100, verifySamples);
    compile('speed'); // Match runtime compilation with the startup/memory baseline.
  }
  if (options.frames !== false) {
    const rounds = options.smoke ? 1 : (options.frameRounds ?? 3);
    for (let round = 0; round < rounds; round++) for (let speedIndex = 0; speedIndex < scrollSpeeds.length; speedIndex++) {
      const speed = scrollSpeeds[speedIndex];
      for (const [renderer, product] of orderedTargets(round + speedIndex)) {
        if (report.frames.some(row => row.renderer === renderer && row.round === round && (row.scroll_speed_dp_per_second ?? 0) === speed)) continue;
        stopAll();
        shell('input', 'keyevent', 'KEYCODE_HOME');
        await delay(500);
        adb('logcat', '-c');
        const stream = captureLog();
        let am = '';
        const deadline = Date.now() + Math.max(180_000, scrollSamples * 100 + 120_000);
        let text = '', announced = 0, complete = null, rows = [], failure = null;
        try {
          am = shell('am', 'start', '-W', '-n', `${product.appId}/${product.activity}`,
            '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER',
            '--ez', 'exact_compare', 'true', '--ei', 'exact_samples', frameSamples,
            '--ei', 'exact_scroll_samples', scrollSamples, '--ez', 'exact_scroll_only', String(scrollOnly),
            '--ez', 'exact_heavy_list', String(heavyList), '--ef', 'exact_scroll_speed', speed);
          do {
            await delay(1000);
            text = stream.read();
            text = text.slice(0, text.lastIndexOf('\n') + 1); // A pipe chunk can end midway through JSON.
            rows = text.split('\n').map(line => line.match(/CompareResult:(\{.*\})/)?.[1]).filter(Boolean).map(JSON.parse);
            for (const row of rows.slice(announced)) console.log(`Frame round ${round + 1}/${rounds} ${renderer} speed=${speed}dp/s: ${row.workload}`);
            announced = rows.length;
            const marker = text.split('\n').map(line => line.match(/CompareComplete:(\{.*\})/)?.[1]).find(Boolean);
            complete = marker ? JSON.parse(marker) : null;
            failure = text.split('\n').find(line => line.includes('CompareError:')) ?? null;
            if (failure || text.includes('FATAL EXCEPTION')) break;
          } while (!complete && Date.now() < deadline);
          // Completion must survive cleanup, including in smoke where no later
          // meminfo dump would otherwise expose a process dying after its marker.
          if (complete && !failure) await delay(250);
        } finally {
          text = await stream.close();
          save(resolve(evidence, `${renderer}-frames-${round}-speed-${speed}.txt`), am + '\n\n' + text + '\n');
        }
        if (failure) throw new Error(`${renderer} frame comparison: ${failure}; see ${evidence}`);
        let parsed;
        try { parsed = parseFrameLog(text, renderer, frameSamples, scrollSamples, { scrollOnly, heavyList, scrollSpeed: speed }); }
        catch (error) { throw new Error(`${error.message}; see ${evidence}`); }
        let processIds;
        try { processIds = shell('pidof', product.appId); }
        catch (error) { throw new Error(`${renderer} process liveness not confirmed after completion: ${error.message}; see ${evidence}`); }
        if (!/^\d+(\s+\d+)*$/.test(processIds)) throw new Error(`${renderer} process exited after benchmark completion; see ${evidence}`);
        const settledMemory = options.smoke || options.postFrameMemory === false ? null :
          await memoryReadings(renderer, product, `post-workloads-${round}-speed-${speed}`);
        report.frames.push({ renderer, round, scroll_speed_dp_per_second: speed, list_mode: heavyList ? 'heavy-retained-1000' : 'simple-retained', compilation: 'speed', ...parsed,
          process_alive_after_completion: { pids: processIds.split(/\s+/).map(Number), quiet_interval_ms: 250 }, settled_memory: settledMemory });
        if (scrollSamples) {
          const all = report.frames.filter(row => (row.scroll_speed_dp_per_second ?? 0) === speed)
            .map(row => row.results.find(result => result.workload === (heavyList ? 'heavy-scroll-1000' : 'scroll-1000'))).filter(Boolean);
          for (const field of ['scroll_range_px', 'scroll_viewport_px', ...(speed === 0 ? ['scroll_step_px'] : ['scroll_speed_dp_per_second'])]) {
            const values = all.map(row => row[field]);
            if (Math.max(...values) - Math.min(...values) > 1) {
              checkpoint();
              throw new Error(`scroll geometry disagreement: ${field}=${values.join(',')}; preserve evidence, fix parity and rerun`);
            }
          }
        }
        checkpoint();
      }
    }
  }
  report.host.load_at_end = loadavg();
  report.finished_at = new Date().toISOString();
  checkpoint();
  stopAll();
  shell('am', 'start', '-W', '-n', `${products.exact.appId}/${products.exact.activity}`, '--ez', 'exact_heavy_list', String(heavyList));
  console.log(`Android comparison: ${destination}`);
  return report;
}
