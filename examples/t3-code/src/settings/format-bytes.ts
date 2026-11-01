// T3 Code 2359643cff (MIT, LICENSE-T3) apps/web/src/lib/formatBytes.ts: the byte sizes of Diagnostics and Storage.
const scaled = (value: number) => value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2);

export function formatBytes(value: number): string {
  if (Math.round(value) < 1_024) return `${Math.round(value)} B`;
  const units = ['KB', 'MB', 'GB', 'TB'] as const;
  let next = value / 1_024, unitIndex = 0;
  // The unit is picked after rounding, so 1023.6 KB reads "1.00 MB", not "1024 KB".
  while (Number(scaled(next)) >= 1_024 && unitIndex < units.length - 1) { next /= 1_024; unitIndex += 1; }
  return `${scaled(next)} ${units[unitIndex]}`;
}
