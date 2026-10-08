// The web build's launch environment: the page's query string stands in for ProcessInfo's environment
// (bench_live, bench_freeze, bench_start, bench_kinds; scenario=rest turns the 1 Hz tick on as
// BENCH_SCENARIO=rest does natively). The web pair's runner passes none of the bench_* keys, because exact2's
// web build cannot read them (std::env is empty on wasm32-unknown-unknown), and it passes scenario=rest
// with bench_notick=1 so neither app ticks.
const q = typeof location === 'undefined' ? new URLSearchParams() : new URLSearchParams(location.search);
const get = (k: string) => q.get(k) ?? '';
export const BENCH_FREEZE = get('bench_freeze') === '1';
export const BENCH_TICKS = !BENCH_FREEZE && get('bench_notick') !== '1' && (get('bench_live') === '1' || get('scenario') === 'rest');
const start = parseInt(get('bench_start'), 10);
export const BENCH_START_INDEX: number | null = Number.isFinite(start) ? start : null;
export const BENCH_KINDS17 = get('bench_kinds') === '17';
export const BENCH_KINDS_LIST: string[] | null =
  get('bench_kinds') && get('bench_kinds') !== '17' ? get('bench_kinds').split(',').map((k) => k.trim()) : null;
