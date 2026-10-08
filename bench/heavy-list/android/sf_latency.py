#!/usr/bin/env python3
"""Independent SurfaceFlinger presentation cohort for all four release renderers.

--latency is a small rolling history, not a cumulative counter. This collector
keeps overlapping snapshots, binds the selected layer to the verified process,
and rejects coverage gaps. Its CPU/memory probes include collector overhead and
must not replace the untraced cohort in run.py. No renderer is modified.
"""
import argparse
import json
import math
import pathlib
import re
import shlex
import statistics
import threading
import time
import uuid

from run import Device, RENDERERS, launch, load_products, save, sha, source_manifest

MAX_TIME = (1 << 63) - 1
UPTIME_QUANTUM_NS = 10_000_000
CLOCK_PAIR_TOLERANCE_NS = 1_000_000
SOURCES = {
    'columns': 'https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/FrameTracker.cpp',
    'ring': 'https://android.googlesource.com/platform/frameworks/native/+/cdb6b16dec3a541b455be99d075004cb2f0a0cd7/services/surfaceflinger/FrameTracker.h',
    'selection': 'https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/SurfaceFlinger.cpp',
    'ready_fallback': 'https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/Layer.cpp',
    'timeline_history': 'https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/FrameTimeline/FrameTimeline.cpp',
}


def parse_layers(text):
    """Galaxy's RequestedLayerState inventory: preserve its entire #id name."""
    layers = {}
    for line in text.splitlines():
        match = re.match(r'^RequestedLayerState\{(.*)\}$', line)
        if not match:
            continue
        body = match.group(1)
        name = re.split(r'\s+(?:parentId|relativeParentId|z)=', body, maxsplit=1)[0]
        suffix = re.search(r'#(\d+)$', name)
        if not suffix:
            continue
        parent = re.search(r'\bparentId=(\d+)', body)
        key = int(suffix.group(1))
        assert key not in layers, 'duplicate layer ID'
        layers[key] = {'id': key, 'name': name, 'parent': int(parent.group(1)) if parent else None}
    return layers


def select_layer(text, product, variant, pid):
    layers = parse_layers(text)
    window_name = product['appId'] + '/' + product['activity'] + '$_' + str(pid)
    windows = [x for x in layers.values() if re.fullmatch(re.escape(window_name) + r'#\d+', x['name'])]
    if not windows:
        return None
    assert len(windows) == 1, 'ambiguous current-process Window layer'
    window = windows[0]
    if variant != 'main':
        return {'window': window, 'content': window}
    def descendant(layer):
        visited = set()
        while layer and layer['id'] not in visited:
            if layer['id'] == window['id']:
                return True
            visited.add(layer['id'])
            layer = layers.get(layer['parent'])
        return False
    identifier = 'SurfaceView[' + product['appId'] + '/' + product['activity'] + ']'
    content = [x for x in layers.values() if identifier in x['name'] and
               re.search(r'\(BLAST\)#\d+$', x['name']) and descendant(x)]
    if not content:
        return None
    assert len(content) == 1, 'ambiguous main BLAST child of current-process Window'
    return {'window': window, 'content': content[0]}


def uptime_ns(line):
    token = line.split()[0]
    assert re.fullmatch(r'\d+\.\d{2}', token), 'unexpected /proc/uptime precision'
    seconds, cents = token.split('.')
    return int(seconds) * 1_000_000_000 + int(cents) * 10_000_000


def parse_dump(text):
    lines = [x.strip() for x in text.splitlines() if x.strip()]
    assert len(lines) >= 3, 'missing latency or device clock bounds'
    before, after = uptime_ns(lines[0]), uptime_ns(lines[-1])
    assert after >= before, 'device uptime moved backwards'
    assert re.fullmatch(r'\d+', lines[1]), 'missing --latency display period header'
    header = int(lines[1])
    assert 0 < header < 1_000_000_000, 'invalid --latency period header'
    rows = []
    for line in lines[2:-1]:
        numbers = line.split()
        assert len(numbers) == 3 and all(re.fullmatch(r'-?\d+', n) for n in numbers), 'malformed latency row'
        rows.append(tuple(map(int, numbers)))
    # This Galaxy has verified empty/growing prefixes and full 126/127-row
    # histories. AOSP can export a filtered FrameTimeline history rather than
    # a full FrameTracker ring. Keep every observed length; bounded overlapping
    # history, not a nominal capacity/header, establishes measurement coverage.
    assert 0 <= len(rows) <= 127, 'unsupported latency history size'
    return {'uptime_before_ns': before, 'uptime_after_ns': after,
            'period_header_ns': header, 'rows': rows}


