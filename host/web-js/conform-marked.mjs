// An interim marker on the wasm reference (theme2 + the lead, 2026-10-30; the
// landing fix removes it). The web's wasm host binds no package implementation
// under the agent (QUEUE.md, "The web's wasm host has no extension modules",
// 1624 in the ruling; LLP 1133.008 §9), while the JS target does, so on these
// three apps the two pages differ where a package answers. Their targets still
// run in full; a wasm-vs-JS difference on them (state, tree, layout, paint,
// screenshot, bound refusals, sound lines) is recorded and counted under the
// marker instead of failing `--strict`. Every other failure on them (a build,
// an open, a crash, a step that did not run or that one page refused, an app
// test that failed) still fails. Exactly these three, with exactly this reason:
// conform.test.mjs refuses an addition.
export const WASM_UNBOUND_REASON = 'reference does not bind packages (LLP 1071 retirement)';
export const WASM_UNBOUND = Object.freeze({
  'photo-editor': WASM_UNBOUND_REASON,
  recorder: WASM_UNBOUND_REASON,
  markdown: WASM_UNBOUND_REASON,
});

/** Moves the wasm-vs-JS differences (failures tagged `difference: true`) of the
 *  marked targets out of `report.failures` into `report.marked[target]`
 *  ({ reason, differences, steps, lines }); every other failure stays. */
export function markUnbound(report, marker = WASM_UNBOUND) {
  report.marked ??= {};
  const kept = [];
  for (const f of report.failures) {
    const reason = Object.hasOwn(marker, f.target) ? marker[f.target] : null;
    if (!reason || f.difference !== true) { kept.push(f); continue; }
    const m = report.marked[f.target] ??= { reason, differences: 0, steps: [], lines: [] };
    m.differences++;
    if (!m.steps.includes(f.step)) m.steps.push(f.step);
    m.lines.push(`${f.step}: ${f.what}`);
  }
  report.failures = kept;
  return report;
}

/** The drive's automatic tap that the reference refused. Elsewhere it is a
 *  skipped step, as before; on a marked target it fails (never a difference,
 *  so never marked): the JS page never takes that tap, so a refused operation
 *  there (recorder's Record button, disabled without a microphone) would
 *  otherwise leave the run green under the marker. */
export function referenceTapRefused({ target, step, why, crossBrowser, fail, report }, marker = WASM_UNBOUND) {
  if (!crossBrowser && Object.hasOwn(marker, target)) fail(step, `${why} (the reference refused the tap, so the JS page never took it)`);
  else report.steps.push({ target, step, skipped: why });
}
