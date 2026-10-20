// Where a token's value comes from (LLP 1121 stage 11, D11; the wire's
// shape is theme2's 16:17Z ruling for tamber/1.14): Exact's base scale
// (`space.*`, `radius.*`: describe's `base` rows, one value a platform
// column, each with its source), a package's design (its rows carry
// `package` and `file`; read-only, the app overrides it in its own file), or
// the app's, whose line may override a base name.

/** What an app's line may start from for a column's value: a length as written, and radius.full's pill
 * (`full`, no length an app may write) as the large one a pill takes. */
export const writable = (value: string) => value === 'full' ? '9999px' : value;

/** The scale table's columns, in order, as the tool names them. */
const COLUMNS: [string, string][] = [['ios', 'iOS'], ['mac', 'Mac'], ['neutral', 'neutral'], ['material', 'Android'], ['terminal', 'terminal']];

export type BaseColumn = { name: string; label: string; value: string; from: string; stretch: boolean; ships: boolean };
export type Source = { source: string; package: string; of: string; over: string; back: string; columns: BaseColumn[]; startFrom: string };

/** A column's source in words: the metric it references, the measurement, the guideline, a stretch with why, or radius.full's pill. */
function fromOf(s: any): string {
  switch (s?.kind) {
    case 'metric': return String(s.ref ?? '');
    case 'measured': return `measured: ${s.id ?? ''}`;
    case 'guideline': return String(s.cite ?? '');
    case 'stretch': return `${s.ref ?? ''}${s.why ? `, ${s.why}` : ''}`;
    case 'terminal': return 'cells';
    case 'full': return 'the pill';
    default: return '';
  }
}

/** A base row's columns: those the app's targets read (`shipped`, in their order, once each) first, then the rest. */
export function baseColumns(row: any, shipped: string[]): BaseColumn[] {
  const all = COLUMNS.filter(([name]) => row?.columns?.[name]).map(([name, label]) => {
    const c = row.columns[name];
    return { name, label, value: String(c.value ?? ''), from: fromOf(c.source), stretch: c.source?.kind === 'stretch', ships: shipped.includes(name) };
  });
  return [...shipped.map(n => all.find(c => c.name === n)).filter((c): c is BaseColumn => !!c), ...all.filter(c => !c.ships)];
}

/** A row's source: a base row (`base`, no override), a package's token, or
 * the app's; for the app's override of a base name, what it replaces (`over`)
 * and the way back (`back`). */
export function sourceOf(t: any, base: any, shipped: string[], appFile = ''): Source {
  const columns = base ? baseColumns(base, shipped) : [];
  // (The column an app value starts from, and the over-line names: the first this app ships, else the first.)
  const shown = columns[0];
  const startFrom = shown ? `${shown.label}'s ${shown.value === 'full' ? 'pill (9999px)' : shown.value}` : '';
  if (base && !base.override) return { source: 'base', package: '', of: 'base', over: '', back: '', columns, startFrom };
  if (base) return { source: 'app', package: '', of: 'base', over: `Set for this app, over Exact's base${shown ? ` (${shown.label} ${shown.value})` : ''}.`, back: 'Back to the base', columns, startFrom };
  // (A package's token the app sets: its row names the app's design as its file, A2; the tool reads that as the
  // app's line over the package's value.)
  if (t?.package && appFile && t.file === appFile) return { source: 'app', package: String(t.package), of: 'package', over: `Set for this app, over the ${t.package} package's value.`, back: 'Back to the package', columns: [], startFrom: '' };
  if (t?.package) return { source: 'package', package: String(t.package), of: 'package', over: '', back: '', columns: [], startFrom: '' };
  return { source: 'app', package: '', of: '', over: '', back: '', columns: [], startFrom: '' };
}

/** The grid group a row is listed in: the base's (an app override of a base name too), a package's, or its name's prefix. */
export const groupOf = (s: Source, prefix: string) => s.of === 'base' ? 'Base scale (Exact)' : s.of === 'package' ? `${s.package} package` : prefix;

/** The scale columns the app's targets read, in their order, once each: describe's `targets`
 * (`[{target, column}]`, theme2's 16:32Z ruling: the server names the column, the tool maps nothing). */
export const shippedOf = (said: unknown): string[] => [...new Set<string>((Array.isArray(said) ? said : []).map((t: any) => String(t?.column ?? '')).filter(Boolean))];
