// Where a token's value comes from (LLP 1121 stage 11, D11; the server's
// half is stage 11 D, tamber/1.14): Exact's base scale (`space.*`,
// `radius.*`, one value a platform, each with its source), a package's
// design (read-only: the app overrides it in its own file), or the app's,
// whose line may override either.

/** The scale table's columns, in order, as the tool names them. */
const COLUMNS: [string, string][] = [['ios', 'iOS'], ['mac', 'Mac'], ['neutral', 'neutral'], ['material', 'Android'], ['terminal', 'terminal']];

export type BaseColumn = { name: string; label: string; value: string; from: string; stretch: boolean; ships: boolean };
export type Source = { source: string; package: string; of: string; over: string; back: string; columns: BaseColumn[] };

/** A base row's columns: those this app's targets draw first (`ships`), then the rest. */
function columnsOf(base: any): BaseColumn[] {
  const ships = new Set<string>(Array.isArray(base?.ships) ? base.ships.map(String) : []);
  const all = COLUMNS.filter(([name]) => base?.columns?.[name]).map(([name, label]) => {
    const c = base.columns[name];
    return { name, label, value: String(c.value ?? ''), from: String(c.from ?? ''), stretch: c.stretch === true, ships: ships.has(name) };
  });
  return [...all.filter(c => c.ships), ...all.filter(c => !c.ships)];
}

/** A describe token's source: base, package or app; for an app line over a
 * base or package name, what it replaces (`over`) and the way back (`back`). */
export function sourceOf(t: any): Source {
  const source = t?.source === 'base' || t?.source === 'package' ? t.source : 'app';
  const pkg = String(t?.package ?? t?.overrides?.package ?? '');
  const columns = t?.base ? columnsOf(t.base) : [];
  const o = t?.overrides;
  if (source !== 'app' || !o) return { source, package: pkg, of: source === 'app' ? '' : source, over: '', back: '', columns };
  const shown = columns.find(c => c.ships) ?? columns[0];
  const was = o.source === 'base' ? `Exact's base${shown ? ` (${shown.label} ${shown.value})` : o.value ? ` (${o.value})` : ''}` : `the ${pkg} package's ${o.value ?? ''}`.trim();
  return { source, package: pkg, of: o.source === 'base' ? 'base' : 'package', over: `Set for this app, over ${was}.`, back: o.source === 'base' ? 'Back to the base' : 'Back to the package', columns };
}

/** The grid group a row is listed in: the base's or a package's (an app line over one of theirs too), or its name's prefix. */
export const groupOf = (s: Source, prefix: string) => s.of === 'base' ? 'Base scale (Exact)' : s.of === 'package' ? `${s.package} package` : prefix;
