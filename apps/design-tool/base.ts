// Where a token's value comes from (LLP 1121 stage 11, D11; the wire's
// shape is theme2's 16:17Z ruling for tamber/1.14): Exact's base scale
// (`space.*`, `radius.*`: describe's `base` rows, one value a platform
// column, each with its source), a package's design (its rows carry
// `package` and `file`; read-only, the app overrides it in its own file), or
// the app's, whose line may override a base name.

/** The scale table's columns, in order, as the tool names them. */
const COLUMNS: [string, string][] = [['ios', 'iOS'], ['mac', 'Mac'], ['neutral', 'neutral'], ['material', 'Android'], ['terminal', 'terminal']];
/** The column each target the app ships draws (a target with none draws none). */
const DRAWS: Record<string, string> = { ios: 'ios', apple: 'ios', macos: 'mac', mac: 'mac', linux: 'neutral', web: 'neutral', android: 'material', terminal: 'terminal' };

export type BaseColumn = { name: string; label: string; value: string; from: string; stretch: boolean; ships: boolean };
export type Source = { source: string; package: string; of: string; over: string; back: string; columns: BaseColumn[] };

/** A column's source in words: the metric it references, the measurement, the guideline, or a stretch with why. */
function fromOf(s: any): string {
  switch (s?.kind) {
    case 'metric': return String(s.ref ?? '');
    case 'measured': return `measured: ${s.id ?? ''}`;
    case 'guideline': return String(s.cite ?? '');
    case 'stretch': return `${s.ref ?? ''}${s.why ? `, ${s.why}` : ''}`;
    case 'terminal': return 'cells';
    default: return '';
  }
}

/** A base row's columns: those the app's targets draw first (`ships`), then the rest. */
export function baseColumns(row: any, targets: string[]): BaseColumn[] {
  const ships = new Set(targets.map(t => DRAWS[t]).filter(Boolean));
  const all = COLUMNS.filter(([name]) => row?.columns?.[name]).map(([name, label]) => {
    const c = row.columns[name];
    return { name, label, value: String(c.value ?? ''), from: fromOf(c.source), stretch: c.source?.kind === 'stretch', ships: ships.has(name) };
  });
  return [...all.filter(c => c.ships), ...all.filter(c => !c.ships)];
}

/** A row's source: a base row (`base`, no override), a package's token, or
 * the app's; for the app's override of a base name, what it replaces (`over`)
 * and the way back (`back`). */
export function sourceOf(t: any, base: any, targets: string[]): Source {
  const columns = base ? baseColumns(base, targets) : [];
  if (base && !base.override) return { source: 'base', package: '', of: 'base', over: '', back: '', columns };
  if (base) {
    const shown = columns.find(c => c.ships) ?? columns[0];
    return { source: 'app', package: '', of: 'base', over: `Set for this app, over Exact's base${shown ? ` (${shown.label} ${shown.value})` : ''}.`, back: 'Back to the base', columns };
  }
  if (t?.package) return { source: 'package', package: String(t.package), of: 'package', over: '', back: '', columns: [] };
  return { source: 'app', package: '', of: '', over: '', back: '', columns: [] };
}

/** The grid group a row is listed in: the base's (an app override of a base name too), a package's, or its name's prefix. */
export const groupOf = (s: Source, prefix: string) => s.of === 'base' ? 'Base scale (Exact)' : s.of === 'package' ? `${s.package} package` : prefix;

/** The targets the app ships: describe's `targets` (asked of theme2 for 1.14), else as its icons'
 * `draws` name them (stage 6 F), where it has an icon. */
export const targetsOf = (ir: any, said: unknown): string[] => Array.isArray(said) ? said.map(String) : [...new Set<string>((ir?.icons ?? []).flatMap((i: any) => Object.keys(i?.draws ?? {})))];
