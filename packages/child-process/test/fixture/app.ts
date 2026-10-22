import type { Answer, Sources } from './app.contract.d.ts';
import { spawn, execFile, processStatus, type Child, type ChildEvent } from '@exact/child-process';

// The drive writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'process.spawn path:fixture-child\nfs.write home:/.fixture\nfs.read home:/.fixture';

// The incarnation's handle the mapper last saw, for a mutation's writes.
let child: Child | null = null;
const events: string[] = [];
// Every pid a real child reported (`listening <pid> grandchild <pid>`), for
// the drive to check after a release, a quit or a crash; and the bytes of
// the echoed long line, counted rather than shown.
const pids: string[] = [];
let echoed = 0;
let relocks = 0;
const show = () => ({ text: `pids ${pids.join(',')} | echoed ${echoed} | ${events.slice(-12).join(' | ')}` });
// Output can be long (the real child echoes a 100,000-byte line): its start and length.
const brief = (data: string | ArrayBuffer) => {
  const text = String(data).trim();
  return text.length > 40 ? `${text.slice(0, 40)}… (${text.length})` : text;
};

const sources: Sources = {
  server: ([generation]) =>
    spawn('path:fixture-child', ['--serve', String(generation)], {
      encoding: 'utf8',
      exactRestart: { when: 'always', delay: 500, maxDelay: 10000, preventExitStatus: [78] },
      exactDrainTimeout: 200,
      exactForceKillAfter: 300,
      exactStream(e: ChildEvent, c: Child) {
        child = c;
        switch (e.type) {
          case 'spawn': {
            events.push(`spawn ${e.pid} r${e.restarts}`);
            // Past the high-water mark: `write` says wait, and `drain` says go on.
            const ok = c.write('x'.repeat(99999) + '\n');
            events.push(`write ${ok} ${c.writableLength}`);
            break;
          }
          case 'drain':
            c.write('after drain\n');
            events.push(`drain ${c.writableLength}`);
            break;
          case 'stdout':
          case 'stderr': {
            const text = String(e.data);
            const found = /listening (\d+) grandchild (\d+)/.exec(text);
            if (found) pids.push(found[1], found[2]);
            if (/^(echo )?x{20,}/.test(text) || /^x+\s*$/.test(text)) echoed += text.length;
            else events.push(`${e.type} ${brief(text)}`);
            break;
          }
          case 'exit':
            events.push(`exit ${e.code} ${e.signal}`);
            break;
          case 'close':
            events.push(`close ${e.code} ${e.drained}`);
            break;
          case 'error':
            events.push(`error ${e.kind}: ${e.message}`);
            break;
          default:
            events.push(e.type);
        }
        return show();
      },
    }),
  version: async () => {
    try {
      const r = await execFile('path:fixture-child', ['--version'], { encoding: 'utf8', timeout: 2000 });
      return { text: `${r.code} ${r.stdout.trim()}` };
    } catch (x) {
      return { text: `${(x as { code?: string }).code}: ${(x as Error).message}` };
    }
  },
  status: () => ({ text: JSON.stringify(processStatus()) }),
  // What an install does under the home directory (D6, D6a): a launcher written
  // and made executable, a lock taken, a digest, and a link checked.
  install: async (_args, _store, storage) => {
    const fs = storage.fs;
    try {
      await fs.mkdir('home:/.fixture/bin');
      await fs.writeFile('home:/.fixture/bin/tool', new TextEncoder().encode('#!/bin/sh\necho tool\n'));
      await fs.chmod('home:/.fixture/bin/tool', 0o755);
      const stat = await fs.stat('home:/.fixture/bin/tool');
      await fs.access('home:/.fixture/bin/tool', 1);
      const lock = await fs.flock('home:/.fixture/.install.lock', { timeout: 1000 });
      const digest = await fs.digest('home:/.fixture/bin/tool', 'SHA-256');
      await fs.rm('home:/.fixture/link');
      const real = await fs.realpath('home:/.fixture/bin/tool');
      await fs.symlink(real, 'home:/.fixture/link');
      const link = await fs.lstat('home:/.fixture/link');
      const target = await fs.readlink('home:/.fixture/link');
      await lock.unlock();
      return { text: `mode ${(stat.mode ?? 0).toString(8)} link ${link.isSymbolicLink} ${target === real} sha256 ${digest.slice(0, 12)}` };
    } catch (x) {
      return { text: `install: ${(x as Error).message}` };
    }
  },
  // A lock its answer takes and never unlocks: the answer's end releases it,
  // so the next press takes it again at once (D6a).
  relock: async (_args, _store, storage) => {
    try {
      await storage.fs.flock('home:/.fixture/.relock', { timeout: 0 });
      return { text: `locked ${++relocks}` };
    } catch (x) {
      return { text: `relock: ${(x as Error).message}` };
    }
  },
  poke: ([what]) => {
    if (!child) return { text: 'no child' };
    try {
      if (what === 'write') return { text: `write ${child.write('poke\n')}` };
      if (what === 'say') return { text: `say ${child.write('hello\n')}` };
      if (what === 'quit') return { text: `quit ${child.write('quit\n')}` };
      if (what === 'close') return { text: `close ${child.write('close\n')}` };
      if (what === 'end') {
        child.end();
        return { text: 'ended' };
      }
      return { text: `kill ${child.kill('SIGTERM')}` };
    } catch (x) {
      return { text: `${(x as Error).name}: ${(x as Error).message}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
