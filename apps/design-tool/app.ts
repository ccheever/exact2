// The design tool's protocol client (LLP 1122 D2): `tamber/1` over HTTP to the
// app's Tamber server, which serves this page from the same local origin
// (`bun exact.mjs tamber tool`). The token comes once from `GET
// /__tamber/token`, which answers only this machine's loopback, and rides
// every POST as `x-exact-tamber-token`, never in a URL. Changes are followed
// by the long poll, `POST /__tamber/events`: a data module's `fetch` reads a
// body whole, so it cannot follow the server-sent stream. A failure is
// words in the answer, never a throw.
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.exact.design-tool';
// The tool's own local URL: `tamber tool` serves it here, and a grant names
// one origin and port.
export const API = 'http://127.0.0.1:7740';
// And the files the person picks to import, which the host copies under
// app:/tmp/picked/.
export const grants = `net.fetch ${API}\nfs.read app:/tmp`;

const RESTARTED = 'The design server restarted (or another took its port) since this session began: nothing was written, and undo and redo start again.';
// A write's wait: longer than the server's own for a write (two minutes),
// so the tool never gives up on one that then lands.
const WRITE_WAIT = 130_000;
const NO_SERVER = `No design server answers at ${API}. Start one for your app with \`bun exact.mjs tamber tool <app>\`, then try again.`;

type Diagnostic = { file?: string; line?: number; id?: string; message?: string };
type Reply = { ok?: any; error?: { message?: string; diagnostics?: Diagnostic[] } };
/** A refusal's words: its message, and each diagnostic's file, line, id
 * and explanation, which say what to correct. */
const refusal = (error: Reply['error'], status: number) => {
  const said = (error?.diagnostics ?? []).map(d => `${d.file ?? ''}${d.line ? `:${d.line}` : ''} ${d.id ?? ''}: ${d.message ?? ''}`.trim());
  return [error?.message ?? `The design server refused the request (${status}).`, ...said].join('\n');
};
let token: string | null = null;
// One ask at a time: the first reads start together.
let asking: Promise<{ token: string | null; error: string }> | null = null;

/** The token, asked once; null with the words to show when there is none. */
function tokenOf(): Promise<{ token: string | null; error: string }> {
  if (token) return Promise.resolve({ token, error: '' });
  asking ??= askToken().finally(() => { asking = null; });
  return asking;
}

async function askToken(): Promise<{ token: string | null; error: string }> {
  try {
    const res = await fetch(`${API}/__tamber/token`, { exactTimeout: 5000, redirect: 'error' });
    if (!res.ok) return { token: null, error: `${API} gave no design server token: it answers only this machine's loopback, and only while \`bun exact.mjs tamber tool\` runs.` };
    token = String(((await res.json()) as { token?: unknown }).token ?? '') || null;
    return { token, error: token ? '' : NO_SERVER };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { token: null, error: NO_SERVER };
  }
}

/** One `tamber/1` request: its reply (and the token it went with), or the
 * words to show. A token the server no longer knows (it restarted: a bare
 * not-found, which an operation's own refusal never is) is asked for again,
 * once, for a read. A write goes with the token of the server run it
 * belongs to (`bound`: the one its review, or its inverse, came from), and
 * never with another, so it cannot reach whatever server holds the port
 * since: that is a refusal, the server restarted. */
async function call(path: string, body: unknown, timeout = 15000, bound?: string): Promise<{ error: string; reply: Reply; used: string }> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const got = bound ? { token: bound, error: '' } : await tokenOf();
    if (!got.token) return { error: got.error, reply: {}, used: '' };
    try {
      const res = await fetch(`${API}${path}`, { method: 'POST', exactTimeout: timeout, redirect: 'error', body: JSON.stringify(body),
        headers: { 'content-type': 'application/json', 'x-exact-tamber-token': got.token } });
      const reply = (await res.json().catch(() => ({}))) as Reply;
      const unknown = res.status === 404 && !reply.error;
      if (unknown && bound) { if (token === bound) token = null; return { error: RESTARTED, reply: {}, used: bound }; }
      if (unknown && attempt === 0) { token = null; continue; }
      if (res.ok) return { error: '', reply, used: got.token };
      return { error: refusal(reply.error, res.status), reply, used: got.token };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { error: NO_SERVER, reply: {}, used: '' };
    }
  }
  return { error: NO_SERVER, reply: {}, used: '' };
}

const files = (digests: Record<string, string> = {}) => Object.entries(digests).map(([path, digest]) => ({ path, digest }));

type Change = Result<'waitChanges'>['recent'][number];
// The session's changes, newest first, as the long poll brought them.
let recent: Change[] = [];
let answers = 0;
let place = { seq: 0, epoch: '' };

