/** Runtime family lists use the same portable `ui-*` fallback as literal
 * stacks; quoted installed names (including commas) stay untouched. */
export function fontFamily(value) {
  if (typeof value !== 'string') return value;
  // CSS-wide keywords as the kernel reads them: `initial` is its family 0
  // (system-ui), not the browser's own default; `revert` on an author style
  // is `unset` (runner bridge.rs `unsets`, `set_style`).
  const wide = value.trim().toLowerCase();
  if (wide === 'initial') return 'system-ui';
  if (wide === 'revert' || wide === 'revert-layer') return 'unset';
  const generic = { 'ui-monospace': 'ui-monospace,monospace', 'ui-serif': 'ui-serif,serif', 'ui-sans-serif': 'ui-sans-serif,system-ui,sans-serif', 'ui-rounded': 'ui-rounded,system-ui,sans-serif', '-apple-system': 'system-ui' };
  let quote = '', escaped = false, start = 0, parts = [];
  const push = end => { const part = value.slice(start, end), name = part.trim().toLowerCase(); parts.push(generic[name] ?? part); start = end + 1; };
  for (let i = 0; i < value.length; i++) {
    const c = value[i];
    if (escaped) { escaped = false; continue; }
    if (c === '\\') { escaped = true; continue; }
    if (quote) { if (c === quote) quote = ''; }
    else if (c === '"' || c === "'") quote = c;
    else if (c === ',') push(i);
  }
  push(value.length);
  return parts.join(',');
}
