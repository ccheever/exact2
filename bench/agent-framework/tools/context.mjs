// Characters an agent wrote (W:) and read back (R:), by kind of call; images are counted, not sized.
import fs from 'node:fs';
const kind = (s) =>
  /start-here|card\.md|contract-for-agents|agent-pitfalls|reference\.md|recipes\/|docs |patterns\/|CLAUDE|AGENTS|SPEC/.test(s) ? 'docs'
  : /vocab/.test(s) ? 'vocab'
  : /contract build|exact.mjs (build|web-build)|npm run build|astro build/.test(s) ? 'build'
  : /test|playwright test/.test(s) ? 'test'
  : /\.png|screenshot|shoot|crop/.test(s) ? 'shot'
  : /curl/.test(s) ? 'curl'
  : 'other';
for (const file of process.argv.slice(2)) {
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
  const uses = {}, sum = {};
  const add = (k, v) => (sum[k] = (sum[k] ?? 0) + v);
  for (const e of lines) {
    if (e.type === 'assistant') for (const c of e.message.content ?? []) {
      if (c.type === 'tool_use') { uses[c.id] = c; add('W:' + (c.name === 'Write' ? 'write' : c.name === 'Edit' ? 'edit' : 'cmd'), JSON.stringify(c.input).length); }
      if (c.type === 'text') add('W:text', c.text.length);
    }
    if (e.type === 'user' && Array.isArray(e.message.content)) for (const c of e.message.content) {
      if (c.type !== 'tool_result') continue;
      const u = uses[c.tool_use_id], s = u ? JSON.stringify(u.input) : '';
      const k = u?.name === 'Read' && /\.png/.test(s) ? 'shot' : kind(s);
      for (const x of Array.isArray(c.content) ? c.content : [{ type: 'text', text: String(c.content) }]) {
        if (x.type === 'image') add(`R:${k}:img`, 1); else add('R:' + k, (x.text ?? '').length);
      }
    }
  }
  const out = Object.entries(sum).sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k}=${k.endsWith(':img') ? v : Math.round(v / 1000) + 'K'}`);
  console.log(file.split('/').slice(-3)[0], out.join(' '));
}
