import type { Answer, Sources } from './app.contract.d.ts';
import { spawn, processStatus, type Child, type ChildEvent } from '@exact/child-process';

// The drive writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'process.spawn path:fixture-child';

// The incarnation's handle the mapper last saw, for a mutation's writes.
let child: Child | null = null;
const events: string[] = [];
const show = () => ({ text: events.slice(-10).join(' | ') });

const sources: Sources = {
  server: () =>
    spawn('path:fixture-child', ['--serve'], {
      encoding: 'utf8',
      exactRestart: { when: 'always', delay: 500, maxDelay: 10000, preventExitStatus: [78] },
      exactStream(e: ChildEvent, c: Child) {
        child = c;
        switch (e.type) {
          case 'spawn': {
            events.push(`spawn ${e.pid} r${e.restarts}`);
            // Past the high-water mark: `write` says wait, and `drain` says go on.
            const ok = c.write('x'.repeat(100000));
            events.push(`write ${ok} ${c.writableLength}`);
            break;
          }
          case 'drain':
            c.write('after drain\n');
            c.end();
            events.push(`drain ${c.writableLength}`);
            break;
          case 'stdout':
          case 'stderr':
            events.push(`${e.type} ${String(e.data).trim()}`);
            break;
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
  status: () => ({ text: JSON.stringify(processStatus()) }),
  poke: ([what]) => {
    if (!child) return { text: 'no child' };
    try {
      if (what === 'write') return { text: `write ${child.write('poke\n')}` };
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
