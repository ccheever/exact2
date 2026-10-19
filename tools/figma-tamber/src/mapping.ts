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
  /** Once written: the collection's id and each context's mode id, so a
   * collection or mode renamed in Figma is still the plan's (a mode's
   * name, cut to 40 characters, is only what Figma shows). */
  id?: string;
  contextIds?: Record<string, string>;
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
  // (an alpha that rounds to the byte 255 is opaque)
  return `#${b(c.r)}${b(c.g)}${b(c.b)}${b(c.a ?? 1) !== 'ff' ? b(c.a) : ''}`;
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
    case 'fontFamily': return { type: 'STRING', value: families(Array.isArray(v) ? v : [String(v)]) };
    default: return { why: `Figma has no variable of a ${token.$type}` };
  }
}

/** A family list as one string, CSS's way: a name with a comma, a quote, an
 * apostrophe or a backslash quoted (`"ACME, Sans", serif`). */
export function families(all: string[]): string {
  return all.map(f => (/[,"'\\]/.test(f) ? `"${f.replace(/["\\]/g, c => `\\${c}`)}"` : f)).join(', ');
}

/** A family list as CSS writes one, back as the list: a name that starts
 * with `"` or `'` quoted whole (a backslash escaping the next character),
 * the others trimmed (`Mike's Font` bare). An unterminated quote is refused. */
export function familiesOf(text: string): string[] {
  const out: string[] = [];
  let cur = '', quote = '', was = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === '\\' && i + 1 < text.length) cur += text[++i];
      else if (c === quote) quote = '';
      else cur += c;
    } else if ((c === '"' || c === "'") && !was && !cur.trim()) { quote = c; was = true; cur = ''; }
    else if (c === ',') { if (was || cur.trim()) out.push(was ? cur : cur.trim()); cur = ''; was = false; }
    else if (!was) cur += c;
  }
  if (quote) throw new Error(`a family name's quote is not closed: ${text}`);
  if (was || cur.trim()) out.push(was ? cur : cur.trim());
  return out;
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
  // The export's default context first: a new collection's default mode.
  const first = doc?.modifiers?.mode?.default;
  const contexts = Object.keys(byContext).sort((a, b) => Number(b === first) - Number(a === first));
  if (!contexts.length) throw new Error('the export has no contexts');
  const tuples = [...new Set(contexts.map(c => split(c).tuple))];
  let groups: { name: string; contexts: Record<string, string> }[];
  if (contexts.length <= modeLimit) {
    // A mode's name is the context, cut to Figma's 40 characters (and told apart) when longer.
    const used = new Set<string>();
    const named = (c: string) => {
      let n = c.length <= 40 ? c : `${c.slice(0, 39)}…`;
      for (let i = 2; used.has(n); i++) n = `${c.slice(0, 40 - String(i).length - 2)}… ${i}`;
      used.add(n);
      return n;
    };
    groups = [{ name: 'Tamber', contexts: Object.fromEntries(contexts.map(c => [named(c), c])) }];
  } else if (modeLimit >= 2) {
    if (contexts.some(c => !/, (light|dark)$/.test(`, ${c}`))) throw new Error('a context that is not light or dark: one collection a tuple needs Light and Dark');
    groups = tuples.map(t => ({
      name: t ? `Tamber · ${t}` : 'Tamber',
      // (the default's scheme first, as the contexts are ordered)
      contexts: Object.fromEntries(contexts.filter(c => split(c).tuple === t).sort((a, b) => Number(split(b).dark === split(first ?? '').dark) - Number(split(a).dark === split(first ?? '').dark)).map(c => [split(c).dark ? 'Dark' : 'Light', c])),
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
      // To the byte, as the export's sRGB fallback is, so re-import reads `#rrggbb[aa]`.
      const byte = (x: number) => Math.round(Math.max(0, Math.min(1, x)) * 255);
      const c = value as Rgba, a = byte(c.a ?? 1);
      return { colorSpace: 'srgb', components: [c.r, c.g, c.b].map(x => byte(x) / 255), ...(a < 255 ? { alpha: a / 255 } : {}), hex: hexFrom({ ...c, a: 1 }) };
    }
    case 'dimension': return { unit: 'px', value };
    case 'number': return value;
    case 'duration': return { unit: 'ms', value };
    case 'fontFamily': return familiesOf(String(value));
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

export interface Conflict {
  token: string;
  context: string;
  collection: string;
  variable: string;
  mode: string;
  /** The design has no such value now (the token or the context went). */
  gone: boolean;
}

/** Values both Figma (`now`) and the design (`fresh`, a new export) changed
 * since the pull (`plan`), by token and context, whatever either's layout
 * of collections: the person keeps Figma's (pushed as a proposal) or takes
 * Tamber's. A Figma edit whose token or context the design no longer has is
 * one too. */
export function conflicts(plan: Plan, fresh: Json, now: Record<string, Record<string, Record<string, FigmaValue>>>): Conflict[] {
  const design = tokensOf(fresh);
  const out: Conflict[] = [];
  for (const c of plan.collections) {
    for (const v of c.variables) {
      for (const mode of c.modes) {
        const context = c.contexts[mode], was = v.values[mode], figma = now[c.name]?.[v.name]?.[mode];
        if (figma === undefined || same(was, figma)) continue;
        const t = design[context]?.[v.token], f = t && toFigma(t);
        const value = f && !('why' in f) ? f.value : undefined;
        if (value === undefined) out.push({ token: v.token, context, collection: c.name, variable: v.name, mode, gone: true });
        else if (!same(was, value) && !same(figma, value)) out.push({ token: v.token, context, collection: c.name, variable: v.name, mode, gone: false });
      }
    }
  }
  return out;
}

/** The contexts a new export adds and drops beside the pull's plan. */
export function contextsMoved(plan: Plan, fresh: Json): { added: string[]; removed: string[] } {
  const had = new Set(plan.collections.flatMap(c => Object.values(c.contexts))), now = new Set(Object.keys(tokensOf(fresh)));
  return { added: [...now].filter(c => !had.has(c)), removed: [...had].filter(c => !now.has(c)) };
}
