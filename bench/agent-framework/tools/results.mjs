// Bytes of tool results per call (TOP=n), largest first, with the call.
import fs from 'node:fs';
const f = process.argv[2], top = +(process.env.TOP ?? 25);
const L = fs.readFileSync(f, 'utf8').split('\n').filter(Boolean).map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
const calls = new Map(), out = [];
for (const e of L) for (const c of (Array.isArray(e.message?.content) ? e.message.content : [])) {
  if (c.type === 'tool_use') calls.set(c.id, c.name === 'Bash' ? c.input.command : c.name === 'Read' ? 'READ ' + c.input.file_path : c.name);
  if (c.type === 'tool_result') { const s = typeof c.content === 'string' ? c.content : JSON.stringify(c.content); out.push([/"type":"image"/.test(s) ? 'IMG' : s.length, calls.get(c.tool_use_id) ?? '?']); }
}
let tot = 0; for (const [n] of out) if (n !== 'IMG') tot += n;
console.log('text bytes', tot, 'images', out.filter(([n]) => n === 'IMG').length);
for (const [n, c] of out.filter(([n]) => n !== 'IMG').sort((a, b) => b[0] - a[0]).slice(0, top)) console.log(String(n).padStart(6), c.replace(/\n/g, ' ⏎ ').slice(0, 150));
