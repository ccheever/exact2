// Copied from T3 Code 2359643cff (MIT reference, see LICENSE-T3: apps/web/src/terminal-links.ts).
// Changes from the reference:
// - `splitFilePathPosition` / `formatFilePathPosition` come from `terminal-host/src/shims/markdown-links.ts` (client-runtime markdownLinks.ts).
// - `isMacPlatform` comes from `terminal-host/src/shims/utils.ts` (lib/utils.ts).
// - 74541bab17's Unicode path segments and quoted paths (`QUOTED_FILE_PATH_PATTERN`, with a position
//   after the closing quote), except that relative paths split only on `/`, as 1e2ecbd975's do:
//   backslash-separated relative paths are Windows-only.
import {
  formatFilePathPosition,
  splitFilePathPosition,
} from "../../terminal-host/src/shims/markdown-links";

import { isMacPlatform } from "../../terminal-host/src/shims/utils";

export type TerminalLinkKind = "url" | "path";

export interface TerminalLinkMatch {
  kind: TerminalLinkKind;
  text: string;
  start: number;
  end: number;
}

export interface TerminalBufferLineLike {
  readonly isWrapped?: boolean;
  translateToString(trimRight?: boolean): string;
}

export interface WrappedTerminalLinkLineSegment {
  bufferLineNumber: number;
  text: string;
  startIndex: number;
  endIndex: number;
}

export interface WrappedTerminalLinkLine {
  text: string;
  segments: ReadonlyArray<WrappedTerminalLinkLineSegment>;
}

