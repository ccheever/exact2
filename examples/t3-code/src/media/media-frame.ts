// The previews' PDF and rendered-page bodies (MIT reference, see LICENSE-T3: components/files/BrowserDocumentFrame.tsx)
// as Contract iframes (media-frame.contract, LLP 1020.002). A PDF frame shows a local file in the platform's viewer
// (D10: PDFKit's PDFView on Apple), so a server's signed PDF is downloaded into app:/cache first (the host writes the
// body, `fetch`'s `exactBodyTo`), and a draft's staged copy, which has no extension, is copied there under a `.pdf`
// name, by which the host knows its type. A sent page is
// framed at its signed asset URL, as the reference frames it; a draft page is its staged copy under an `.html` name,
// which the host serves as the frame's own document with no siblings, as a Blob URL's page has none.
import { appFiles } from './save-file';
import { STAGED } from '../composer/composer-staged-copies';

/** Where fetched PDFs and copied draft documents are kept while they are shown (app.ts grants). */
export const MEDIA_CACHE = 'app:/cache/media-preview';
/** The prefix a draft's PDF or page URL carries (composer-chip-file.ts draftSource): its staged copy's name follows. */
export const STAGED_SCHEME = 't3-staged:';
/** The largest PDF downloaded for a frame: `fetch`'s maximum for a body written to a file, which goes to disk. */
const MAX_BYTES = 1024 * 1024 * 1024;
/** Documents kept at once; the oldest file goes when another arrives. */
const KEPT = 16;
const FAILED = 'Could not load this file.';

interface FrameFiles {
  copyFile(from: string, to: string): Promise<unknown>;
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
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

async function fetched(url: string, path: string): Promise<void> {
  const reply = await fetch(url, { exactTimeout: 60_000, exactBodyTo: path, exactIndependentHttp: { maxResponseBytes: MAX_BYTES } } as RequestInit);
  if (!reply.ok) throw new Error(`HTTP ${reply.status}`);
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
 * The `src` of a preview's frame for `kind` ("pdf" or "html") at `url` (a signed asset URL, or a draft's
 * `t3-staged:<name>`), or the message the surface shows when there is none.
 */
export async function mediaFrameSource(kind: string, url: string): Promise<FrameSource> {
  if (!url || (kind !== 'pdf' && kind !== 'html')) return { src: '', error: '' };
  const staged = stagedCopyName(url);
  if (!staged && !isAssetUrl(url)) return { src: '', error: FAILED };
  if (kind === 'html' && !staged) return { src: url, error: '' };
  try {
    return { src: await local(url, kind, path => staged ? copied(staged, path) : fetched(url, path)), error: '' };
  } catch {
    return { src: '', error: FAILED };
  }
}

/** Forgets the kept documents (tests). */
export function forgetMediaFrames(): void { kept.clear(); }
