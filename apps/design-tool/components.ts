// The component panel (LLP 1122 D7, T3): each component's params from
// `describe`'s `components` (LLP 1121 D15, tamber/1.10), grouped and ordered
// as the dials panel is, each with its declared control. A param takes a
// token as well as a literal, and its value is the app's line in the
// design's `params` block when it has one, else the default as written.
// Pure functions of `describe`; app.ts's design read uses them.
import type { Result } from './app.contract.d.ts';
import { controlOf, numberOf, sides } from './dials.ts';

type Design = Result<'loadDesign'>;
type Panel = Design['components'][number];
type Row = Panel['groups'][number]['params'][number];

/** A param as `describe` gives it (docs/tamber-protocol.md, `components`). */
type Param = {
  name: string; short: string | null; kind: string; default: string; values: unknown[] | null;
  range: [number, number] | null; step: number | null; control: string | null; options: string[] | null;
  label: string | null; help: string | null; group: string | null; priority: number | null;
  set: { value: string; whens?: number } | null; pinned: string[] | null;
  reaches?: { token: string | null; base: string | null; app_set: boolean } | null;
};

/** A param's winning level, in the words the panel says (LLP 1139 D5; stage 12 B2-levels' `level` on each of
 * `resolve`'s tuples): one for each raw level, the raw one kept as the row's tooltip. */
export const LEVEL_WORDS: Record<string, string> = {
  'app-param': 'set for this app',
  'app-token': "the app's token",
  package: "the package's token",
  base: "Exact's base",
  role: "the platform's role",
  default: "the component's default",
};
/** The words for a winning `level` at `name`, said of the param `self`: its own app line or default names nothing
 * more; another param's (one it reads) is named, by its short form (after the file's `#`). */
export const levelWords = (level: string, name: string, self = '') => {
  const words = LEVEL_WORDS[level] ?? level;
  if (level === 'app-param' || level === 'default') return !name || name === self ? words : `${words}, on ${name.split('#').pop()}`;
  return name ? `${words} ${name}` : words;
};

/** Each package param's winning level (stage 12 B2-levels' `resolve` tuples, each side's `{light, dark}`), read after
 * the design shows (each `ask` its answer, all asked at once): one that fails or does not fit the design's read
 * (`fits`) has no row, so no words; a reply with no level says so. Said for the first tuple: one level where both
 * sides agree, else each side's. */
export async function levelsOf(names: string[], ask: (name: string) => Promise<{ error: string; reply: any }>, still: () => boolean, fits: (ok: any) => boolean) {
  const rows: { name: string; level: string; text: string }[] = [];
  const side = (v: unknown, s: 'light' | 'dark') => String((v as Record<string, unknown> | null)?.[s] ?? '');
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, names.length) }, async () => {
    while (next < names.length && still()) {
      const name = names[next++], r = await ask(name), t = (r.reply?.ok?.tuples ?? []) as { level?: unknown; name?: unknown }[];
      if (r.error || !fits(r.reply.ok)) continue;
      const [l, d] = (['light', 'dark'] as const).map(s => ({ level: side(t[0]?.level, s), name: side(t[0]?.name, s) }));
      if (!l.level || !d.level) { rows.push({ name, level: '', text: 'wins: not known (this server says no level)' }); continue; }
      const one = l.level === d.level && l.name === d.name, key = (x: unknown) => JSON.stringify(x);
      const varies = t.some(x => key(x.level) !== key(t[0].level) || key(x.name) !== key(t[0].name)) ? ' (other modes differ)' : '';
      rows.push({ name, level: one ? l.level : `light: ${l.level}, dark: ${d.level}`, text: (one ? `wins: ${levelWords(l.level, l.name, name)}` : `wins in Light: ${levelWords(l.level, l.name, name)}; in Dark: ${levelWords(d.level, d.name, name)}`) + varies });
    }
  }));
  return rows;
}

/** One param's row: its control, by the dials panel's rules, on the value
 * the app has (its line, else the default); a number control starts at the
 * folded value when the written one is a token. `control=token` (the
 * default) is a field that takes a token or a literal, and so is every
 * kind the dials have no control for (a shadow, a type, an easing); a
 * duration's slider or stepper writes ms. An icon param is shown, not
 * edited: its sources. */
