import { T3Client } from './client';
import { snapshot } from './presentation';
import { nativeFiles, type Native, type Files } from './protocol';

export const appId = 'com.exact.t3code.macos';
export const grants = '';
const client = new T3Client();

// The disconnected answer is baked. Native work starts only when Exact reports
// that the module is available, after its ordinary first-frame adoption.
export async function answer(source: string, args: unknown[], _store: unknown, _storage: Files, native: Native | null | undefined) {
  const storage = native?.available ? nativeFiles(native) : _storage;
  if (source === 'snapshot') {
    await client.refresh(native, storage);
    return snapshot(client, Number(args[0]) || 0);
  }
  if (source === 'command') return client.command(String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), Number(args[3]) || 0, native, storage);
  throw new Error(`Unknown T3 source: ${source}`);
}
