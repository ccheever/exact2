#!/usr/bin/env python3
"""Audit Cargo timing edges and partition completed dependency chains."""
import argparse
import collections
import datetime
import json
import pathlib
import re
import subprocess
import sys


def bad_edges(units):
    by = {u['i']: u for u in units}
    bad = []
    for unit in units:
        metadata = dict(unit.get('sections') or []).get('frontend', {}).get('end', unit['duration'])
        for key, length in [('unblocked_units', unit['duration']), ('unblocked_rmeta_units', metadata)]:
            for child in unit[key]:
                if unit['start'] + length > by[child]['start'] + .05:
                    bad.append((unit['name'], by[child]['name']))
    return bad


def chain(units):
    by = {u['i']: u for u in units}
    parents = collections.defaultdict(list)
    for unit in units:
        meta = dict(unit.get('sections') or []).get('frontend', {}).get('end', unit['duration'])
        for key, length in [('unblocked_units', unit['duration']), ('unblocked_rmeta_units', meta)]:
            for child in unit[key]:
                parents[child].append((unit['start'] + length, unit['i'], length))
    root = max(units, key=lambda u: u['start'] + u['duration'])
    parts = []

    def walk(unit, length):
        gap = unit['start']
        if parents[unit['i']]:
            end, idx, amount = max(parents[unit['i']])
            walk(by[idx], min(amount, max(0, unit['start'] - by[idx]['start'])))
            gap = max(0, unit['start'] - end)
        parts.append((unit, length, gap))

    walk(root, root['duration'])
    return parts


def packages(lock):
    # The SDK already requires Bun; use its TOML parser without a Python dependency.
    parsed = subprocess.check_output(['bun', '-e',
        'process.stdout.write(JSON.stringify(Bun.TOML.parse(await Bun.file(process.argv[1]).text()).package))',
        str(lock)], text=True)
    rows = json.loads(parsed)
    registry = {(p['name'], p['version']) for p in rows if p.get('source', '').startswith('registry+')}
    tainted = {p['name'] for p in rows if not p.get('source', '').startswith('registry+')}
    while True:
        above = {p['name'] for p in rows if any(dep.split(' ')[0] in tainted for dep in p.get('dependencies', []))}
        if above <= tainted:
            break
        tainted |= above
    return registry, tainted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directories', nargs='+', type=pathlib.Path, help='directories containing cargo-timings/ (searched recursively)')
    parser.add_argument('--phases', type=pathlib.Path, help='probe log directory containing units.jsonl')
    parser.add_argument('--lock', type=pathlib.Path, help='measured shell Cargo.lock (required with --phases)')
    parser.add_argument('--game-prefix', default='exact-1132-a2-cold')
    args = parser.parse_args()
    if bool(args.phases) != bool(args.lock):
        parser.error('--phases and --lock are used together')
    files = sorted({file.resolve() for directory in args.directories for file in directory.rglob('cargo-timing-*.html')})
    if not files:
        parser.error('no Cargo timing reports found')
    rows = [json.loads(line) for line in (args.phases / 'units.jsonl').read_text().splitlines()] if args.phases else []
    if args.phases:
        commands = [r for r in rows if r['kind'] == 'command']
        if not commands or any(r.get('code') != 0 for r in commands):
            parser.error('phase samples require a completed command with exit code 0')
    compiled = [r for r in rows if r['kind'] == 'rustc' and '--crate-name' in r['args']]
    registry, tainted = packages(args.lock) if args.lock else (set(), set())
    totals = collections.defaultdict(float)
    excluded = collections.defaultdict(float)
    reports = []
    for file in files:
        source = file.read_text()
        units = json.JSONDecoder().raw_decode(source.split('const UNIT_DATA = ', 1)[1])[0]
        assert not bad_edges(units), f'{file}: dependency ordering invalid'
        changed = json.loads(json.dumps(units))
        parent = next((unit for unit in changed if unit['unblocked_units']), None)
        if parent:
            by = {u['i']: u for u in changed}
            child = by[parent['unblocked_units'][0]]
            parent['duration'] = child['start'] - parent['start'] + 1
            assert bad_edges(changed), f'{file}: negative control missed'
        print(f'{file}: PASS edge audit; ' + ('PASS late-predecessor control' if parent else 'no edge for control'), file=sys.stderr)
        if not args.phases or not units:
            continue
        epoch = datetime.datetime.fromisoformat(re.search(r'<td>Build start:</td><td>([^<]+)', source)[1].replace('Z', '+00:00')).timestamp()
        parts = []
        for unit, length, gap in chain(units):
            name, version = unit['name'], unit['version']
            normalized = name.replace('_', '-')
            kind = ('game' if name == args.game_prefix or name.startswith(args.game_prefix + '-') else
                    'engine' if normalized.startswith(('exact-game', 'exact-gpu')) else
                    'registry' if (name, version) in registry else
                    'std' if normalized in ['core', 'alloc', 'std', 'panic-abort', 'panic-unwind', 'unwind', 'proc-macro', 'compiler-builtins', 'rustc-std-workspace-core', 'rustc-std-workspace-alloc', 'rustc-std-workspace-std', 'std-detect'] else 'core/vendor/path')
            crate = unit['target'].strip().split()[0] if unit['target'] and not unit['target'].startswith(' build-script') else name
            matches = [r for r in compiled if r['args'][r['args'].index('--crate-name') + 1].replace('_', '-') == crate and abs(r['start'] - (epoch + unit['start'])) < 3]
            link = 0
            if matches and length >= unit['duration'] - .01:
                match = min(matches, key=lambda r: abs(r['start'] - (epoch + unit['start'])))
                lto = any(a in ['lto', 'lto=fat', 'lto=thin', 'lto=yes', 'lto=true', '-Clto', '-Clto=fat', '-Clto=thin'] for a in match['args'])
                link = min(length, sum(p['time'] for p in match['phases'] if p['pass'] == 'run_linker' or lto and p['pass'] in ['LLVM_fatlto', 'LLVM_thinlto']))
            totals['cargo/scheduling'] += gap
            totals[kind] += length - link
            totals['link'] += link
            if kind == 'registry' and name in tainted:
                excluded[name] += length - link
            parts.append(dict(unit=name + unit['target'], phase=kind, seconds=length - link, link=link, gap=gap))
        reports.append(dict(file=str(file), chain=parts))
    if args.phases:
        command = [r for r in rows if r['kind'] == 'command']
        opt = sum(r['last'] - r['first'] + .2 for r in rows if r['kind'] == 'wasm-opt-sampled')
        elapsed = sum(r['end'] - r['start'] for r in command)
        eligible = totals['registry'] - sum(excluded.values())
        result = dict(chain=totals, excludedRegistry=excluded, eligibleRegistry=eligible,
                      eligibleFraction=eligible / elapsed if elapsed else None,
                      elapsed=elapsed, wasmOptSampled=opt, other=elapsed - sum(totals.values()) - opt,
                      commands=command, reports=reports)
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
