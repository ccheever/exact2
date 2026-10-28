// What a previewed file is (MIT reference, see LICENSE-T3: packages/shared/src/filePreview.ts with the parts of
// mediaPreview.ts and video.ts it reads, delimitedPreview.ts filePreviewDelimiter, markdownFrontmatter.ts, and
// components/files/filePreviewMode.ts isMarkdownPreviewFile and FilePreviewPanel.tsx's Mermaid and document tests;
// T3 Code 7c3b3b3605ef). A sent attachment is classified by its MIME type, then its name; a workspace file by its literal
// path, never a URL's query or fragment.
import { videoMimeType } from '../composer/composer-video';

export type FilePreviewKind = 'image' | 'video' | 'audio' | 'pdf' | 'html' | 'markdown' | 'mermaid' | 'font' | 'archive' | 'sqlite'
  | 'docx' | 'xlsx' | 'pptx' | 'odt' | 'ods' | 'odp' | 'rtf' | 'epub' | 'ipynb' | 'text' | 'unsupported';
type FileLike = { readonly name: string; readonly mimeType?: string };

const mimeOf = (file: FileLike) => (file.mimeType?.split(';', 1)[0] ?? '').trim().toLowerCase();

const IMAGE_MIME_TYPE_BY_EXTENSION = new Map([
  ['.avif', 'image/avif'], ['.bmp', 'image/bmp'], ['.heic', 'image/heic'], ['.heif', 'image/heif'], ['.gif', 'image/gif'], ['.ico', 'image/x-icon'],
  ['.jpeg', 'image/jpeg'], ['.jpg', 'image/jpeg'], ['.png', 'image/png'], ['.svg', 'image/svg+xml'], ['.tif', 'image/tiff'], ['.tiff', 'image/tiff'], ['.webp', 'image/webp'],
]);
const AUDIO_MIME_TYPE_BY_EXTENSION = new Map([
  ['.mp3', 'audio/mpeg'], ['.wav', 'audio/wav'], ['.ogg', 'audio/ogg'], ['.oga', 'audio/ogg'], ['.flac', 'audio/flac'], ['.aac', 'audio/aac'],
  ['.m4a', 'audio/mp4'], ['.opus', 'audio/ogg'], ['.aiff', 'audio/aiff'], ['.webm-audio', 'audio/webm'], ['.weba', 'audio/webm'],
]);
function audioMimeTypeFromExtension(extension: string): string | null {
  if (!/^\.[a-z0-9-]+$/i.test(extension)) return null;
  return AUDIO_MIME_TYPE_BY_EXTENSION.get(extension.toLowerCase()) ?? null;
}
function mediaMimeTypeFromExtension(extension: string): string | null {
  if (!/^\.[a-z0-9]+$/i.test(extension)) return null;
  return IMAGE_MIME_TYPE_BY_EXTENSION.get(extension.toLowerCase()) ?? videoMimeType({ name: `media${extension}`, mimeType: '' });
}
export function isWorkspaceAudioPreviewPath(path: string): boolean {
  const extensionIndex = path.lastIndexOf('.');
  return extensionIndex >= 0 && audioMimeTypeFromExtension(path.slice(extensionIndex)) !== null;
}
export const isWorkspaceFontPreviewPath = (path: string): boolean => /\.(ttf|otf|woff2?)$/i.test(path);

const BINARY_MIME_TYPES = new Map([
  ['.zip', 'application/zip'], ['.jar', 'application/java-archive'], ['.tar', 'application/x-tar'], ['.gz', 'application/gzip'], ['.tgz', 'application/gzip'],
  ['.sqlite', 'application/vnd.sqlite3'], ['.sqlite3', 'application/vnd.sqlite3'], ['.db', 'application/vnd.sqlite3'],
]);
const ARCHIVE_MIME_TYPES = ['application/zip', 'application/x-zip-compressed', 'application/java-archive', 'application/x-java-archive', 'application/x-tar', 'application/gzip', 'application/x-gzip', 'application/x-compressed-tar'];
export function binaryPreviewKind(file: FileLike): 'archive' | 'sqlite' | null {
  const mime = mimeOf(file);
  if (['application/vnd.sqlite3', 'application/x-sqlite3', 'application/x-sqlite'].includes(mime)) return 'sqlite';
  if (ARCHIVE_MIME_TYPES.includes(mime)) return 'archive';
  if (mime && mime !== 'application/octet-stream' && mime !== 'text/plain') return null;
  const inferred = BINARY_MIME_TYPES.get(file.name.slice(file.name.lastIndexOf('.')).toLowerCase());
  return inferred === 'application/vnd.sqlite3' ? 'sqlite' : inferred ? 'archive' : null;
}

export const FILE_TEXT_PREVIEW_MAX_BYTES = 1024 * 1024;

const OFFICE_MIME_TYPES = {
  docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  odt: 'application/vnd.oasis.opendocument.text',
  ods: 'application/vnd.oasis.opendocument.spreadsheet',
  odp: 'application/vnd.oasis.opendocument.presentation',
  rtf: 'application/rtf',
  epub: 'application/epub+zip',
  ipynb: 'application/x-ipynb+json',
} as const;
export type OfficePreviewKind = keyof typeof OFFICE_MIME_TYPES;
export function officePreviewKind(file: FileLike): OfficePreviewKind | null {
  const mime = mimeOf(file);
  if (mime === 'text/rtf') return 'rtf';
  for (const kind of Object.keys(OFFICE_MIME_TYPES) as OfficePreviewKind[]) {
    if (mime === OFFICE_MIME_TYPES[kind]) return kind;
    if ((!mime || mime === 'application/octet-stream' || mime === 'text/plain' || (kind === 'ipynb' && mime === 'application/json')) && file.name.toLowerCase().endsWith(`.${kind}`)) return kind;
  }
  return null;
}

