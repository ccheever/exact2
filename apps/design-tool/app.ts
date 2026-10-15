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
export const grants = `net.fetch ${API}`;

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

/** One `tamber/1` request: its reply, or the words to show. A token the
 * server no longer knows (it restarted) is asked for again, once. */
async function call(path: string, body: unknown, timeout = 15000): Promise<{ error: string; reply: Reply }> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const got = await tokenOf();
    if (!got.token) return { error: got.error, reply: {} };
    try {
      const res = await fetch(`${API}${path}`, { method: 'POST', exactTimeout: timeout, redirect: 'error', body: JSON.stringify(body),
        headers: { 'content-type': 'application/json', 'x-exact-tamber-token': got.token } });
      if (res.status === 404 && attempt === 0) { token = null; continue; }
      const reply = (await res.json().catch(() => ({}))) as Reply;
      if (res.ok) return { error: '', reply };
      return { error: refusal(reply.error, res.status), reply };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { error: NO_SERVER, reply: {} };
    }
  }
  return { error: NO_SERVER, reply: {} };
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
/** A colour written as hex or `rgb()`, folded as the server folds a dial's
 * colour (`#rrggbbaa`, lower case); anything else as written. */
export function folded(v: string): string {
  const t = v.trim().toLowerCase();
  const hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(t)?.[1];
  if (hex) {
    const full = hex.length <= 4 ? [...hex].map(c => c + c).join('') : hex;
    return `#${full}${full.length === 6 ? 'ff' : ''}`;
  }
  const rgb = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/.exec(t);
  if (rgb) {
    const a = rgb[4] === undefined ? 1 : rgb[4].endsWith('%') ? Number(rgb[4].slice(0, -1)) / 100 : Number(rgb[4]);
    return `#${[rgb[1], rgb[2], rgb[3]].map(n => Math.min(255, Number(n))).concat(Math.round(Math.min(1, a) * 255)).map(n => n.toString(16).padStart(2, '0')).join('')}`;
  }
  return t;
}

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
  const literal = (o: string) => (d.kind === 'color' ? !tokens.has(o) && !o.includes('.') : numberOf(o) !== null);
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
  const declaredStep = m.step !== undefined && Number.isFinite(Number(m.step)) ? Number(m.step) : null;
  const step = declaredStep !== null && declaredStep > 0 ? declaredStep : d.kind === 'number' ? finest : 1;
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

/** The tokens and dials a value names, as the checker reads a name: a
 * token by its name, a role by its row's spelling or its bare name in any
 * case (`label` for `-exact-label`), a dial by its name. Words inside a hex
 * colour or a number are not names. */
