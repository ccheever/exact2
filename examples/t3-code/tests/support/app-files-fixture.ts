// The app's storage (Exact's `storage.fs`) in memory, for the attachment staging tests (not a test file). It refuses
// what the host refuses: copyFile and compressImage from a `doc:` path. compressImage "encodes" a 4×3 PNG, or fails
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
export const text = (value: string) => new TextEncoder().encode(value);

const failure = (code: string) => Object.assign(new Error(code), { code });

export function memoryFiles(entries: Record<string, Uint8Array | 'folder'> = {}) {
  const store = new Map<string, Uint8Array>(), folders = new Set<string>(), compressed: Obj[] = [];
  for (const [path, value] of Object.entries(entries)) if (value === 'folder') folders.add(path); else store.set(path, value);
  const read = (path: string) => { const bytes = store.get(path); if (!bytes) throw failure('ENOENT'); return bytes; };
  const fs = {
    async readFile(path: string) { return read(path).slice().buffer; },
    async writeFile(path: string, bytes: Uint8Array) { store.set(path, new Uint8Array(bytes)); },
    async atomicWriteFile(path: string, bytes: Uint8Array) { store.set(path, new Uint8Array(bytes)); },
    async mkdir(path: string) { folders.add(path); },
    async rm(path: string) { if (!store.delete(path) && !folders.delete(path)) throw failure('ENOENT'); },
    async stat(path: string) {
      if (folders.has(path)) return { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };
      return { size: read(path).byteLength, isFile: true, isDirectory: false, modifiedMs: 0 };
    },
    async copyFile(from: string, to: string) { if (from.startsWith('doc:')) throw failure('failed'); store.set(to, read(from)); },
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
  return { fs, store, folders, compressed };
}
