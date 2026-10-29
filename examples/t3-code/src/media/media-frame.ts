// The previews' rendered-page bodies (MIT reference, see LICENSE-T3: components/files/BrowserDocumentFrame.tsx) as
// Contract iframes (media-frame.contract, LLP 1020.002), and the copies in app:/cache that frames show. A sent page is
// framed at its signed asset URL, as the reference frames it; a draft page is its staged copy under an `.html` name,
// which the host serves as the frame's own document with no siblings, as a Blob URL's page has none. A draft past what a
// message carries is copied there for the viewer page, which is given it (LLP 1020.001 D7). PDFs are the viewer page's.
// A draft's copies go with the draft (composer-staged-copies.ts removeStaged) and when its preview closes; what an
// earlier launch left is swept before this launch's first copy.
import { appFiles } from './save-file';
import { STAGED } from '../composer/composer-staged-copies';

/** Where copied draft documents are kept while they are shown (app.ts grants). */
export const MEDIA_CACHE = 'app:/cache/media-preview';
/** The prefix a draft's file URL carries (composer-chip-file.ts draftSource): its staged copy's name follows. */
export const STAGED_SCHEME = 't3-staged:';
/** Documents kept at once; the oldest file goes when another arrives. */
const KEPT = 16;
const FAILED = 'Could not load this file.';

interface FrameFiles {
  copyFile(from: string, to: string): Promise<unknown>;
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  readdir?(path: string): Promise<string[]>;
  rm?(path: string): Promise<unknown>;
}
const files = (): FrameFiles => (appFiles() as unknown as { fs: FrameFiles }).fs;

export type FrameSource = { src: string; error: string };

/** Only a signed asset route of an http(s) server: never a file URL, credentials or another path. */
export function isAssetUrl(raw: string): boolean {
  try {
    const url = new URL(raw);
    return (url.protocol === 'http:' || url.protocol === 'https:') && !!url.hostname && !url.username && !url.password && url.pathname.startsWith('/api/assets/');
  } catch { return false; }
}

/** A staged copy's name (`<uuid>`, composer-staged-copies.ts stagedName), else null. */
export function stagedCopyName(raw: string): string | null {
  if (!raw.startsWith(STAGED_SCHEME)) return null;
  const name = raw.slice(STAGED_SCHEME.length).toLowerCase();
  return /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(name) ? name : null;
}

const kept = new Map<string, Promise<string>>();
let serial = 0;
/** The numbered copies left in app:/cache by an earlier launch, whose numbers this one's start over: removed once. */
let swept: Promise<void> | null = null;
async function sweep(): Promise<void> {
  const fs = files(), names = await fs.readdir?.(MEDIA_CACHE).catch(() => [] as string[]) ?? [];
  await Promise.all(names.filter(name => /^\d+\.[a-z0-9]{1,10}$/.test(name)).map(name => fs.rm?.(`${MEDIA_CACHE}/${name}`)?.catch(() => undefined)));
}

async function copied(name: string, path: string): Promise<void> {
  await files().copyFile(`${STAGED}/${name}`, path);
}

/** The local file for `key`, written once by `write`; a failure is not kept, so Try again writes it afresh. */
function local(key: string, extension: string, write: (path: string) => Promise<void>): Promise<string> {
  const known = kept.get(key);
  if (known) return known;
  const path = `${MEDIA_CACHE}/${++serial}.${extension}`;
  const made = (async () => {
    await (swept ??= sweep());
    await files().mkdir(MEDIA_CACHE, { recursive: true }).catch(() => undefined);
    await write(path);
    return path;
  })();
  kept.set(key, made);
  made.catch(() => { if (kept.get(key) === made) kept.delete(key); });
  while (kept.size > KEPT) {
    const [oldest, gone] = kept.entries().next().value!;
    kept.delete(oldest);
    void gone.then(file => files().rm?.(file)).catch(() => undefined);
  }
  return made;
}

/**
 * A draft's staged copy under app:/cache, where a bundled page can be given it (LLP 1020.001 D7; app:/data, where the
 * composer keeps it, is never given), named with `name`'s extension.
 */
export function cachedStagedCopy(staged: string, name: string): Promise<string> {
  const extension = /\.([a-z0-9]{1,10})$/i.exec(name)?.[1]?.toLowerCase() ?? 'bin';
  return local(`given:${staged}`, extension, path => copied(staged, path));
}

/**
 * The `src` of a rendered page's frame at `url` (a signed asset URL, or a draft's `t3-staged:<name>`), or the message the
 * surface shows when there is none.
 */
export async function mediaFrameSource(url: string): Promise<FrameSource> {
  if (!url) return { src: '', error: '' };
  const staged = stagedCopyName(url);
  if (!staged && !isAssetUrl(url)) return { src: '', error: FAILED };
  if (!staged) return { src: url, error: '' };
  try {
    return { src: await local(`${STAGED_SCHEME}${staged}`, 'html', path => copied(staged, path)), error: '' };
  } catch {
    return { src: '', error: FAILED };
  }
}

/**
 * Deletes a draft's copies, its page's and the file its viewer page was given: the draft was removed or sent, its
 * preview closed, or Try again asks for them afresh (a copy can be gone from the cache while its path is kept).
 */
export function forgetStagedCopies(staged: string): void {
  for (const key of [`${STAGED_SCHEME}${staged}`, `given:${staged}`]) {
    const copy = kept.get(key);
    if (!copy) continue;
    kept.delete(key);
    void copy.then(path => files().rm?.(path)).catch(() => undefined);
  }
}

/** Forgets the kept documents and the sweep (tests). */
export function forgetMediaFrames(): void { kept.clear(); swept = null; }