const URL_PATTERN = /https?:\/\/[^\s"'`<>]+/giu;
// The default PowerShell prompt delimits its unquoted path with `PS ` and `>`.
const POWERSHELL_PROMPT_PATH_PATTERN = /^PS ((?:[A-Za-z]:[\\/]|\\\\|\/)[^>\r\n]*)>/u;
// Bare paths start only at a segment boundary: hover detection runs this on
// every frame, so separator-free output must not be rescanned at every suffix.
const FILE_PATH_PATTERN =
  /(?:~\/|\.{1,2}\/|\/|[A-Za-z]:[\\/]|\\\\)[^\s"'`<>]+|(?<![\p{L}\p{M}\p{N}._-])[\p{L}\p{M}\p{N}._-]+(?:\/[\p{L}\p{M}\p{N}._-]+)+(?::\d+){0,2}/gu;
const QUOTED_FILE_PATH_PATTERN =
  /(["'`])(?<path>(?:~\/|\.{1,2}\/|\/|[A-Za-z]:[\\/]|\\\\|[\p{L}\p{M}\p{N}._ -]+\/)[^\r\n]*?|[\p{L}\p{M}\p{N}._ -]+\.[\p{L}\p{M}\p{N}_-]+(?::\d+){0,2})\1(?<position>(?::\d+){0,2})/gu;
const TRAILING_PUNCTUATION_PATTERN = /[.,;!?]+$/;
// Paths also drop a trailing colon: compilers end `file:line:col:` with one.
const TRAILING_PATH_PUNCTUATION_PATTERN = /[.,;:!?]+$/;
// A question mark is legal in filenames; only exclude query-bearing request targets (74541bab17).
const REQUEST_ROUTE_PATTERN =
  /(?:^|[\s"'`])(?:GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS|CONNECT|TRACE|proxying)\s+["'`]?(?:\/|[A-Za-z0-9_-]+\/)[^\s?]*\?\S*/g;

function trimClosingDelimiters(value: string, kind: TerminalLinkKind): string {
  let output = value.replace(
    kind === "path" ? TRAILING_PATH_PUNCTUATION_PATTERN : TRAILING_PUNCTUATION_PATTERN,
    "",
  );
  if (output.length === 0) return output;

  const trimUnbalanced = (open: string, close: string) => {
    while (output.endsWith(close)) {
      const opens = output.split(open).length - 1;
      const closes = output.split(close).length - 1;
      if (opens >= closes) return;
      output = output.slice(0, -1);
    }
  };

  trimUnbalanced("(", ")");
  trimUnbalanced("[", "]");
  trimUnbalanced("{", "}");
  return output;
}

function overlaps(a: { start: number; end: number }, b: { start: number; end: number }): boolean {
  return a.start < b.end && b.start < a.end;
}

function collectMatches(
  line: string,
  kind: TerminalLinkKind,
  pattern: RegExp,
  existing: TerminalLinkMatch[],
): TerminalLinkMatch[] {
  const matches: TerminalLinkMatch[] = [];
  pattern.lastIndex = 0;

  for (const rawMatch of line.matchAll(pattern)) {
    const quotedPath = rawMatch.groups?.path;
    const raw =
      quotedPath === undefined ? rawMatch[0] : quotedPath + (rawMatch.groups?.position ?? "");
    const start = (rawMatch.index ?? -1) + (quotedPath === undefined ? 0 : 1);
    if (start < 0 || raw.length === 0) continue;

    const trimmed = quotedPath === undefined ? trimClosingDelimiters(raw, kind) : raw;
    if (trimmed.length === 0) continue;
    if (kind === "path" && isTerminalUrl(trimmed)) continue;

    const candidate: TerminalLinkMatch = {
      kind,
      text: trimmed,
      start,
      // A position outside quotes spans the closing quote in the terminal buffer.
      end: start + trimmed.length + (rawMatch.groups?.position ? 1 : 0),
    };

    const collides = [...existing, ...matches].some((other) => overlaps(candidate, other));
    if (collides) continue;

    matches.push(candidate);
  }

  return matches;
}

function isWindowsAbsolutePath(value: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\");
}

export function isAbsolutePath(value: string): boolean {
  return value.startsWith("/") || isWindowsAbsolutePath(value);
}

function isWindowsPathStyle(value: string): boolean {
  return isWindowsAbsolutePath(value) || /[A-Za-z]:\\/.test(value);
}

function joinPath(base: string, next: string, separator: "/" | "\\"): string {
  const cleanBase = base.replace(/[\\/]+$/, "");
  if (separator === "\\") {
    return `${cleanBase}\\${next.replaceAll("/", "\\")}`;
  }
  return `${cleanBase}/${next.replace(/^\/+/, "")}`;
}

function inferHomeFromCwd(cwd: string): string | undefined {
  const posixUser = cwd.match(/^\/Users\/([^/]+)/);
  if (posixUser?.[1]) {
    return `/Users/${posixUser[1]}`;
  }

  const posixHome = cwd.match(/^\/home\/([^/]+)/);
  if (posixHome?.[1]) {
    return `/home/${posixHome[1]}`;
  }

  const windowsUser = cwd.match(/^([A-Za-z]:\\Users\\[^\\]+)/);
  if (windowsUser?.[1]) {
    return windowsUser[1];
  }

  return undefined;
}

export function extractTerminalLinks(line: string): TerminalLinkMatch[] {
  const promptPath = POWERSHELL_PROMPT_PATH_PATTERN.exec(line)?.[1];
  const promptMatches: TerminalLinkMatch[] = promptPath
    ? [{ kind: "path", text: promptPath, start: 3, end: 3 + promptPath.length }]
    : [];
  const requestRoutes = Array.from(line.matchAll(REQUEST_ROUTE_PATTERN), (match) => ({
    start: match.index,
    end: match.index + match[0].length,
  }));
  const urlMatches = collectMatches(line, "url", URL_PATTERN, promptMatches);
  const quotedMatches = collectMatches(line, "path", QUOTED_FILE_PATH_PATTERN, [
    ...promptMatches,
    ...urlMatches,
  ]);
  const pathMatches = collectMatches(line, "path", FILE_PATH_PATTERN, [
    ...promptMatches,
    ...urlMatches,
    ...quotedMatches,
  ]);
  return [...promptMatches, ...urlMatches, ...quotedMatches, ...pathMatches]
    .filter(
      (match) => match.kind === "url" || !requestRoutes.some((route) => overlaps(match, route)),
    )
    .toSorted((a, b) => a.start - b.start);
}

export function isTerminalUrl(value: string): boolean {
  return /^https?:\/\//iu.test(value);
}

export function collectWrappedTerminalLinkLine(
  bufferLineNumber: number,
  getLine: (bufferLineIndex: number) => TerminalBufferLineLike | null | undefined,
): WrappedTerminalLinkLine | null {
  const anchorLine = getLine(bufferLineNumber - 1);
  if (!anchorLine) return null;

  let startBufferLineNumber = bufferLineNumber;
  let startLine = anchorLine;

  while (startBufferLineNumber > 1 && startLine.isWrapped) {
    const previousLine = getLine(startBufferLineNumber - 2);
    if (!previousLine) return null;
    startBufferLineNumber -= 1;
    startLine = previousLine;
  }

  const segments: WrappedTerminalLinkLineSegment[] = [];
  let nextStartIndex = 0;
  let currentBufferLineNumber = startBufferLineNumber;

  while (true) {
    const currentLine = getLine(currentBufferLineNumber - 1);
    if (!currentLine) break;

    const nextLine = getLine(currentBufferLineNumber);
    const hasWrappedContinuation = nextLine?.isWrapped === true;
    const text = currentLine.translateToString(!hasWrappedContinuation);

    segments.push({
      bufferLineNumber: currentBufferLineNumber,
      text,
      startIndex: nextStartIndex,
      endIndex: nextStartIndex + text.length,
    });
    nextStartIndex += text.length;

    if (!hasWrappedContinuation) break;
    currentBufferLineNumber += 1;
  }

  return {
    text: segments.map((segment) => segment.text).join(""),
    segments,
  };
}

export function isTerminalLinkActivation(
  event: { readonly metaKey: boolean; readonly ctrlKey: boolean },
  platform = typeof navigator === "undefined" ? "" : navigator.platform,
): boolean {
  if (platform.length === 0) return false;
  return isMacPlatform(platform)
    ? event.metaKey && !event.ctrlKey
    : event.ctrlKey && !event.metaKey;
}

export function resolvePathLinkTarget(rawPath: string, cwd: string): string {
  const position = splitFilePathPosition(rawPath);
  const { path } = position;

  let resolvedPath = path;
  if (path.startsWith("~/")) {
    const home = inferHomeFromCwd(cwd);
    if (home) {
      const separator: "/" | "\\" = isWindowsPathStyle(home) ? "\\" : "/";
      resolvedPath = joinPath(home, path.slice(2), separator);
    }
  } else if (!isAbsolutePath(path)) {
    const separator: "/" | "\\" = isWindowsPathStyle(cwd) ? "\\" : "/";
    resolvedPath = joinPath(cwd, path, separator);
  }

  return formatFilePathPosition({ ...position, path: resolvedPath });
}