export function inputsOf(value: string, names: string[], dials: string[]): { tokens: string[]; dials: string[] } {
  const words = new Set((value.match(/#?[A-Za-z_-][\w.-]*/g) ?? []).filter(w => !w.startsWith('#')));
  const lower = new Set([...words].map(w => w.toLowerCase()));
  const named = (n: string) => words.has(n) || lower.has(n.toLowerCase()) || (n.toLowerCase().startsWith('-exact-') && lower.has(n.toLowerCase().slice(7)));
  return { tokens: names.filter(named), dials: dials.filter(d => words.has(d)) };
}

// Each design read, by its number (given as the read starts): its axes,
// its columns, its dials' groups and each token's stored values per tuple.
// A read that a newer one has overtaken is never shown (the runner drops
// its answer), so it keeps nothing; the shown one is the newest kept, and
// what follows it (a cell's branch, contrast) names it.
type Stored = { css: string; srgb: string; residual: boolean; note: string };
type Snapshot = { file: string; axes: Axis[]; columns: Column[]; stored: Map<string, Stored[][]>; dials: Map<string, { group: string; more: boolean }> };
const snapshots = new Map<number, Snapshot>();
let issued = 0;
// Whether running apps take a value edit with no reload: a server says so
// in its hello once it has the live table (LLP 1121 stage 4). Until then
// every edit reaches them through the dev loop's reload.
let liveTable = false;

/** One consistent read of a design: `describe`, each token's `resolve`, and
 * the files' digests again after them; if a write landed between, the
 * replies may be of two revisions, so it reads again. */
async function readDesign(file: string): Promise<{ error: string; ir?: any; resolved?: { tuples: any[] }[] }> {
  const which = file ? { file } : {};
  for (let attempt = 0; attempt < 3; attempt++) {
    const described = await call('/__tamber', { method: 'describe', ...which });
    if (described.error) return { error: described.error };
    const ir = described.reply.ok?.ir;
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
    const placed = resolved.every(r => (r.reply.ok?.tuples ?? []).every((t: { modes?: Record<string, string> }, i: number) => {
      const col = tuples.find(c => c.tuple === i);
      return !col || ir.axes.every((a: Axis, k: number) => t.modes?.[a.name] === col.values[k]);
    }));
    if (same && placed) return { error: '', ir, resolved: resolved.map(r => r.reply.ok) };
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
type Entry = { inverse: string; ops: Record<string, unknown>[]; label: string };
const undoStack: Entry[] = [], redoStack: Entry[] = [];
let session = 0;
const stacks = (ok: boolean, error: string, last: string, key = '') => ({ ok, error, undo: undoStack.length, redo: redoStack.length, last, key, session });
/** Who moved a file a refused write or undo found moved: its client, or
 * "changed on disk". */
const byWhom = (reply: Reply) => {
  const by = (reply.error as { by?: Record<string, string> } | undefined)?.by ?? {};
  const said = Object.entries(by).map(([file, who]) => `${file} was ${who === 'changed on disk' ? 'changed on disk' : `changed by ${who}`}`);
  return said.length ? said.join('; ') : '';
};
const conflict = (reply: Reply) => (reply.error as { conflict?: boolean } | undefined)?.conflict === true;

const sources: Sources = {
  loadHello: async () => {
    const { error, reply } = await call('/__tamber', { method: 'hello' });
    if (error) return { ok: false, error, protocol: '', format: 0, app: '', files: [] };
    const ok = reply.ok ?? {};
    liveTable = ok.live === true;
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
      // Each token's stored value per tuple and scheme, a residual as its
      // fallback (no running app reports it until T2).
      const stored = new Map<string, Stored[][]>();
      const tokens = ir.tokens.map((t: { name: string; kind: string; dials: string[]; role: string | null }, i: number) => {
        const tuples = (resolved[i].tuples ?? []) as any[];
        const sides = tuples.map(r => [false, true].map(dark => {
          const side = dark ? 'dark' : 'light';
          if (r.residual) return { css: r.value, srgb: r.fallback?.[side] ?? '', residual: true, note: r.value };
          return { css: r[side], srgb: r.srgb?.[side] ?? r[side], residual: false, note: '' };
        }));
        stored.set(t.name, sides);
        const cells = columns.map(c => {
          const v = sides[c.tuple]?.[c.dark ? 1 : 0] ?? { css: '', srgb: '', residual: false, note: '' };
          return { column: c.id, label: `${t.name}, ${c.label}${v.residual ? ', fallback' : ''}`, css: v.css, swatch: v.residual ? v.srgb : v.css, residual: v.residual, note: v.note };
        });
        return { name: t.name, kind: t.kind, follows: (t.dials ?? []).join(', '), cells };
      });
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
        snapshots.set(number, { file, axes, columns, stored, dials });
        for (const old of snapshots.keys()) if (old < number - 4) snapshots.delete(old);
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
    const none = { ok: true, error: '', token: '', column: '', label: '', when: '', value: '', expression: false, inputs: [], dials: [], exact: true, own: '' };
    const snap = snapshots.get(answer);
    const col = snap?.columns.find(c => c.id === column);
    if (!token || !col || !snap || snap.file !== file) return none;
    const { error, reply } = await call('/__tamber', { method: 'explain', token, ...(file ? { file } : {}) });
    if (error) return { ...none, ok: false, error };
    const x = reply.ok as { value: string; whens: { when: When[]; value: string }[]; dials: string[]; names: string[] };
    const b = branchOf(snap.axes, col, x.whens);
    if (b.tie) return { ...none, ok: false, error: `${token} has two branches for ${col.label}, ${b.tie}; the checker refuses that, so fix the file by hand.` };
    const value = b.index >= 0 ? x.whens[b.index].value : x.value;
    // An expression's inputs: the tokens and dials it names; a dial's
    // control is where it is edited.
    const inputs = inputsOf(value, x.names, x.dials);
    const dials = inputs.dials.map(name => ({ name, group: snap.dials.get(name)?.group ?? '', more: snap.dials.get(name)?.more ?? false }));
    return {
      ok: true, error: '', token, column, label: `${token} · ${b.when || 'default'} (${col.label})`, when: b.when, value,
      expression: inputs.tokens.length + inputs.dials.length > 0, inputs: inputs.tokens, dials, exact: b.own === '' || b.when === b.own, own: b.own,
    };
  },
  proposeEdit: async ([file, kind, target, when, value]) => {
    const key = keyOf(file, kind, target, when, value);
    const idle = { ok: true, error: '', active: false, chip: '', diff: '', tokens: [], key, review: '' };
    if (!kind || !target) return idle;
    if (kind !== 'remove-token' && !value) return { ...idle, ok: false, error: 'Give it a value.' };
    const { error, reply } = await call('/__tamber', { method: 'propose', ops: [opOf(file, kind, target, when, value)] });
    if (error) return { ...idle, ok: false, error };
    const ok = reply.ok as { diff: string; class: string; files: Record<string, string>; changed: { token: string }[] };
    // What Apply writes: this edit, against the digests this proposal read,
    // carried on the proposal itself, so what is shown is what is written.
    return { ok: true, error: '', active: true, chip: ok.class === 'live' && liveTable ? 'live' : 'rebuild', diff: ok.diff, tokens: ok.changed.map(c => c.token), key, review: JSON.stringify({ key, files: ok.files }) };
  },
  writeEdit: async ([file, kind, target, when, value, client, review]) => {
    const key = keyOf(file, kind, target, when, value);
    let read: { key?: string; files?: Record<string, string> } = {};
    try { read = JSON.parse(review || '{}'); } catch { /* none */ }
    if (read.key !== key || !read.files) return stacks(false, 'Review the change before applying it.', '');
    const op = opOf(file, kind, target, when, value);
    // Every file's digest as the proposal read it: the server checks the
    // ones the edit writes, whichever file the app's design is.
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: read.files, ops: [op] });
    if (error) return stacks(false, byWhom(reply) ? `Not written: ${byWhom(reply)} since you reviewed it.` : error, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    // A write that changed no byte has no inverse, and leaves redo be.
    if (inverse) { undoStack.push({ inverse, ops: [op], label: label(kind, target, when, value) }); redoStack.length = 0; }
    return stacks(true, '', kind.split('-')[0], key);
  },
  undoLast: async ([client]) => {
    const top = undoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to undo.', '');
    const { error, reply } = await call('/__tamber', { method: 'undo', client, inverse: top.inverse });
    if (error) {
      // Refused for a conflict, it can't apply while the file stays as it
      // is: say who moved it, and set it aside. Any other failure (no
      // answer) keeps it to try again.
      if (!conflict(reply)) return stacks(false, error, '');
      undoStack.pop();
      return stacks(false, `Can't undo ${top.label}: ${byWhom(reply) || 'the file changed'} since. It is set aside.`, '');
    }
    undoStack.pop();
    redoStack.push(top);
    return stacks(true, '', 'undo');
  },
  redoLast: async ([client]) => {
    const top = redoStack.at(-1);
    if (!top) return stacks(false, 'Nothing to redo.', '');
    // The original transaction again, against the files as they are now.
    const hello = await call('/__tamber', { method: 'hello' });
    if (hello.error) return stacks(false, hello.error, '');
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: hello.reply.ok.files ?? {}, ops: top.ops });
    if (error) {
      if (!conflict(reply)) return stacks(false, `Can't redo ${top.label}: ${error}`, '');
      return stacks(false, `Can't redo ${top.label}: ${byWhom(reply) || 'the file changed'} since. Try again.`, '');
    }
    redoStack.pop();
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    if (inverse) undoStack.push({ ...top, inverse });
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
    const { error, reply } = await call('/__tamber/events', epoch ? { after: since, epoch } : { after: 0, wait: false }, 40000);
    answers += 1;
    const state = () => ({ answer: answers, seq: place.seq, epoch: place.epoch, recent, undo: undoStack.length, redo: redoStack.length, session });
    if (error) return { ok: false, error, missed: false, ...state() };
    const r = reply as { events?: unknown[]; seq?: number; epoch?: string; missed?: boolean };
    // Another run of the server (or another app's, on this port), or more
    // missed than it keeps: its sequence numbers start again, or the list
    // has a gap, so the list starts again. Another run has none of this
    // session's inverses: undo and redo start again too.
    if (place.epoch && r.epoch !== place.epoch) { undoStack.length = 0; redoStack.length = 0; session += 1; }
    if ((place.epoch && r.epoch !== place.epoch) || r.missed) recent = [];
    const fresh = (r.events ?? []).map(change).filter((c): c is Change => c !== null).reverse();
    recent = [...fresh, ...recent].slice(0, 50);
    place = { seq: Number(r.seq ?? 0), epoch: String(r.epoch ?? '') };
    return { ok: true, error: '', missed: r.missed === true, ...state() };
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
