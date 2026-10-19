// The Figma plugin's mapping between a Tamber export (`export {kind: "dtcg"}`,
// LLP 1121 D14) and Figma Variables (LLP 1121 §5 stage 10b, rt1's plan E4),
// with no Figma API in it, so it is tested as plain TypeScript.
//
// Pull: the export's contexts (each a full app tuple, light and dark) become
// one collection whose modes are the contexts; when the file's plan allows
// fewer modes than that, one collection per app tuple with Light and Dark
// (theme2's Q8). A colour is COLOR (its 8-bit sRGB `hex`, the export's own
// fallback), a dimension, number or duration FLOAT, a family STRING; a type,
// shadow, easing or anything else has no variable and is listed as skipped.
//
// Push: the variables as Figma has them now, written back into a copy of the
// export they were pulled from. A value Figma left as it was keeps the
// export's own object, so re-import sees no change there; a changed one is
// written as the export writes its kind. The server then plans the edits
// against the design as it is (`import` with the pulled export as `baseline`).

export type Rgba = { r: number; g: number; b: number; a: number };
export type FigmaValue = Rgba | number | string;
export type FigmaType = 'COLOR' | 'FLOAT' | 'STRING';

export interface Variable {
  /** Figma's name: the token's, `/` for `.`. */
  name: string;
  /** The token it is. */
  token: string;
  type: FigmaType;
  /** Its value per mode, by mode name. */
  values: Record<string, FigmaValue>;
}

export interface Collection {
  name: string;
  /** Mode names, in order; each a context of the export (or Light/Dark). */
  modes: string[];
  /** The export context each mode is. */
  contexts: Record<string, string>;
  variables: Variable[];
}

export interface Plan {
  collections: Collection[];
  /** Tokens with no Figma variable type, and why. */
  skipped: { token: string; why: string }[];
}

type Json = any;

/** Each context's tokens, by name (a group's `$root` the group's). */
export function tokensOf(doc: Json): Record<string, Record<string, Json>> {
  const out: Record<string, Record<string, Json>> = {};
  for (const [context, sources] of Object.entries<Json>(doc?.modifiers?.mode?.contexts ?? {})) {
    const tokens: Record<string, Json> = {};
    const walk = (v: Json, prefix: string) => {
      if (!v || typeof v !== 'object') return;
      if ('$value' in v) { tokens[prefix] = v; return; }
      for (const [k, x] of Object.entries<Json>(v)) {
        if (k.startsWith('$') && k !== '$root') continue;
        walk(x, k === '$root' ? prefix : prefix ? `${prefix}.${k}` : k);
      }
    };
    for (const source of sources) walk(source, '');
    out[context] = tokens;
  }
  return out;
}

/** A context's app tuple and scheme: `a=x, b=y, dark` → (`a=x, b=y`, dark). */
export function split(context: string): { tuple: string; dark: boolean } {
  const parts = context.split(', ');
  const scheme = parts.pop();
  return { tuple: parts.join(', '), dark: scheme === 'dark' };
}

const hexOf = (h: string): Rgba | null => {
  const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(h ?? '');
  if (!m) return null;
  const n = (i: number) => parseInt(m[1].slice(i, i + 2), 16) / 255;
  return { r: n(0), g: n(2), b: n(4), a: m[2] ? parseInt(m[2], 16) / 255 : 1 };
};
const hexFrom = (c: Rgba): string => {
  const b = (x: number) => Math.round(Math.max(0, Math.min(1, x)) * 255).toString(16).padStart(2, '0');
  return `#${b(c.r)}${b(c.g)}${b(c.b)}${c.a < 1 ? b(c.a) : ''}`;
};

/** A token's value as Figma takes it, or why it has none. */
export function toFigma(token: Json): { type: FigmaType; value: FigmaValue } | { why: string } {
  const v = token.$value;
  switch (token.$type) {
    case 'color': {
      const rgba = hexOf(v?.hex) ?? (v?.colorSpace === 'srgb' && Array.isArray(v.components) ? { r: v.components[0], g: v.components[1], b: v.components[2], a: v.alpha ?? 1 } : null);
      if (!rgba) return { why: 'a colour with no sRGB fallback' };
      if (v?.alpha !== undefined) rgba.a = v.alpha;
      return { type: 'COLOR', value: rgba };
    }
    case 'dimension': return typeof v?.value === 'number' ? { type: 'FLOAT', value: v.unit === 'rem' ? v.value * 16 : v.value } : { why: 'a dimension with no number' };
    case 'number': return typeof v === 'number' ? { type: 'FLOAT', value: v } : { why: 'not a number' };
    case 'duration': return typeof v?.value === 'number' ? { type: 'FLOAT', value: v.unit === 's' ? v.value * 1000 : v.value } : { why: 'a duration with no number' };
    case 'fontFamily': return { type: 'STRING', value: Array.isArray(v) ? v.join(', ') : String(v) };
    default: return { why: `Figma has no variable of a ${token.$type}` };
  }
}

