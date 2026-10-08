#!/usr/bin/env python3
"""Offline native FrameTimeline analysis; never substitutes callbacks for presented FPS.

App actual slices end at max(GPU completion, buffer post), not display update.
Join display_frame_token to SurfaceFlinger actual DisplayFrame end instead.
https://perfetto.dev/docs/data-sources/frametimeline
"""
import argparse
import collections
import csv
import io
import json
import math
import pathlib
import re
import statistics
import subprocess
from run import RENDERERS, fields, markers, save, service_counters, sha

DEFAULT_PROCESSOR = '/Users/krzys/.local/share/perfetto/prebuilts/trace_processor_shell-d29864d1ba3b3685'
NULLS = ('', '[NULL]', None)


def num(value):
    return None if value in NULLS else int(value)


def distribution(values):
    values = sorted(values)
    def at(q):
        return values[max(0, math.ceil(len(values) * q) - 1)] if values else None
    return {'samples': len(values), 'p50': at(.5), 'p95': at(.95), 'p99': at(.99),
            'min': values[0] if values else None, 'max': values[-1] if values else None}


def present_type(value):
    value = str(value).lower()
    if 'drop' in value:
        return 'dropped'
    if any(s in value for s in ('on-time', 'on time', 'early present', 'late present')):
        return 'presented'
    return 'unknown'


def raw_health(trace, prefix, receipt):
    prefix_packets = list(fields(prefix.read_bytes(), partial=True))
    packets = list(fields(trace.read_bytes()))
    assert packets[:len(prefix_packets)] == prefix_packets, 'initial prefix differs from final trace'
    counter = service_counters(trace)
    baseline = receipt['service_counter_baseline']
    counter_changed = not counter or any(c != baseline for c in counter)
    counter_deltas = [{k: c[k] - baseline[k] for k in baseline} for c in counter]
    sequences = collections.defaultdict(lambda: {'types': set(), 'loss': []})
    raw_actual, ends, schema_errors = [], {}, []
    for index, (field, blob) in enumerate(packets):
        assert field == 1
        packet = dict(fields(blob))
        assert 50 not in packet, 'compressed packets require another decoder'
        seq = sequences[packet.get(10, 0)]
        seq['types'].update(packet.keys())
        if packet.get(42) and packet.get(87) != 1:
            seq['loss'].append({'packet': index, 'ts': packet.get(8)})
        if 76 not in packet:
            continue
        assert packet.get(58, 6) == 6, 'native FrameTimeline clock is not BOOTTIME'
        events = list(fields(packet[76])); assert len(events) == 1
        kind, payload = events[0]; value = dict(fields(payload)); ts = packet.get(8)
        if kind == 5:
            assert value.get(1) not in ends, 'duplicate FrameTimeline end cookie'
            ends[value.get(1)] = ts
        elif kind == 4:
            if ts is None or ts >= 2 ** 63 or any(k not in value for k in (1, 2, 3, 4)):
                schema_errors.append({'packet': index, 'kind': kind, 'ts': ts, 'pid': value.get(4)})
                continue
            raw_actual.append({'pid': value[4], 'ts': ts, 'cookie': value[1],
                               'surface_frame_token': value[2], 'display_frame_token': value[3],
                               'layer_name': value.get(5, b'').decode('utf8'),
                               'present_enum': value.get(6), 'jank_mask': value.get(9)})
    loss = [row for value in sequences.values() if 76 in value['types'] or 39 in value['types'] for row in value['loss']]
    for frame in raw_actual:
        frame['end_ns'] = ends.get(frame['cookie'])
    return {'service_counters': counter, 'baseline': baseline,
            'service_counters_changed_or_missing': counter_changed, 'service_counter_deltas': counter_deltas,
            'service_counter_scope': 'Global service counters are not attributed to a packet sequence or app window; unexplained changes withhold all phase headlines.',
            'middle_frame_or_log_sequence_loss': loss,
            'schema_errors': schema_errors, 'raw_actual_surface_frames': len(raw_actual),
            'source': 'Uncompressed official TracePacket field76 FrameTimelineEvent actual-surface4/end5; field35 TraceStats service counters8/9/10.'}, raw_actual


