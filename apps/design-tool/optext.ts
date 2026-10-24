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
