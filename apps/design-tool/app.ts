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

type Reply = { ok?: any; error?: { message?: string } };
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
      return { error: reply.error?.message ?? `The design server refused the request (${res.status}).`, reply };
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
const noDesign = (error: string): Design => ({ ok: false, error, file: '', groups: [], columns: [], tokenGroups: [], names: [] });

/** A dial's control from its declared metadata (LLP 1121 D4, D15): its
 * `control`, else its kind's own (a colour's field, a ranged number's
 * slider); `none` is not drawn. `token` names a picker of tokens, which a
 * dial (a literal) has none of, so it is its kind's too. */
export function controlOf(d: { name: string; kind: string; value: string; range: [string, string] | null; meta: Record<string, string> }): Control | null {
  const m = d.meta ?? {};
  const own = d.kind === 'color' ? 'color' : d.range ? 'slider' : 'stepper';
  const declared = m.control;
  if (declared === 'none') return null;
  const control = ['color', 'slider', 'stepper', 'select'].includes(declared) && (declared !== 'color' || d.kind === 'color') && (declared !== 'slider' || d.range) ? declared : own;
  const num = (v: string | undefined, or: number) => (v !== undefined && Number.isFinite(parseFloat(v)) ? parseFloat(v) : or);
  const priority = Math.round(num(m.priority, 2));
  return {
    name: d.name, kind: d.kind, control, label: m.label ?? d.name, help: m.help ?? '', value: d.value,
    number: num(d.value, 0), min: num(d.range?.[0], 0), max: num(d.range?.[1], 1), step: num(m.step, d.kind === 'number' ? 0.01 : 1),
    priority: Math.min(3, Math.max(1, priority)), options: (m.options ?? '').split('|').filter(Boolean),
  };
}

/** The dials panel: groups in the order they first appear, each by
 * priority (1 first), priority 3 behind "more". */