def select_layer(renderer, product, inventory):
    candidates = []
    for row in inventory:
        layer = row['layer_name']
        if layer in NULLS or not (product['appId'] in layer or product['activity'] in layer):
            continue
        is_surface = 'SurfaceView' in layer or 'MainSurface' in layer
        if renderer == 'main':
            if is_surface and 'Background for' not in layer:
                candidates.append(layer)
        elif not is_surface and not any(s in layer for s in ('Splash', 'Starting ', 'Snapshot')):
            candidates.append(layer)
    return sorted(set(candidates))


def summarize_phase(phase, product, actual, expected, display, observed_period_ns, health_ok):
    start = phase['markers']['Measure']['start_elapsed_ns']
    end = phase['markers']['Measure']['end_elapsed_ns']
    pid = phase['pid']; upid = phase['trace_upid']
    inventory = collections.Counter(f['layer_name'] for f in actual if num(f['upid']) == upid)
    layers = [{'layer_name': layer, 'frames': count} for layer, count in inventory.items()]
    choices = select_layer(phase['renderer'], product, layers)
    result = {'renderer': phase['renderer'], 'round': phase['round'], 'speed_dp_s': phase['speed_dp_s'],
              'pid': pid, 'upid': upid, 'token': phase['token'], 'start_ns': start, 'end_exclusive_ns': end,
              'window_ms': (end - start) / 1e6, 'layer_inventory': layers, 'candidate_layers': choices,
              'presented_fps': None, 'status': 'NO_UNIQUE_TRACKED_CONTENT_LAYER',
              'pacing_observation': phase['markers']['Measure'],
              'scope': 'Unique SurfaceFlinger DisplayFrame presentation timestamps containing the selected content layer in [Begin.elapsedRealtimeNanos, Measure.endElapsedRealtimeNanos). App completion duration is a distinct metric. No callback correspondence is forced.'}
    if len(choices) != 1:
        return result, []
    layer = choices[0]; result['layer'] = layer
    # A submitted frame may begin before the measured window and present in it.
    # Retain all selected-layer rows for that process; presentation defines scope.
    frames = [f for f in actual if num(f['upid']) == upid and f['layer_name'] == layer]
    display_by_token = collections.defaultdict(list)
    for row in display:
        display_by_token[num(row['display_frame_token'])].append(row)
    attributed, unknown, dropped, present = [], [], [], []
    for frame in frames:
        state = present_type(frame['present_type'])
        app_start, app_duration = num(frame['ts']), num(frame['dur'])
        overlaps = app_start < end and (app_duration is None or app_duration < 0 or app_start + app_duration >= start)
        linked = display_by_token.get(num(frame['display_frame_token']), [])
        row = dict(frame)
        row['display_rows'] = linked
        row['display_present_ns'] = None
        if state == 'dropped':
            if start <= app_start < end:
                dropped.append(row); attributed.append(row)
            continue
        if len(linked) == 1 and num(linked[0]['dur']) is not None and num(linked[0]['dur']) >= 0:
            sf = linked[0]
            row['display_present_ns'] = num(sf['ts']) + num(sf['dur'])
            row['display_present_type'] = sf['present_type']
            if start <= row['display_present_ns'] < end:
                attributed.append(row)
                if state == 'presented' and present_type(sf['present_type']) == 'presented':
                    present.append(row)
                else:
                    unknown.append(row)
        elif overlaps:
            row['unmapped_reason'] = 'missing/ambiguous/incomplete actual DisplayFrame'
            unknown.append(row); attributed.append(row)
    timestamps = sorted({r['display_present_ns'] for r in present})
    gaps = [b - a for a, b in zip(timestamps, timestamps[1:])]
    duration = (end - start) / 1e9
    types = collections.Counter(r['present_type'] for r in attributed)
    janks = collections.Counter(r['jank_type'] for r in attributed)
    durations = [num(r['dur']) / 1e6 for r in attributed if num(r['dur']) is not None and num(r['dur']) >= 0]
    expected_tokens = {(num(f['surface_frame_token']), f['layer_name']) for f in expected
                       if num(f['upid']) == upid and start <= num(f['ts']) < end}
    actual_tokens = {(num(f['surface_frame_token']), f['layer_name']) for f in frames}
    result.update(status='NATIVE_TIMELINE_MAPPED' if timestamps else 'NO_MAPPED_PRESENTATIONS',
                  presented_surface_frame_count=len(present), unique_display_present_timestamps=len(timestamps),
                  dropped_surface_frames=len(dropped), unknown_or_unmapped_surface_frames=len(unknown),
                  native_present_type_counts=dict(types), native_jank_type_counts=dict(janks),
                  app_actual_completion_duration_ms=distribution(durations),
                  observed_display_period_ns=observed_period_ns,
                  displayed_gap_ms=distribution([g / 1e6 for g in gaps]),
                  displayed_max_gap_ms=max(gaps) / 1e6 if gaps else None,
                  late_display_intervals=sum(g > 1.5 * observed_period_ns for g in gaps) if observed_period_ns else None,
                  late_display_intervals_per_s=sum(g > 1.5 * observed_period_ns for g in gaps) / duration if observed_period_ns else None,
                  expected_only_surface_tokens=sorted(t for t, l in expected_tokens - actual_tokens if l == layer),
                  actual_only_surface_tokens=sorted(t for t, l in actual_tokens - expected_tokens if l == layer),
                  mapped_present_rate_fps=len(timestamps) / duration if timestamps else None,
                  complete_mapping=bool(timestamps) and not unknown and not any(l == layer for t, l in expected_tokens - actual_tokens))
    if health_ok and result['complete_mapping']:
        result['presented_fps'] = result['mapped_present_rate_fps']
    else:
        result['headline_withheld_reason'] = 'trace health not certified or content presentation mapping incomplete'
        result['mapped_displayed_gap_ms'] = result['displayed_gap_ms']
        result['mapped_late_display_intervals_per_s'] = result['late_display_intervals_per_s']
        result['mapped_displayed_max_gap_ms'] = result['displayed_max_gap_ms']
        result['late_display_intervals_per_s'] = None
        result['displayed_max_gap_ms'] = None
    return result, attributed


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--capture', required=True, type=pathlib.Path)
    p.add_argument('--out', required=True, type=pathlib.Path)
    p.add_argument('--processor', default=DEFAULT_PROCESSOR, type=pathlib.Path)
    args = p.parse_args()
    receipt_path = args.capture / 'receipt.json'; receipt = json.loads(receipt_path.read_text())
    assert receipt['status'] == 'CAPTURE_COMPLETE_PENDING_TIMELINE_AUDIT'
    assert receipt['recorder_stopped'] and receipt['service_after_sessions'] == 0
    assert set(receipt['products']) == set(RENDERERS)
    assert len(receipt['scroll']) == receipt['planned_scroll_phases']
    assert len(receipt['startup']) == receipt['config']['startup_reps'] * 4
    trace = args.capture / 'timeline.pftrace'; prefix = args.capture / 'initial-prefix.pftrace'
    assert sha(trace) == receipt['trace_sha256'] and sha(prefix) == receipt['initial_prefix_sha256']
    assert sha(args.capture / 'source-manifest.json') == receipt['source_manifest_sha256']
    args.out.mkdir(parents=True, exist_ok=False)
    def query(name, sql):
        path = args.out / (name + '.sql'); save(path, sql)
        r = subprocess.run([str(args.processor), 'query', '-f', str(path), str(trace)], capture_output=True, text=True, timeout=180)
        save(args.out / (name + '.csv'), r.stdout); save(args.out / (name + '.stderr.txt'), r.stderr)
        assert r.returncode == 0, name + ' query failed'
        return list(csv.DictReader(io.StringIO(r.stdout)))
    actual_schema = query('actual-schema', 'SELECT * FROM pragma_table_info("actual_frame_timeline_slice")')
    required = {'ts', 'dur', 'upid', 'layer_name', 'surface_frame_token', 'display_frame_token', 'present_type', 'jank_type'}
    assert required <= {r['name'] for r in actual_schema}
    clock = query('clock', 'SELECT * FROM metadata WHERE name="trace_time_clock_id"')
    assert len(clock) == 1 and num(clock[0]['int_value']) == 6, 'trace normalized clock is not BOOTTIME'
    stats = query('stats', 'SELECT * FROM stats WHERE value != 0 ORDER BY name')
    health, raw = raw_health(trace, prefix, receipt)
    cumulative = {'traced_chunks_discarded': '8', 'traced_patches_discarded': '9', 'traced_invalid_packets': '10'}
    potential_loss = [r for r in stats if num(r['value']) and
        (r['severity'] in ('error', 'data_loss') or re.search(r'discard|overwrite|drop|loss|lost|frame_timeline_event_parser_errors', r['name'], re.I)) and
        not (r['name'] in cumulative and r['idx'] in NULLS and num(r['value']) == int(receipt['service_counter_baseline'][cumulative[r['name']]]))]
    health['nonzero_stats'] = stats; health['unresolved_parser_or_data_loss'] = potential_loss
    health_ok = not health['service_counters_changed_or_missing'] and not health['middle_frame_or_log_sequence_loss'] and not health['schema_errors'] and not potential_loss
    health['status'] = 'PASS' if health_ok else 'UNRESOLVED_WITHHOLD_HEADLINE'
    save(args.out / 'trace-health.json', health)
    actual = query('actual-frames', 'SELECT a.*,p.pid,p.name AS process_name FROM actual_frame_timeline_slice a LEFT JOIN process p USING(upid) ORDER BY a.ts')
    expected = query('expected-frames', 'SELECT a.*,p.pid,p.name AS process_name FROM expected_frame_timeline_slice a LEFT JOIN process p USING(upid) ORDER BY a.ts')
    logs = query('markers', 'SELECT l.ts,t.tid,p.pid,p.upid,p.name AS process_name,HEX(CAST(l.msg AS BLOB)) AS msg_hex FROM android_logs l LEFT JOIN thread t USING(utid) LEFT JOIN process p USING(upid) WHERE l.tag="HeavyBench" ORDER BY l.ts')
    for row in logs:
        row['msg'] = bytes.fromhex(row['msg_hex']).decode('utf8')
    sf = [r for r in actual if r['layer_name'] in NULLS and 'surfaceflinger' in r['process_name'].lower() and num(r['surface_frame_token']) in (None, 0)]
    sf_expected = [r for r in expected if 'surfaceflinger' in r['process_name'].lower() and num(r['surface_frame_token']) in (None, 0) and num(r['dur']) is not None and num(r['dur']) >= 0]
    def cadence(start, end):
        deadlines = sorted({num(r['ts']) + num(r['dur']) for r in sf_expected
                            if start <= num(r['ts']) + num(r['dur']) < end})
        gaps = [b - a for a, b in zip(deadlines, deadlines[1:]) if 1_000_000 <= b - a <= 100_000_000]
        period = sorted(gaps)[max(0, math.ceil(len(gaps) * .1) - 1)] if len(gaps) >= 20 else None
        return {'observed_period_ns': period, 'expected_deadline_gaps_ns': distribution(gaps), 'gap_vector_ns': gaps,
                'estimator': 'phase-local lower decile of distinct expected SurfaceFlinger DisplayFrame deadline gaps in [1,100]ms; minimum20 gaps; not proof of hardware refresh mode. Sparse/changing-mode schedules remain a limitation; consult this launch display/window dumps.'}
    results = []
    identities = set()
    for phase in receipt['startup'] + receipt['scroll']:
        identity = (phase['uid'], phase['pid'], phase['proc_start_ticks'])
        assert identity not in identities; identities.add(identity)
        folder = args.capture / phase['folder']
        assert sha(folder / 'logcat.txt') == phase['logcat_sha256']
        assert sha(folder / 'identity.json') == phase['identity_sha256']
        assert sha(folder / 'am-start.txt') == phase['am_start_sha256']
        local = markers((folder / 'logcat.txt').read_text().splitlines(), phase['token'])
        assert local == phase['markers'], 'raw log marker differs from receipt'
    for index, phase in enumerate(receipt['scroll']):
        matched = [r for r in logs if phase['token'] in r['msg'] and r['msg'].startswith(('Startup:', 'Begin:', 'Measure:'))]
        kinds = {}
        for row in matched:
            kind, value = row['msg'].split(':', 1); data = json.loads(value)
            if data.get('token') != phase['token']:
                continue
            assert kind not in kinds; kinds[kind] = row
            assert num(row['pid']) == phase['pid'], 'trace log PID differs'
            assert data == {k: v for k, v in phase['markers'][kind].items() if not k.startswith('_capture_')}
        assert set(kinds) == {'Startup', 'Begin', 'Measure'}, 'trace lost phase log markers'
        upids = {num(r['upid']) for r in kinds.values()}; assert len(upids) == 1 and None not in upids
        phase = {**phase, 'trace_upid': upids.pop()}
        assert num(kinds['Begin']['ts']) >= phase['markers']['Begin']['start_elapsed_ns']
        assert num(kinds['Measure']['ts']) >= phase['markers']['Measure']['end_elapsed_ns']
        phase_cadence = cadence(phase['markers']['Measure']['start_elapsed_ns'], phase['markers']['Measure']['end_elapsed_ns'])
        result, rows = summarize_phase(phase, receipt['products'][phase['renderer']], actual, expected, sf, phase_cadence['observed_period_ns'], health_ok)
        result['display_cadence'] = phase_cadence
        # Every valid raw completed app surface frame in this phase must have a
        # native SQL row. Open/malformed events remain visible, never synthesized.
        start, end = result['start_ns'], result['end_exclusive_ns']
        raw_keys = collections.Counter((r['ts'], r['end_ns'] - r['ts'], r['surface_frame_token'], r['layer_name'])
                 for r in raw if r['pid'] == phase['pid'] and start <= r['ts'] < end and r['end_ns'] is not None)
        sql_keys = collections.Counter((num(r['ts']), num(r['dur']), num(r['surface_frame_token']), r['layer_name'] if r['layer_name'] not in NULLS else '')
                 for r in actual if num(r['upid']) == phase['trace_upid'] and start <= num(r['ts']) < end and num(r['dur']) is not None and num(r['dur']) >= 0)
        result['raw_completed_phase_frames'] = sum(raw_keys.values())
        result['sql_completed_phase_frames'] = sum(sql_keys.values())
        result['raw_sql_completed_frames_equal'] = raw_keys == sql_keys
        if raw_keys != sql_keys:
            result['presented_fps'] = None; result['status'] = 'RAW_SQL_FRAME_MISMATCH'
            result['late_display_intervals_per_s'] = None; result['displayed_max_gap_ms'] = None
        save(args.out / f'phase-{index:03d}-frames.json', rows)
        save(args.out / f'phase-{index:03d}-summary.json', result)
        results.append(result)
    startup = {}
    for renderer in RENDERERS:
        runs = [r for r in receipt['startup'] if r['renderer'] == renderer]
        startup[renderer] = {'process_to_ready_ms': distribution([r['process_to_ready_ms'] for r in runs]),
                             'on_create_to_ready_ms': distribution([r['on_create_to_ready_ms'] for r in runs])}
    groups = []
    for renderer in RENDERERS:
        for speed in sorted({r['speed_dp_s'] for r in results}):
            runs = [r for r in results if r['renderer'] == renderer and r['speed_dp_s'] == speed]
            groups.append({'renderer': renderer, 'speed_dp_s': speed, 'runs': len(runs),
                'presented_fps': distribution([r['presented_fps'] for r in runs if r['presented_fps'] is not None]),
                'late_display_intervals_per_s': distribution([r['late_display_intervals_per_s'] for r in runs if r.get('late_display_intervals_per_s') is not None]),
                'max_display_gap_ms': distribution([r['displayed_max_gap_ms'] for r in runs if r.get('displayed_max_gap_ms') is not None]),
                'memory_pss_kib_after_measure': distribution([r['pacing_observation']['memory_pss_kib_after_measure'] for r in runs]),
                'mapping_status_counts': dict(collections.Counter(r['status'] for r in runs))})
    report = {'capture_receipt_sha256': sha(receipt_path), 'trace_sha256': sha(trace), 'processor_sha256': sha(args.processor),
              'analyzer_sha256': sha(__file__), 'trace_health': health['status'], 'startup': startup,
              'groups': groups, 'phases': results, 'art': receipt.get('art'),
              'footprint': {r: receipt['products'][r]['footprint'] for r in RENDERERS},
              'limitations': [receipt.get('position_balance_note'), 'Startup endpoint is renderer ready after Window draw, not independently observed first displayed content.',
                 'SurfaceView/Vulkan frames may lack FrameTimeline metadata; no window-frame or callback fallback.',
                 'Unknown/dropped/unmapped/native expected-only frames are retained. Display gap endpoint is actual linked SurfaceFlinger DisplayFrame end; app duration is buffer/GPU completion.',
                 'Cadence baseline is a phase-local observed expected-display estimator, not guaranteed hardware Hz. Consult every launch display dump for active-mode changes.',
                 'Native Main realized scroll distance/visible row count may be unavailable; requested speed and FPS do not prove equivalent realized motion or pixel workload.',
                 'Startup sample counts and ranges are retained; small cohorts do not establish tail percentiles or equivalence. Incomplete phase mappings withhold headline FPS.'],
              'sources': ['https://perfetto.dev/docs/data-sources/frametimeline', 'https://github.com/google/perfetto/blob/main/protos/perfetto/trace/android/frame_timeline_event.proto']}
    save(args.out / 'report.json', report)
    lines = ['Four-way Android heavy list', '', 'Startup: fresh process, warm OS caches. Endpoint is renderer ready after Window draw.', '',
             '| Renderer | Starts | Ready nearest-rank p50 (ms) | Ready range (ms) | APK (bytes) |', '|---|---:|---:|---:|---:|']
    for r in RENDERERS:
        d = startup[r]['process_to_ready_ms']; lines.append(f"| {r} | {d['samples']} | {d['p50']:.2f} | {d['min']:.2f}–{d['max']:.2f} | {report['footprint'][r]['apk_bytes']} |")
    lines += ['', '| Renderer | Speed (dp/s) | Presented FPS p50 | Late display gaps/s p50 | Worst displayed gap p50 (ms) | PSS after scroll p50 (KiB) |', '|---|---:|---:|---:|---:|---:|']
    def fmt(v):
        return 'unavailable' if v is None else f'{v:.2f}'
    for r in groups:
        lines.append(f"| {r['renderer']} | {r['speed_dp_s']:g} | {fmt(r['presented_fps']['p50'])} | {fmt(r['late_display_intervals_per_s']['p50'])} | {fmt(r['max_display_gap_ms']['p50'])} | {fmt(r['memory_pss_kib_after_measure']['p50'])} |")
    lines += ['', 'Trace health: ' + health['status'] + '.', '', 'Presented FPS uses unique linked actual DisplayFrame presentation timestamps. App slice completion is [a different endpoint](https://perfetto.dev/docs/data-sources/frametimeline). Choreographer pacing and Window listener timings are retained separately in phase JSON.', '']
    lines += ['- ' + x for x in report['limitations'] if x]
    save(args.out / 'report.md', '\n'.join(lines) + '\n')
    print(args.out / 'report.md')


if __name__ == '__main__':
    main()