/** Whether two Figma values are the same (colours to the 8-bit byte). */
export function same(a: FigmaValue, b: FigmaValue): boolean {
  if (typeof a === 'object' && typeof b === 'object') return hexFrom(a) === hexFrom(b);
  if (typeof a === 'number' && typeof b === 'number') return Math.abs(a - b) < 1e-6;
  return a === b;
}

/** The collections the export becomes, under a file's mode limit. */
export function pull(doc: Json, modeLimit: number): Plan {
  const byContext = tokensOf(doc);
  const contexts = Object.keys(byContext);
  if (!contexts.length) throw new Error('the export has no contexts');
  const tuples = [...new Set(contexts.map(c => split(c).tuple))];
  let groups: { name: string; contexts: Record<string, string> }[];
  if (contexts.length <= modeLimit) {
    groups = [{ name: 'Tamber', contexts: Object.fromEntries(contexts.map(c => [c, c])) }];
  } else if (modeLimit >= 2) {
    groups = tuples.map(t => ({
      name: t ? `Tamber · ${t}` : 'Tamber',
      contexts: Object.fromEntries(contexts.filter(c => split(c).tuple === t).map(c => [split(c).dark ? 'Dark' : 'Light', c])),
    }));
  } else {
    throw new Error(`this file allows ${modeLimit} mode${modeLimit === 1 ? '' : 's'} a collection; Tamber needs at least Light and Dark`);
  }
  const skipped = new Map<string, string>();
  const collections = groups.map(g => {
    const modes = Object.keys(g.contexts);
    const names = Object.keys(byContext[g.contexts[modes[0]]]);
    const variables: Variable[] = [];
    for (const token of names) {
      const values: Record<string, FigmaValue> = {};
      let type: FigmaType | null = null;
      for (const mode of modes) {
        const t = byContext[g.contexts[mode]][token];
        const f = t ? toFigma(t) : { why: 'missing from a context' };
        if ('why' in f) { skipped.set(token, f.why); type = null; break; }
        type = f.type;
        values[mode] = f.value;
      }
      if (type) variables.push({ name: token.replaceAll('.', '/'), token, type, values });
    }
    return { name: g.name, modes, contexts: g.contexts, variables };
  });
  return { collections, skipped: [...skipped].map(([token, why]) => ({ token, why })) };
}

/** A Figma value back as the export writes its kind, or null to keep the old. */
function fromFigma(old: Json, value: FigmaValue): Json {
  switch (old.$type) {
    case 'color': {
      const c = value as Rgba;
      const hex = hexFrom({ ...c, a: 1 });
      return { colorSpace: 'srgb', components: [c.r, c.g, c.b], ...(c.a < 1 ? { alpha: c.a } : {}), hex };
    }
    case 'dimension': return { unit: 'px', value };
    case 'number': return value;
    case 'duration': return { unit: 'ms', value };
    case 'fontFamily': return String(value).split(',').map(s => s.trim()).filter(Boolean);
    default: return old.$value;
  }
}

/** The export pulled (`baseline`, its text), with each variable's values as
 * Figma has them (`now`, by collection name, then variable name, then mode),
 * as the text `import` takes. */
export function push(baseline: string, plan: Plan, now: Record<string, Record<string, Record<string, FigmaValue>>>): string {
  const doc = JSON.parse(baseline);
  const byContext = tokensOf(doc);
  for (const c of plan.collections) {
    for (const v of c.variables) {
      for (const mode of c.modes) {
        const value = now[c.name]?.[v.name]?.[mode];
        const context = c.contexts[mode];
        const old = byContext[context]?.[v.token];
        if (value === undefined || !old) continue;
        const was = toFigma(old);
        if (!('why' in was) && same(was.value, value)) continue;
        old.$value = fromFigma(old, value);
      }
    }
  }
  return JSON.stringify(doc);
}

/** Variables both Figma (`now`) and the design (`fresh`, a new export's plan)
 * changed since the pull (`plan`), each mode where both moved: the person
 * keeps Figma's (pushed as a proposal) or takes Tamber's. */
export function conflicts(plan: Plan, fresh: Plan, now: Record<string, Record<string, Record<string, FigmaValue>>>): { collection: string; variable: string; mode: string }[] {
  const out: { collection: string; variable: string; mode: string }[] = [];
  for (const c of plan.collections) {
    const f = fresh.collections.find(x => x.name === c.name);
    for (const v of c.variables) {
      const fv = f?.variables.find(x => x.name === v.name);
      for (const mode of c.modes) {
        const was = v.values[mode], figma = now[c.name]?.[v.name]?.[mode], design = fv?.values[mode];
        if (figma === undefined || design === undefined) continue;
        if (!same(was, figma) && !same(was, design) && !same(figma, design)) out.push({ collection: c.name, variable: v.name, mode });
      }
    }
  }
  return out;
}