export function paramRow(p: Param, tokens: ReadonlySet<string>): Row | null {
  const written = typeof (p.set?.value ?? p.default) === 'string' ? (p.set?.value ?? p.default) : '';
  const first = p.values?.[0];
  const folded = typeof first === 'string' ? first : written;
  const unit = p.kind === 'length' ? 'px' : p.kind === 'duration' ? 'ms' : '';
  const meta: Record<string, string> = {};
  if (p.control && p.control !== 'token') meta.control = p.control;
  if (p.options?.length) meta.options = p.options.join('|');
  if (p.label) meta.label = p.label;
  if (p.help) meta.help = p.help;
  if (p.step != null) meta.step = String(p.step);
  if (p.priority != null) meta.priority = String(p.priority);
  const range: [string, string] | null = p.range ? [`${p.range[0]}${unit === 'px' ? 'px' : ''}`, `${p.range[1]}${unit === 'px' ? 'px' : ''}`] : null;
  // A duration's slider or stepper reads it as a number to the dials' rules,
  // its ms written back; a select or a field keeps every value as written.
  const numeric = p.kind === 'duration' && (p.control === 'slider' || p.control === 'stepper' || (!p.control && !!p.range));
  const c = controlOf({ name: p.name, kind: numeric ? 'number' : p.kind, value: numeric ? written.replace(/ms$/, '') : written, range, meta }, tokens, true);
  if (!c) return null;
  const amount = (v: string) => numberOf(numeric ? v.replace(/ms$/, '') : v);
  // (The kinds a slider or stepper writes.)
  const ranged = p.kind === 'length' || p.kind === 'number' || numeric;
  // A length or number control needs an amount: a value in rem or a token
  // with none to start from is typed instead.
  const counted = (c.control !== 'slider' && c.control !== 'stepper') || amount(written) !== null || amount(folded) !== null;
  const free = p.kind === 'icon' ? 'icon' : p.control === 'token' || !counted || !(ranged || p.kind === 'color') && c.control !== 'select' ? 'token' : c.control;
  return {
    c: {
      ...c,
      kind: p.kind,
      control: free,
      value: p.kind === 'icon' ? iconText(first) : c.control === 'select' ? c.value : written,
      options: c.options,
      number: amount(written) ?? amount(folded) ?? 0,
      unit,
      bounds: free === 'slider' || free === 'stepper' ? [p.range ? `${p.range[0]}${unit} – ${p.range[1]}${unit}` : '', `step ${c.step}${unit}`].filter(Boolean).join(' · ') : '',
      // The swatch is the value as the app's first tuple folds it, a token named or not.
      swatch: p.kind === 'color' ? folded : '',
    },
    short: p.short ?? p.name,
    set: p.set != null,
    pinned: p.pinned ?? [],
    default: typeof p.default === 'string' ? p.default : '',
    // The token its default names (by its name or its role's spelling, or a base name the app leaves unset), which
    // editing instead changes for every use (LLP 1139 D5's second choice), that token's package ("" for the app's
    // own), and whether it is a base name (Exact's, set for this app as stage 11 D does).
    token: '',
    tokenPackage: '',
    tokenBase: false,
    package: false,
    // The package token its default reaches and whether the app has its own line for it (describe's `reaches`):
    // "Back to the package" removes that line; and how many `when` branches the app's own param line has.
    packageToken: '',
    appSet: false,
    whens: p.set?.whens ?? 0,
    // Whether the app's line is a literal with no `when` branch: the one case its own value wins on both scheme sides.
    levelSelf: false,
    swatches: [],
    words: '',
  };
}

/** An icon param's value in words: its sources. */
const iconText = (v: unknown) => {
  const i = (v ?? {}) as Record<string, string | null>;
  return ['sf', 'material', 'svg'].filter(k => i[k]).map(k => `${k} ${i[k]}`).join(' · ');
};

/** The panel: every component with params or variants (a native one declared in the
 * design too, `app.tamber#Name`; a library one's variant props, D3's `variants`, listed), its groups in the order they first
 * appear, each by priority (1 first), priority 3 behind "more". A package's
 * component (its identity's file none of the app's own, `own`, describe's
 * `files`) says so: "Set for this app" writes the app's line, the package's
 * own files read only (LLP 1139 D5). */
