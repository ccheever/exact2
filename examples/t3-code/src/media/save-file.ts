// A file the window saves where the person chooses: Contract's saveFile (the host's Save panel, a sheet on the window,
// LLP 1069.010 D3) copies an app file out. The bytes of a server's media or attachment are fetched and written under
// app:/tmp/saves/ first; a draft's staged copy is saved as it is. The root asks for the save (app.contract saveNow)
// when the snapshot carries a new request. The desktop reference downloads through Electron's own Save dialog
// (apps/desktop/src/preview/Manager.ts: "Electron opens a native Save dialog for a download with no save path").

/** The app's own files: the storage Exact hands the data module. */
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
/** The storage the last answer handed over, or null before any (the build, a test that hands none). */
export function adoptedFiles(): AppFiles | null { return files; }
export function appFiles(): AppFiles {
  if (!files) throw new Error('The app\'s files are unavailable.');
  return files;
}

/** The directory Save writes fetched bytes under (app.ts grants). */
export const SAVES = 'app:/tmp/saves';

type Request = { from: string; name: string; nonce: number };
const requests = new WeakMap<object, { serial: number; pending: Request[] }>();
let written = 0;

/** A file name without path separators, else `fallback` (T3MediaActions.fileName's rule). */
export function saveName(raw: string | undefined, fallback: string): string {
  const cleaned = (raw ?? '').replace(/[/:]/g, '-').trim();
  return cleaned ? cleaned.slice(0, 200) : fallback;
}

/** Asks the window to save the app file `from` as `name`. */
export function requestSave(client: object, from: string, name: string): void {
  const queue = requests.get(client) ?? { serial: 0, pending: [] };
  if (queue.pending.length >= 32) throw new Error('Too many files are waiting to be saved.');
  queue.pending.push({ from, name, nonce: ++queue.serial }); requests.set(client, queue);
}

/** The hidden Save field acknowledges change or cancel before the next panel is requested. */
export function completeSave(client: object, nonce: number): void {
  const queue = requests.get(client);
  if (queue?.pending[0]?.nonce === nonce) queue.pending.shift();
}

/** The largest download Save takes: `fetch`'s maximum for a body written to a file (`exactBodyTo`). */
export const DOWNLOAD_BYTES = 1024 * 1024 * 1024;

/**
 * Downloads `url` under app:/tmp/saves/ and asks the window to save it as `name`. The host writes the reply's body to
 * the file (`fetch`'s `exactBodyTo`), so its bytes never enter JavaScript, as a browser's download does not. Throws on
 * a non-2xx reply; asks nothing when `current` says the request was let go meanwhile.
 */
export async function saveDownload(client: object, url: string, name: string, options: { init?: RequestInit; maxBytes?: number; current?: () => boolean } = {}): Promise<void> {
  const fs = appFiles().fs, directory = `${SAVES}/${++written}`, path = `${directory}/${name}`;
  await fs.mkdir(directory, { recursive: true });
  const reply = await fetch(url, { ...options.init, exactBodyTo: path, exactIndependentHttp: { maxResponseBytes: options.maxBytes ?? DOWNLOAD_BYTES } } as RequestInit);
  if (!reply.ok) throw new Error(`HTTP ${reply.status}`);
  if (options.current?.() !== false) requestSave(client, path, name);
}

/** Writes `bytes` under app:/tmp/saves/ and asks the window to save them as `name`. */
export async function saveBytes(client: object, bytes: Uint8Array, name: string): Promise<void> {
  const fs = appFiles().fs, directory = `${SAVES}/${++written}`;
  await fs.mkdir(directory, { recursive: true });
  await fs.writeFile(`${directory}/${name}`, bytes);
  requestSave(client, `${directory}/${name}`, name);
}

/** The snapshot's save request (shapes.contract SaveRequest): none, or the first file waiting for its panel. */
export function saveSnapshot(client: object): { saveRequest: Request } {
  return { saveRequest: requests.get(client)?.pending[0] ?? { from: '', name: '', nonce: 0 } };
}