export function panelOf(dials: Parameters<typeof controlOf>[0][]): Design['groups'] {
  const groups = new Map<string, Control[]>();
  for (const d of dials) {
    const c = controlOf(d);
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
/** The grid's columns: every app-mode tuple, light and dark. A light column
 * for an `under scheme=dark` axis's other values repeats the default's (the
 * axis varies the dark side only), so it is left out. */
export function columnsOf(axes: Axis[]): { id: string; label: string; tuple: number; dark: boolean; values: string[] }[] {
  const tuples: string[][] = [[]];
  for (const a of axes) tuples.splice(0, tuples.length, ...tuples.flatMap(t => a.values.map(v => [...t, v])));
  const out: { id: string; label: string; tuple: number; dark: boolean; values: string[] }[] = [];
  for (const dark of [false, true]) {
    tuples.forEach((values, tuple) => {
      if (!dark && axes.some((a, i) => a.under === 'dark' && values[i] !== a.values[0])) return;
      const modes = values.filter((v, i) => !(axes[i].under === 'dark' && !dark) && v !== axes[i].values[0]);
      out.push({ id: `${tuple}-${dark ? 'dark' : 'light'}`, label: [dark ? 'Dark' : 'Light', ...modes].join(' · '), tuple, dark, values });
    });
  }
  return out;
}

// Each token's stored values, per tuple, as the last design read gave them.
type Stored = { css: string; srgb: string; residual: boolean; note: string };
let stored = new Map<string, Stored[][]>();
let columns: ReturnType<typeof columnsOf> = [];
let axes: Axis[] = [];
// Whether running apps take a value edit with no reload: a server says so
// in its hello once it has the live table (LLP 1121 stage 4). Until then
// every edit reaches them through the dev loop's reload.
let liveTable = false;

type When = { axis: string; value: string };
const whenOf = (text: string): When[] => text.split(' ').filter(Boolean).map(w => { const [axis, value] = w.split('='); return { axis, value }; });
const whenText = (w: When[]) => w.map(c => `${c.axis}=${c.value}`).sort().join(' ');

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

// The edit last reviewed (its key) and the digests `propose` read: a write
// carries those, so anything written since is a conflict, never overwritten.
let reviewed: { key: string; files: Record<string, string> } | null = null;
type Entry = { inverse: string; ops: Record<string, unknown>[]; file: string; label: string };
const undoStack: Entry[] = [], redoStack: Entry[] = [];
const stacks = (ok: boolean, error: string, last: string) => ({ ok, error, undo: undoStack.length, redo: redoStack.length, last });
/** Who moved a file a refused write or undo found moved: its client, or
 * "changed on disk". */
const byWhom = (reply: Reply) => {
  const by = (reply.error as { by?: Record<string, string> } | undefined)?.by ?? {};
  const said = Object.entries(by).map(([file, who]) => `${file} was ${who === 'changed on disk' ? 'changed on disk' : `changed by ${who}`}`);
  return said.length ? said.join('; ') : '';
};

const sources: Sources = {
  loadHello: async () => {
    const { error, reply } = await call('/__tamber', { method: 'hello' });
    if (error) return { ok: false, error, protocol: '', format: 0, app: '', files: [] };
    const ok = reply.ok ?? {};
    liveTable = ok.live === true;
    return { ok: true, error: '', protocol: String(ok.protocol ?? ''), format: Number(ok.format ?? 0), app: String(ok.app ?? ''), files: files(ok.files) };
  },
  loadDesign: async ([file]) => {
    const which = file ? { file } : {};
    const described = await call('/__tamber', { method: 'describe', ...which });
    if (described.error) return noDesign(described.error);
    const ir = described.reply.ok.ir;
    axes = ir.axes ?? [];
    columns = columnsOf(axes);
    // Each token's stored value per tuple and scheme, a residual as its
    // fallback (no running app reports it until T2).
    const resolved = await Promise.all((ir.tokens ?? []).map((t: { name: string }) => call('/__tamber', { method: 'resolve', token: t.name, ...which })));
    const failed = resolved.find(r => r.error);
    if (failed) return noDesign(failed.error);
    stored = new Map();
    const tokens = (ir.tokens ?? []).map((t: { name: string; kind: string; dials: string[]; role: string | null }, i: number) => {
      const tuples = resolved[i].reply.ok.tuples as any[];
      const sides = tuples.map(r => [false, true].map(dark => {
        const side = dark ? 'dark' : 'light';
        if (r.residual) return { css: r.value, srgb: r.fallback?.[side] ?? '', residual: true, note: `${r.value} · fallback` };
        return { css: r[side], srgb: r.srgb?.[side] ?? r[side], residual: false, note: '' };
      }));
      stored.set(t.name, sides);
      const cells = columns.map(c => {
        const v = sides[c.tuple]?.[c.dark ? 1 : 0] ?? { css: '', srgb: '', residual: false, note: '' };
        return { column: c.id, css: v.css, swatch: v.residual ? v.srgb : v.css, residual: v.residual, note: v.note };
      });
      return { name: t.name, kind: t.kind, follows: (t.dials ?? []).join(', '), cells };
    });
    // Rows grouped by their dotted prefix (an undotted name's before its
    // first `_`, as `contrast_0`'s); a role override among the roles.
    const groups = new Map<string, typeof tokens>();
    for (const t of tokens) {
      const g = t.name.startsWith('-') || /^[A-Z]/.test(t.name) ? 'roles' : t.name.split(t.name.includes('.') ? '.' : '_')[0];
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g)!.push(t);
    }
    return {
      ok: true, error: '', file: file || '', groups: panelOf(ir.dials ?? []),
      columns: columns.map(({ id, label }) => ({ id, label })),
      tokenGroups: [...groups].map(([name, rows]) => ({ name, tokens: rows })),
      names: tokens.map((t: { name: string }) => t.name),
    };
  },
  loadBranch: async ([file, token, column]) => {
    const none = { ok: true, error: '', token: '', column: '', label: '', when: '', value: '', expression: false, inputs: [], exact: true, own: '' };
    const col = columns.find(c => c.id === column);
    if (!token || !col) return none;
    const { error, reply } = await call('/__tamber', { method: 'explain', token, ...(file ? { file } : {}) });
    if (error) return { ...none, ok: false, error };
    const x = reply.ok as { value: string; whens: { when: When[]; value: string }[]; dials: string[]; names: string[] };
    // The axes this cell's side reads: an `under scheme=dark` axis only on dark.
    const applies = (i: number) => col.dark || axes[i].under !== 'dark';
    const mode = (a: string) => { const i = axes.findIndex(x => x.name === a); return i >= 0 && applies(i) ? col.values[i] : axes[i]?.values[0]; };
    // The `when` that won here: every condition holds, the most axes named
    // (the checker refuses two that tie); else the default.
    const won = x.whens.filter(w => w.when.every(c => mode(c.axis) === c.value)).sort((a, b) => b.when.length - a.when.length)[0];
    const when = won ? whenText(won.when) : '';
    // "Only this mode": a `when` naming every axis this side reads, so it
    // holds for this tuple alone and ties with no other `when`.
    const own = whenText(axes.flatMap((a, i) => (applies(i) ? [{ axis: a.name, value: col.values[i] }] : [])));
    const value = won ? won.value : x.value;
    // An expression's inputs: the tokens and dials it names.
    const inputs = [...new Set([...x.names, ...x.dials])].filter(n => new RegExp(`(^|[^\\w.-])${n.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}($|[^\\w.-])`).test(value));
    return { ok: true, error: '', token, column, label: `${token} · ${when || 'default'} (${col.label})`, when, value, expression: inputs.length > 0, inputs, exact: when === own, own };
  },
  proposeEdit: async ([file, kind, target, when, value]) => {
    const idle = { ok: true, error: '', active: false, chip: '', diff: '', tokens: [] };
    if (!kind || !target) return idle;
    if (kind !== 'remove-token' && !value) return { ...idle, ok: false, error: 'Give it a value.' };
    const { error, reply } = await call('/__tamber', { method: 'propose', ops: [opOf(file, kind, target, when, value)] });
    if (error) { reviewed = null; return { ...idle, ok: false, error }; }
    const ok = reply.ok as { diff: string; class: string; files: Record<string, string>; changed: { token: string }[] };
    reviewed = { key: JSON.stringify([file, kind, target, when, value]), files: ok.files };
    return { ok: true, error: '', active: true, chip: ok.class === 'live' && liveTable ? 'live' : 'rebuild', diff: ok.diff, tokens: ok.changed.map(c => c.token) };
  },
  writeEdit: async ([file, kind, target, when, value, client]) => {
    const key = JSON.stringify([file, kind, target, when, value]);
    if (!reviewed || reviewed.key !== key) return stacks(false, 'Review the change before applying it.', '');
    const op = opOf(file, kind, target, when, value);
    const edited = (op.file as string | undefined) ?? Object.keys(reviewed.files)[0];
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: { [edited]: reviewed.files[edited] }, ops: [op] });
    reviewed = null;
    if (error) return stacks(false, byWhom(reply) ? `Not written: ${byWhom(reply)} since you reviewed it.` : error, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    if (inverse) undoStack.push({ inverse, ops: [op], file: edited, label: label(kind, target, when, value) });
    redoStack.length = 0;
    return stacks(true, '', kind.split('-')[0]);
  },
  undoLast: async ([client]) => {
    const top = undoStack.pop();
    if (!top) return stacks(false, 'Nothing to undo.', '');
    const { error, reply } = await call('/__tamber', { method: 'undo', client, inverse: top.inverse });
    // Refused, it can never apply: the file moved under it. Say who moved it.
    if (error) return stacks(false, byWhom(reply) ? `Can't undo ${top.label}: ${byWhom(reply)} since.` : error, '');
    redoStack.push(top);
    return stacks(true, '', 'undo');
  },
  redoLast: async ([client]) => {
    const top = redoStack.pop();
    if (!top) return stacks(false, 'Nothing to redo.', '');
    // The original transaction again, against the files as they are now.
    const hello = await call('/__tamber', { method: 'hello' });
    if (hello.error) { redoStack.push(top); return stacks(false, hello.error, ''); }
    const { error, reply } = await call('/__tamber', { method: 'transaction', client, files: { [top.file]: hello.reply.ok.files[top.file] }, ops: top.ops });
    if (error) return stacks(false, `Can't redo ${top.label}: ${error}`, '');
    const inverse = (reply.ok as { inverse: string | null }).inverse;
    if (inverse) undoStack.push({ ...top, inverse });
    return stacks(true, '', 'redo');
  },
  loadContrast: async ([a, b]) => {
    if (!a || !b) return { ok: true, error: '', ratios: [] };
    const x = stored.get(a), y = stored.get(b);
    if (!x || !y) return { ok: false, error: `${!x ? a : b} is not in this design.`, ratios: [] };
    return { ok: true, error: '', ratios: columns.map(c => {
      const p = x[c.tuple]?.[c.dark ? 1 : 0], q = y[c.tuple]?.[c.dark ? 1 : 0];
      const r = p && q ? contrast(p.srgb, q.srgb) : null;
      return { column: c.id, label: c.label, ratio: r === null ? 'not a colour' : `${r.toFixed(2)}:1${p!.residual || q!.residual ? ' (fallback)' : ''}` };
    }) };
  },
  waitChanges: async ([since, epoch]) => {
    // Up to 25 s with nothing new, and the server answers once there is.
    // The first asks, at once, from the start of what the server keeps.
    const { error, reply } = await call('/__tamber/events', epoch ? { after: since, epoch } : { after: 0, wait: false }, 40000);
    answers += 1;
    if (error) return { ok: false, error, answer: answers, seq: place.seq, epoch: place.epoch, missed: false, recent };
    const r = reply as { events?: unknown[]; seq?: number; epoch?: string; missed?: boolean };
    // Another run of the server (or another app's, on this port), or more
    // missed than it keeps: its sequence numbers start again, or the list
    // has a gap, so the list starts again.
    if ((place.epoch && r.epoch !== place.epoch) || r.missed) recent = [];
    const fresh = (r.events ?? []).map(change).filter((c): c is Change => c !== null).reverse();
    recent = [...fresh, ...recent].slice(0, 50);
    place = { seq: Number(r.seq ?? 0), epoch: String(r.epoch ?? '') };
    return { ok: true, error: '', answer: answers, seq: place.seq, epoch: place.epoch, missed: r.missed === true, recent };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);

/** WCAG 2's contrast ratio between two `#rrggbb[aa]` colours, alpha
 * ignored; null for anything else. */
export function contrast(a: string, b: string): number | null {
  const lum = (hex: string) => {
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex.trim());
    if (!m) return null;
    const [r, g, bl] = [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16) / 255).map(c => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)];
  if (x === null || y === null) return null;
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
