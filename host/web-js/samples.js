// Sampled sources on the JS target (LLP 1133.013 D5, §9), as the runner's
// `modules/samples.rs`: a polled source that declares `"samples"` answers
// each reader with the latest reading and, when it asks a bound, every
// sample on its grid since its last answer, with what the bound left out
// (`dropped`) and whether the page was hidden meanwhile (`interrupted`).
//
// The cursor is the runtime's: each reader's is the `sequence` of its last
// answer, sent to an implementation as `after`; here no commit refuses a
// module's answer, so an answer is delivered once it is made. The
// substitute samples a reader's grid (every 1 / `frequency` of the session
// clock from its group's first ask) from the agent's preference set last
// before each point: a reading, or steps each `at` ms after the moment it was
// preferred. It computes nothing about the values.
import { clock } from "./rt.js";

const MOST = 1024;

/** A reader group's key: its arguments without the cursor or what it left out. */
export const groupOf = args => JSON.stringify(Object.fromEntries(Object.entries(args ?? {}).filter(([k, v]) => k !== "after" && v != null)));

export function samples() {
  const cursors = new Map(), grids = new Map(), tracks = new Map(), hidden = [];
  const hiddenAt = t => hidden.some(([from, to]) => from <= t && (to == null || t < to));
  const pausedWithin = (from, to) => hidden.some(([a, b]) => a <= to && (b == null || b > from));
  const decl = row => row.sub.samples ?? null;
  const readingAt = (row, t) => {
    for (const [set, value] of [...(tracks.get(row.key) ?? [])].reverse()) {
      if (set > t) continue;
      if (value === null) break;
      if (Array.isArray(value)) {
        const step = value.filter(s => set + (s.at ?? 0) <= t).at(-1);
        if (step) { const { at, ...reading } = step; return reading; }
        continue;
      }
      return value;
    }
    return row.sub.default;
  };
  return {
    sampled: row => !!decl(row),
    /** The page left the foreground or came back (modules.js's `shown`): a hidden span the grid skips. */
    visibility(isHidden) {
      const now = clock.now, last = hidden.at(-1);
      if (isHidden && !(last && last[1] == null)) hidden.push([now, null]);
      if (!isHidden && last && last[1] == null) last[1] = now;
      if (hidden.length > 64) hidden.shift();
    },
    /** Each computed option clamped to its declared bounds (D12 item 2), with the journal's line. */
    clamp(row, args, say) {
      for (const [option, b] of Object.entries(row.sub.bounds ?? {})) {
        const given = args?.[option];
        if (typeof given !== "number") continue;
        const clamped = Math.min(b.max ?? Infinity, Math.max(b.min ?? -Infinity, given));
        if (clamped !== given) { say(`${row.key}: ${option} ${given} clamped to ${clamped}`); args[option] = clamped; }
      }
      return args;
    },
    /** The cursor `reader` carries to an implementation, if any. */
    cursor: (row, reader) => (decl(row) ? cursors.get(`${row.key} ${reader}`) : undefined),
    /** `reader`'s answer delivered: its cursor holds. */
    answered(row, reader, json) {
      const field = decl(row)?.cursor ?? "sequence";
      if (typeof json?.[field] === "number") cursors.set(`${row.key} ${reader}`, json[field]);
    },
    /** Refuses a preferred value that is no reading, or a track whose steps are not readings each `at` ms after it is
     * preferred; `validate(answer, key)` throws for an answer outside the source's shape. */
    check(row, value, validate) {
      const d = decl(row), one = reading => {
        if (reading === null || typeof reading !== "object" || Array.isArray(reading)) throw new Error(`${row.key}: a reading is an object`);
        const { at, ...rest } = reading;
        validate({ ...rest, [d.stamp ?? "timestamp"]: 0, [d.cursor ?? "sequence"]: 0, samples: [], dropped: 0, interrupted: false }, row.key);
      };
      if (!Array.isArray(value)) return one(value);
      if (!value.length) throw new Error(`${row.key}: a track has a step`);
      for (const step of value) {
        if (!(typeof step?.at === "number" && step.at >= 0)) throw new Error(`${row.key}: each step has \`at\`, ms after it is preferred`);
        one(step);
      }
    },
    /** The agent preferred `value` for `row` now: a step of its track. */
    prefer(row, value) {
      if (row.sub.track !== true) return;
      const track = tracks.get(row.key) ?? tracks.set(row.key, []).get(row.key);
      track.push([clock.now, value === "clear" ? null : value]);
      if (track.length > 64) track.shift();
    },
    /** The substitute's answer to `reader` of sampled source `row` (§9). */
    substitute(row, args, reader) {
      const d = decl(row), field = d.cursor ?? "sequence", stamp = d.stamp ?? "timestamp";
      const frequency = d.frequency && args?.[d.frequency] > 0 ? args[d.frequency] : d.rate ?? 60, period = 1000 / frequency;
      const bound = Math.min(MOST, Math.max(0, d.bound ? args?.[d.bound] ?? row.sub.bounds?.[d.bound]?.default ?? 0 : 0));
      const key = `${row.key} ${groupOf(args)}`;
      if (!grids.has(key)) grids.set(key, clock.now);
      const start = grids.get(key), point = n => start + n * period;
      const atOrBefore = t => Math.max(0, Math.floor((t - start) / period + 1e-9));
      const now = atOrBefore(clock.now), last = hidden.at(-1);
      const latest = last && last[1] == null && last[0] > start ? atOrBefore(last[0] - 1e-6) : now;
      const reading = n => ({ ...readingAt(row, point(n)), [stamp]: point(n) });
      const out = { ...reading(latest), [field]: latest, samples: [], dropped: 0, interrupted: false };
      const after = cursors.get(`${row.key} ${reader}`);
      if (after != null) {
        const visible = [];
        for (let n = Math.max(0, after) + 1; n <= now; n++) if (!hiddenAt(point(n))) visible.push(n);
        const kept = bound ? visible.slice(Math.max(0, visible.length - bound)) : [];
        out.samples = kept.map(reading);
        out.dropped = bound ? visible.length - kept.length : 0;
        out.interrupted = pausedWithin(point(after), clock.now);
      }
      return out;
    },
    /** `state.extensions`' tracks and cursors, for `extension`. */
    state(extension) {
      const preferred = Object.fromEntries([...tracks].filter(([k, t]) => k.startsWith(`${extension}.`) && t.at(-1)?.[1] != null).map(([k, t]) => [k, t.at(-1)[1]]));
      const sampled = [...cursors].filter(([k]) => k.startsWith(`${extension}.`)).map(([k, cursor]) => ({ name: k.slice(extension.length + 1, k.indexOf(" ")), resource: k.slice(k.indexOf(" ") + 1), cursor, origin: "app" }));
      return { tracks: preferred, sampled };
    },
    /** Readers no longer asking are let go with their cursors and grids. */
    release(key) {
      for (const k of [...cursors.keys()]) if (k.startsWith(`${key} `)) cursors.delete(k);
      for (const k of [...grids.keys()]) if (k.startsWith(`${key} `)) grids.delete(k);
    },
  };
}
