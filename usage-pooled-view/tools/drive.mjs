#!/usr/bin/env bun
// usage-pooled-view: one agent-mode launch driven op by op (uncommitted). It opens the macOS app through
// scripts/agent.mjs `open` and then runs the op lines it finds in <dir>/cmd-<n>.txt, writing each
// result to <dir>/out-<n>.json, until a `quit` line. The ops are the CLI's (tree, state, logs,
// screenshot <png> [window], tap <target> [hover|mouse|…], type <target> <text>|key <Name>, clock,
// prefer, resize, layout). Pairing links are read from files and typed with `pair <field> <file>`,
// so a token never appears in an op line or a transcript.
//   bun drive.mjs <dir> <epoch ms> [<w>x<h>] [storage]
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { open, tapWords, typeCommand } from '../../scripts/agent.mjs';

const [dirArg, epochArg, size = '1280x840', storage = 'upv-drive'] = process.argv.slice(2);
const dir = resolve(dirArg);
mkdirSync(dir, { recursive: true });
const redact = (text) => text.replace(/token=[A-Za-z0-9._~-]+/g, 'token=<redacted>');
const s = await open({ host: 'macos', size, epoch: Number(epochArg), storage, onProcess: (pid) => writeFileSync(join(dir, 'app.pid'), String(pid?.pid ?? pid)) });
writeFileSync(join(dir, 'ready.json'), JSON.stringify({ boot: s.boot }));
let n = 0;
for (;;) {
  const cmd = join(dir, `cmd-${n}.txt`);
  if (!existsSync(cmd)) { await Bun.sleep(150); continue; }
  const lines = readFileSync(cmd, 'utf8').split('\n').map((line) => line.trim()).filter(Boolean);
  const results = [];
  let quit = false;
  for (const line of lines) {
    if (line === 'quit') { quit = true; break; }
    const started = Date.now();
    try {
      let r;
      if (line.startsWith('pair ')) {
        const [, field, file] = line.split(/\s+/);
        r = await s.type(field, readFileSync(file, 'utf8').trim());
      } else if (/^type(\s|$)/.test(line)) r = await s.type(...typeCommand(line));
      else {
        const named = line.match(/^(tap|tree|state|layout)\s+"((?:[^"\\]|\\.)*)"(?:\s+([\s\S]*))?$/);
        const [op, ...args] = named ? [named[1], JSON.parse(`"${named[2]}"`), ...(named[3] ? named[3].split(/\s+/) : [])] : line.split(/\s+/);
        if (op === 'tree') r = await s.tree(args[0]);
        else if (op === 'state') r = await s.state(args[0]);
        else if (op === 'logs') r = await s.logs();
        else if (op === 'layout') r = await s.layout(args[0]);
        else if (op === 'screenshot') r = await s.screenshot(resolve(dir, args[0]), args[1] === 'window');
        else if (op === 'clock') r = await s.clock(args.join(' ') || 'settle');
        else if (op === 'prefer') r = await s.prefer(Object.fromEntries(args.flatMap((a, i) => (i % 2 ? [] : [[a, args[i + 1]]]))));
        else if (op === 'resize') { const [w, h] = args[0].split('x').map(Number); r = await s.resize(w, h); }
        else if (op === 'tap') { const [target, opts] = tapWords(args, false); r = await s.tap(target, opts); }
        else throw new Error(`unknown op ${op}`);
      }
      results.push({ op: redact(line), ms: Date.now() - started, r });
    } catch (error) {
      results.push({ op: redact(line), ms: Date.now() - started, error: redact(String(error?.message ?? error)) });
    }
  }
  writeFileSync(join(dir, `out-${n}.json`), redact(JSON.stringify(results, null, 1)));
  n += 1;
  if (quit) break;
}
await s.close?.();
writeFileSync(join(dir, 'closed.json'), '{}');
process.exit(0);