const TEXT_EXTENSIONS = /^\.(txt|log|json|jsonc|jsonl|ndjson|yaml|yml|toml|ini|conf|config|env|csv|tsv|xml|css|scss|sass|less|js|jsx|mjs|cjs|ts|tsx|mts|cts|py|pyi|rb|go|rs|swift|kt|kts|java|c|h|cc|cpp|hpp|cs|php|sh|bash|zsh|fish|sql|graphql|gql|vue|svelte|r|lua|ex|exs|erl|hs|clj|dart|diff|patch|lock|properties|gradle)$/;
const TEXT_NAMES = /^(dockerfile|makefile|gemfile|rakefile|license|readme|\.gitignore|\.gitattributes|\.editorconfig|\.env)(\.|$)/i;

/** filePreviewKind: what an attachment's body draws, from its MIME type, then its name (`richDocuments` off keeps
 *  Mermaid, fonts and documents out, as a surface without their viewers asks). */
export function filePreviewKind(file: FileLike, options?: { readonly richDocuments?: boolean }): FilePreviewKind {
  const mime = mimeOf(file), name = file.name.toLowerCase(), extension = name.slice(name.lastIndexOf('.'));
  const generic = !mime || mime === 'application/octet-stream' || mime === 'text/plain';
  const richDocuments = options?.richDocuments !== false;
  const office = richDocuments ? officePreviewKind(file) : null;
  if (office) return office;
  const binary = binaryPreviewKind(file);
  if (binary) return binary;
  if (mime === 'application/pdf') return 'pdf';
  if (mime === 'text/html') return 'html';
  if (mime === 'text/markdown' || mime === 'text/x-markdown') return 'markdown';
  if (richDocuments && mime === 'text/vnd.mermaid') return 'mermaid';
  if (richDocuments && mime.startsWith('font/')) return 'font';
  // Linux's MIME database also assigns .otf to OpenDocument formula templates; the font check validates the bytes.
  if (richDocuments && extension === '.otf' && mime === 'application/vnd.oasis.opendocument.formula-template') return 'font';
  if (mime.startsWith('image/')) return 'image';
  if (mime.startsWith('video/')) return 'video';
  if (mime.startsWith('audio/')) return 'audio';
  if (generic) {
    if (extension === '.pdf') return 'pdf';
    if (/^\.html?$/.test(extension)) return 'html';
    if (/^\.(md|markdown|mdown|mkd|mdx)$/.test(extension)) return 'markdown';
    if (richDocuments && /^\.(mmd|mermaid)$/.test(extension)) return 'mermaid';
    if (richDocuments && isWorkspaceFontPreviewPath(name)) return 'font';
    if (richDocuments && extension === '.rtf') return 'text';
    const media = mediaMimeTypeFromExtension(extension);
    if (media?.startsWith('image/')) return 'image';
    if (media?.startsWith('video/')) return 'video';
    if (audioMimeTypeFromExtension(extension) !== null) return 'audio';
    if (TEXT_EXTENSIONS.test(extension) || TEXT_NAMES.test(name.split(/[\\/]/).pop() ?? '')) return 'text';
  }
  if (mime.startsWith('text/') || /^(application\/(json|.*\+json|xml|.*\+xml|javascript|x-javascript|yaml|x-yaml|toml|sql))$/.test(mime)) return 'text';
  return 'unsupported';
}

/** filePreviewDelimiter: a CSV or TSV table, by its MIME type or, for a generic one, its name. */
export function filePreviewDelimiter(file: FileLike): ',' | '\t' | null {
  const mime = mimeOf(file);
  if (mime === 'text/csv') return ',';
  if (mime === 'text/tab-separated-values') return '\t';
  if (mime && mime !== 'text/plain' && mime !== 'application/octet-stream') return null;
  return /\.csv$/i.test(file.name) ? ',' : /\.tsv$/i.test(file.name) ? '\t' : null;
}

/** isMarkdownPreviewFile: the Files paths that render as Markdown. */
export const isMarkdownPreviewFile = (path: string): boolean => /\.(?:md|mdx|markdown|mdown|mkd)$/i.test(path);
/** FilePreviewPanel's `isMermaid`: a diagram file. */
export const isMermaidPreviewFile = (path: string): boolean => /\.(mmd|mermaid)$/i.test(path);
/** FilePreviewPanel's `isDocument`: a file only a document viewer reads (an archive, a database, an Office file or a font). */
export const isWorkspaceDocumentPath = (path: string): boolean => binaryPreviewKind({ name: path }) !== null || officePreviewKind({ name: path }) !== null || isWorkspaceFontPreviewPath(path);

const LEADING_FRONTMATTER = /^\uFEFF?---[ \t]*(?:\r\n|\r|\n)(?:[\s\S]*?(?:\r\n|\r|\n))?(?:---|\.\.\.)[ \t]*(?=\r\n|\r|\n|$)/;
/**
 * maskMarkdownFrontmatter: a leading YAML frontmatter block blanked, so a rendered preview skips it instead of drawing a
 * rule and a setext heading. Each masked UTF-16 unit becomes a space and line breaks stay, so the task checkboxes'
 * offsets, which the data module counts in UTF-16 units, still name the file's own text.
 */
export function maskMarkdownFrontmatter(markdown: string): string {
  return markdown.replace(LEADING_FRONTMATTER, block => block.replace(/[^\r\n]/g, ' '));
}
