// The inspector's view of a selection (LLP 1122 D5, D6), apart from the
// client's state: what a selection was made under, when it stops being
// current, and each row as the panel shows it.

type Column = { id: string; label: string; dark: boolean; values: string[] };
type Axis = { name: string; values: string[]; under: string | null };
type Build = { status: any; plan?: string | null; design?: string | null; pinRevision: number } | undefined;
/** A selection with what it was made under: its build's facts (`at`), the
 * app design's axes, the scheme it is shown in ('' when not known), and why
 * it is no longer current, once it is not. */
export type Selected = { e: any; at: string; axes: string; scheme: string; stale: string };

const SOURCES: Record<string, string> = { authored: 'set here', dynamic: 'a dynamic expression', inherited: 'inherited', initial: 'initial' };
// The wire's row names that authors spell otherwise (contract/lower's
// style_names.rs); any other is its name with dashes.
const CSS: Record<string, string> = {
  text_color: 'color', position_type: 'position', svg_mask: 'mask', placeholder_color: '::placeholder color',
  text_stroke_width: '-webkit-text-stroke-width', text_stroke_color: '-webkit-text-stroke-color', control_corner_style: '-exact-corner-style',
  ...Object.fromEntries(['tint_color', 'exit_animation', 'layout_transition', 'press_scale', 'drag_timeline', 'symbol_rendering', 'symbol_palette', 'symbol_value',
    'symbol_effect', 'press_haptic', 'content_transition', 'scroll_edge_effect', 'hover_effect', 'smart_invert', 'animation_trigger', 'control_size', 'fit', 'mode']
    .map(n => [n, `-exact-${n.replace(/_/g, '-')}`])),
};
/** A row's author-facing name, as its source spells it: a border side's
 * longhand puts the side first (`border-top-left-radius`). */
export const rowName = (name: string) => CSS[name] ?? name.replace(/^border_(width|style|color|radius)_(.+)$/, 'border_$2_$1').replace(/_/g, '-');

/** The build's facts a selection was made under: a reload (its status
 * gone, then told again), a pin, a mode or scheme, a new plan or design
 * moves them. */
export const buildKey = (b: Build) => b ? JSON.stringify([b.status?.revision ?? null, b.status?.scheme ?? null, b.status?.mode ?? null, b.plan ?? null, b.design ?? null, b.pinRevision]) : '';

/** Why the selection is not current, or ''. */
export const staleness = (s: Selected, build: Build, axes: Axis[]) =>
  s.stale || (!build ? 'its build is gone' : buildKey(build) !== s.at ? 'its build has moved on (a reload, a pin, a mode, a new status)' : JSON.stringify(axes) !== s.axes ? "the design's axes changed" : '');

/** The scheme a selected copy is shown in: its own `color-scheme` row, else
 * its build's; not known when a trimmed report dropped that row. */
export const schemeOf = (e: any, build: Build) => {
  const own = (e.rows ?? []).find((r: any) => r.name === 'color_scheme');
  const scheme = own?.value === 'dark' || own?.value === 'light' ? own.value : !own && e.trimmed ? '' : build?.status?.scheme;
  return scheme === 'dark' || scheme === 'light' ? scheme : '';
};

/** A column's place, whole: its scheme and every axis's value by name (a
 * label leaves the defaults out), so a cell opened from a selection is the
 * same tuple in the design read it is opened on. */
export const placeOf = (axes: Axis[], col: Column) => `${col.dark ? 'dark' : 'light'}|${axes.map((a, i) => `${a.name}=${col.values[i]}`).sort().join(' ')}`;

const noSelection = { has: false, stale: '', build: '', node: 0, readOnly: '', source: '', chain: [], modes: '', place: '', rows: [], trimmed: false, inspecting: false };
/** The selection as the inspector shows it: each row's source, and its edit
 * on the branch that won for the copy (D6: its modes' column and scheme), its
 * token link or why it is read-only; nothing to do while stale. */
export function selectionView(s: Selected | null, stale: string, axes: Axis[], columns: Column[], inspecting: boolean) {
  if (!s) return { ...noSelection, inspecting };
  const e = s.e, dark = s.scheme === 'dark';
  // On a light copy an under-dark axis is its default; any other the copy
  // names. With no axes read yet, or one it does not name, it is not placed.
  const values = axes.map(a => (dark || a.under !== 'dark' ? e.tuple?.[a.name] : a.values[0]));
  const col = s.scheme && axes.length && values.every(v => v != null) ? columns.find(c => c.dark === dark && c.values.every((v, i) => v === values[i])) : undefined;
  const at = (x: any, file = String(x?.file)) => x ? `${file}:${x.line} · ${x.component}` : '';
  const usable = !stale && !!col;
  return {
    has: true, stale: stale || (col ? '' : !s.scheme ? 'the scheme it is shown in is not known' : !axes.length ? "the app's design is not read yet" : "its modes are not a column of the app's design"), build: String(e.build), node: Number(e.node ?? 0), readOnly: String(e.readOnly ?? ''),
    source: at(e.source, String(e.source?.file).split('/').pop()), chain: (e.source?.chain ?? []).map((x: any) => at(x)), modes: col?.label ?? '', place: col ? placeOf(axes, col) : '', trimmed: !!e.trimmed, inspecting,
    rows: (e.rows ?? []).map((r: any) => ({
      name: rowName(String(r.name)), value: typeof r.value === 'string' ? r.value : JSON.stringify(r.value),
      source: `${SOURCES[r.source] ?? String(r.source)}${r.source === 'inherited' && r.from != null ? ` from #${r.from}` : ''}${r.origin ? ` (${r.origin})` : ''}${r.token?.implied ? ' · a mode boundary' : ''}`,
      rung: usable ? String(r.editable?.rung ?? '') : '', token: usable ? String(r.editable?.token ?? r.editable?.link ?? '') : '',
      reason: String(r.editable?.reason ?? ''), column: col?.id ?? '',
    })),
  };
}
