import type { Answer, Sources } from './app.contract.d.ts';
import { write, writeText, ClipboardItem } from '@exact/clipboard';
import { png, apng, huge, green, red } from './images.ts';

// The suite writes the id `exact new` gave the app. The module is imported
// from data code only (LLP 1133.006 §11 question 3).
export const appId = '__APP_ID__';
export const grants = 'fs.read app:/tmp/clip\nfs.write app:/tmp/clip';

const bytes = (b64: string) => Uint8Array.from(atob(b64), c => c.charCodeAt(0));
const folder = (storage: any) => storage.fs.mkdir('app:/tmp/clip').catch(() => {});
/** `ok`, or the rejection's name. */
const outcome = (p: Promise<void>) => p.then(() => 'ok', (e: any) => `${e?.name}`);
/** A file the source writes, then names by its path. */
const file = async (storage: any, name: string, data: Uint8Array) => {
  await folder(storage);
  await storage.fs.writeFile(`app:/tmp/clip/${name}`, data);
  return `app:/tmp/clip/${name}`;
};

const sources: Sources = {
  clipPngBytes: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/png': bytes(png), 'text/plain': 'a gradient' })])) }),
  clipPngPath: async (_a, _s, storage) => ({ said: await outcome(write([new ClipboardItem({ 'image/png': await file(storage, 'gradient.png', bytes(png)) })])) }),
  clipApng: async (_a, _s, storage) => ({ said: await outcome(write([new ClipboardItem({ 'image/png': await file(storage, 'hidden.png', bytes(apng)) })])) }),
  clipHuge: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/png': bytes(huge) })])) }),
  clipNotPng: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/png': new Uint8Array([1, 2, 3]) })])) }),
  clipText: async () => ({ said: await outcome(writeText('exact clipboard fixture')) }),
  // A write whose data resolves late, then one whose data is ready: B is what the clipboard holds (D2).
  clipRace: async (_a, _s, storage) => {
    const path = await file(storage, 'late.txt', new Uint8Array([65]));
    const late = storage.fs.readFile(path).then(() => 'A');
    const a = outcome(write([new ClipboardItem({ 'text/plain': late })]));
    const b = outcome(write([new ClipboardItem({ 'text/plain': 'B' })]));
    return { said: `${await a} ${await b}` };
  },
  clipTwo: async () => ({ said: await outcome(write([new ClipboardItem({ 'text/plain': 'a' }), new ClipboardItem({ 'text/plain': 'b' })])) }),
  clipJpeg: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/jpeg': new Uint8Array([1]) })])) }),
  // The APNG's frame 0 and its default image as stills: the APNG's written PNG hashes as the green one does.
  clipGreen: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/png': bytes(green) })])) }),
  clipRed: async () => ({ said: await outcome(write([new ClipboardItem({ 'image/png': bytes(red) })])) }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
