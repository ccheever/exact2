// The inspector's view of a selection (LLP 1122 D5, D6), apart from the
// client's state: what a selection was made under, when it stops being
// current, and each row as the panel shows it.
import { holdView, type Holding } from './holds';

type Column = { id: string; label: string; dark: boolean; values: string[] };
type Axis = { name: string; values: string[]; under: string | null };
type Build = { status: any; plan?: string | null; design?: string | null; pinRevision: number } | undefined;
/** A selection with what it was made under: its build's facts (`at`), the
 * app design's axes, the scheme it is shown in ('' when not known), and why
 * it is no longer current, once it is not. */
export type Selected = { e: any; at: string; axes: string; scheme: string; stale: string; files?: string; asking?: boolean; visit?: number };
// Each selection's visit, counted as the selections come: it moves when one names another view than the one before
// it, so a round trip (A, then B, then A, however close together) is a new visit to A, and the Inspector's rows
// nothing sets, opened for one visit, fold again (a refresh of the same view is the same visit).
let lastSelected = '', visits = 0;
export const visitOf = (e: any) => { const k = [e.build, e.node, e.site, JSON.stringify(e.instance ?? [])].join('|'); if (k !== lastSelected) { lastSelected = k; visits++; } return visits; };

// An `:active` part's rows as their press paint carries them (LLP 1121 D8), by the part's spelling.
const ACTIVE: Record<string, string> = { press_background_color: 'background-color', press_color: 'color', press_opacity_to: 'opacity', press_scale: '-exact-press-scale' };
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

const noHold = { can: false, state: '', held: false, status: '' };
/** A state rung as the inspector offers it: `set-state`'s target, `style|state|row|file`, and its words. */
// The template or style rung's (T5): `set-row`'s address, as the join gave it, its words and the value as
// written; a template row's promotion to a param or token (`component` the one whose view it is in; a token
// only for an unconditional value, D6), and a param row's detach to its template.
const at = (x: any) => (x.rung === 'style' ? { style: x.style, row: x.row, file: x.file } : { file: x.file, at: x.at, sha256: x.sha256, row: x.row, ...(x.state ? { state: x.state } : {}) });
const rowOf = (x: any, component: string) => x?.rung === 'template' || x?.rung === 'style'
  ? { edit: JSON.stringify(at(x)), editLabel: x.rung === 'style' ? `Edit style ${x.style}` : `Edit this template · ${x.instances === 1 ? '1 instance' : `${x.instances} instances`}`, editText: String(x.text ?? ''),
      // (The editor's heading names the attribute it writes: a shorthand's whole, for a longhand of it.)
      editWords: `${x.rung === 'style' ? `Style ${x.style}` : `This template · ${x.instances === 1 ? '1 instance' : `${x.instances} instances`}`}: ${x.state ? `:${x.state} ` : ''}${x.row}`,
      promote: x.rung === 'template' && x.text ? JSON.stringify({ addr: at(x), component: x.component ?? component, text: x.text }) : '', tokenize: x.rung === 'template' && x.text && !x.state, detach: '' }
  : { edit: '', editLabel: '', editText: '', editWords: '', promote: '', tokenize: false, detach: x?.rung === 'param' && x.template ? JSON.stringify({ addr: at(x.template), param: x.param }) : '' };
const stateOf = (x: any) => x?.style ? { state: [x.style, x.state, x.row, x.file].join('|'), stateLabel: `Edit ${x.style} :${x.state} ${x.row}`, stateText: String(x.text ?? '') } : { state: '', stateLabel: '', stateText: '' };
const noSelection = { key: '', visit: '', files: '', has: false, stale: '', build: '', node: 0, readOnly: '', source: '', chain: [], modes: '', place: '', rows: [], trimmed: false, inspecting: false, hold: noHold, heldState: '', heldRows: [], heldNote: '', libraryIdentity: '', libraryPart: '', libraryParts: [] };
/** The part outline's fields: none outside a library component. */
function libraryOf(library: any, partsOf: (identity: string) => { name: string; slot: boolean }[] | null) {
  if (!library || typeof library.identity !== 'string') return { libraryIdentity: '', libraryPart: '', libraryParts: [] };
  const part = typeof library.part === 'string' ? library.part : '';
  return { libraryIdentity: library.identity, libraryPart: part, libraryParts: (partsOf(library.identity) ?? []).map(p => ({ name: p.name, here: p.name === part, slot: p.slot })) };
}
/** The selection as the inspector shows it: each row's source, and its edit
 * on the branch that won for the copy (D6: its modes' column and scheme), its
 * token link or why it is read-only; nothing to do while stale. */
