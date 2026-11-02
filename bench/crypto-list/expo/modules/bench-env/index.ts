import { requireNativeModule } from 'expo';

// The launch environment's BENCH_* variables, read once by native at module load
// (ProcessInfo.processInfo.environment). Empty strings when unset.
const BenchEnv = requireNativeModule<{
  BENCH_LIVE: string;
  BENCH_SCENARIO: string;
  BENCH_FREEZE: string;
  BENCH_START_INDEX: string;
}>('BenchEnv');

export const BENCH_FREEZE = BenchEnv.BENCH_FREEZE === '1';
/** Ticks on iff BENCH_LIVE=1 or BENCH_SCENARIO=rest, and BENCH_FREEZE is not 1 (SPEC Decisions). */
export const BENCH_TICKS =
  !BENCH_FREEZE && (BenchEnv.BENCH_LIVE === '1' || BenchEnv.BENCH_SCENARIO === 'rest');
const start = parseInt(BenchEnv.BENCH_START_INDEX, 10);
export const BENCH_START_INDEX: number | null = Number.isFinite(start) ? start : null;
