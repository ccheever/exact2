// The finding lines of a verify report: check's groups, each line's rule and place, and the lines by rule (LLP 1116.004
// D1.3). Pure text; verify.mjs re-exports what its callers import.

/** check's text by scheme: each header's finding lines, how many its "… and N more" left out, and its advice
 * (LLP 1116.004 D1.3), which is never a finding. */
export function groups(text) {
  const out = [];
  for (const line of String(text ?? '').split('\n')) {
    if (/^check /.test(line)) { out.push({ head: line, lines: [], hidden: 0, advice: [] }); continue; }
    const g = out.at(-1), more = line.match(/^\s+… and (\d+) more/);
    if (!g || !line.trim()) continue;
    if (more) g.hidden += Number(more[1]);
    else if (/^\s+advice\s/.test(line)) g.advice.push(line);
    else g.lines.push(line);
  }
  return out;
}

const PLACES = 4; // where a rule's findings are, named after its first in full

/** A finding line's rule: a compiler warning's id, else its first word after any `(… only)` tag (`contrast`). */
export function ruleOf(line) {
  const t = line.trim(), id = /^\S+:\d+:\d+ warning \[([\w-]+)\]/.exec(t);
  return id ? id[1] : t.replace(/^\([^)]*\) /, '').split(/\s+/)[0];
}

/** Where a finding line points: a compiler warning's `file:line:col`, else what follows its rule, up to its first `: `. */
function placeOf(line) {
  const t = line.trim(), id = /^(\S+:\d+:\d+) warning \[/.exec(t);
  if (id) return id[1];
  const body = t.replace(/^\([^)]*\) /, '').replace(/^\S+\s+/, ''), at = body.indexOf(': ');
  return at < 0 ? body : body.slice(0, at);
}

/** `lines` (each indented two) by rule, the commonest first: a rule's one finding as it is; several as the rule and
 *  their count, the first in full, then where the next PLACES are and how many more. */
export function byRule(lines) {
  const rules = new Map();
  for (const l of lines) rules.set(ruleOf(l), [...(rules.get(ruleOf(l)) ?? []), l]);
  return [...rules].sort((a, b) => b[1].length - a[1].length).flatMap(([rule, all]) => {
    if (all.length === 1) return all;
    const rest = all.slice(1), more = rest.length - PLACES;
    return [`  ${rule} ×${all.length}`, `    ${all[0].trim()}`, `    also ${rest.slice(0, PLACES).map(placeOf).join('; ')}${more > 0 ? ` and ${more} more` : ''}`];
  });
}