export function selectionView(s: Selected | null, stale: string, axes: Axis[], columns: Column[], inspecting: boolean, holding: Holding = { can: false, hold: null, held: null }, note = '', partsOf: (identity: string) => { name: string; slot: boolean }[] | null = () => null) {
  if (!s) return { ...noSelection, inspecting };
  const e = s.e, dark = s.scheme === 'dark';
  // On a light copy an under-dark axis is its default; any other the copy
  // names. With no axes read yet, or one it does not name, it is not placed.
  const values = axes.map(a => (dark || a.under !== 'dark' ? e.tuple?.[a.name] : a.values[0]));
  const col = s.scheme && axes.length && values.every(v => v != null) ? columns.find(c => c.dark === dark && c.values.every((v, i) => v === values[i])) : undefined;
  const at = (x: any, file = String(x?.file)) => x ? `${file}:${x.line} · ${x.component}` : '';
  const usable = !stale && !!col;
  const shown = holdView(e, holding, note, !!stale);
  // The selection's report of the state held here (one of an earlier hold's is not it).
  const reported = e.held?.state === shown.state;
  // A held row's source is the place its part is written (an object), a base row's a word.
  const partAt = (x: any) => `${String(x.file ?? '').split('/').pop()}:${x.line}`;
  const row = (r: any) => ({
    // Nothing sets it (its initial value): the Inspector folds it behind a count, unless it is the row being edited.
    initial: r.source === 'initial',
    name: r.state ? `:${r.state} ${ACTIVE[String(r.name)] ?? rowName(String(r.name))}` : rowName(String(r.name)), value: typeof r.value === 'string' ? r.value : JSON.stringify(r.value),
    source: r.source && typeof r.source === 'object' ? `${r.state ? `:${r.state} part at ` : ''}${partAt(r.source)}` : `${SOURCES[r.source] ?? String(r.source)}${r.source === 'inherited' && r.fromAt?.line != null ? ` from ${r.fromAt.component ?? 'a node'} (${String(r.fromAt.file).split('/').pop()}:${r.fromAt.line})` : r.source === 'inherited' && r.from != null ? ` from #${r.from}` : ''}${r.origin ? ` (${r.origin})` : ''}${r.token?.implied ? ' · a mode boundary' : ''}`,
    rung: usable ? String(r.editable?.rung ?? '') : '', token: usable ? String(r.editable?.token ?? r.editable?.link ?? '') : '',
    param: usable ? String(r.editable?.param ?? '') : '', short: usable ? String(r.editable?.short ?? r.editable?.param ?? '') : '',
    // The state rung, or beneath a token rung (T4): `set-state`'s target.
    ...stateOf(usable && r.editable?.rung === 'state' ? r.editable : null),
    ...rowOf(usable ? r.editable : null, String(e.source?.component ?? '')),
    reason: String(r.editable?.reason ?? ''), column: col?.id ?? '',
  });
  return {
    // (With the build's facts it was made under: a pin, a reload or a new plan since makes another copy.)
    key: [e.build, e.node, e.site, JSON.stringify(e.instance ?? []), s.at].join('|'), visit: `${s.visit ?? 0}`, files: s.files ?? '',
    has: true, stale: stale || (col ? '' : !s.scheme ? 'the scheme it is shown in is not known' : !axes.length ? "the app's design is not read yet" : "its modes are not a column of the app's design"), build: String(e.build), node: Number(e.node ?? 0), readOnly: String(e.readOnly ?? ''),
    source: at(e.source, String(e.source?.file).split('/').pop()), chain: (e.source?.chain ?? []).map((x: any, i: number) => ({ text: at(x), up: i + 1 })), modes: col?.label ?? '', place: col ? placeOf(axes, col) : '', trimmed: !!e.trimmed, inspecting,
    // A node of an exported library component's own view (stage 12 D4, LLP 1139 D5): the component by its declaring
    // identity, the part this node is or none, and its parts as the server lists them, this one and the slot's marked.
    ...libraryOf(e.source?.library, partsOf),
    hold: shown,
    // The rows the state held here shows (stage 8 E), or why they are not
    // known: a host that reports none, or says it can't name their tokens,
    // leaves the held look read-only (D8).
    heldState: shown.held && reported ? String(e.held.state) : '',
    heldRows: shown.held && reported ? (e.held.rows ?? []).map(row) : [],
    // (Its page asked to report them again, the answer not in yet: `asking`.)
    heldNote: !shown.held ? '' : !reported && s.asking ? 'Its page is asked for the held look\'s rows.'
      : !reported || !e.held.rows?.length || e.held.provenance === 'incomplete' ? 'Provenance incomplete: this host reports no rows (or not their tokens) for the held look, so it is read-only here.' : '',
    // A folded `:disabled` or `:checked` row's parts, each beneath its row (queue item (2)): at its place, the value
    // it writes as written, its own edit; two of one state (a class's and the node's) told apart by their places (file,
    // line and column: two on one line).
    rows: (e.rows ?? []).flatMap((r: any) => [row(r), ...(r.states ?? []).map((p: any, _: number, all: any[]) => {
      const x = row({ name: r.name, value: String(p.editable?.text ?? ''), state: p.state, source: p.source, editable: p.editable });
      return all.filter((q: any) => q.state === p.state).length > 1 ? { ...x, name: `${x.name} · ${String(p.source?.file ?? '')}:${p.source?.line}:${p.source?.col}` } : x;
    })]),
  };
}
