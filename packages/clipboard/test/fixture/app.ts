import type { Answer, Sources } from './app.contract.d.ts';
import { write, writeText, ClipboardItem } from '@exact/clipboard';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'fs.read app:/tmp/copy\nfs.write app:/tmp/copy';

// An 8×8 red PNG carrying a `tEXt` chunk the sanitized write drops; an APNG whose default image (red) is not
// part of its animation, frame 0 green and frame 1 blue; a PNG whose header declares 9000×9000 pixels; and the
// 33 bytes of a header that declares 2³²−1 × 2³²−1, past the PNG specification's 2³¹−1.
const RED = 'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAKHRFWHRDb21tZW50AG1ldGFkYXRhIGEgc2FuaXRpemVkIHdyaXRlIGRyb3BzdXr2SAAAABFJREFUeNpj+M/AgBUxDC0JACj/P8HOd8hPAAAAAElFTkSuQmCC';
const APNG = 'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAACGFjVEwAAAACAAAAAPONk3AAAAARSURBVHjaY/jPwIAVMQwtCQAo/z/BznfITwAAABpmY1RMAAAAAAAAAAgAAAAIAAAAAAAAAAAAAQAKAADyTrpiAAAAFGZkQVQAAAABeNpjYPjPgB0NLQkA6TA/wSwJCQUAAAAaZmNUTAAAAAIAAAAIAAAACAAAAAAAAAAAAAEACgAAH9hpiwAAABRmZEFUAAAAA3jaY2Bg+I8DDSkJAKlwP8EZfwA/AAAAAElFTkSuQmCC';
const HUGE_HEADER = 'iVBORw0KGgoAAAANSUhEUv//////////CAIAAABdlLlp';
const BIG = 'iVBORw0KGgoAAAANSUhEUgAAIygAACMoCAIAAADit+XtAAAAC0lEQVR4nGNggAEAAAoAAX+AdF4AAAAASUVORK5CYII=';

const bytes = (b64: string) => Uint8Array.from(atob(b64), c => c.charCodeAt(0));
const said = (e: unknown) => { const x = e as { name?: string; message?: string }; return `${x.name}: ${x.message}`; };

const sources: Sources = {
  supports: async () => ({ text: ['image/png', 'text/plain', 'text/html', 'image/svg+xml'].map(t => `${t}=${ClipboardItem.supports(t)}`).join(' ') }),
  copy: async ([kind], _store, storage) => {
    const file = async (name: string, b64: string) => { const path = `app:/tmp/copy/${name}`; await storage.fs.mkdir('app:/tmp/copy').catch(() => {}); await storage.fs.writeFile(path, bytes(b64)); return path; };
    try {
      switch (kind) {
        case 'png': await write([new ClipboardItem({ 'image/png': await file('red.png', RED) })]); break;
        case 'png-bytes': await write([new ClipboardItem({ 'image/png': bytes(RED) })]); break;
        case 'apng': await write([new ClipboardItem({ 'image/png': await file('anim.png', APNG) })]); break;
        case 'text': await writeText('Copied from the fixture'); break;
        case 'both': await write([new ClipboardItem({ 'image/png': await file('red.png', RED), 'text/plain': 'https://example.com/red.png' })]); break;
        case 'bad': await write([new ClipboardItem({ 'image/png': new TextEncoder().encode('not a png') })]); break;
        case 'large': await write([new ClipboardItem({ 'image/png': await file('big.png', BIG) })]); break;
        case 'big-bytes': await write([new ClipboardItem({ 'image/png': new Uint8Array(2 * 1024 * 1024 + 1) })]); break;
        case 'overflow': await write([new ClipboardItem({ 'image/png': bytes(HUGE_HEADER) })]); break;
        case 'long-text': await write([new ClipboardItem({ 'text/plain': 'x'.repeat(1024 * 1024 + 1) })]); break;
        case 'empty': await write([]); break;
        case 'two': await write([new ClipboardItem({ 'text/plain': 'a' }), new ClipboardItem({ 'text/plain': 'b' })]); break;
        case 'jpeg': await write([new ClipboardItem({ 'image/jpeg': bytes(RED) })]); break;
        case 'nested': return { text: (() => { try { new ClipboardItem({}); return 'constructed'; } catch (e) { return said(e); } })() };
        case 'race': {
          // A's data resolves after B is written: B is what the clipboard holds, and A is superseded (D2).
          let late: (v: string) => void = () => {};
          const a = write([new ClipboardItem({ 'text/plain': new Promise<string>(r => { late = r; }) })]).then(() => 'A ok', said);
          const b = write([new ClipboardItem({ 'text/plain': 'B' })]).then(() => 'B ok', said);
          const second = await b;
          late('A');
          return { text: `${await a} | ${second}` };
        }
        case 'refused-first': {
          // A write refused before it is sent supersedes nothing: A, still awaiting its data, lands.
          let late: (v: string) => void = () => {};
          const a = write([new ClipboardItem({ 'text/plain': new Promise<string>(r => { late = r; }) })]).then(() => 'A ok', said);
          const refused = await write([new ClipboardItem({ 'image/png': new Uint8Array(2 * 1024 * 1024 + 1) })]).then(() => 'refused ok', said);
          late('A');
          return { text: `${await a} | ${refused}` };
        }
        default: return { text: `no kind ${kind}` };
      }
      return { text: `ok ${kind}` };
    } catch (e) {
      return { text: said(e) };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
