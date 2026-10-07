import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.example.probe126';
export const grants = 'net.websocket ws://127.0.0.1:8791';

// The documented form: a receive-only stream under net.websocket.
const feed = (url: string) =>
  fetch(url, { exactStream: (event: any) => ({ text: `${event.type}: ${event.data ?? event.message}` }) } as any) as any;

// The module's own WebSocket: open, send one frame, answer with what happened.
const direct = (url: string): Promise<Result<'direct'>> =>
  new Promise((done) => {
    try {
      const ws = new (globalThis as any).WebSocket(url);
      ws.onopen = () => { ws.send('ping from module'); };
      ws.onmessage = (m: any) => { if (String(m.data).startsWith('echo')) done({ text: `sent; ${String(m.data)}` }); };
      ws.onerror = () => done({ text: 'error event' });
    } catch (e) { done({ text: `threw: ${e}` }); }
  });

export const answer: Answer = (source, [url]) =>
  (source === 'feed' ? feed(url as string) : direct(url as string)) as any;
