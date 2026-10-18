// A file the window saves where the person chooses: Contract's saveFile (the host's Save panel, a sheet on the window,
// LLP 1069.010 D3) copies an app file out. The bytes of a server's media or attachment are fetched and written under
// app:/tmp/saves/ first; a draft's staged copy is saved as it is. The root asks for the save (app.contract saveNow)
// when the snapshot carries a new request. The desktop reference downloads through Electron's own Save dialog
// (apps/desktop/src/preview/Manager.ts: "Electron opens a native Save dialog for a download with no save path").

/** The app's own files: the storage Exact hands the data module (the native module answers only the preferences). */
export interface AppFiles {
  fs: {
    readFile(path: string): Promise<ArrayBuffer>;
    writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
    mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  };
}
let files: AppFiles | null = null;
/** app.ts hands every answer's storage here. */
export function adoptAppFiles(storage: unknown): void {
  const fs = (storage as { fs?: Partial<AppFiles['fs']> } | null)?.fs;
  if (fs?.readFile && fs.writeFile && fs.mkdir) files = storage as AppFiles;
}
export function appFiles(): AppFiles {
  if (!files) throw new Error('The app\'s files are unavailable.');
  return files;
}

/** The directory Save writes fetched bytes under (app.ts grants). */
export const SAVES = 'app:/tmp/saves';

type Request = { from: string; name: string; nonce: number };
const requests = new WeakMap<object, Request>();
let written = 0;

/** A file name without path separators, else `fallback` (T3MediaActions.fileName's rule). */
export function saveName(raw: string | undefined, fallback: string): string {
  const cleaned = (raw ?? '').replace(/[/:]/g, '-').trim();
  return cleaned ? cleaned.slice(0, 200) : fallback;
}

/** Asks the window to save the app file `from` as `name`. */
export function requestSave(client: object, from: string, name: string): void {
  requests.set(client, { from, name, nonce: (requests.get(client)?.nonce ?? 0) + 1 });
}

/** Writes `bytes` under app:/tmp/saves/ and asks the window to save them as `name`. */
export async function saveBytes(client: object, bytes: Uint8Array, name: string): Promise<void> {
  const fs = appFiles().fs, directory = `${SAVES}/${++written}`;
  await fs.mkdir(directory, { recursive: true });
  await fs.writeFile(`${directory}/${name}`, bytes);
  requestSave(client, `${directory}/${name}`, name);
}

/** The snapshot's save request (shapes.contract SaveRequest): none, or the newest. */
export function saveSnapshot(client: object): { saveRequest: Request } {
  return { saveRequest: requests.get(client) ?? { from: '', name: '', nonce: 0 } };
}