class Collector:
    def __init__(self, device, product, variant, pid, folder, poll_ms, drain_ms, presentation_lead_ms, presentation_tail_ms):
        self.device, self.product, self.variant, self.pid = device, product, variant, pid
        self.folder = folder / 'surfaceflinger'
        self.folder.mkdir()
        self.poll_s, self.drain_s = poll_ms / 1000, drain_ms / 1000
        self.presentation_lead_ms, self.presentation_tail_ms = presentation_lead_ms, presentation_tail_ms
        self.stop_event = threading.Event()
        self.samples, self.binding, self.error = [], None, None
        self.thread = threading.Thread(target=self.work, name='heavy-sf-latency', daemon=True)
        self.thread.start()

    def discover(self, label):
        text = self.device.shell('dumpsys', 'SurfaceFlinger', '--list', timeout=8).stdout
        path = self.folder / (label + '.txt')
        save(path, text)
        selected = select_layer(text, self.product, self.variant, self.pid)
        return selected, {'path': str(path), 'sha256': sha(path)}

    def snapshot(self):
        name = shlex.quote(self.binding['content']['name'])
        # Device clock bounds are in the same shell operation; quoting names
        # protects the $_PID suffix from expansion and preserves parentheses.
        command = 'cat /proc/uptime\ndumpsys SurfaceFlinger --latency ' + name + '\ncat /proc/uptime'
        before = time.monotonic_ns()
        output = self.device.run('shell', command, timeout=8)
        after = time.monotonic_ns()
        path = self.folder / ('poll-%04d.txt' % len(self.samples))
        save(path, output.stdout + output.stderr)
        parsed = parse_dump(output.stdout)
        sample = {**parsed, 'path': path.name, 'sha256': sha(path),
                  'host_before_monotonic_ns': before, 'host_after_monotonic_ns': after}
        self.samples.append(sample)
        save(self.folder / 'capture.json', self.capture())

    def capture(self):
        return {'binding': self.binding, 'samples': self.samples, 'error': self.error,
                'pid': self.pid, 'variant': self.variant, 'poll_ms': self.poll_s * 1000,
                'drain_ms': self.drain_s * 1000,
                'presentation_lead_ms': self.presentation_lead_ms,
                'presentation_tail_ms': self.presentation_tail_ms,
                'ring_records_per_poll': sorted(set(len(x['rows']) for x in self.samples))}

    def work(self):
        try:
            next_poll = time.monotonic()
            discovery_count = 0
            while not self.stop_event.is_set():
                if self.binding is None:
                    selected, evidence = self.discover('layers-discovery-%03d' % discovery_count)
                    discovery_count += 1
                    if selected:
                        self.binding = {**selected, 'initial_inventory': evidence}
                if self.binding is not None:
                    self.snapshot()
                next_poll = max(next_poll + self.poll_s, time.monotonic())
                self.stop_event.wait(max(0, next_poll - time.monotonic()))
            if self.binding is None:
                raise RuntimeError('never found a uniquely bound application content layer')
            # Fetch once after the regular polling stopped, before app force-stop.
            self.snapshot()
            final, evidence = self.discover('layers-after')
            assert final and final['content'] == self.binding['content'] and final['window'] == self.binding['window'], 'layer identity or parentage changed'
            self.binding['final_inventory'] = evidence
        except BaseException as error:
            self.error = repr(error)
        finally:
            save(self.folder / 'capture.json', self.capture())

    def finish(self):
        # Let late fences settle without continuing the benchmark's motion.
        # This period is not part of the measured window.
        self.stop_event.wait(self.drain_s)
        self.stop_event.set()
        self.thread.join(timeout=20)
        assert not self.thread.is_alive(), 'SurfaceFlinger collector did not exit'
        if self.error:
            raise RuntimeError(self.error)
        return self.capture()


