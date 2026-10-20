// The component panel (LLP 1122 D7, T3): each component's params from
// `describe`'s `components` (LLP 1121 D15, tamber/1.10), grouped and ordered
// as the dials panel is, each with its declared control. A param takes a
// token as well as a literal, and its value is the app's line in the
// design's `params` block when it has one, else the default as written.
// Pure functions of `describe`; app.ts's design read uses them.
import type { Result } from './app.contract.d.ts';
import { controlOf, numberOf } from './dials.ts';

type Design = Result<'loadDesign'>;
type Panel = Design['components'][number];
type Row = Panel['groups'][number]['params'][number];

/** A param as `describe` gives it (docs/tamber-protocol.md, `components`). */
type Param = {
  name: string; short: string | null; kind: string; default: string; values: unknown[] | null;
  range: [number, number] | null; step: number | null; control: string | null; options: string[] | null;
  label: string | null; help: string | null; group: string | null; priority: number | null;
  set: { value: string } | null; pinned: string[] | null;
};

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
  };
}

/** An icon param's value in words: its sources. */
const iconText = (v: unknown) => {
  const i = (v ?? {}) as Record<string, string | null>;
  return ['sf', 'material', 'svg'].filter(k => i[k]).map(k => `${k} ${i[k]}`).join(' · ');
};

/** The panel: every component with params (a native one declared in the
 * design too, `app.tamber#Name`), its groups in the order they first
 * appear, each by priority (1 first), priority 3 behind "more". */
export function componentsOf(list: { identity: string; name: string; params: Param[] }[] | undefined, design: string, tokens: ReadonlySet<string>): Design['components'] {
  return (list ?? []).filter(c => c.params?.length).map(c => {
    const groups = new Map<string, Row[]>();
    for (const p of c.params) {
      const row = paramRow(p, tokens);
      if (!row) continue;
      const g = p.group ?? '';
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g)!.push(row);
    }
    return {
      identity: c.identity,
      name: c.name,
      native: c.identity.startsWith(`${design}#`),
      groups: [...groups].map(([name, all]) => {
        const sorted = all.map((r, i) => [r, i] as const).sort((a, b) => a[0].c.priority - b[0].c.priority || a[1] - b[1]).map(([r]) => r);
        return { name, params: sorted.filter(r => r.c.priority < 3), more: sorted.filter(r => r.c.priority === 3) };
      }),
    };
  });
}
