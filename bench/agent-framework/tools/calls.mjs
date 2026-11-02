// Categorize an agent transcript's tool calls and count build errors, per session.
import fs from 'node:fs';
const cats = { exactSource: /\/opt\/exact2\/(host|scripts|runner|js|contract\/(?!corpus)|kernel)/, exactDocs: /\/opt\/exact2\/(docs|AGENTS|CLAUDE|README|llp|rules|apps|contract\/corpus)/, vocab: /contract vocab|exact\.mjs docs/, build: /contract build|web-build|npm run build/, edit: /python3 - <<|sed -i|cat > .*\.contract|cat > app\.ts|> app\.contract/, shot: /snap|shot|screenshot|crop|agent-browser|playwright|\.png/, test: /npm test|bun exact.mjs test/, serve: /serve|curl/ };
for (const file of process.argv.slice(2)) {
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
  const count = {}; let n = 0, errs = 0, reads = 0;
  const seen = new Set();
  for (const e of lines) for (const c of (Array.isArray(e.message?.content) ? e.message.content : [])) {
    if (c.type === 'tool_use' && !seen.has(c.id)) {
      seen.add(c.id); n++;
      const s = c.name === 'Bash' ? c.input.command : c.name === 'Read' ? c.input.file_path : JSON.stringify(c.input);
      if (c.name === 'Read') reads++;
      for (const [k, re] of Object.entries(cats)) if (re.test(s)) { count[k] = (count[k] ?? 0) + 1; break; }
    }
    if (c.type === 'tool_result' && /\[[a-z]+-[a-z-]+\]|error:/.test(JSON.stringify(c.content)) ) errs++;
  }
  console.log(file.split('/').slice(-3, -2)[0], 'calls', n, 'reads', reads, JSON.stringify(count), 'results-with-errors', errs);
}
