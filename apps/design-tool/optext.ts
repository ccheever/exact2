// An operation in plain words, as a held proposal lists its items (app.ts `opText` adds the file it edits).

/** A `when` as words: each `axis=value`, sorted. */
export const whenText = (w: { axis: string; value: string }[]) => w.map(c => `${c.axis}=${c.value}`).sort().join(' ');

/** What `o` does, in words. */
export const opWords = (o: any): string =>
  o?.op === 'set-token' ? `${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''} = ${o.value}`
  : o?.op === 'unset-token-when' ? `${o.name}: drop when ${whenText(o.when ?? [])}`
  : o?.op === 'set-dial' ? `${o.name} = ${o.value}`
  : o?.op === 'set-param' ? `${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''} = ${o.value}`
  : o?.op === 'unset-param' ? `reset ${o.name}${o.when?.length ? ` when ${whenText(o.when)}` : ''}`
  : o?.op === 'add-token' ? `add ${o.name} = ${o.value}`
  : o?.op === 'rename-token' ? `rename ${o.from} → ${o.to}`
  : o?.op === 'remove-token' ? `remove ${o.name}`
  : o?.op === 'add-axis' ? `add axis ${o.name ?? o.axis} = ${[].concat(o.values ?? []).join(' | ')}`
  : o?.op === 'add-axis-value' ? `add ${o.value} to axis ${o.axis ?? o.name}`
  : o?.op === 'remove-axis-value' ? `remove ${o.value} from axis ${o.axis ?? o.name}`
  : o?.op === 'set-state' ? `set ${o.style} :${String(o.state).replace(/^:/, '')} ${o.row} = ${typeof o.value === 'string' ? JSON.stringify(o.value) : o.value}`
  : o?.op === 'set-row' ? `set ${o.style ? `${o.style} ` : ''}${o.state ? `:${o.state} ` : ''}${o.row}${o.style ? '' : ` at ${o.file}:${o.at?.line}`} = ${typeof o.value === 'string' ? JSON.stringify(o.value) : o.value}`
  : o?.op === 'unset-state' ? `unset ${o.style} :${String(o.state).replace(/^:/, '')} ${o.row}`
  : typeof o?.op === 'string' && /axis/.test(o.op) ? `${o.op.replace(/-/g, ' ')} ${o.name ?? o.axis ?? ''}${o.value ? ` ${o.value}` : ''}${o.values ? ` = ${[].concat(o.values).join(' | ')}` : ''}`.trim()
  : JSON.stringify(o);

/** A param or token as a person names it: a param's `component.param` (never its declaring path, `@exact/cards/x.contract#Card.fill`). */
export const shortName = (name: string): string => name.includes('#') ? name.slice(name.lastIndexOf('#') + 1) : name;

/** A held proposal's title (stage 15 D4 batch 1 #4): the tokens and params its operations edit, then the ones it
 * changes only because they read those, counted and named; everything it changes when no operation names one. */
export function heldTitle(ops: any[], changed: { file: string; token: string }[], design = ''): string {
  // What each operation names, by its file where it says one: a rename both its names, a param's add or removal its
  // component's, a param by its short name (one spelled either way is one).
  const named = (o: any): string[] => o?.op === 'rename-token' ? [o.from, o.to] : (o?.op === 'add-param' || o?.op === 'remove-param') && o.component ? [`${o.component}.${o.name}`] : [o?.name];
  const edited = ops.flatMap(o => named(o).filter((n): n is string => typeof n === 'string').map(n => ({ name: n, file: typeof o?.file === 'string' && o.file ? o.file : design || null })));
  const words = [...new Set(edited.map(e => shortName(e.name)))];
  const all = [...new Set(changed.map(c => shortName(c.token)))];
  if (!words.length) return all.join(', ');
  // Each it changes that no operation edits there (a reader in one file of a token edited in another among them).
  // (Matched whole, a declaring path against its own; a name written short matches any of its spellings. An operation
  // that names no file edits the app's design.)
  const same = (a: string, b: string) => a === b || ((!a.includes('#') || !b.includes('#')) && shortName(a) === shortName(b));
  const others = [...new Set(changed.filter(c => !edited.some(e => same(e.name, c.token) && (e.file === null || e.file === c.file))).map(c => shortName(c.token)))];
  return `${words.join(', ')}${others.length ? ` (and ${others.length} that read ${words.length === 1 ? 'it' : 'them'}: ${others.join(', ')})` : ''}`;
}
