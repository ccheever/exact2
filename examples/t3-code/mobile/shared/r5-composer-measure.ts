// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r5-composer-measure.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r5-composer: label widths measured from laid-out text (r5-composer.contract's
// probes, reported by T3Composer.swift's t3-anchor hook as [x, width]), adapted
// from T3 Code (MIT; see LICENSE-T3): chat/restingComposerControlsMeasurement.ts
// reads the same natural widths from labels kept mounted while icons replace them.
import { obj, type Obj } from './domain';

export type ProbeKind = 'model' | 'traits' | 'runtime' | 'plan';
/** A label's measured width at the toolbar's size ("sm") or the resting row's ("xs"); null until laid out. */
export type Measure = (kind: ProbeKind, small: boolean) => number | null;

/** The probes' widths from the module's anchors (0 or missing: not laid out yet). */
export function measuredLabels(presentation: Obj): Measure {
  const anchors = obj(presentation.anchors);
  return (kind, small) => {
    const value = anchors[`m-${kind}-${small ? 'xs' : 'sm'}`];
    const width = Array.isArray(value) ? Number(value[1]) : NaN;
    return Number.isFinite(width) && width > 0 ? width : null;
  };
}

/** A label's width: the measurement when there is one, else the estimate. An empty label is 0 wide. */
export function labelWidth(text: string, small: boolean, kind: ProbeKind, measure: Measure | undefined, estimate: (text: string, small: boolean) => number): number {
  if (!text) return 0;
  return measure?.(kind, small) ?? estimate(text, small);
}