class LatencyDevice(Device):
    def __init__(self, adb, serial, poll_ms, drain_ms, presentation_lead_ms, presentation_tail_ms):
        super().__init__(adb, serial)
        self.poll_ms, self.drain_ms = poll_ms, drain_ms
        self.presentation_lead_ms, self.presentation_tail_ms = presentation_lead_ms, presentation_tail_ms
        self.context, self.collector, self.finished_capture = None, None, None
    def prepare(self, product, variant, folder):
        assert self.collector is None
        self.context = product, variant, folder
        self.finished_capture = None
    def identity(self, pid):
        result = super().identity(pid)
        if self.context and self.collector is None and self.finished_capture is None:
            product, variant, folder = self.context
            self.collector = Collector(self, product, variant, pid, folder, self.poll_ms, self.drain_ms, self.presentation_lead_ms, self.presentation_tail_ms)
        return result
    def stopped(self, products):
        try:
            if self.collector:
                owned, self.collector = self.collector, None
                self.finished_capture = owned.finish()
        finally:
            super().stopped(products)


def percentile(values, percent):
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * percent / 100) - 1)]



def audit_record_settlement(samples, end):
    """Retain row identities despite independently resolving fence timestamps.

    Ordered ring suffix/prefix alignment uses desired/ready pairs and immutable
    settled timestamps. Desired is not a presentation lower bound. Ready can be
    an artificial desired-time fallback, so only a distinct real ready time can
    exclude an unresolved record after End.
    """
    def occupied(row):
        return tuple(row) not in ((0, 0, 0), (MAX_TIME, MAX_TIME, MAX_TIME))

    def compatible(before, after):
        return (before[0] == after[0] and
                (before[2] == MAX_TIME or before[2] == after[2]) and
                (before[1] == MAX_TIME or before[1] == after[1]))

    def settled(row):
        return 0 < row[1] < MAX_TIME

    def future_ready(row):
        # Layer substitutes desiredPresentTime when it has no acquire fence.
        # That substituted value must not be treated as a GPU causality bound.
        return 0 < row[2] < MAX_TIME and row[2] > end and row[2] != row[0]

    prior, identities, records = [], [], []
    alignment_counts, pending_seen = [], set()
    for sample in samples:
        rows = [tuple(row) for row in sample['rows'] if occupied(row)]
        for row in rows:
            assert 0 < row[0] < MAX_TIME, 'occupied row has no stable desired timestamp'
            assert row[1] != 0, 'occupied row has unknown zero actual-present timestamp'
            if settled(row) and 0 < row[2] < MAX_TIME and row[2] != row[0]:
                assert row[2] <= row[1], 'actual presentation precedes its real ready fence'
        if prior:
            candidates = []
            for shift in range(max(0, len(prior) - len(rows)), len(prior)):
                count = len(prior) - shift
                if all(compatible(prior[shift + i], rows[i]) for i in range(count)):
                    candidates.append((shift, count))
            assert len(candidates) == 1, 'ordered desired/ready ring alignment missing or ambiguous'
            shift, count = candidates[0]
            for identity in identities[:shift]:
                row = records[identity]
                assert settled(row) or future_ready(row), 'unresolved actual-present record disappeared from ring'
            next_ids = identities[shift:]
            alignment_counts.append(count)
            for identity, row in zip(next_ids, rows[:count]):
                records[identity] = row
        else:
            count, next_ids = 0, []
        for row in rows[count:]:
            next_ids.append(len(records))
            records.append(row)
        for identity, row in zip(next_ids, rows):
            if not settled(row):
                pending_seen.add(identity)
        prior, identities = rows, next_ids
    unresolved = [records[i] for i in pending_seen if not settled(records[i])]
    assert all(future_ready(row) for row in unresolved), 'unresolved actual-present record could fall within measurement'
    return {'pending_or_invalid_actual_records_seen': len(pending_seen),
            'pending_actual_records_resolved': sum(settled(records[i]) for i in pending_seen),
            'unresolved_actual_records_proven_ready_after_end': len(unresolved),
            'minimum_ordered_retained_rows': min(alignment_counts) if alignment_counts else None,
            'occupied_record_identities_observed': len(records),
            'pending_gate': 'Every occupied row tracked by unambiguous ordered desired/ready alignment. Missing actual fences must resolve or have a distinct real ready fence after End; desired timestamps and newer settled presentation timestamps alone do not prove exclusion.'}


