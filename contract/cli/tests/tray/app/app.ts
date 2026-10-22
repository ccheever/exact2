import type { Answer, Sources } from './app.contract.d.ts';
import { put, TrayItem } from '@fixture/tray';

// The suite writes the id `exact new` gave the app. The module is imported
// from data code only: the app's Contract names none of it.
export const appId = '__APP_ID__';
export const grants = 'fs.read app:/tmp/tray\nfs.write app:/tmp/tray\nfs.write app:/data';

// The folder, made once; a second `mkdir` of it is refused, which is fine.
const folder = (storage: any) => storage.fs.mkdir('app:/tmp/tray').catch(() => {});
const outcome = (p: Promise<void>) => p.then(() => 'ok', (e: any) => `${e?.name}`);

const sources: Sources = {
  // Bytes from a view: copied when called, so a later write to its buffer does not reach the call.
  trayBytes: async () => {
    const view = new Uint8Array([1, 2, 3, 4, 5]);
    const sent = put([new TrayItem({ 'application/octet-stream': view.subarray(1, 4), 'text/plain': 'hi' })]);
    view.fill(9);
    return { said: await outcome(sent) };
  },
  // A path the source's `fs.read` covers: the host reads the file.
  trayFile: async (_args, _store, storage) => {
    await folder(storage);
    await storage.fs.writeFile('app:/tmp/tray/a.bin', new Uint8Array(1000));
    return { said: await outcome(put([new TrayItem({ 'application/octet-stream': 'app:/tmp/tray/a.bin' })])) };
  },
  // One it does not cover: refused before the implementation sees it.
  trayDenied: async (_args, _store, storage) => {
    await storage.fs.writeFile('app:/data/b.bin', new Uint8Array(10));
    return { said: await outcome(put([new TrayItem({ 'application/octet-stream': 'app:/data/b.bin' })])) };
  },
  // A put whose data resolves late, then one whose data is ready: the later
  // call supersedes the earlier, whatever resolves first (LLP 1133.006 D2).
  trayRace: async (_args, _store, storage) => {
    await folder(storage);
    await storage.fs.writeFile('app:/tmp/tray/late.txt', new Uint8Array([65]));
    const late = storage.fs.readFile('app:/tmp/tray/late.txt').then(() => 'A');
    const a = outcome(put([new TrayItem({ 'text/plain': late })]));
    const b = outcome(put([new TrayItem({ 'text/plain': 'B' })]));
    return { said: `${await a} ${await b}` };
  },
  // More bytes than a call carries: `DataError`, naming the path form.
  trayBig: async () => ({ said: await outcome(put([new TrayItem({ 'application/octet-stream': new Uint8Array(3 * 1024 * 1024) })])) }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
