import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.example.x21-websocket-send';
export const grants = 'net.websocket ws://127.0.0.1:28791';

// The documented form: a receive-only stream (LLP 1016.000).
const feed = (url: string) =>
  fetch(url, {
    exactStream: (event: any, socket?: any): Result<'feed'> => {
      let canSend = `mapper socket: ${typeof socket}; global WebSocket: ${typeof (globalThis as any)['Web' + 'Socket']}`;
      try { socket?.send?.('{"_tag":"Request","id":"1"}'); } catch (e) { canSend += `; send threw ${e}`; }
      return { text: `${event.type}: ${event.data ?? event.message}`, canSend };
    },
  } as any) as any;

// The module's own I/O globals, reached through an alias a build cannot see.
const io: any = globalThis;
const attempt = (open: () => unknown) => { try { open(); return 'opened'; } catch (e) { return `threw: ${e}`; } };

// The web's own API: open, send one frame, answer with the echo.
const direct = (url: string): Promise<Result<'direct'>> =>
  new Promise((done) => {
    try {
      const ws = new (globalThis as any).WebSocket(url);
      ws.onopen = () => ws.send('ping from module');
      ws.onmessage = (m: any) => { if (String(m.data).startsWith('echo')) done({ text: String(m.data) }); };
      ws.onerror = () => done({ text: 'error event' });
    } catch (e) { done({ text: `new WebSocket threw: ${e}` }); }
  });

const kinds = (url: string): Result<'kinds'> => ({
  kinds: ['WebSocket', 'XMLHttpRequest', 'EventSource'].map((name) => `typeof ${name}: ${typeof io[name]}`).join('; '),
  xhr: `new XMLHttpRequest() ${attempt(() => new io.XMLHttpRequest())}`,
  eventSource: `new EventSource(url) ${attempt(() => new io.EventSource(url))}`,
});

export const answer: Answer = (source, [url]) =>
  (source === 'feed' ? feed(url as string) : source === 'direct' ? direct(url as string) : kinds(url as string)) as any;