def analyze_capture(capture, result):
    assert capture['error'] is None, 'collector failed'
    markers = result['markers']
    motion_start, motion_end = markers['Begin']['start_nano_ns'], markers['Measure']['end_nano_ns']
    assert markers['Measure']['start_nano_ns'] == motion_start and motion_end > motion_start
    lead_ms, tail_ms = capture['presentation_lead_ms'], capture['presentation_tail_ms']
    assert isinstance(lead_ms, int) and isinstance(tail_ms, int) and lead_ms >= 0 and tail_ms >= 0
    start, end = motion_start + lead_ms * 1_000_000, motion_end - tail_ms * 1_000_000
    assert end > start, 'presentation subwindow is empty'
    boot_delta = markers['Begin']['start_elapsed_ns'] - motion_start
    end_delta = markers['Measure']['end_elapsed_ns'] - motion_end
    assert abs(boot_delta - end_delta) <= CLOCK_PAIR_TOLERANCE_NS, 'clock mapping changed, possibly device suspend'
    frequency = markers['Startup']['display_refresh_rate_api_hz']
    assert isinstance(frequency, (float, int)) and 1 <= frequency <= 500
    period = 1_000_000_000 / frequency
    samples = capture['samples']
    assert len(samples) >= 2, 'too few snapshots'
    observed_counts = sorted(set(len(x['rows']) for x in samples if x['rows']))
    assert observed_counts, 'selected layer never produced frame history'
    for sample in samples:
        sample['capture_before_nano_ns_lower'] = sample['uptime_before_ns'] - boot_delta - CLOCK_PAIR_TOLERANCE_NS
        sample['capture_after_nano_ns_upper'] = sample['uptime_after_ns'] + UPTIME_QUANTUM_NS - boot_delta + CLOCK_PAIR_TOLERANCE_NS
        present = sorted(set(row[1] for row in sample['rows'] if 0 < row[1] < MAX_TIME))
        assert not present or present[-1] <= sample['capture_after_nano_ns_upper'], 'future actual-present timestamp'
        sample['settled_present_ns'] = present
        sample['unresolved_actual_present_records'] = sum(row[1] == MAX_TIME for row in sample['rows'])
    # AOSP can expose an old/growing prefix for a newly launched layer. FPS
    # therefore covers only the declared steady presentation subwindow. Keep
    # all raw prefix evidence, but start strict ordered/overlap validation at
    # the closest settled snapshot fully before that subwindow; continue
    # through the first fully post-End snapshot and the remaining drain polls.
    all_samples = samples
    before_indices = [i for i, x in enumerate(all_samples) if x['settled_present_ns'] and
                      x['capture_after_nano_ns_upper'] <= start]
    assert before_indices, 'no settled snapshot fully before presentation subwindow'
    first_index = before_indices[-1]
    after_indices = [i for i, x in enumerate(all_samples) if i > first_index and
                     x['settled_present_ns'] and x['capture_before_nano_ns_lower'] >= end]
    assert after_indices, 'no settled snapshot fully after presentation subwindow'
    first_post_end_index = after_indices[0]
    samples = all_samples[first_index:]
    assert max(t for x in samples for t in x['settled_present_ns']) >= end, 'exported actual-present history did not reach presentation subwindow End'
    # Check every pair of consecutive nonempty valid snapshots. Even a slow
    # collector interval must retain overlap, rather than treating missing
    # ring history as dropped application frames.
    previous = None
    overlap_counts = []
    for sample in samples:
        present = sample['settled_present_ns']
        if not present:
            assert previous is None, 'settled history disappeared'
            assert sample['capture_after_nano_ns_upper'] <= start, 'empty or pending history extends into presentation subwindow'
            continue
        if previous is not None:
            common = set(previous['settled_present_ns']).intersection(present)
            assert common, 'latency ring has no overlap; history may have been overwritten'
            assert present[-1] >= previous['settled_present_ns'][-1], 'actual-present history moved backwards'
            overlap_counts.append(len(common))
        previous = sample
    settlement = audit_record_settlement(samples, end)
    all_present = sorted(set(t for sample in samples for t in sample['settled_present_ns']))
    measured = [t for t in all_present if start <= t <= end]
    duration_s = (end - start) / 1_000_000_000
    fps = len(measured) / duration_s
    assert fps <= frequency * 1.03 + 2 / duration_s, 'present count exceeds requested display cadence'
    internal = [b - a for a, b in zip(measured, measured[1:])]
    clipped = [b - a for a, b in zip([start] + measured, measured + [end])]
    late = sum(g > 1.5 * period for g in internal)
    return {'status': 'VALID_OVERLAPPING_PRESENT_HISTORY', **settlement, 'actual_presented_updates': len(measured),
            'actual_presented_fps': fps, 'measured_duration_s': duration_s,
            'actual_present_gap_p50_ms': None if not internal else statistics.median(internal) / 1e6,
            'actual_present_gap_p95_ms': None if not internal else percentile(internal, 95) / 1e6,
            'late_present_gaps_over_1_5_period': late, 'late_present_gaps_per_s': late / duration_s,
            'worst_internal_present_gap_ms': None if not internal else max(internal) / 1e6,
            'worst_measurement_clipped_gap_ms': max(clipped) / 1e6,
            'first_present_after_begin_ms': None if not measured else (measured[0] - start) / 1e6,
            'last_present_to_end_ms': None if not measured else (end - measured[-1]) / 1e6,
            'nominal_refresh_hz_from_startup_api': frequency,
            'latency_header_periods_ns': sorted(set(x['period_header_ns'] for x in samples)),
            'minimum_retained_overlap_records': min(overlap_counts) if overlap_counts else None,
            'poll_count': len(samples), 'source_poll_count': len(all_samples),
            'discarded_prefix_poll_count': first_index,
            'empty_or_pending_history_polls_total': sum(not x['settled_present_ns'] for x in all_samples),
            'history_rows_per_poll_observed': sorted(set(len(x['rows']) for x in all_samples)),
            'history_rows_per_selected_poll_observed': sorted(set(len(x['rows']) for x in samples)),
            'first_selected_poll_index': first_index, 'first_fully_post_end_poll_index': first_post_end_index,
            'drain_polls_after_first_fully_post_end': len(all_samples) - first_post_end_index - 1,
            'motion_start_nano_ns': motion_start, 'motion_end_nano_ns': motion_end,
            'motion_duration_s': (motion_end - motion_start) / 1e9,
            'presentation_start_nano_ns': start, 'presentation_end_nano_ns': end,
            'presentation_excluded_lead_ms': lead_ms, 'presentation_excluded_tail_ms': tail_ms,
            'maximum_poll_host_duration_ms': max(x['host_after_monotonic_ns'] - x['host_before_monotonic_ns'] for x in samples) / 1e6,
            'pending_actual_present_records_final': samples[-1]['unresolved_actual_present_records'],
            'observed_before_window_frames': sum(t < start for t in all_present),
            'observed_after_window_frames_excluded': sum(t > end for t in all_present),
            'scope': 'Distinct second-column SurfaceFlinger actual-present fence timestamps only in the explicitly bounded steady presentation subwindow of unchanged app motion. This is not whole-motion FPS. Whole-motion CPU/memory/startup diagnostics are separate. Late gaps use startup API refresh rate, not the latency header. No per-buffer frame IDs: gaps are not literal dropped-buffer counts.'}