/** An event from the long poll as the view lists it. */
function change(e: any): Change | null {
  const who = typeof e.client === 'string' ? e.client : 'changed on disk';
  switch (e.event) {
    case 'changed': return { seq: e.seq, kind: 'changed', who, files: e.edited ?? [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'committed': return { seq: e.seq, kind: 'committed', who, files: e.files ?? [], tokens: [] };
    case 'proposal': return { seq: e.seq, kind: 'proposal', who, files: [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'proposal-dropped': return { seq: e.seq, kind: `proposal ${e.reason ?? 'dropped'}`, who: typeof e.client === 'string' ? e.client : 'the server', files: [], tokens: [] };
    case 'down': return { seq: e.seq, kind: 'down', who: 'the server', files: [], tokens: [] };
    default: return null;
  }
}

type Design = Result<'loadDesign'>;
type Control = Design['groups'][number]['dials'][number];
const noDesign = (error: string): Design => ({ ok: false, error, file: '', answer: ++designs, groups: [], columns: [], tokenGroups: [], names: [] });
let designs = 0;

/** A number as Tamber writes one: a plain number, or a length's `Npx`; null
 * for anything else. */
const numberOf = (v: string | undefined) => {
  const t = v?.trim().replace(/px$/i, '') ?? '';
  return t !== '' && Number.isFinite(Number(t)) ? Number(t) : null;
};
/** The decimal places a number is written to, an exponent counted:
 * `5e-3` is 3, `0.25` is 2, `12px` is 0. */
const decimals = (v: string | undefined) => {
  const m = /^[+-]?\d*(?:\.(\d+))?(?:e([+-]?\d+))?$/i.exec(v?.trim().replace(/px$/i, '') ?? '');
  return m ? Math.max(0, (m[1]?.length ?? 0) - Number(m[2] ?? 0)) : 0;
};
// CSS's named colours (CSS Color 4 §6.1), as color/src/named.rs has them.
const NAMED = new Map((
  'aliceblue:f0f8ff antiquewhite:faebd7 aqua:00ffff aquamarine:7fffd4 azure:f0ffff beige:f5f5dc ' +
  'bisque:ffe4c4 black:000000 blanchedalmond:ffebcd blue:0000ff blueviolet:8a2be2 brown:a52a2a ' +
  'burlywood:deb887 cadetblue:5f9ea0 chartreuse:7fff00 chocolate:d2691e coral:ff7f50 ' +
  'cornflowerblue:6495ed cornsilk:fff8dc crimson:dc143c cyan:00ffff darkblue:00008b darkcyan:008b8b ' +
  'darkgoldenrod:b8860b darkgray:a9a9a9 darkgreen:006400 darkgrey:a9a9a9 darkkhaki:bdb76b ' +
  'darkmagenta:8b008b darkolivegreen:556b2f darkorange:ff8c00 darkorchid:9932cc darkred:8b0000 ' +
  'darksalmon:e9967a darkseagreen:8fbc8f darkslateblue:483d8b darkslategray:2f4f4f ' +
  'darkslategrey:2f4f4f darkturquoise:00ced1 darkviolet:9400d3 deeppink:ff1493 deepskyblue:00bfff ' +
  'dimgray:696969 dimgrey:696969 dodgerblue:1e90ff firebrick:b22222 floralwhite:fffaf0 ' +
  'forestgreen:228b22 fuchsia:ff00ff gainsboro:dcdcdc ghostwhite:f8f8ff gold:ffd700 ' +
  'goldenrod:daa520 gray:808080 green:008000 greenyellow:adff2f grey:808080 honeydew:f0fff0 ' +
  'hotpink:ff69b4 indianred:cd5c5c indigo:4b0082 ivory:fffff0 khaki:f0e68c lavender:e6e6fa ' +
  'lavenderblush:fff0f5 lawngreen:7cfc00 lemonchiffon:fffacd lightblue:add8e6 lightcoral:f08080 ' +
  'lightcyan:e0ffff lightgoldenrodyellow:fafad2 lightgray:d3d3d3 lightgreen:90ee90 lightgrey:d3d3d3 ' +
  'lightpink:ffb6c1 lightsalmon:ffa07a lightseagreen:20b2aa lightskyblue:87cefa ' +
  'lightslategray:778899 lightslategrey:778899 lightsteelblue:b0c4de lightyellow:ffffe0 lime:00ff00 ' +
  'limegreen:32cd32 linen:faf0e6 magenta:ff00ff maroon:800000 mediumaquamarine:66cdaa ' +
  'mediumblue:0000cd mediumorchid:ba55d3 mediumpurple:9370db mediumseagreen:3cb371 ' +
  'mediumslateblue:7b68ee mediumspringgreen:00fa9a mediumturquoise:48d1cc mediumvioletred:c71585 ' +
  'midnightblue:191970 mintcream:f5fffa mistyrose:ffe4e1 moccasin:ffe4b5 navajowhite:ffdead ' +
  'navy:000080 oldlace:fdf5e6 olive:808000 olivedrab:6b8e23 orange:ffa500 orangered:ff4500 ' +
  'orchid:da70d6 palegoldenrod:eee8aa palegreen:98fb98 paleturquoise:afeeee palevioletred:db7093 ' +
  'papayawhip:ffefd5 peachpuff:ffdab9 peru:cd853f pink:ffc0cb plum:dda0dd powderblue:b0e0e6 ' +
  'purple:800080 rebeccapurple:663399 red:ff0000 rosybrown:bc8f8f royalblue:4169e1 ' +
  'saddlebrown:8b4513 salmon:fa8072 sandybrown:f4a460 seagreen:2e8b57 seashell:fff5ee sienna:a0522d ' +
  'silver:c0c0c0 skyblue:87ceeb slateblue:6a5acd slategray:708090 slategrey:708090 snow:fffafa ' +
  'springgreen:00ff7f steelblue:4682b4 tan:d2b48c teal:008080 thistle:d8bfd8 tomato:ff6347 ' +
  'turquoise:40e0d0 violet:ee82ee wheat:f5deb3 white:ffffff whitesmoke:f5f5f5 yellow:ffff00 ' +
  'yellowgreen:9acd32 '
).trim().split(' ').map(p => p.split(':') as [string, string]));

/** A colour written as hex, `rgb()` (numbers or percentages, commas or
 * spaces, any alpha) or a CSS name, folded as the server folds a dial's
 * colour (`#rrggbbaa`, lower case); another colour function as written,
 * its spaces tidied. */
export function folded(v: string): string {
  const t = v.trim().toLowerCase().replace(/\s+/g, ' ');
  const hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(t)?.[1];
  if (hex) {
    const full = hex.length <= 4 ? [...hex].map(c => c + c).join('') : hex;
    return `#${full}${full.length === 6 ? 'ff' : ''}`;
  }
  if (t === 'transparent') return '#00000000';
  if (NAMED.has(t)) return `#${NAMED.get(t)}ff`;
  const num = (x: string, of: number) => (x.endsWith('%') ? (Number(x.slice(0, -1)) / 100) * of : Number(x));
  const rgb = /^rgba?\( ?([\d.]+%?)[ ,]+([\d.]+%?)[ ,]+([\d.]+%?)(?: ?[,/] ?([\d.]+%?))? ?\)$/.exec(t);
  if (rgb) {
    const byte = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, '0');
    return `#${[rgb[1], rgb[2], rgb[3]].map(x => byte(num(x, 255))).join('')}${byte(num(rgb[4] ?? '1', 1) * 255)}`;
  }
  return t;
}
/** Whether `v` is written as a colour literal: a hex, a CSS name, or a
 * colour function's call (its arguments the server's to judge); a
 * token's name is not, whatever it is spelled. */
export const colourLiteral = (v: string, tokens: ReadonlySet<string>) => {
  const t = v.trim();
  if (tokens.has(t)) return false;
  return /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(t) || t.toLowerCase() === 'transparent' || NAMED.has(t.toLowerCase())
    || /^(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\([^()]*\)$/i.test(t);
};
/** Whether `v` is a dial's literal of `kind`: a colour's, a plain number
 * for a number (no unit), `0` or `Npx` for a length. */
const literalOf = (kind: string, v: string, tokens: ReadonlySet<string>) =>
  kind === 'color' ? colourLiteral(v, tokens)
  : kind === 'length' ? /^(0|[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?px)$/i.test(v.trim())
  : /^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(v.trim());

/** A dial's control from its declared metadata (LLP 1121 D4, D15): its
 * `control`, else its kind's own (a colour's field, a ranged number's
 * slider, else a stepper); `none` is not drawn. Metadata is kept, not
 * checked, by Tamber, so a control that does not fit the kind (a stepper
 * for a colour, a slider with no range, a select with no options) is the
 * kind's own, said in its help; a step or priority that is no number is
 * left at its default. `token` names a picker of tokens, which a dial (a
 * literal) has none of. A select's option that is no literal of the kind
 * (a token's name: `tokens` are the design's) is left out, said so, since
 * `set-dial` takes a literal. */
export function controlOf(d: { name: string; kind: string; value: string; range: [string, string] | null; meta: Record<string, string> }, tokens: ReadonlySet<string> = new Set()): Control | null {
  const m = d.meta ?? {};
  const own = d.kind === 'color' ? 'color' : d.range ? 'slider' : 'stepper';
  const declared = m.control;
  if (declared === 'none') return null;
  const written = (m.options ?? '').split('|').map(o => o.trim()).filter(Boolean);
  const literal = (o: string) => literalOf(d.kind, o, tokens);
  const options = written.filter(literal);
  const dropped = written.filter(o => !literal(o));
  const fits = declared === 'color' ? d.kind === 'color'
    : declared === 'slider' ? d.kind !== 'color' && !!d.range
    : declared === 'stepper' ? d.kind !== 'color'
    : declared === 'select' ? options.length > 0
    : false;
  const control = fits ? declared : own;
  // With no step, the finest the value and the range are written in, so
  // every declared value is a step (at least 0.01).
  const finest = 10 ** -Math.max(2, decimals(d.value), decimals(d.range?.[0]), decimals(d.range?.[1]));
  // A length's step is in px (LLP 1121 D15), written either way.
  const declaredStep = m.step !== undefined && numberOf(m.step) !== null && (d.kind === 'length' || !/px$/i.test(m.step)) ? numberOf(m.step) : null;
  const step = declaredStep !== null && declaredStep > 0 ? declaredStep : d.kind === 'number' ? finest : d.kind === 'length' ? 10 ** -Math.max(0, decimals(d.value), decimals(d.range?.[0]), decimals(d.range?.[1])) : 1;
  const priority = [1, 2, 3].includes(Number(m.priority)) ? Number(m.priority) : 2;
  const said = [
    declared && declared !== 'token' && !fits ? `control=${declared} does not fit this ${d.kind} dial; its own control is shown.` : '',
    dropped.length ? `${dropped.join(', ')} ${dropped.length === 1 ? 'is' : 'are'} not a value a dial can take (a dial is a literal), so ${dropped.length === 1 ? 'it is' : 'they are'} not offered.` : '',
  ].filter(Boolean).join(' ');
  // A select's options as given, the current value matched by meaning (a
  // colour folded as the server folds it), else listed first.
  const same = (o: string) => (d.kind === 'color' ? folded(o) === folded(d.value) : numberOf(o) === numberOf(d.value));
  const matched = options.find(same);
  return {
    name: d.name, kind: d.kind, control, label: m.label ?? d.name, help: [m.help ?? '', said].filter(Boolean).join(' '),
    value: control === 'select' ? matched ?? d.value : d.value,
    // With no range a stepper is bounded by nothing a person would type.
    number: numberOf(d.value) ?? 0, min: numberOf(d.range?.[0]) ?? -1e12, max: numberOf(d.range?.[1]) ?? 1e12, step, priority,
    bounds: control === 'slider' || control === 'stepper' ? [d.range ? `${numberOf(d.range[0])} – ${numberOf(d.range[1])}` : '', `step ${step}`].filter(Boolean).join(' · ') : '',
    options: control === 'select' && !matched ? [d.value, ...options] : options,
  };
}

/** The dials panel: groups in the order they first appear, each by
 * priority (1 first), priority 3 behind "more". */
export function panelOf(dials: Parameters<typeof controlOf>[0][], tokens: ReadonlySet<string> = new Set()): Design['groups'] {
  const groups = new Map<string, Control[]>();
  for (const d of dials) {
    const c = controlOf(d, tokens);
    if (!c) continue;
    const g = d.meta?.group ?? '';
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g)!.push(c);
  }
  return [...groups].map(([name, all]) => {
    const sorted = all.map((c, i) => [c, i] as const).sort((a, b) => a[0].priority - b[0].priority || a[1] - b[1]).map(([c]) => c);
    return { name, dials: sorted.filter(c => c.priority < 3), more: sorted.filter(c => c.priority === 3) };
  });
}

type Axis = { name: string; values: string[]; under: string | null };
type Column = { id: string; label: string; tuple: number; dark: boolean; values: string[] };
/** The grid's columns: every app-mode tuple, light and dark. A light column
 * for an `under scheme=dark` axis's other values repeats the default's (the
 * axis varies the dark side only), so it is left out. */
export function columnsOf(axes: Axis[]): Column[] {
  const tuples: string[][] = [[]];
  for (const a of axes) tuples.splice(0, tuples.length, ...tuples.flatMap(t => a.values.map(v => [...t, v])));
  const out: Column[] = [];
  for (const dark of [false, true]) {
    tuples.forEach((values, tuple) => {
      if (!dark && axes.some((a, i) => a.under === 'dark' && values[i] !== a.values[0])) return;
      // Each mode off its default, by axis, so two axes' like-named values
      // never make one label.
      const modes = values.flatMap((v, i) => (!(axes[i].under === 'dark' && !dark) && v !== axes[i].values[0] ? [`${axes[i].name}=${v}`] : []));
      out.push({ id: `${tuple}-${dark ? 'dark' : 'light'}`, label: [dark ? 'Dark' : 'Light', ...modes].join(' · '), tuple, dark, values });
    });
  }
  return out;
}

type When = { axis: string; value: string };
const whenOf = (text: string): When[] => text.split(' ').filter(Boolean).map(w => { const [axis, value] = w.split('='); return { axis, value }; });
const whenText = (w: When[]) => w.map(c => `${c.axis}=${c.value}`).sort().join(' ');

/** The branch a cell shows (LLP 1122 D6's target branch), as the checker
 * picks it: of the `when`s whose every condition holds at the column's
 * tuple, the one naming the most axes; on a light column a `when` naming an
 * `under scheme=dark` axis is passed over (it sets the dark side only). Two
 * that tie are ambiguous, which the checker refuses, so a checked design
 * has none; said, if one comes. And "only this mode": a `when` naming every
 * axis at this tuple, which holds here alone; on a light column with an
 * under-dark axis such a `when` would set the dark side only, so there is
 * none to offer ("" ). */
export function branchOf(axes: Axis[], col: Column, whens: { when: When[]; value: string }[]): { when: string; index: number; tie: string; own: string } {
  const at = (axis: string) => col.values[axes.findIndex(a => a.name === axis)];
  const under = new Set(axes.filter(a => a.under === 'dark').map(a => a.name));
  const holding = whens.map((w, index) => ({ ...w, index })).filter(w => (col.dark || !w.when.some(c => under.has(c.axis))) && w.when.every(c => at(c.axis) === c.value));
  const most = Math.max(0, ...holding.map(w => w.when.length));
  const top = holding.filter(w => w.when.length === most && most > 0);
  const own = !col.dark && under.size > 0 ? '' : whenText(axes.map((a, i) => ({ axis: a.name, value: col.values[i] })));
  return { when: top[0] ? whenText(top[0].when) : '', index: top[0]?.index ?? -1, tie: top.length > 1 ? top.map(w => `when ${whenText(w.when)}`).join(' and ') : '', own };
}

// CSS's system colours, which a value may name as a platform colour.
const SYSTEM = new Set(['canvas', 'canvastext', 'linktext', 'visitedtext', 'activetext', 'buttonface', 'buttontext', 'buttonborder', 'field', 'fieldtext', 'highlight', 'highlighttext', 'selecteditem', 'selecteditemtext', 'mark', 'marktext', 'graytext', 'accentcolor', 'accentcolortext']);
/** What a value names, as the checker reads a name: a token by its name, a
 * role override by its row's spelling or its bare name in any case
 * (`label` for `-exact-label`), a dial by its name; and a platform role or
 * system colour no token overrides (`-exact-secondary-label`, `Canvas`),
 * which also makes the value an expression. A word is a whole word of the
 * value: never part of a hex colour or a number. */
export function inputsOf(value: string, names: string[], dials: string[]): { tokens: string[]; dials: string[]; roles: string[] } {
  const words = value.split(/[\s,()/*+]+/).filter(w => /^[A-Za-z_-][\w.-]*$/.test(w));
  const set = new Set(words), lower = new Set(words.map(w => w.toLowerCase()));
  const named = (n: string) => set.has(n) || lower.has(n.toLowerCase()) || (n.toLowerCase().startsWith('-exact-') && lower.has(n.toLowerCase().slice(7)));
  const tokens = names.filter(named), found = dials.filter(d => set.has(d));
  const taken = new Set([...tokens, ...found].map(n => n.toLowerCase()));
  const roles = words.filter(w => (w.toLowerCase().startsWith('-exact-') || SYSTEM.has(w.toLowerCase())) && !taken.has(w.toLowerCase()) && !tokens.some(n => n.toLowerCase() === `-exact-${w.toLowerCase()}`));
  return { tokens, dials: found, roles: [...new Set(roles)] };
}

// Each design read, by its number (given as the read starts): its axes,
// its columns, its dials' groups and each token's stored values per tuple.
// A read that a newer one has overtaken is never shown (the runner drops
// its answer), so it keeps nothing; the shown one is the newest kept, and
// what follows it (a cell's branch, contrast) names it.
type Stored = { css: string; srgb: string; residual: boolean; note: string; value: string };
type Snapshot = { file: string; axes: Axis[]; columns: Column[]; stored: Map<string, Stored[][]>; dials: Map<string, { group: string; more: boolean }>; digests: string };
const snapshots = new Map<number, Snapshot>();
let issued = 0;
// Whether running apps take a value edit with no reload: a server says so
// in its hello once it has the live table (LLP 1121 stage 4). Until then
// every edit reaches them through the dev loop's reload.
let liveTable = false;

/** One consistent read of a design: `describe`, each token's `resolve`, and
 * the files' digests again after them; if a write landed between, the
 * replies may be of two revisions, so it reads again. */
async function readDesign(file: string): Promise<{ error: string; ir?: any; resolved?: { tuples: any[] }[]; files?: Record<string, string>; pairs?: unknown }> {
  const which = file ? { file } : {};
  for (let attempt = 0; attempt < 3; attempt++) {
    const described = await call('/__tamber', { method: 'describe', ...which });
    if (described.error) return { error: described.error };
    const ir = described.reply.ok?.ir, pairs = described.reply.ok?.pairs;
    if (!ir || !Array.isArray(ir.tokens) || !Array.isArray(ir.axes) || ir.axes.some((a: Axis) => !Array.isArray(a.values))) return { error: 'The design server answered describe with no design in it.' };
    const resolved = await Promise.all(ir.tokens.map((t: { name: string }) => call('/__tamber', { method: 'resolve', token: t.name, ...which })));
    const failed = resolved.find(r => r.error);
    if (failed) return { error: failed.error };
    const after = await call('/__tamber', { method: 'hello' });
    if (after.error) return { error: after.error };
    const before = described.reply.ok.files ?? {};
    const now = after.reply.ok?.files ?? {};
    const same = Object.keys(before).length === Object.keys(now).length && Object.entries(before).every(([f, d]) => now[f] === d);
    // And each tuple is the one its index says, by its modes.
    const tuples = columnsOf(ir.axes);
    // Each resolve read the revision describe did (an edit and its undo
    // between leave the same digests at the end, not in between).
    const one = resolved.every(r => digestKey(r.reply.ok?.files) === digestKey(before));
    const placed = one && resolved.every(r => (r.reply.ok?.tuples ?? []).every((t: { modes?: Record<string, string> }, i: number) => {
      const col = tuples.find(c => c.tuple === i);
      return !col || ir.axes.every((a: Axis, k: number) => t.modes?.[a.name] === col.values[k]);
    }));
    if (same && placed) return { error: '', ir, resolved: resolved.map(r => r.reply.ok), files: before, pairs };
  }
  return { error: 'The design kept changing while it was read. Try again.' };
}

/** An edit's operation in `tamber/1`'s shape, on `file` (the app's design
 * when ""). */
function opOf(file: string, kind: string, target: string, when: string, value: string): Record<string, unknown> {
  const on = file ? { file } : {};
  switch (kind) {
    case 'set-dial': return { op: kind, name: target, value, ...on };
    case 'set-token': return { op: kind, name: target, when: whenOf(when), value, ...on };
    case 'add-token': return { op: kind, name: target, value, ...on };
    case 'rename-token': return { op: kind, from: target, to: value, ...on };
    default: return { op: 'remove-token', name: target, ...on };
  }
}
const label = (kind: string, target: string, when: string, value: string) =>
  kind === 'rename-token' ? `${target} → ${value}` : kind === 'remove-token' ? `removed ${target}` : `${target}${when ? ` when ${when}` : ''} = ${value}`;
/** An edit's identity, as the view spells its draft (`draftKey`). */
const keyOf = (file: string, kind: string, target: string, when: string, value: string) => [file, kind, target, when, value].join('␟');

// The session's undo and redo, for one run of one server: another run (or
// another app's server on the port) has none of these inverses, and a redo
// there would write this app's edit into that one, so a new epoch empties
// them, and `session` counts those.
// Each with the token of the server run that issued its inverse.
type Entry = { inverse: string; ops: Record<string, unknown>[]; label: string; token: string };
const undoStack: Entry[] = [], redoStack: Entry[] = [];
let session = 0;
/** The stacks emptied: another run of the server has none of their
 * inverses. `session` counts these, and an operation that began before one
 * changes neither stack when it ends. */
function forget() {
  undoStack.length = 0; redoStack.length = 0; mine.length = 0; session += 1;
}
const stacks = (ok: boolean, error: string, last: string, key = '') => ({ ok, error, undo: undoStack.length, redo: redoStack.length, last, key, session, note: '' });
/** Who moved a file a refused write or undo found moved: its client, or
 * "changed on disk". */
const byWhom = (reply: Reply) => {
  const by = (reply.error as { by?: Record<string, string> } | undefined)?.by ?? {};
  const said = Object.entries(by).map(([file, who]) => `${file} was ${who === 'changed on disk' ? 'changed on disk' : `changed by ${who}`}`);
  return said.length ? said.join('; ') : '';
};
const conflict = (reply: Reply) => (reply.error as { conflict?: boolean } | undefined)?.conflict === true;


// The session changeset (LLP 1122 D9): every write since the tool opened,
// its own and others', newest first. Each says its files, who, and the
// token values before and after; under the tool's own (an undo and a redo
// among them), the diff its reply gave. The tool's own write and its event
// are matched by the server's number for the write, which both carry,
// whichever comes first.
//
// Before and after are each file's digest either side of the event (each
// event carries the digests after it; the ones before are the last
// event's, or the poll's when it starts again: the first answer, a gap, a
// new run). A design read is kept by its file's digest, so a change is
// measured, file by file, between two reads of exactly those states,
// whenever each was read; a state no read saw leaves its tokens named only.
type Noted = { seq: number; who: string; acceptedBy: string; write: number; changed: { file: string; token: string }[]; files: string[]; before: Record<string, string>; after: Record<string, string>; mine: boolean; diff: string; values: Map<string, string[]>; committed: boolean };
let changeset: Noted[] = [];
// The tool's own writes not yet matched to their events, by write number.
const mine: { write: number; diff: string }[] = [];
/** The tool's own write, as its reply says it: matched to its event, now
 * or when the event comes. */
function wroteMine(ok: { write?: number; diff?: string }) {
  if (!ok.write) return;
  const n = changeset.find(c => c.write === ok.write);
  if (n) { n.mine = true; n.diff = ok.diff ?? ''; } else mine.push({ write: ok.write, diff: ok.diff ?? '' });
}
// The app's design file, as hello names it: what "" reads.
let designFile = '';
let lastFiles: Record<string, string> = {};
const digestKey = (files: Record<string, string> = {}) => JSON.stringify(Object.keys(files).sort().map(f => [f, files[f]]));
// Each read's stored values, axes and columns, by `<file>|<its digest>`; a few.
const reads = new Map<string, { stored: Map<string, Stored[][]>; axes: Axis[]; columns: Column[] }>();
const named = (file: string) => file || designFile;
/** An entry's tokens in one file. */
const tokensIn = (n: Noted, file: string) => [...new Set(n.changed.filter(c => c.file === file).map(c => c.token))];

/** A client's colour, the same each time (LLP 1122 D10: each client's
 * edits in its own colour). */
const TINTS = ['-exact-system-blue', '-exact-system-green', '-exact-system-orange', '-exact-system-purple', '-exact-system-pink', '-exact-system-teal', '-exact-system-indigo', '-exact-system-brown'];
// Given in the order clients first appear, so the first eight differ.
const tinted = new Map<string, string>();
export const tintOf = (who: string) => {
  if (!tinted.has(who)) tinted.set(who, TINTS[tinted.size % TINTS.length]);
  return tinted.get(who)!;
};

// Held proposals, as the last poll answer listed them (LLP 1122 D10).
type Held = Result<'waitChanges'>['held'][number];
let held: Held[] = [];
// Each held proposal's operations (for redoing one accepted, and editing an
// import's before accepting it) and its review.
let heldOps = new Map<string, any[]>();
let heldReview = new Map<string, any>();
// The token of the server run whose poll listed them.
let heldToken = '';
/** A value's sides, light and dark, as swatches show them: `light-dark(a,
 * b)` split at its top-level comma, else one value for both. */
export function sides(v: string): [string, string] {
  const m = /^light-dark\((.*)\)$/s.exec(v.trim());
  if (!m) return [v.trim(), v.trim()];
  let depth = 0;
  for (let i = 0; i < m[1].length; i++) {
    const c = m[1][i];
    if (c === '(') depth++;
    else if (c === ')') depth--;
    else if (c === ',' && depth === 0) return [m[1].slice(0, i).trim(), m[1].slice(i + 1).trim()];
  }
  return [v.trim(), v.trim()];
}
/** An import's review (LLP 1122 D11) as the view lists it: the mapped
 * tokens with their values per mode, the role overrides it offers, the
 * values it could not map and why, and the count check. */
function reviewOf(p: any) {
  const r = p.review, none = { importer: '', mapped: [], offers: [], unmapped: [], counted: '', countOk: true };
  if (!r?.importer) return none;
  const ops: any[] = (p.ops ?? []).map((o: { op: unknown }) => o.op);
  const mapped = ops.slice(0, Number(r.tokens ?? 0)).map((op, index) => {
    const modes = [{ label: 'default', value: String(op.value ?? '') }, ...(op.whens ?? []).map((w: any) => ({ label: whenText(w.when ?? []), value: String(w.value ?? '') }))];
    return { index, name: String(op.name ?? ''), modes: modes.map(m => ({ label: m.label, value: m.value, light: sides(m.value)[0], dark: sides(m.value)[1] })) };
  });
  const c = r.counted ?? {};
  return {
    importer: String(r.importer), mapped,
    // Off until ticked; in a recomputed import, the ones it kept were
    // ticked already, so they are on.
    // An offer with no items is one the importer could not make (its note
    // says why), or, in a recomputed import, one not chosen: that is left out.
    offers: (r.offers ?? []).filter((o: any) => (o.items ?? []).length > 0 || (!p.replaces && o.note)).map((o: any) => ({ role: String(o.role), token: String(o.token ?? ''), items: (o.items ?? []).join(','), note: String(o.note ?? ''), on: !!p.replaces })),
    unmapped: (r.unmapped ?? []).map((u: any) => ({ name: String(u.name), value: String(u.value ?? ''), why: String(u.why ?? ''), derived: u.block === '(derived)' })),
    counted: `${c.source} source values = ${c.mapped} mapped + ${c.reported} reported`, countOk: c.source === c.mapped + c.reported,
  };
}
/** An operation in plain words, as a held proposal lists its items. */
const opText = (o: any): string => {
  const what =
    o?.op === 'set-token' ? `${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''} = ${o.value}`
    : o?.op === 'unset-token-when' ? `${o.name}: drop when ${whenText(o.when ?? [])}`
    : o?.op === 'set-dial' ? `${o.name} = ${o.value}`
    : o?.op === 'add-token' ? `add ${o.name} = ${o.value}`
    : o?.op === 'rename-token' ? `rename ${o.from} → ${o.to}`
    : o?.op === 'remove-token' ? `remove ${o.name}`
    : typeof o?.op === 'string' && /axis/.test(o.op) ? `${o.op.replace(/-/g, ' ')} ${o.name ?? o.axis ?? ''}${o.values ? ` = ${[].concat(o.values).join(' | ')}` : ''}`.trim()
    : JSON.stringify(o);
  // The file it edits, when it names one: two items may differ only there.
  return o?.file ? `${what} (${o.file})` : what;
};

/** An event as the changeset keeps it. */
function noted(e: any) {
  if (e.event === 'committed') {
    // What was written before the commit, in its files.
    for (const n of changeset) if (n.seq < e.seq && n.files.some(f => (e.files ?? []).includes(f))) n.committed = true;
    return;
  }
  if (e.event !== 'changed') return;
  const who = typeof e.client === 'string' ? e.client : 'changed on disk';
  const before = lastFiles, after: Record<string, string> = e.files ?? {};
  lastFiles = after;
  const at = e.write ? mine.findIndex(m => m.write === e.write) : -1;
  const own = at >= 0 ? mine.splice(at, 1)[0] : null;
  changeset.unshift({
    seq: e.seq, who, acceptedBy: typeof e.acceptedBy === 'string' && e.acceptedBy !== who ? e.acceptedBy : '', write: Number(e.write ?? 0),
    changed: (e.changed ?? []).map((c: { file: string; token: string }) => ({ file: String(c.file), token: String(c.token) })), files: e.edited ?? [],
    before, after, mine: !!own, diff: own?.diff ?? '', values: new Map(), committed: false,
  });
  // Only what is committed is let go of, past 200.
  while (changeset.length > 200 && changeset.findLastIndex(n => n.committed) >= 0) changeset.splice(changeset.findLastIndex(n => n.committed), 1);
}

/** Each tuple's modes, as a key that names it whatever the axes' order of
 * values: a tuple is compared with the one of the same modes. */
const tupleKeys = (axes: Axis[]) => columnsOf(axes).filter(c => c.dark).map(c => ({ tuple: c.tuple, key: axes.map((a, i) => `${a.name}=${c.values[i]}`).join(' '), label: c.label }));
/** A token's value change between two reads, in plain words: its first
 * tuple, by modes, that moved (named when not the first), or added or
 * removed; a tuple only one side has is said so. */
function moved(token: string, a: { stored: Map<string, Stored[][]>; axes: Axis[] }, b: { stored: Map<string, Stored[][]>; axes: Axis[] }): string | null {
  const before = a.stored.get(token), after = b.stored.get(token);
  if (!before && !after) return null;
  if (!before) return `added ${token} = ${after![0]?.[0]?.value ?? ''}`;
  if (!after) return `removed ${token} (was ${before[0]?.[0]?.value ?? ''})`;
  const was = new Map(tupleKeys(a.axes).map(t => [t.key, before[t.tuple]?.[0]?.value]));
  const keys = tupleKeys(b.axes);
  for (const [i, t] of keys.entries()) {
    const now = after[t.tuple]?.[0]?.value, then = was.get(t.key);
    if (now === then) continue;
    const where = i === 0 ? '' : ` (${t.label.replace(/^Dark · /, '')})`;
    return then === undefined ? `${token} = ${now}${where}, a mode it had not` : `${token} ${then} → ${now}${where}`;
  }
  // A mode the design no longer has.
  const gone = tupleKeys(a.axes).find(t => !keys.some(k => k.key === t.key));
  return gone ? `${token} (${gone.label.replace(/^Dark · /, '')}) removed, was ${before[gone.tuple]?.[0]?.value ?? ''}` : null;
}

/** Each entry's files not yet measured whose states either side have been
 * read: that file's tokens' value changes; a hand edit, which names no
 * tokens, every token of the file that moved. */
function measure() {
  for (const n of changeset) {
    for (const f of n.files) {
      if (n.values.has(f)) continue;
      const a = reads.get(`${f}|${n.before[f]}`), b = reads.get(`${f}|${n.after[f]}`);
      if (!a || !b) continue;
      let tokens = tokensIn(n, f);
      if (!tokens.length) {
        tokens = [...new Set([...a.stored.keys(), ...b.stored.keys()])].filter(t => moved(t, a, b) !== null);
        n.changed.push(...tokens.map(token => ({ file: f, token })));
      }
      n.values.set(f, tokens.map(t => moved(t, a, b)).filter((v): v is string => v !== null));
    }
  }
}
/** One file of an entry in plain words: what moved in it (its tokens
 * named, until measured), who. */
const fileWords = (n: Noted, f: string) => `${f}: ${n.values.get(f)?.join('; ') || tokensIn(n, f).join(', ') || 'edited'} · ${n.who}${n.acceptedBy ? `, accepted by ${n.acceptedBy}` : ''}`;
/** An entry in plain words: each file and what moved in it (its tokens
 * named, until measured), who. */
const plain = (n: Noted) => `${n.files.map(f => `${f}: ${n.values.get(f)?.join('; ') || tokensIn(n, f).join(', ') || 'edited'}`).join('; ') || 'the design: edited'} · ${n.who}${n.acceptedBy ? `, accepted by ${n.acceptedBy}` : ''}`;

const sources: Sources = {
  loadHello: async () => {
    const { error, reply } = await call('/__tamber', { method: 'hello' });
    if (error) return { ok: false, error, protocol: '', format: 0, app: '', files: [] };
    const ok = reply.ok ?? {};
    liveTable = ok.live === true;
    designFile = String(ok.design ?? '');
    return { ok: true, error: '', protocol: String(ok.protocol ?? ''), format: Number(ok.format ?? 0), app: String(ok.app ?? ''), files: files(ok.files) };
  },
  loadDesign: async ([file]) => {
    const number = ++issued;
    try {
      const read = await readDesign(file);
      if (read.error) return noDesign(read.error);
      const ir = read.ir, resolved = read.resolved!;
      const axes: Axis[] = ir.axes;
      const columns = columnsOf(axes);
      // The first read after changes arrived highlights the tokens they
      // changed, in their client's colour; a read again at the same place
      // does not.
      // Each token's stored value per tuple and scheme, a residual as its
      // fallback (no running app reports it until T2).
      const stored = new Map<string, Stored[][]>();
      const tokens = ir.tokens.map((t: { name: string; kind: string; dials: string[]; role: string | null }, i: number) => {
        const tuples = (resolved[i].tuples ?? []) as any[];
        const sides = tuples.map(r => [false, true].map(dark => {
          const side = dark ? 'dark' : 'light';
          if (r.residual) return { css: r.value, srgb: r.fallback?.[side] ?? '', residual: true, note: r.value, value: r.value };
          return { css: r[side], srgb: r.srgb?.[side] ?? r[side], residual: false, note: '', value: r.value };
        }));
        stored.set(t.name, sides);
        const cells = columns.map(c => {
          const v = sides[c.tuple]?.[c.dark ? 1 : 0] ?? { css: '', srgb: '', residual: false, note: '', value: '' };
          return { column: c.id, label: `${t.name}, ${c.label}, ${v.residual ? `${v.note}, fallback ${v.srgb}` : v.css}`, css: v.css, swatch: v.residual ? v.srgb : v.css, residual: v.residual, note: v.note, pairs: [] as { name: string; ratio: string; low: boolean }[] };
        });
        return { name: t.name, kind: t.kind, follows: (t.dials ?? []).join(', '), cells };
      });
      // Each declared pair's contrast per cell (LLP 1121 D3, `pair=`): the
      // token over what it is drawn on, a token's stored value or a role's
      // fallback; below 4.5:1, WCAG's for text, it is marked low.
      const declared = (read.pairs ?? {}) as Record<string, { name: string; token: boolean; fallback: { light: string; dark: string } | null }[]>;
      for (const row of tokens) {
        for (const pair of declared[row.name] ?? []) {
          const under = stored.get(pair.name), mine = stored.get(row.name)!;
          for (const [k, c] of columns.entries()) {
            const fg = mine[c.tuple]?.[c.dark ? 1 : 0];
            const bg = under ? under[c.tuple]?.[c.dark ? 1 : 0]?.srgb : pair.fallback?.[c.dark ? 'dark' : 'light'];
            const r = fg && bg ? contrast(fg.srgb, bg) : 'not a colour';
            const residual = fg?.residual || (under ? under[c.tuple]?.[c.dark ? 1 : 0]?.residual : true);
            row.cells[k].pairs.push(typeof r === 'string'
              ? { name: pair.name, ratio: r, low: false }
              : { name: pair.name, ratio: `${r.toFixed(2)}:1${residual ? ' (fallback)' : ''}`, low: r < 4.5 });
          }
        }
      }
      // Rows grouped by their dotted prefix (an undotted name's before its
      // first `_`, as `contrast_0`'s); a role override among the roles.
      const groups = new Map<string, typeof tokens>();
      // A role override, as the IR marks it (`role`), whatever its name.
      const roles = new Set(ir.tokens.filter((t: { role: string | null }) => t.role).map((t: { name: string }) => t.name));
      for (const t of tokens) {
        const g = roles.has(t.name) ? 'roles' : t.name.split(t.name.includes('.') ? '.' : '_')[0];
        if (!groups.has(g)) groups.set(g, []);
        groups.get(g)!.push(t);
      }
      const names = new Set<string>(tokens.map((t: { name: string }) => t.name));
      const panel = panelOf(ir.dials ?? [], names);
      const dials = new Map<string, { group: string; more: boolean }>();
      for (const g of panel) for (const [list, more] of [[g.dials, false], [g.more, true]] as const) for (const c of list) dials.set(c.name, { group: g.name, more });
      // Overtaken: never shown, so nothing to keep.
      if (number === issued) {
        snapshots.set(number, { file, axes, columns, stored, dials, digests: digestKey(read.files) });
        for (const old of snapshots.keys()) if (old < number - 4) snapshots.delete(old);
        reads.set(`${named(file)}|${read.files?.[named(file)] ?? ''}`, { stored, axes, columns });
        while (reads.size > 24) reads.delete(reads.keys().next().value!);
      }
      return {
        ok: true, error: '', file: file || '', answer: number, groups: panel,
        columns: columns.map(({ id, label }) => ({ id, label })),
        tokenGroups: [...groups].map(([name, rows]) => ({ name, tokens: rows })),
        names: [...names],
      };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return noDesign(`The design could not be read: ${(e as Error).message ?? e}`);
    }
  },
  loadBranch: async ([file, token, column, answer]) => {
    const none = { ok: true, error: '', token: '', column: '', label: '', when: '', value: '', expression: false, inputs: [], dials: [], roles: [], exact: true, own: '', answer };
    const snap = snapshots.get(answer);
    const col = snap?.columns.find(c => c.id === column);
    if (!token || !col || !snap || snap.file !== file) return none;
    const { error, reply } = await call('/__tamber', { method: 'explain', token, ...(file ? { file } : {}) });
    if (error) return { ...none, ok: false, error };
    const x = reply.ok as { value: string; whens: { when: When[]; value: string }[]; dials: string[]; names: string[]; files?: Record<string, string> };
    // Of the revision the grid shows, or it is read again with the grid.
    if (x.files && digestKey(x.files) !== snap.digests) return { ...none, ok: false, error: 'The design changed while this cell was read; it is read again with the grid.' };
    const b = branchOf(snap.axes, col, x.whens);
    if (b.tie) return { ...none, ok: false, error: `${token} has two branches for ${col.label}, ${b.tie}; the checker refuses that, so fix the file by hand.` };
    const value = b.index >= 0 ? x.whens[b.index].value : x.value;
    // An expression's inputs: the tokens and dials it names; a dial's
    // control is where it is edited.
    const inputs = inputsOf(value, x.names, x.dials);
    // A dial with no control (`control=none`) is said, not offered.
    const dials = inputs.dials.map(name => ({ name, group: snap.dials.get(name)?.group ?? '', more: snap.dials.get(name)?.more ?? false, hidden: !snap.dials.has(name) }));
    return {
      ok: true, error: '', token, column, label: `${token} · ${b.when || 'default'} (${col.label})`, when: b.when, value,
      expression: inputs.tokens.length + inputs.dials.length + inputs.roles.length > 0, inputs: inputs.tokens, dials, roles: inputs.roles, exact: b.own === '' || b.when === b.own, own: b.own, answer,
    };
  },
  proposeEdit: async ([file, kind, target, when, value]) => {
    const key = keyOf(file, kind, target, when, value);
    const idle = { ok: true, error: '', active: false, chip: '', diff: '', tokens: [], key, review: '' };
    if (!kind || !target) return idle;
    if (kind !== 'remove-token' && !value) return { ...idle, ok: false, error: 'Give it a value.' };
    const { error, reply, used } = await call('/__tamber', { method: 'propose', ops: [opOf(file, kind, target, when, value)] });
    if (error) return { ...idle, ok: false, error };
    const ok = reply.ok as { diff: string; class: string; files: Record<string, string>; changed: { token: string }[] };
    // What Apply writes: this edit, against the digests this proposal read,
    // to the server run that read them (its token), carried on the proposal
    // itself, so what is shown is what is written, and where.
    return { ok: true, error: '', active: true, chip: ok.class === 'live' && liveTable ? 'live' : 'rebuild', diff: ok.diff, tokens: ok.changed.map(c => c.token), key, review: JSON.stringify({ key, files: ok.files, diff: ok.diff, token: used }) };
  },
  writeEdit: async ([file, kind, target, when, value, client, review]) => {
    const at = session;
    const key = keyOf(file, kind, target, when, value);
    let read: { key?: string; files?: Record<string, string>; diff?: string; token?: string } = {};
    try { read = JSON.parse(review || '{}'); } catch { /* none */ }
    if (read.key !== key || !read.files || !read.token) return stacks(false, 'Review the change before applying it.', '');
    const op = opOf(file, kind, target, when, value);
    // Every file's digest as the proposal read it: the server checks the
    // ones the edit writes, whichever file the app's design is.
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: read.files, ops: [op] }, WRITE_WAIT, read.token);
    if (error === RESTARTED) forget();
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) return stacks(false, byWhom(reply) ? `Not written: ${byWhom(reply)} since you reviewed it.` : error, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    // A write that changed no byte has no inverse, and leaves redo be.
    if (inverse) { undoStack.push({ inverse, ops: [op], label: label(kind, target, when, value), token: read.token }); redoStack.length = 0; wroteMine(reply.ok); }
    return stacks(true, '', kind.split('-')[0], key);
  },
  undoLast: async ([client]) => {
    const at = session;
    const top = undoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to undo.', '');
    const { error, reply } = await call('/__tamber', { method: 'undo', client, inverse: top.inverse }, WRITE_WAIT, top.token);
    if (error === RESTARTED) forget();
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) {
      // Refused for a conflict, it can't apply while the file stays as it
      // is: say who moved it, and set it aside. An inverse the server no
      // longer has (used by an undo whose answer was lost, or dropped) is
      // set aside too, said so. Any other failure (no answer) keeps it to
      // try again.
      if (/^no inverse/.test(error)) {
        undoStack.pop();
        return stacks(false, `Can't undo ${top.label}: the server no longer has its inverse (it may be undone already). It is set aside.`, '');
      }
      if (!conflict(reply)) return stacks(false, error, '');
      undoStack.pop();
      return stacks(false, `Can't undo ${top.label}: ${byWhom(reply) || 'the file changed'} since. It is set aside.`, '');
    }
    undoStack.pop();
    redoStack.push(top);
    wroteMine(reply.ok);
    return stacks(true, '', 'undo');
  },
  redoLast: async ([client]) => {
    const at = session;
    const top = redoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to redo.', '');
    // The original transaction again, against the files as they are now,
    // on the run of the server it was undone on (its token).
    const hello = await call('/__tamber', { method: 'hello' }, 15000, top.token);
    if (hello.error === RESTARTED) forget();
    if (session !== at) return stacks(false, RESTARTED, '');
    if (hello.error) return stacks(false, hello.error, '');
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: hello.reply.ok?.files ?? {}, ops: top.ops }, WRITE_WAIT, top.token);
    if (error === RESTARTED) forget();
    if (session !== at) return stacks(false, RESTARTED, '');
    if (error) {
      if (!conflict(reply)) return stacks(false, `Can't redo ${top.label}: ${error}`, '');
      return stacks(false, `Can't redo ${top.label}: ${byWhom(reply) || 'the file changed'} since. Try again.`, '');
    }
    redoStack.pop();
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    if (inverse) { undoStack.push({ ...top, inverse }); wroteMine(reply.ok); }
    return stacks(true, '', 'redo');
  },
  loadContrast: async ([a, b, answer]) => {
    if (!a || !b) return { ok: true, error: '', ratios: [] };
    const snap = snapshots.get(answer);
    if (!snap) return { ok: true, error: '', ratios: [] };
    const x = snap.stored.get(a), y = snap.stored.get(b);
    if (!x || !y) return { ok: false, error: `${!x ? a : b} is not in this design.`, ratios: [] };
    return { ok: true, error: '', ratios: snap.columns.map(c => {
      const p = x[c.tuple]?.[c.dark ? 1 : 0], q = y[c.tuple]?.[c.dark ? 1 : 0];
      const r = p && q ? contrast(p.srgb, q.srgb) : 'not a colour';
      return { column: c.id, label: c.label, ratio: typeof r === 'string' ? r : `${r.toFixed(2)}:1${p!.residual || q!.residual ? ' (fallback)' : ''}` };
    }) };
  },
  waitChanges: async ([since, epoch]) => {
    // Up to 25 s with nothing new, and the server answers once there is.
    // The first asks, at once, from the start of what the server keeps.
    const { error, reply, used } = await call('/__tamber/events', epoch ? { after: since, epoch } : { after: 0, wait: false }, 40000);
    answers += 1;
    const state = () => ({ answer: answers, seq: place.seq, epoch: place.epoch, recent, undo: undoStack.length, redo: redoStack.length, session, held });
    if (error) return { ok: false, error, missed: false, ...state() };
    const r = reply as { events?: unknown[]; seq?: number; epoch?: string; missed?: boolean; proposals?: any[] };
    // The open proposals, each with its proposer, diff and tokens; a
    // recomputed one says which it replaces, and is reviewed afresh.
    heldOps = new Map((r.proposals ?? []).map(p => [String(p.proposal), (p.ops ?? []).map((o: { op: unknown }) => o.op)]));
    heldReview = new Map((r.proposals ?? []).map(p => [String(p.proposal), p.review]));
    heldToken = used;
    held = (r.proposals ?? []).map(p => ({
      proposal: String(p.proposal), client: String(p.client ?? ''), tint: tintOf(String(p.client ?? '')), diff: String(p.diff ?? ''),
      tokens: [...new Set<string>((p.changed ?? []).map((c: { token: string }) => c.token))], replaces: String(p.replaces ?? ''),
      items: (p.ops ?? []).map((o: { index: number; op: unknown }) => ({ index: Number(o.index), text: opText(o.op) })),
      ...reviewOf(p),
    }));
    // Another run of the server (or another app's, on this port), or more
    // missed than it keeps: its sequence numbers start again, or the list
    // has a gap, so the list starts again. Another run has none of this
    // session's inverses: undo and redo start again too.
    // Another run's session has none of this one's writes to commit: the
    // changeset starts again with it.
    if (place.epoch && r.epoch !== place.epoch) { forget(); changeset = []; reads.clear(); }
    if ((place.epoch && r.epoch !== place.epoch) || r.missed) recent = [];
    // The session's changeset: every write since the tool opened (not the
    // backlog the first answer carries).
    // Where the changes start from: the files as the poll has them when it
    // starts again (the first answer, a gap it says it missed, a new run).
    const anew = !epoch || r.missed || (place.epoch && r.epoch !== place.epoch);
    if (anew) { lastFiles = (r as { files?: Record<string, string> }).files ?? {}; if (r.missed) mine.length = 0; }
    if (epoch && !r.missed) for (const e of (r.events ?? []) as any[]) noted(e);
    const fresh = (r.events ?? []).map(change).filter((c): c is Change => c !== null).reverse();
    recent = [...fresh, ...recent].slice(0, 50);
    place = { seq: Number(r.seq ?? 0), epoch: String(r.epoch ?? '') };
    return { ok: true, error: '', missed: r.missed === true, ...state() };
  },
  loadLit: async ([file, , faded, fadedEpoch]) => {
    // The tokens of this file changed since the highlight last faded (in
    // this run of the server), each by its latest client, for a short
    // highlight in its colour: the newest change a read has shown, and
    // every one before it; a newer one no read has shown waits for one.
    const f = named(file), since = fadedEpoch === place.epoch ? faded : 0;
    measure();
    const shown = changeset.findIndex(n => n.seq > since && n.files.includes(f) && reads.has(`${f}|${n.after[f]}`));
    const by = new Map<string, string>();
    const lit = shown < 0 ? [] : changeset.slice(shown).filter(n => n.seq > since && n.files.includes(f));
    for (const n of [...lit].reverse()) for (const t of tokensIn(n, f)) by.set(t, n.who);
    return { seq: lit[0]?.seq ?? since, epoch: place.epoch, names: [...by.keys()], whos: [...by.values()], tints: [...by.values()].map(tintOf) };
  },
  loadScope: async ([client]) => {
    // What a commit would take now (a dry run, of the run the poll is on),
    // so Commit is on when there is something, writes from before the tool
    // opened among them, a refusal to say, or no answer (it says why when
    // pressed). Read again with each poll answer.
    const dry = await call('/__tamber', { method: 'commit', client, dry: true }, 15000, heldToken || undefined);
    const refused = ((dry.reply.error as { refused?: unknown[] } | undefined)?.refused ?? []).length;
    const nothing = /nothing to commit|written nothing/.test(dry.error);
    return { open: dry.error ? 0 : ((dry.reply.ok as { edits?: unknown[] })?.edits ?? []).length, refused: refused > 0, unavailable: !!dry.error && !refused && !nothing };
  },
  loadChangeset: async () => (measure(), {
    ok: true, error: '',
    entries: changeset.map(n => ({ seq: n.seq, who: n.who, tint: tintOf(n.who), mine: n.mine, text: plain(n), diff: n.diff, committed: n.committed })),
  }),
  commitSession: async ([client]) => {
    // What the commit takes is the server's session, which can hold writes
    // from before the tool opened: a dry run, of the run the poll is on,
    // says it, and the message is composed from that, each edit by its
    // write's number in the words of that write's row (for its file), else
    // its tokens and client. The commit goes to the run the dry run read,
    // with its scope, so a write in between is refused, not committed under
    // a message that leaves it out. The server adds each edit's author and
    // file, and never pushes.
    const at = session;
    const refusedOf = (reply: Reply) => ((reply.error as { refused?: { file: string; why: string }[] } | undefined)?.refused ?? []).map(r => ({ file: String(r.file), why: String(r.why) }));
    const fail = (error: string, refused: { file: string; why: string }[]) => ({ ok: false, error: refused.length ? 'Not committed: these files are not only the session\'s. Commit them by hand.' : error, commit: '', files: [], refused, edits: [] });
    const dry = await call('/__tamber', { method: 'commit', client, dry: true }, 15000, heldToken || undefined);
    if (dry.error === RESTARTED) forget();
    if (session !== at) return fail(RESTARTED, []);
    if (dry.error) return fail(dry.error, refusedOf(dry.reply));
    type Edit = { file: string; client: string; tokens: string[]; write?: number };
    const scope = dry.reply.ok as { files: string[]; edits: Edit[]; scope: string };
    const words = (e: Edit) => {
      const n = e.write ? changeset.find(m => m.write === e.write) : undefined;
      return n ? fileWords(n, e.file) : `${e.file}: ${e.tokens.join(', ') || 'edited'} · ${e.client}`;
    };
    const lines = scope.edits.map(words);
    const authors = [...new Set(scope.edits.map(e => e.client))];
    const message = `design: ${scope.edits.length} change${scope.edits.length === 1 ? '' : 's'} by ${authors.join(', ')}\n\n${lines.map(l => `- ${l}`).join('\n')}`;
    const sent = changeset[0]?.seq ?? 0;
    const { error, reply } = await call('/__tamber', { method: 'commit', client, message, scope: scope.scope }, WRITE_WAIT, dry.used);
    if (error === RESTARTED) forget();
    if (session !== at) return fail(RESTARTED, []);
    if (error) return fail((reply.error as { stale?: boolean } | undefined)?.stale ? 'Something was written while the commit was being composed: nothing was committed. Commit again.' : error, refusedOf(reply));
    const ok = reply.ok as { commit: string; files: string[] };
    // What was written before it was sent, in its files; a write since is
    // not in it. The edits it took are the scope's, as composed.
    for (const n of changeset) if (n.seq <= sent && n.files.some(f => ok.files.includes(f))) n.committed = true;
    return { ok: true, error: '', commit: ok.commit.slice(0, 12), files: ok.files, refused: [], edits: lines.map((text, n) => ({ n, text })) };
  },
  loadHistory: async ([shown, pages]) => {
    const none = { ok: true, error: '', commits: [], more: false };
    if (!shown) return none;
    // Ten a page, the newest first.
    const { error, reply } = await call('/__tamber', { method: 'history', limit: 10, skip: 10 * (Math.max(1, pages) - 1) }, 30000);
    if (error) return { ...none, ok: false, error };
    const ok = reply.ok as { commits: any[]; more: boolean };
    const when = (w: When[]) => (w?.length ? ` when ${whenText(w)}` : '');
    const edit = (e: { what: string; name: string; when: When[]; before: string | null; after: string | null }) =>
      e.before === null ? `added ${e.what} ${e.name}${when(e.when)} = ${e.after}`
      : e.after === null ? `removed ${e.what} ${e.name}${when(e.when)} (was ${e.before})`
      : `${e.name}${when(e.when)} ${e.before} → ${e.after}`;
    return {
      ok: true, error: '', more: ok.more === true,
      commits: ok.commits.map(c => ({
        commit: String(c.commit), short: String(c.commit).slice(0, 12), author: String(c.author ?? ''), date: String(c.date ?? '').slice(0, 10),
        subject: String(c.message ?? '').split('\n')[0],
        files: (c.files ?? []).map((f: any) => ({ file: String(f.file), refused: f.refused ? String(f.refused) : '', edits: (f.edits ?? []).map(edit) })),
      })),
    };
  },
  acceptHeld: async ([client, proposal, items]) => {
    // All, or the items picked (indices of its operations). Some, or a
    // file moved since it was held: the server recomputes it on today's
    // files and holds that instead, to be reviewed and accepted afresh.
    // Its operations, kept now: the poll that answers as it is accepted no
    // longer lists it.
    const at = session, some = items ? items.split(',').map(Number) : undefined, ops = (heldOps.get(proposal) ?? []) as Record<string, unknown>[];
    // To the server run that listed it.
    const { error, reply } = await call('/__tamber', { method: 'accept', client, proposal, ...(some ? { items: some } : {}) }, WRITE_WAIT, heldToken || undefined);
    if (error === RESTARTED) forget();
    if (session !== at) return { ...stacks(false, RESTARTED, ''), note: '' };
    if (error) return { ...stacks(false, error, ''), note: '' };
    const ok = reply.ok as { recomputed?: boolean; proposal?: string; inverse?: string | null; changed?: { token: string }[] };
    if (ok.recomputed) return { ...stacks(true, '', 'recomputed'), note: `${proposal} was recomputed on the files as they are now, as ${ok.proposal}: review its diff and accept it again.` };
    // Accepted: written as its proposer's, and one step of this session's undo.
    if (ok.inverse) { undoStack.push({ inverse: ok.inverse, ops, label: `the proposal ${proposal}`, token: heldToken }); redoStack.length = 0; }
    return { ...stacks(true, '', 'accepted'), note: `Accepted ${proposal}.` };
  },
  importFiles: async ([client, kind, paths, names], _store, storage) => {
    // The picked files' text, sent with `import`: the server runs the
    // importer and holds its proposal, with its review, for the inbox.
    const list = paths.split('\n').filter(Boolean), called = names.split('\n');
    if (!list.length) return { ok: false, error: 'Choose a file to import.', proposal: '', note: '' };
    let texts: { name: string; text: string }[];
    try {
      texts = await Promise.all(list.map(async (path, i) => ({ name: called[i] || path.split('/').pop()!, text: new TextDecoder().decode(await storage.fs.readFile(path)) })));
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { ok: false, error: `The file could not be read (${(e as { code?: string }).code ?? 'failed'}).`, proposal: '', note: '' };
    }
    const body = texts.length === 1 ? { source: texts[0].text, name: texts[0].name } : { sources: texts };
    const { error, reply } = await call('/__tamber', { method: 'import', client, kind, ...body }, 60000);
    if (error) return { ok: false, error, proposal: '', note: '' };
    const ok = reply.ok as { proposal: string | null; message?: string };
    if (!ok.proposal) return { ok: true, error: '', proposal: '', note: ok.message ?? 'Nothing to propose.' };
    return { ok: true, error: '', proposal: ok.proposal, note: `Held for review as ${ok.proposal}.` };
  },
  acceptImport: async ([client, proposal, picked]) => {
    // The mapped tokens unless unticked, the role overrides only if ticked
    // (LLP 1121 D1): all of it at once is the proposal accepted whole;
    // anything less is recomputed as those items, to accept again.
    const r = heldReview.get(proposal), ops = heldOps.get(proposal) ?? [];
    const recomputed = !!held.find(h => h.proposal === proposal)?.replaces;
    if (!r) return { ...stacks(false, `No held import \`${proposal}\`.`, ''), note: '' };
    // `picked`: the view's flipped items, `<proposal>:<i[,j]>` joined by `;`.
    const flipped = new Set(picked.split(';').filter(x => x.startsWith(`${proposal}:`)).flatMap(x => x.slice(proposal.length + 1).split(',').map(Number)));
    const tokens = Number(r.tokens ?? 0);
    const items = ops.map((_, i) => i).filter(i => (i < tokens || recomputed ? !flipped.has(i) : flipped.has(i)));
    if (!items.length) return { ...stacks(false, 'Nothing is chosen to import.', ''), note: '' };
    return sources.acceptHeld([client, proposal, items.length === ops.length ? '' : items.join(',')], undefined as never, undefined as never, undefined as never);
  },
  editImport: async ([client, proposal, index, value]) => {
    // A mapped token's value changed before accepting: the import held again
    // with it, under the same review, and the old one dropped.
    const r = heldReview.get(proposal), ops = (heldOps.get(proposal) ?? []).map(o => ({ ...o }));
    if (!r || !ops[index]) return { ...stacks(false, `No held import \`${proposal}\`.`, ''), note: '' };
    if (!value.trim()) return { ...stacks(false, 'Give it a value.', ''), note: '' };
    ops[index].value = value.trim();
    const held = await call('/__tamber', { method: 'propose', hold: true, client, ops, review: r }, WRITE_WAIT);
    if (held.error) return { ...stacks(false, held.error, ''), note: '' };
    await call('/__tamber', { method: 'drop', client, proposal });
    return { ...stacks(true, '', 'edited'), note: `${ops[index].name} is ${value.trim()} in the import, held again as ${held.reply.ok.proposal}: review it and accept.` };
  },
  dropHeld: async ([client, proposal]) => {
    const { error } = await call('/__tamber', { method: 'drop', client, proposal });
    return { ...stacks(!error, error, error ? '' : 'dropped'), note: error ? '' : `Dropped ${proposal}.` };
  },
  revertCommit: async ([client, commit]) => {
    // Held, never written: reviewed and accepted like any held proposal.
    const { error, reply } = await call('/__tamber', { method: 'revert', client, commit }, 30000);
    const refused = ((reply.error as { refused?: any[] } | undefined)?.refused ?? []).map(r => `${r.file}: ${r.edit ? `${r.edit.name} ${r.edit.before ?? '(absent)'} → ${r.edit.after ?? '(absent)'}` : 'the file'}, ${r.why}${r.now != null ? ` (now ${r.now})` : ''}`);
    if (error) return { ok: false, error, commit, proposal: '', refused };
    return { ok: true, error: '', commit, proposal: String((reply.ok as { proposal: string }).proposal), refused: [] };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);

/** WCAG 2's contrast ratio of a text colour `fg` over a background `bg`,
 * each `#rrggbb[aa]`: the text composited over the background first. A
 * background that is not opaque has no one ratio until what is behind it
 * is known: said, as is anything that is not such a colour. */
export function contrast(fg: string, bg: string): number | string {
  const rgba = (hex: string) => {
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex.trim());
    return m ? [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16) / 255).concat(m[2] ? parseInt(m[2], 16) / 255 : 1) : null;
  };
  const [f, b] = [rgba(fg), rgba(bg)];
  if (!f || !b) return 'not a colour';
  if (b[3] < 1) return 'translucent background';
  const over = f.slice(0, 3).map((c, i) => c * f[3] + b[i] * (1 - f[3]));
  const lum = (c: number[]) => {
    const [r, g, bl] = c.map(v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(over), lum(b.slice(0, 3))];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
