// Where a build's input went: every tool call's output, classified by what it read or ran, summed per class.
// Tool output is what the builder takes in and re-sends each step, so it is the tokens' main lever.
//   bun bench/bakeoff/anatomy.mjs <round dir> [spec-arm …]
import { readFileSync, readdirSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const ROUND = resolve(process.argv[2]);
const only = process.argv.slice(3);

/** One build's calls: [{ cmd, bytes }] from a Codex or a Claude Code stream. */
function calls(file) {
  const out = [], pending = new Map();
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    let e; try { e = JSON.parse(line); } catch { continue; }
    if (e.type === 'item.completed' && e.item?.type === 'command_execution') out.push({ cmd: e.item.command ?? '', bytes: (e.item.aggregated_output ?? '').length });
    if (e.type === 'item.completed' && e.item?.type === 'file_change') out.push({ cmd: 'edit', bytes: 0 });
    if (e.type === 'assistant') for (const c of e.message?.content ?? []) if (c.type === 'tool_use') pending.set(c.id, `${c.name} ${c.input?.command ?? c.input?.file_path ?? c.input?.pattern ?? ''}`);
    if (e.type === 'user') for (const c of e.message?.content ?? []) if (c.type === 'tool_result') {
      const text = typeof c.content === 'string' ? c.content : (c.content ?? []).map((x) => x.text ?? '').join('');
      out.push({ cmd: pending.get(c.tool_use_id) ?? '?', bytes: text.length });
    }
  }
  return out;
}

const CLASSES = [
  ['start-here', /start-here\.md/], ['card', /card\.md/], ['recipes', /docs\/recipes\//], ['patterns', /pattern|docs\/patterns/],
  ['long guides', /contract-for-agents|contract-grammar|reference\.md|agent-pitfalls/], ['vocab/docs verb', /exact\.mjs (contract vocab|docs)/],
  ['exact source', /exact2[^ ]*\/(scripts|host|contract|kernel|runner|apps)\//], ['verify', /exact\.mjs verify/], ['test', /exact\.mjs test/],
  ['agent drive', /exact\.mjs agent|agent\.mjs/], ['ios build', /exact\.mjs ios|build\.sh|swiftc|xcodebuild/], ['contract build', /contract build|exact\.mjs check/],
  ['simctl', /simctl/], ['edit', /^(edit|Edit|Write|MultiEdit)/], ['read own', /^Read |cat |sed -n|head |nl /], ['search', /rg |grep |Grep|Glob|find /],
];
const classOf = (cmd) => CLASSES.find(([, re]) => re.test(cmd))?.[0] ?? 'other';

for (const f of readdirSync(resolve(ROUND, 'logs')).filter((f) => f.endsWith('.jsonl'))) {
  const name = basename(f, '.jsonl');
  if (only.length && !only.includes(name)) continue;
  const cs = calls(resolve(ROUND, 'logs', f));
  const by = {};
  for (const c of cs) { const k = classOf(c.cmd); by[k] ??= { n: 0, kb: 0 }; by[k].n++; by[k].kb += c.bytes / 1024; }
  const total = cs.reduce((a, c) => a + c.bytes, 0) / 1024;
  console.log(`\n${name}: ${cs.length} calls, ${total.toFixed(0)} KB of tool output`);
  for (const [k, v] of Object.entries(by).sort((a, b) => b[1].kb - a[1].kb)) console.log(`  ${k.padEnd(16)} ${String(v.n).padStart(4)} calls ${v.kb.toFixed(1).padStart(8)} KB`);
  for (const c of [...cs].sort((a, b) => b.bytes - a.bytes).slice(0, 5)) console.log(`    ${(c.bytes / 1024).toFixed(1).padStart(6)} KB  ${c.cmd.replace(/\s+/g, ' ').slice(0, 110)}`);
}