def summarize(receipt):
    rows = []
    headline_allowed = receipt['status'] == 'COMPLETE_VALIDATED_PRESENT_COHORT'
    for variant in RENDERERS:
        for speed in receipt['speeds_dp_s']:
            samples = [r for r in receipt['scroll'] if r['renderer'] == variant and r['speed_dp_s'] == speed]
            if not samples:
                continue
            valid = [r for r in samples if r.get('surfaceflinger_analysis', {}).get('status') == 'VALID_OVERLAPPING_PRESENT_HISTORY']
            entry = {'renderer': variant, 'speed_dp_s': speed, 'samples': len(samples), 'valid_samples': len(valid)}
            for key in ('actual_presented_fps', 'actual_present_gap_p95_ms', 'late_present_gaps_per_s',
                        'worst_internal_present_gap_ms', 'worst_measurement_clipped_gap_ms'):
                values = [r['surfaceflinger_analysis'][key] for r in valid if r['surfaceflinger_analysis'][key] is not None] if headline_allowed else []
                entry[key + '_median'] = statistics.median(values) if values else None
                entry[key + '_range'] = [min(values), max(values)] if values else None
            rows.append(entry)
    config = receipt.get('config', {})
    return {'status': receipt['status'], 'headline_metrics_withheld': not headline_allowed,
            'presentation_window_config': {key: config.get(key) for key in ('duration_ms', 'presentation_lead_ms', 'presentation_tail_ms')},
            'rows': rows, 'methodology': receipt['scope'], 'sources': SOURCES}



