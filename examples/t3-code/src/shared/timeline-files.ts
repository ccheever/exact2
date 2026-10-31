// Pierre file-icon resolution (@pierre/trees createFileTreeIconResolver, Apache-2.0, with T3's
// T3_PIERRE_ICONS overrides, MIT; see LICENSE-T3): a path or fence language to the icon token that
// timeline-files.contract draws (T3 Code 9c161462c9). Pierre's own tables are generated (timeline-files-tables.ts,
// tools/generate/gen-icons.mjs); T3's are below.

import { BY_EXTENSION, BY_NAME, COMPLETE } from './timeline-files-tables';

// LANGUAGE_EXTENSION_ALIASES (apps/web/src/pierre-icons.ts): a fence language's file extension.
const LANGUAGE_EXTENSION: Record<string, string> = {"bash":"sh","csharp":"cs","elixir":"ex","haskell":"hs","javascript":"js","jsx":"jsx","kotlin":"kt","markdown":"md","mdx":"mdx","plaintext":"txt","powershell":"ps1","python":"py","ruby":"rb","rust":"rs","shell":"sh","shellscript":"sh","swift":"swift","typescript":"ts","tsx":"tsx","yaml":"yml"};
// T3_PIERRE_ICONS remaps: the icon's shape without a colour token, so they draw in the default grey.
const T3_NAMES: Record<string, string> = {"package.json":"npm-plain","tsconfig.json":"typescript-plain","agents.md":"agents","pnpm-lock.yaml":"pnpm","pnpm-workspace.yaml":"pnpm"};

/** The icon token for a file path ("default" when no specific icon exists). */
export function fileIconToken(path: string): string {
  const base = (path.split('/').pop() ?? path).toLowerCase();
  const t3 = T3_NAMES[base];
  if (t3) return t3;
  const named = BY_NAME[base];
  if (named) return named;
  const parts = base.split('.');
  for (let index = 1; index < parts.length; index++) {
    const extension = parts.slice(index).join('.');
    const token = COMPLETE[extension] ?? BY_EXTENSION[extension];
    if (token) return token;
  }
  return 'default';
}
/** MarkdownCodeBlockTitleContent: a fence language draws its icon only when it has a specific one. */
export function languageIconToken(language: string): string {
  const lower = language.trim().toLowerCase();
  if (!lower) return '';
  const token = fileIconToken(lower === 'dockerfile' ? 'Dockerfile' : `file.${LANGUAGE_EXTENSION[lower] ?? lower}`);
  return token === 'default' ? '' : token;
}
