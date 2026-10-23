// The listener fixture's source (LLP 1133.005 D1): `exactListen` on a
// `unix:` path under /tmp, answered as a stream. The mapper keeps each
// connection's handle, echoes a line at once and queues `later` for the
// Answer mutation, which replies on the connection it kept and ends it.
import type { Answer, ListenConnection, ListenEvent } from './app.contract.d.ts';

export const appId = 'com.exact.listenfixture';
const DIR = '/tmp/exact-listen-fixture';
export const grants = `net.listen unix:${DIR}`;

type Control = { status: string; connections: string; lines: string; last: string; waiting: string };

// The listener incarnation's connections and what it has read.
let connections = new Map<number, ListenConnection>();
let waiting: number[] = [];
let lines = 0, last = '', status = 'binding';

const snapshot = (): Control => ({
  status, connections: String(connections.size), lines: String(lines), last, waiting: String(waiting.length),
});

function echo(running: boolean): Control | Promise<Control> {
  connections = new Map(); waiting = []; lines = 0; last = '';
  if (!('exactListen' in globalThis)) { status = 'no listener on this platform'; return snapshot(); }
  if (!running) { status = 'stopped'; return snapshot(); }
  status = 'binding';
  return exactListen(`unix:${DIR}/echo.sock`, {
    takeover: true,
    firstLineTimeout: 5000,
    exactStream(event: ListenEvent, connection: ListenConnection) {
      switch (event.type) {
        case 'listening': status = `listening on ${event.address}`; break;
        case 'error': status = `error ${event.kind}: ${event.message}`; break;
        case 'connection': connections.set(event.id, connection); break;
        case 'end': connections.delete(event.id); waiting = waiting.filter(id => id !== event.id); break;
        case 'message':
          lines++; last = event.data;
          if (event.data === 'later') waiting.push(event.id);
          else if (event.data === 'bye') { connection.send('bye'); connection.end(); }
          else connection.send(`echo: ${event.data}`);
          break;
      }
      return snapshot();
    },
  });
}

// A reply after the line's own turn: the connection kept from the mapper.
function answerLater(): string {
  const id = waiting.shift();
  const connection = id === undefined ? undefined : connections.get(id);
  if (!connection) return 'Nothing is waiting';
  connection.send('answered');
  connection.end();
  return `Answered connection ${id}`;
}

export const answer: Answer = (source, args) => {
  switch (source) {
    case 'echo': return echo(args[0] as boolean) as never;
    case 'answerLater': return answerLater() as never;
    default: throw new Error(`unknown source ${source}`);
  }
};