def audit_capture_eligibility(receipt):
    """Analysis cannot turn capture, reboot, or cleanup failures into success."""
    assert receipt['status'] in ('CAPTURE_COMPLETE_PENDING_AUDIT', 'COMPLETE_VALIDATED_PRESENT_COHORT'), 'capture status is ineligible for success audit'
    assert not receipt.get('error') and not receipt.get('cleanup_error'), 'capture or cleanup recorded an error'
    assert receipt.get('boot_id_verified_unchanged') is True, 'capture did not confirm unchanged device boot'
    assert receipt.get('boot_id') and receipt.get('boot_id_final') == receipt['boot_id'], 'capture boot identities differ or are missing'
    assert receipt.get('service_before_sessions') == 0 and receipt.get('service_after_sessions') == 0, 'trace-service cleanup was not confirmed clear'


def audit_receipt(folder):
    receipt = json.loads((folder / 'receipt.json').read_text())
    audit_capture_eligibility(receipt)
    assert receipt['script_sha256'] == sha(__file__), 'analyzer differs from capture script'
    assert receipt['launch_helper_sha256'] == sha(pathlib.Path(__file__).with_name('run.py')), 'launch helper changed since capture'
    assert receipt['source_manifest_sha256'] == sha(folder / 'source-manifest.json'), 'source manifest changed'
    failures = []
    for result in receipt['scroll']:
        path = folder / result['folder'] / 'surfaceflinger' / 'capture.json'
        assert result['surfaceflinger_capture_sha256'] == sha(path), 'capture manifest changed'
        capture = json.loads(path.read_text())
        assert capture['pid'] == result['pid'] and capture['variant'] == result['renderer'], 'capture process differs'
        raw_log = folder / result['folder'] / 'logcat.txt'
        assert result['logcat_sha256'] == sha(raw_log), 'raw measurement log changed'
        for key in ('initial_inventory', 'final_inventory'):
            inventory = capture['binding'][key]
            assert sha(inventory['path']) == inventory['sha256'], 'layer binding evidence changed'
            selected = select_layer(pathlib.Path(inventory['path']).read_text(), receipt['products'][result['renderer']], result['renderer'], result['pid'])
            assert selected and selected['window'] == capture['binding']['window'] and selected['content'] == capture['binding']['content'], 'layer ancestry does not bind to process'
        for sample in capture['samples']:
            raw = path.parent / sample['path']
            assert sha(raw) == sample['sha256'], 'raw latency dump changed'
            parsed = parse_dump(raw.read_text())
            assert all(parsed[k] == sample[k] for k in ('uptime_before_ns', 'uptime_after_ns', 'period_header_ns'))
            assert parsed['rows'] == [tuple(x) for x in sample['rows']], 'numeric history differs from raw evidence'
        try:
            result['surfaceflinger_analysis'] = analyze_capture(capture, result)
        except (AssertionError, ValueError) as error:
            result['surfaceflinger_analysis'] = {'status': 'INVALID_PRESENT_HISTORY', 'error': str(error)}
            failures.append(result['folder'])
    receipt['analysis_failures'] = failures
    receipt['status'] = 'COMPLETE_VALIDATED_PRESENT_COHORT' if not failures and len(receipt['scroll']) == receipt['planned_scroll_phases'] else 'INCOMPLETE_OR_INVALID_PRESENT_COHORT'
    save(folder / 'receipt.json', receipt)
    save(folder / 'summary.json', summarize(receipt))
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--products', type=pathlib.Path)
    parser.add_argument('--serial')
    parser.add_argument('--out', type=pathlib.Path)
    parser.add_argument('--adb', default='adb')
    parser.add_argument('--rounds', type=int, default=4)
    parser.add_argument('--speeds', default='1000,24000')
    parser.add_argument('--duration-ms', type=int, default=8000)
    parser.add_argument('--warmup-ms', type=int, default=1500)
    parser.add_argument('--timeout-s', type=int, default=120)
    parser.add_argument('--cooldown-ms', type=int, default=0)
    parser.add_argument('--poll-ms', type=int, default=400)
    parser.add_argument('--drain-ms', type=int, default=600)
    parser.add_argument('--presentation-lead-ms', type=int, default=3000)
    parser.add_argument('--presentation-tail-ms', type=int, default=200)
    parser.add_argument('--live', action='store_true')
    parser.add_argument('--skip-install', action='store_true', help='Keep installed APKs; each launch still verifies exact installed hash')
    parser.add_argument('--plan-only', action='store_true')
    parser.add_argument('--analyze-only', type=pathlib.Path)
    args = parser.parse_args()
    if args.analyze_only:
        audited = audit_receipt(args.analyze_only)
        print(json.dumps(summarize(audited), indent=2)); return
    assert args.products and args.serial and args.out, '--products, --serial and --out are required for capture'
    assert args.rounds > 0 and 1000 <= args.duration_ms <= 60000
    assert 1000 <= args.warmup_ms <= 30000 and 100 <= args.poll_ms <= 500 and 100 <= args.drain_ms <= 3000
    assert 0 <= args.cooldown_ms <= 60000 and args.timeout_s > 0
    assert args.presentation_lead_ms >= 0 and args.presentation_tail_ms >= 0
    assert args.duration_ms - args.presentation_lead_ms - args.presentation_tail_ms >= 1000, 'presentation subwindow must be at least one second'
    speeds = [float(x) for x in args.speeds.split(',')]
    assert speeds and all(math.isfinite(x) and x > 0 for x in speeds)
    products = load_products(args.products)
    args.out.mkdir(parents=True, exist_ok=False)
    save(args.out / 'source-manifest.json', source_manifest())
    plan = [(round_, speed, variant) for round_ in range(args.rounds) for index, speed in enumerate(speeds)
            for variant in RENDERERS[(round_ + index) % 4:] + RENDERERS[:(round_ + index) % 4]]
    receipt = {'status': 'PREPARED', 'products': products, 'serial': args.serial,
               'config': {key: str(value) if isinstance(value, pathlib.Path) else value for key, value in vars(args).items()},
               'speeds_dp_s': speeds, 'planned_scroll_phases': len(plan), 'scroll': [],
               'script_sha256': sha(__file__), 'launch_helper_sha256': sha(pathlib.Path(__file__).with_name('run.py')),
               'source_manifest_sha256': sha(args.out / 'source-manifest.json'), 'sources': SOURCES,
               'position_balance_note': 'Use a multiple of four rounds for balanced renderer positions. Actual order is retained.',
               'scope': 'Separate equal-polling-overhead release cohort for C9, main, Views and Compose. SurfaceFlinger FPS covers only the declared steady subwindow [Begin + presentation-lead-ms, MeasureEnd - presentation-tail-ms], with unchanged whole-motion harness and CLOCK_MONOTONIC bounds. No whole-motion FPS claim. All raw startup/prefill polls remain available; ordered overlap and pending validation cover snapshots bracketing the subwindow and drain. Whole-motion process/main CPU, memory and startup-ready are separate collector-overhead diagnostics, not untraced headline metrics. Main scroll acknowledgment remains unavailable, so equal requested speed does not prove equal realized scrolling. No CPU sampling, Perfetto trace, screenshots or video during capture.'}
    save(args.out / 'receipt.json', receipt)
    if args.plan_only:
        receipt['status'] = 'PLAN_ONLY'; save(args.out / 'receipt.json', receipt); return
    device = LatencyDevice(args.adb, args.serial, args.poll_ms, args.drain_ms, args.presentation_lead_ms, args.presentation_tail_ms)
    seen = set()
    try:
        receipt['service_before_sessions'] = device.sessions(args.out / 'service-before.txt')
        assert receipt['service_before_sessions'] == 0, 'existing trace; do not mix collector overhead'
        boot = device.shell('cat', '/proc/sys/kernel/random/boot_id').stdout.strip()
        receipt['boot_id'] = boot
        save(args.out / 'device-properties.txt', device.shell('getprop').stdout)
        assert device.shell('getprop', 'ro.kernel.qemu').stdout.strip() != '1', 'physical Android device required'
        if not args.skip_install:
            for variant, product in products.items():
                save(args.out / (variant + '-install.txt'), device.run('install', '--no-incremental', '-r', product['apk']).stdout)
        for variant, product in products.items():
            device.installed(product, args.out / (variant + '-installed.json'))
            compiled = device.shell('cmd', 'package', 'compile', '-f', '-m', 'speed', product['appId'])
            save(args.out / (variant + '-compile.txt'), compiled.stdout + compiled.stderr)
            assert 'Success' in compiled.stdout, 'ART speed compile did not report success'
            dumped = device.shell('cmd', 'art', 'dump', product['appId'], check=False)
            save(args.out / (variant + '-art.txt'), dumped.stdout + dumped.stderr)
        for index, (round_, speed, variant) in enumerate(plan):
            folder = args.out / ('scroll-%03d-%s' % (index, variant))
            device.prepare(products[variant], variant, folder)
            result = launch(device, products, variant, folder, 'heavy-sf-' + uuid.uuid4().hex, args, speed)
            result.update(round=round_, speed_dp_s=speed, slot=index % 4, folder=folder.name)
            identity = (result['uid'], result['pid'], result['proc_start_ticks'])
            assert identity not in seen, 'process identity reused'
            seen.add(identity)
            capture_path = folder / 'surfaceflinger' / 'capture.json'
            assert device.finished_capture is not None and capture_path.is_file(), 'missing owned collector result'
            result['surfaceflinger_capture_sha256'] = sha(capture_path)
            try:
                result['surfaceflinger_analysis'] = analyze_capture(device.finished_capture, result)
            except (AssertionError, ValueError) as error:
                result['surfaceflinger_analysis'] = {'status': 'INVALID_PRESENT_HISTORY', 'error': str(error)}
            receipt['scroll'].append(result)
            save(args.out / 'receipt.json', receipt)
            analysis = result['surfaceflinger_analysis']
            print('sf', round_, speed, variant, analysis['status'],
                  'fps', analysis.get('actual_presented_fps'),
                  'p95_ms', analysis.get('actual_present_gap_p95_ms'),
                  analysis.get('error', ''), flush=True)
        receipt['boot_id_final'] = device.shell('cat', '/proc/sys/kernel/random/boot_id').stdout.strip()
        receipt['boot_id_verified_unchanged'] = receipt['boot_id_final'] == boot
        assert receipt['boot_id_verified_unchanged'], 'device rebooted'
        receipt['status'] = 'CAPTURE_COMPLETE_PENDING_AUDIT'
    except BaseException as error:
        receipt['status'], receipt['error'] = 'FAILED_PRESERVE_EVIDENCE', repr(error)
        raise
    finally:
        try:
            device.context = None
            device.stopped(products)
            receipt['service_after_sessions'] = device.sessions(args.out / 'service-after.txt')
            assert receipt['service_after_sessions'] == 0, 'unexpected trace was started during latency cohort'
        except BaseException as error:
            receipt['cleanup_error'], receipt['status'] = repr(error), 'FAILED_PRESERVE_EVIDENCE'
            raise
        finally:
            receipt['finished_unix_s'] = time.time()
            save(args.out / 'receipt.json', receipt)
    receipt = audit_receipt(args.out)
    print(json.dumps(summarize(receipt), indent=2))


if __name__ == '__main__':
    main()
