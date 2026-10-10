// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/terminal-host/src/shims/markdown-links.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The two file-position helpers terminal-links.ts needs from T3 Code's client-runtime
// (MIT reference, see LICENSE-T3: packages/client-runtime/src/markdownLinks.ts:11-12,243-266),
// unchanged.
const POSITION_SUFFIX_CAPTURE_PATTERN = /:(\d+)(?::(\d+))?$/;
const POSITION_HASH_PATTERN = /^#L(\d+)(?:C(\d+))?$/i;

export interface FilePathPosition {
  readonly path: string;
  readonly line?: number;
  readonly column?: number;
}

export function splitFilePathPosition(path: string, hash = ""): FilePathPosition {
  const suffixMatch = path.match(POSITION_SUFFIX_CAPTURE_PATTERN);
  const match = suffixMatch ?? hash.match(POSITION_HASH_PATTERN);
  if (!match?.[1]) return { path };

  const line = Number.parseInt(match[1], 10);
  const column = match[2] === undefined ? undefined : Number.parseInt(match[2], 10);
  return {
    path: suffixMatch ? path.slice(0, -suffixMatch[0].length) : path,
    ...(line > 0 ? { line } : {}),
    ...(column !== undefined && column > 0 ? { column } : {}),
  };
}

export function formatFilePathPosition(position: FilePathPosition): string {
  if (!position.line) return position.path;
  return `${position.path}:${position.line}${position.column ? `:${position.column}` : ""}`;
}
