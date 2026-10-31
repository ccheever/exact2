// The app's storage (Exact's `storage.fs`) in memory, for the attachment staging tests (not a test file): `released`
// names the staged copies removed. It refuses
// what the host refuses: copyFile from a `doc:` path to anything but an `app:` one, and compressImage from a `doc:` path. compressImage "encodes" a 4×3 PNG, or fails
// `undecodable` for bytes that start with "X" (no image).
import type { Obj } from '../../src/client/domain';
import { adoptAppFiles } from '../../src/media/save-file';

/** A PNG's signature and IHDR at `width`×`height`, then `extra` bytes. */
export function png(width: number, height: number, extra = 0): Uint8Array {
  const bytes = new Uint8Array(33 + extra);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, width); view.setUint32(20, height);
  return bytes;
}
/** A JPEG's SOI and SOF0 at `width`×`height`. */
export function jpeg(width: number, height: number): Uint8Array {
  return new Uint8Array([0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, height >> 8, height & 255, width >> 8, width & 255, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1, 0xff, 0xd9]);
}
/** A GIF's header at `width`×`height`, then `extra` bytes. */
export function gif(width: number, height: number, extra = 0): Uint8Array {
  const bytes = new Uint8Array(13 + extra);
  bytes.set([...'GIF89a'].map(char => char.charCodeAt(0)));
  bytes.set([width & 255, width >> 8, height & 255, height >> 8], 6);
  return bytes;
}
/** A WebP's RIFF header and VP8X chunk at `width`×`height` (opaque unless `alpha`), then `extra` bytes. */
export function webp(width: number, height: number, alpha = false, extra = 0): Uint8Array {
  const bytes = new Uint8Array(30 + extra);
  bytes.set([...'RIFF'].map(char => char.charCodeAt(0)));
  bytes.set([...'WEBPVP8X'].map(char => char.charCodeAt(0)), 8);
  bytes[20] = alpha ? 0x10 : 0;
  const w = width - 1, h = height - 1;
  bytes.set([w & 255, (w >> 8) & 255, w >> 16, h & 255, (h >> 8) & 255, h >> 16], 24);
  return bytes;
}
export const text = (value: string) => new TextEncoder().encode(value);

const failure = (code: string) => Object.assign(new Error(code), { code });

export function memoryFiles(entries: Record<string, Uint8Array | 'folder'> = {}) {
  const store = new Map<string, Uint8Array>(), folders = new Set<string>(), compressed: Obj[] = [], removed: string[] = [];
  for (const [path, value] of Object.entries(entries)) if (value === 'folder') folders.add(path); else store.set(path, value);
  const read = (path: string) => { const bytes = store.get(path); if (!bytes) throw failure('ENOENT'); return bytes; };
  const fs = {
    async readFile(path: string) { return read(path).slice().buffer; },
    async writeFile(path: string, bytes: Uint8Array) { store.set(path, new Uint8Array(bytes)); },
    async atomicWriteFile(path: string, bytes: Uint8Array) { store.set(path, new Uint8Array(bytes)); },
    async mkdir(path: string) { folders.add(path); },
    async rm(path: string) { if (!store.delete(path) && !folders.delete(path)) throw failure('ENOENT'); removed.push(path); },
    async readdir(path: string) { return [...store.keys()].filter(key => key.startsWith(`${path}/`) && !key.slice(path.length + 1).includes('/')).map(key => key.slice(path.length + 1)); },
    async stat(path: string) {
      if (folders.has(path)) return { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };
      return { size: read(path).byteLength, isFile: true, isDirectory: false, modifiedMs: 0 };
    },
    async copyFile(from: string, to: string) { if (from.startsWith('doc:') && !to.startsWith('app:/')) throw failure('failed'); store.set(to, read(from)); },
    async compressImage(from: string, to: string, options: Obj) {
      compressed.push({ from, to, ...options });
      if (!from.startsWith('app:/')) throw failure('failed');
      if (read(from)[0] === 0x58) throw failure('undecodable');
      const out = png(4, 3);
      store.set(to, out);
      return { path: to, type: options.type ?? 'image/jpeg', size: out.byteLength, width: 4, height: 3 };
    },
  };
  adoptAppFiles({ fs });
  /** The staged copies (composer-files/<id>) removed, by id. */
  const released = () => removed.filter(path => /^app:\/data\/composer-files\/[0-9a-f-]{36}$/.test(path)).map(path => path.slice(path.lastIndexOf('/') + 1));
  return { fs, store, folders, compressed, removed, released };
}
