import { requireNativeModule } from 'expo';

// The launch environment's BENCH_* variables, read once by native at module load
// (ProcessInfo.processInfo.environment). Empty strings when unset.
const BenchEnv = requireNativeModule<{ BENCH_LIVE: string; BENCH_START_INDEX: string }>('BenchEnv');

export const BENCH_LIVE = BenchEnv.BENCH_LIVE === '1';
const start = parseInt(BenchEnv.BENCH_START_INDEX, 10);
export const BENCH_START_INDEX: number | null = Number.isFinite(start) ? start : null;