export function componentsOf(list: { identity: string; name: string; params: Param[]; variants?: { prop: string; options: string[] }[] }[] | undefined, design: string, tokens: ReadonlySet<string>, own: ReadonlySet<string> = new Set(), tokenOf: (value: string) => string | null = v => (tokens.has(v) ? v : null), packageOf: ReadonlyMap<string, string> = new Map(), bases: ReadonlySet<string> = new Set(), look: (token: string, p: Param) => { swatches: { label: string; css: string }[]; words: string } = () => ({ swatches: [], words: '' })): Design['components'] {
  return (list ?? []).filter(c => c.params?.length || c.variants?.length).map(c => {
    const groups = new Map<string, Row[]>();
    // (A listing with none of the app's Contract sources, an older server's, says nothing of packages.)
    const home = c.identity.split('#')[0], fromPackage = [...own].some(f => f.endsWith('.contract')) && !own.has(home) && home !== design;
    for (const p of c.params ?? []) {
      const found = paramRow(p, tokens);
      if (!found) continue;
      const token = typeof p.default === 'string' ? tokenOf(p.default) ?? '' : '';
      const reached = p.reaches?.token ?? '';
      // A colour param's colour in each grid column, and the chain to it in words (review2 09:29: the grid's columns,
      // labelled; the platform's role said as one).
      const row = { ...found, ...(p.kind === 'color' ? look(token, p) : { swatches: [], words: '' }), package: fromPackage, token, tokenPackage: token ? packageOf.get(token) ?? '' : '', tokenBase: !!token && bases.has(token),
        packageToken: fromPackage && reached && packageOf.has(reached) ? reached : '', appSet: !!p.reaches?.app_set,
        levelSelf: !!p.set && !p.set.whens && /^(#[0-9a-f]{3,8}|-?[\d.]+[a-z%]*)$/i.test(p.set.value.trim()) };
      const g = p.group ?? '';
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g)!.push(row);
    }
    return {
      identity: c.identity,
      name: c.name,
      native: c.identity.startsWith(`${design}#`),
      package: fromPackage,
      variants: (c.variants ?? []).map(v => ({ prop: String(v.prop), options: (v.options ?? []).map(String) })),
      groups: [...groups].map(([name, all]) => {
        const sorted = all.map((r, i) => [r, i] as const).sort((a, b) => a[0].c.priority - b[0].c.priority || a[1] - b[1]).map(([r]) => r);
        return { name, params: sorted.filter(r => r.c.priority < 3), more: sorted.filter(r => r.c.priority === 3) };
      }),
    };
  });
}

// A role a page paints by the platform: Exact's (`-exact-…`, any case) or a CSS system colour.
const SYSTEM = new Set(['canvas', 'canvastext', 'linktext', 'visitedtext', 'activetext', 'buttonface', 'buttontext', 'buttonborder', 'field', 'fieldtext', 'highlight', 'highlighttext', 'selecteditem', 'selecteditemtext', 'mark', 'marktext', 'graytext', 'accentcolor', 'accentcolortext']);
const isRole = (v: string) => /^-exact-/i.test(v.trim()) || SYSTEM.has(v.trim().toLowerCase());

/** A colour param's look (the second hands-on pass, #4; review2 09:29): its colour in each of the token grid's
 * columns, in their order and with their labels, and the chain to it in words. A param the app sets, or one naming
 * no token, reads its own values (one per tuple, its scheme side taken as a swatch shows it); one following a token
 * reads that token's cells as the grid paints them (a residual by its fallback). The words follow the token: what the
 * app's row for it follows (its dials) or its value, the platform's role where that is a role (or the token is one
 * the app does not set), Exact's base for a base name the app leaves unset. */
export function lookOf(columns: { label: string; tuple: number; dark: boolean }[], stored: ReadonlyMap<string, { css: string; srgb?: string; residual?: boolean; value?: unknown }[][]>, rows: { name: string; start?: string; follows?: string }[]) {
  return (token: string, p: Param) => {
    const cells = !p.set && token ? stored.get(token) : undefined;
    const swatches = columns.map(c => {
      const v = cells?.[c.tuple]?.[c.dark ? 1 : 0];
      const css = v ? (v.residual ? v.srgb ?? '' : v.css) : sides(String(p.values?.[c.tuple] ?? ''))[c.dark ? 1 : 0];
      return { label: c.label, css };
    // (A role the page alone paints, Exact's or a system colour, has no colour to show here: its words say it.)
    }).filter(s => s.css && !isRole(s.css));
    // What the app's row follows (its dials), else its value where every column shares it; else that it differs.
    const row = rows.find(r => r.name === token), same = new Set((stored.get(token) ?? []).map(t => String(t[0]?.value ?? ''))).size <= 1;
    const written = row?.follows || (same ? row?.start : 'its value by mode, in its row') || '';
    const def = typeof p.default === 'string' ? p.default : '';
    const words = p.set ? `set for this app: ${p.set.value}`
      : token ? `${token} → ${row ? (isRole(written) ? "the platform's role" : written || "Exact's base") : isRole(token) ? "the platform's role" : "Exact's base"}`
      : isRole(def) ? `${def} → the platform's role` : '';
    return { swatches, words };
  };
}
