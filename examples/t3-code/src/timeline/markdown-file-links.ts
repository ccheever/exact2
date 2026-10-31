// Which chat link destinations are files: packages/shared/src/markdownLinks.ts parseMarkdownFileLink and
// resolveMarkdownFileLinkTarget's network-path rule, with fileLinks.ts parseFileUrlHref, at T3 Code 7c3b3b3605ef
// (MIT, see LICENSE-T3). data/src/markdown_links.rs `chat_file_link` is the same rule for the parsed runs: a
// network path (`\\h`, `/\h`, `file://h/`) is a file link only inside the project's own share, which the parse
// does not know, so both leave it an ordinary link.

const RELATIVE_PATH_PREFIX = /^(~\/|\.{1,2}\/)/;
// Names may use any script (`線性代數/考試.pdf`); a folder link may end in `/` (`patches/`).
const RELATIVE_FILE_PATH = /^(?:[\p{L}\p{M}\p{N}._-]+(?: +[\p{L}\p{M}\p{N}._-]+)*\/)+(?:[\p{L}\p{M}\p{N}._-]+(?: +[\p{L}\p{M}\p{N}._-]+)*(?::\d+){0,2})?$/u;
const RELATIVE_FILE_NAME = /^[\p{L}\p{M}\p{N}._-]+(?: +[\p{L}\p{M}\p{N}._-]+)*\.[\p{L}\p{M}\p{N}_-]+(?::\d+){0,2}$/u;
const EXTERNAL_SCHEME = /^([A-Za-z][A-Za-z0-9+.-]*):(.*)$/;
const POSITION_SUFFIX = /:\d+(?::\d+)?$/;
const POSITION_ONLY = /^\d+(?::\d+)?$/;
const FILE_EXTENSION = /\.[A-Za-z0-9_-]+$/;
const POSIX_FILE_ROOT_PREFIXES = ['/Users/', '/home/', '/tmp/', '/var/', '/etc/', '/opt/', '/mnt/', '/Volumes/', '/private/', '/root/', '/usr/',
  '/bin/', '/sbin/', '/lib/', '/lib64/', '/srv/', '/dev/', '/proc/', '/sys/', '/run/', '/boot/', '/media/', '/workspace/', '/workspaces/'];
const EXTENSIONLESS_FILE_NAMES = new Set(['Makefile', 'makefile', 'GNUmakefile', 'Dockerfile', 'Containerfile', 'Justfile', 'justfile', 'Rakefile',
  'Gemfile', 'Procfile', 'Brewfile', 'Caddyfile', 'Vagrantfile', 'Jenkinsfile', 'Podfile', 'Fastfile', 'BUILD', 'WORKSPACE', 'LICENSE', 'LICENCE',
  'COPYING', 'NOTICE', 'AUTHORS', 'CONTRIBUTORS', 'CHANGELOG', 'README', 'CODEOWNERS']);

const windowsAbsolute = (path: string) => path.startsWith('\\\\') || /^[a-zA-Z]:([/\\]|$)/.test(path);
const stripSlashDrive = (path: string) => /^\/[A-Za-z]:[\\/]/.test(path) ? path.slice(1) : path;
function safeDecode(value: string): string {
  try { return decodeURIComponent(value); } catch { return value; }
}

function looksLikeFilePath(path: string, authored: string): boolean {
  if (windowsAbsolute(path) || RELATIVE_PATH_PREFIX.test(path)) return true;
  if (path.startsWith('/')) {
    if (POSIX_FILE_ROOT_PREFIXES.some(prefix => authored.startsWith(prefix)) || POSITION_SUFFIX.test(authored)) return true;
    const basename = authored.slice(authored.lastIndexOf('/') + 1);
    return EXTENSIONLESS_FILE_NAMES.has(basename) || FILE_EXTENSION.test(basename);
  }
  return EXTENSIONLESS_FILE_NAMES.has(path) || RELATIVE_FILE_PATH.test(authored) || RELATIVE_FILE_NAME.test(path);
}
function hasExternalScheme(path: string): boolean {
  if (windowsAbsolute(path)) return false;
  const match = EXTERNAL_SCHEME.exec(path);
  if (!match) return false;
  const rest = match[2] ?? '';
  return rest.startsWith('//') || !POSITION_ONLY.test(rest);
}
/** parseFileUrlHref's path: a non-localhost authority becomes a UNC share. */
function fileUrlPath(href: string): string | null {
  try {
    const url = new URL(href);
    if (url.protocol.toLowerCase() !== 'file:') return null;
    const host = url.hostname.toLowerCase() === 'localhost' ? '' : url.hostname;
    const path = host ? `\\\\${host}${url.pathname.replaceAll('/', '\\')}` : url.pathname;
    return path ? stripSlashDrive(path) : null;
  } catch { return null; }
}

/** The file a destination names without its position (splitFilePathPosition), or null when it names none. */
export function markdownFileLinkPath(href: string): string | null {
  const trimmed = href.trim();
  const normalized = trimmed.startsWith('<') && trimmed.endsWith('>') ? trimmed.slice(1, -1) : trimmed;
  if (!normalized || normalized.startsWith('#') || normalized.startsWith('//')) return null;
  const raw = (normalized.toLowerCase().startsWith('file:') ? fileUrlPath(normalized) : null) ?? normalized.split('#')[0]!.split('?')[0]!;
  const path = stripSlashDrive(safeDecode(raw.trim()));
  if (!path || hasExternalScheme(path)) return null;
  const file = path.replace(POSITION_SUFFIX, '');
  return looksLikeFilePath(file, path) ? file : null;
}

/** A link the chat draws as a file chip: file-shaped, and no network path. */
export function chatFileLink(href: string): boolean {
  const path = markdownFileLinkPath(href);
  return path !== null && !/^[\\/]{2}/.test(path);
}
