// The local listener's fixture (LLP 1133.005 §10 step 6): `exactListen`
// on a `unix:` path, each line echoed on its own connection by the mapper;
// `later` kept for a mutation to answer, `bye` ending the connection.
import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.exact.listen-fixture';
export const grants = 'net.listen unix:/tmp/exact-listen-fixture';

type Echo = Result<'echo'>;
type Connection = { readonly readyState: number; send(line: string): void; end(): void };
type ListenEvent = { type: string; id?: number; data?: string; reason?: string; address?: string; kind?: string; message?: string };

/** The connections whose `later` waits for the button, oldest first. */
const later: [number, Connection][] = [];
const lines: Echo['lines'] = [];
let seen = 0;

function shown(status: string): Echo {
  return { status, supported: true, waiting: later.length, lines: lines.slice(-20) };
}

function echo(path: string, on: boolean, takeover: boolean): unknown {
  if (!('exactListen' in globalThis)) return { status: 'unsupported', supported: false, waiting: 0, lines: [] };
  if (!on) return shown('off');
  later.length = 0;
  return (globalThis as any).exactListen(`unix:${path}`, {
    takeover,
    firstLineTimeout: 30000,
    exactStream(e: ListenEvent, connection?: Connection) {
      if (e.type === 'error') return shown(`error ${e.kind}: ${e.message}`);
      if (e.type === 'listening') return shown('listening');
      if (e.type === 'message') {
        lines.push({ key: String(++seen), text: `${e.id}: ${e.data}` });
        if (e.data === 'later') later.push([e.id!, connection!]);
        else if (e.data === 'bye') connection!.end();
        else connection!.send(`echo: ${e.data}`);
      }
      if (e.type === 'end') {
        const at = later.findIndex(([id]) => id === e.id);
        if (at >= 0) later.splice(at, 1);
        lines.push({ key: String(++seen), text: `${e.id} ended (${e.reason})` });
      }
      return shown('listening');
    },
  });
}

function answerLater(): { sent: boolean } {
  const next = later.shift();
  if (!next) return { sent: false };
  next[1].send('later: answered');
  next[1].end();
  return { sent: true };
}

export const answer: Answer = (source, args) => {
  switch (source) {
    case 'echo': return echo(...(args as [string, boolean, boolean])) as any;
    case 'answerLater': return answerLater() as any;
  }
  throw new Error(`unknown source ${source}`);
};
