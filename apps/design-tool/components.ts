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
  name: string; short: string | null; kind: string; default: string; values: string[] | null;
  range: [number, number] | null; step: number | null; control: string | null; options: string[] | null;
  label: string | null; help: string | null; group: string | null; priority: number | null;
  set: { value: string } | null; pinned: string[] | null;
};

/** One param's row: its control, by the dials panel's rules, on the value
 * the app has (its line, else the default); a number control starts at the
 * folded value when the written one is a token. `control=token` (the
 * default) is a field that takes a token or a literal. */
export function paramRow(p: Param, tokens: ReadonlySet<string>): Row | null {
  const written = p.set?.value ?? p.default ?? '';
  const unit = p.kind === 'length' ? 'px' : '';
  const meta: Record<string, string> = {};
  if (p.control && p.control !== 'token') meta.control = p.control;
  if (p.options?.length) meta.options = p.options.join('|');
  if (p.label) meta.label = p.label;
  if (p.help) meta.help = p.help;
  if (p.step != null) meta.step = String(p.step);
  if (p.priority != null) meta.priority = String(p.priority);
  const range: [string, string] | null = p.range ? [`${p.range[0]}${unit}`, `${p.range[1]}${unit}`] : null;
  const c = controlOf({ name: p.name, kind: p.kind, value: written, range, meta }, tokens, true);
  if (!c) return null;
  const folded = p.values?.[0] ?? written;
  return {
    c: {
      ...c,
      control: p.control === 'token' ? 'token' : c.control,
      number: numberOf(written) ?? numberOf(folded) ?? 0,
      // The swatch is the value as the app's first tuple folds it, a token named or not.
      swatch: p.kind === 'color' ? folded : '',
    },
    short: p.short ?? p.name,
    set: p.set != null,
    pinned: p.pinned ?? [],
    default: p.default ?? '',
  };
}

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
