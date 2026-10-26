// Development observations (LLP 1142 D9). None of these identifiers decide
// whether a value or a ticket can be reused.
import { equal } from './shape.js';

export function resourceInspection(enabled, now) {
  let serial = 0;
  const since = enabled ? now() : null;
  return {
    track(r, fallback, pending) {
      if (!enabled) return;
      r.inspection = { request: null, successful: null, displayed: null, fallback, placeholder: false };
      r.observation = resource => {
        const d = r.inspection;
        return { resource, source: r.source, requestGeneration: d.request,
          lastSuccessfulGeneration: d.successful, displayedGeneration: d.displayed,
          pending: pending(),
          displayed: d.fallback ? 'fallback' : d.displayed !== null && d.displayed === d.request ? 'current' : 'retained',
          argumentsEqual: d.fallback || r.settled === undefined || r.asked === undefined ? null : equal(r.settled, r.asked),
          placeholder: d.placeholder, keptSeed: false, keptSeedSupported: false };
      };
      r.resourcesMeasuredFrom = since;
    },
    begin(r) { if (r.inspection) r.inspection.request = ++serial; },
    placeholder(r) { if (r.inspection) r.inspection.placeholder = true; },
    take(r, generation = r.inspection?.request) {
      if (r.inspection) Object.assign(r.inspection, { successful: generation, displayed: generation, fallback: false, placeholder: false });
    },
    fail(r) {
      if (!enabled) return;
      const at = now(), f = r.failures ??= { count: 0, first: at, last: at };
      f.count++; f.last = at;
      console.error(`exact: resource ${r.name} failed: ${r.error ?? 'it failed'}`);
    },
  };
}
