// Context size per assistant turn, and the tool calls before the largest jumps (TOP=n; 0 prints only the totals).
import fs from 'node:fs';
const lines = fs.readFileSync(process.argv[2], 'utf8').split('\n').filter(Boolean).map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
let prev = 0, lastCall = '', jumps = [], first = null, seen = new Set();
for (const e of lines) {
  const m = e.message;
  if (e.type === 'assistant' && m?.usage) {
    if (seen.has(m.id)) continue; seen.add(m.id);
    const ctx = (m.usage.cache_read_input_tokens ?? 0) + (m.usage.cache_creation_input_tokens ?? 0) + (m.usage.input_tokens ?? 0);
    first ??= ctx;
    if (prev) jumps.push([ctx - prev, lastCall]);
    prev = ctx;
    for (const c of (Array.isArray(m.content) ? m.content : [])) if (c.type === 'tool_use') lastCall = c.name + ' ' + (c.input.command ?? c.input.file_path ?? JSON.stringify(c.input)).split('\n')[0].slice(0, 120);
  }
}
console.log('first', first, 'last', prev, 'turns', seen.size);
for (const [d, c] of jumps.sort((a, b) => b[0] - a[0]).slice(0, +(process.env.TOP ?? 12))) console.log(String(d).padStart(7), c);
