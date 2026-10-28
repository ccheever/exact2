// The serial monitor's log (LLP 1133.037 §12.2): every byte a port sends, read at 100 ms through `serialInput`,
// decoded as UTF-8 across answers (a code point split between two stays whole), split into lines, and kept here,
// the last 10,000 per connection. Contract reads the visible window and sends lines with the chosen ending.
import { serialInput, write } from '@exact/serial';

export type LogLine = { id: number; text: string; error: string };
export type SerialLog = { lines: LogLine[]; total: number; done: boolean; error: string };

const KEPT = 10_000, SHOWN = 500;

type Log = { decoder: TextDecoder; partial: string; lines: LogLine[]; next: number; done: boolean; error: string };
const logs = new Map<string, Log>();

/** "— break —" and its kin: D9's non-fatal errors, marked in the log where they arrived. */
const MARKS: Record<string, string> = {
  BreakError: 'break', FramingError: 'framing error', ParityError: 'parity error', BufferOverrunError: 'overrun',
  UnknownError: 'read error',
};

export function decodeBase64(text: string): Uint8Array {
  return Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
}

export function encodeBase64(bytes: Uint8Array): string {
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

function push(log: Log, text: string, error = '') {
  log.lines.push({ id: log.next++, text, error });
  if (log.lines.length > KEPT) log.lines.splice(0, log.lines.length - KEPT);
}

/** Take `text` into the log: whole lines out, the rest kept for the next answer. A CR LF ends one line. */
export function take(log: Pick<Log, 'partial'> & Partial<Log>, text: string, out: (line: string) => void) {
  const all = log.partial + text;
  const parts = all.split(/\r?\n/);
  log.partial = parts.pop() ?? '';
  for (const line of parts) out(line.replace(/\r$/, ''));
}

/** The connection's log as it stands after reading what arrived since the last read. */
export async function serialLog(connection: string): Promise<SerialLog> {
  if (!connection) return { lines: [], total: 0, done: false, error: '' };
  let log = logs.get(connection);
  if (!log) {
    log = { decoder: new TextDecoder('utf-8'), partial: '', lines: [], next: 0, done: false, error: '' };
    logs.set(connection, log);
  }
  if (!log.done) {
    try {
      const input = await serialInput({ connection, interval: 100 });
      const bytes = decodeBase64(input.data);
      // Errors at their byte offsets: the text before each, the mark, then the rest.
      let from = 0;
      const start = input.position - bytes.length;
      for (const e of input.errors) {
        const upto = Math.max(from, Math.min(bytes.length, e.at - start));
        const current = log;
        take(current, current.decoder.decode(bytes.subarray(from, upto), { stream: true }), (line) => push(current, line));
        from = upto;
        if (e.code === 'NetworkError') { current.error = 'Disconnected'; continue; }
        if (current.partial) { push(current, current.partial); current.partial = ''; }
        push(current, `— ${MARKS[e.code] ?? e.code} —`, e.code);
      }
      const current = log;
      take(current, current.decoder.decode(bytes.subarray(from), { stream: !input.done }), (line) => push(current, line));
      if (input.done) {
        if (current.partial) { push(current, current.partial); current.partial = ''; }
        current.done = true;
      }
    } catch (e) {
      log.done = true;
      log.error = (e as { name?: string }).name === 'InvalidStateError' ? '' : String((e as Error).message ?? e);
    }
  }
  const shown = log.partial ? [...log.lines, { id: log.next, text: log.partial, error: '' }] : log.lines;
  return { lines: shown.slice(-SHOWN), total: log.lines.length, done: log.done, error: log.error };
}

/** Clear a connection's log; what arrives next starts it again. */
export async function clearLog(connection: string): Promise<{ cleared: boolean }> {
  const log = logs.get(connection);
  if (log) { log.lines = []; log.partial = ''; }
  return { cleared: true };
}

const ENDINGS: Record<string, string> = { none: '', lf: '\n', cr: '\r', crlf: '\r\n' };

/** Write one line with the chosen ending. */
export async function sendLine(connection: string, text: string, ending: string): Promise<{ sent: boolean; error: string }> {
  try {
    await write({ connection, data: encodeBase64(new TextEncoder().encode(text + (ENDINGS[ending] ?? ''))) });
    return { sent: true, error: '' };
  } catch (e) {
    return { sent: false, error: (e as { name?: string }).name ?? 'Error' };
  }
}
