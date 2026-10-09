// Rich composer projection, adapted from T3 Code (MIT); see LICENSE-T3.
// Source365aa87982 composerInlineTokens/contextReferences, nativeMarkdownText,
// ComposerEditor.tsx and T3ComposerEditor.native.tsx. Source helper bodies retained.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { formatAttachmentSize } from './shared/composer-editor-files';
import { videoMimeType } from './shared/r4-composer-video';
import type { Obj } from './shared/domain';
import type { MobileMessageContext } from './mobile-new-task-context';
const COMPOSER_CONTEXT_LABEL_MAX_CHARS = 200;
type ComposerContextKind = string;
type ComposerContextId = string;

export type ComposerInlineToken =
  | {
      readonly type: "mention";
      readonly value: string;
      readonly source: string;
      readonly start: number;
      readonly end: number;
    }
  | {
      readonly type: "skill";
      readonly value: string;
      readonly source: string;
      readonly start: number;
      readonly end: number;
    };

export interface CollectComposerInlineTokensOptions {
  readonly preserveTrailingFrom?: ReadonlyArray<ComposerInlineToken>;
}

/**
 * A skill name may start with a digit, but compact monetary amounts and
 * numeric expressions like "$20", "$20k", "$100M", and "$1e6" must stay prose:
 * the composer chips any matched `$name` token, known or not. Tokens beginning
 * with digits must not match numbers with currency/exponent suffixes, and must
 * contain at least one letter. Any currency symbol is accepted as the sigil.
 */
const SKILL_MENTION_SOURCE =
  /(^|\s)\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)/u
    .source;
// While typing, a token only becomes a chip once a delimiter follows it, so a
// half-typed name at the end of the text stays plain.
const SKILL_TOKEN_REGEX = new RegExp(`${SKILL_MENTION_SOURCE}(?=\\s)`, "gu");
/**
 * Skill mentions in a sent prompt, which may also end at the end of the text.
 * Group 1 is the leading delimiter and group 2 the skill name. The pattern is
 * global, so use it with `matchAll` or `replace`, not `test` or `exec`.
 */
export const SKILL_MENTION_PATTERN = new RegExp(`${SKILL_MENTION_SOURCE}(?=\\s|$)`, "gu");
const MENTION_TOKEN_REGEX = /(^|\s)@(?:"((?:\\.|[^"\\])*)"|([^\s@"]+))(?=\s)/g;
/**
 * The label body is bounded rather than `*`. Unbounded, every whitespace in
 * the composer is a candidate start: the engine scans the rest of the text for
 * a closing `]`, fails, and rescans from the next whitespace — quadratic on
 * input like " [[[[[…". A cap makes each attempt constant-bounded.
 *
 * Only a basename ever survives the `label !== basename` check below, so this
 * cannot reject a link a user could meaningfully write; the longest filename
 * any common filesystem allows is 255.
 */
const MAX_FILE_LINK_LABEL_LENGTH = 512;
const FILE_LINK_TOKEN_REGEX = new RegExp(
  `(^|\\s)\\[((?:\\\\.|[^\\]\\\\]){0,${MAX_FILE_LINK_LABEL_LENGTH}})\\]\\(([^)\\s]+)\\)(?=\\s)`,
  "g",
);
const URI_SCHEME_REGEX = /^[A-Za-z][A-Za-z0-9+.-]*:/;
const WINDOWS_DRIVE_PATH_REGEX = /^[A-Za-z]:[\\/]/;
// Autocomplete emits canonical file links, so ambiguous bare @scope/package text stays a package.
const SCOPED_PACKAGE_REFERENCE_REGEX =
  /^[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._-]*(?:\/[^\s@"]+)*$/;

function collectMentionTokens(text: string): ComposerInlineToken[] {
  const matches: ComposerInlineToken[] = [];

  for (const match of text.matchAll(FILE_LINK_TOKEN_REGEX)) {
    const fullMatch = match[0];
    const prefix = match[1] ?? "";
    const label = (match[2] ?? "").replace(/\\(.)/g, "$1");
    const encodedPath = match[3] ?? "";
    let path = encodedPath;
    try {
      path = decodeURIComponent(encodedPath);
    } catch {
      // Preserve malformed source rather than dropping a user-authored token.
    }
    const separatorIndex = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    const basename = separatorIndex >= 0 ? path.slice(separatorIndex + 1) : path;
    const hasExternalScheme = URI_SCHEME_REGEX.test(path) && !WINDOWS_DRIVE_PATH_REGEX.test(path);
    if (!path || hasExternalScheme || label !== basename) {
      continue;
    }
    const start = (match.index ?? 0) + prefix.length;
    const end = start + fullMatch.length - prefix.length;
    matches.push({
      type: "mention",
      value: path,
      source: text.slice(start, end),
      start,
      end,
    });
  }

  for (const match of text.matchAll(MENTION_TOKEN_REGEX)) {
    const fullMatch = match[0];
    const prefix = match[1] ?? "";
    const quotedPath = match[2];
    const path = quotedPath !== undefined ? quotedPath.replace(/\\(.)/g, "$1") : (match[3] ?? "");
    if (!path || (quotedPath === undefined && SCOPED_PACKAGE_REFERENCE_REGEX.test(path))) {
      continue;
    }
    const start = (match.index ?? 0) + prefix.length;
    const end = start + fullMatch.length - prefix.length;
    matches.push({
      type: "mention",
      value: path,
      source: text.slice(start, end),
      start,
      end,
    });
  }

  return matches;
}

export function collectComposerInlineTokens(
  text: string,
  options: CollectComposerInlineTokensOptions = {},
): ReadonlyArray<ComposerInlineToken> {
  const matches = collectMentionTokens(text);

  for (const match of text.matchAll(SKILL_TOKEN_REGEX)) {
    const fullMatch = match[0];
    const prefix = match[1] ?? "";
    const value = match[2] ?? "";
    if (!value) {
      continue;
    }
    const start = (match.index ?? 0) + prefix.length;
    const end = start + fullMatch.length - prefix.length;
    matches.push({
      type: "skill",
      value,
      source: text.slice(start, end),
      start,
      end,
    });
  }

  for (const token of options.preserveTrailingFrom ?? []) {
    if (
      token.end === text.length &&
      text.slice(token.start, token.end) === token.source &&
      !matches.some(
        (match) =>
          match.type === token.type && match.start === token.start && match.end === token.end,
      )
    ) {
      matches.push(token);
    }
  }

  return [...matches].sort((left, right) => left.start - right.start);
}

const CONTEXT_PROTOCOL = "t3-context:";
const COMPOSER_CONTEXT_HREF_PREFIX = `${CONTEXT_PROTOCOL}//v1/`;
const CONTEXT_KIND_PATTERN = /^[a-z][a-z0-9-]{0,39}$/;
const CONTEXT_ID_PATTERN = /^[a-z0-9_-]{1,128}$/i;
const MAX_LINK_LABEL_LENGTH = 512;
const CONTEXT_LINK = new RegExp(
  String.raw`(!?)\[([^\]\n]{0,${MAX_LINK_LABEL_LENGTH}})\]\((${COMPOSER_CONTEXT_HREF_PREFIX}[^\s)]{1,200})\)`,
  "g",
);

export function formatComposerContextHref(kind: ComposerContextKind, contextId: ComposerContextId) {
  return `${COMPOSER_CONTEXT_HREF_PREFIX}${kind}/${contextId}`;
}

export function parseComposerContextHref(
  href: string,
): { kind: ComposerContextKind; contextId: ComposerContextId } | null {
  if (!href.startsWith(COMPOSER_CONTEXT_HREF_PREFIX)) return null;
  const rest = href.slice(COMPOSER_CONTEXT_HREF_PREFIX.length);
  const parts = rest.split("/");
  if (parts.length !== 2) return null;
  const [kind, contextId] = parts as [string, string];
  if (!CONTEXT_KIND_PATTERN.test(kind) || !CONTEXT_ID_PATTERN.test(contextId)) return null;
  return { kind, contextId: contextId as ComposerContextId };
}

/** Labels must survive a Markdown link: no brackets or line breaks, bounded, never empty. */
export function sanitizeComposerContextLabel(label: string, kind: ComposerContextKind): string {
  const cleaned = label
    .replace(/[[\]\\\r\n]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, COMPOSER_CONTEXT_LABEL_MAX_CHARS);
  return cleaned.length > 0 ? cleaned : kind;
}

export function formatComposerContextReference(reference: {
  kind: ComposerContextKind;
  contextId: ComposerContextId;
  label: string;
}): string {
  const label = sanitizeComposerContextLabel(reference.label, reference.kind);
  const href = formatComposerContextHref(reference.kind, reference.contextId);
  return `${reference.kind === "image" ? "!" : ""}[${label}](${href})`;
}

export interface ComposerContextReferenceOccurrence {
  kind: ComposerContextKind;
  contextId: ComposerContextId;
  label: string;
  /** Whether the occurrence used the `![...]` image form. */
  image: boolean;
  source: string;
  start: number;
  end: number;
}

export function collectComposerContextReferences(
  text: string,
): ComposerContextReferenceOccurrence[] {
  const occurrences: ComposerContextReferenceOccurrence[] = [];
  // No link can match without the protocol prefix; skip the scan entirely on
  // plain prose so long messages never pay for a regex walk per `[`.
  if (!text.includes("](t3-context:")) return occurrences;
  for (const match of text.matchAll(CONTEXT_LINK)) {
    const parsed = parseComposerContextHref(match[3]!);
    if (!parsed) continue;
    occurrences.push({
      ...parsed,
      label: sanitizeComposerContextLabel(match[2]!, parsed.kind),
      image: match[1] === "!",
      source: match[0],
      start: match.index,
      end: match.index + match[0].length,
    });
  }
  return occurrences;
}

export function replaceComposerContextReferences(
  text: string,
  replace: (occurrence: ComposerContextReferenceOccurrence) => string,
): string {
  let result = "";
  let cursor = 0;
  for (const occurrence of collectComposerContextReferences(text)) {
    result += text.slice(cursor, occurrence.start) + replace(occurrence);
    cursor = occurrence.end;
  }
  return result + text.slice(cursor);
}


/**
 * Only the types a provider turn accepts. A picture the provider would reject is still a
 * file, so widening this map would promote attachments the send path cannot carry.
 * Mirrors `PROVIDER_SEND_TURN_SUPPORTED_IMAGE_MIME_TYPES`.
 */
const IMAGE_MIME_TYPE_BY_EXTENSION = new Map([
  ["gif", "image/gif"],
  ["jpeg", "image/jpeg"],
  ["jpg", "image/jpeg"],
  ["png", "image/png"],
  ["webp", "image/webp"],
]);

const SUPPORTED_IMAGE_MIME_TYPES = new Set(IMAGE_MIME_TYPE_BY_EXTENSION.values());

/** What a picker writes when it did not recognize the file; the name is better evidence. */
export const GENERIC_MIME_TYPES = new Set([
  "application/octet-stream",
  "binary/octet-stream",
  "application/unknown",
]);

/**
 * Recognizes pictures even when the picker omitted their MIME type. A picture chosen through
 * the document picker arrives typed as a plain file, so what it *is* has to come from its own
 * name and type rather than from which picker produced it.
 */
export function imageMimeType(attachment: {
  readonly name: string;
  readonly mimeType: string;
}): string | null {
  const mimeType = attachment.mimeType.split(";", 1)[0]?.trim().toLowerCase() ?? "";
  if (SUPPORTED_IMAGE_MIME_TYPES.has(mimeType)) return mimeType;
  // A declared but unsupported image type stays a file: the send path cannot carry it.
  if (mimeType.startsWith("image/")) return null;
  // The name is only evidence when nothing recorded what this is. A definite type already
  // answers the question, and a `.png` on a PDF must not override it.
  if (mimeType !== "" && !GENERIC_MIME_TYPES.has(mimeType)) return null;
  const dotIndex = attachment.name.lastIndexOf(".");
  return dotIndex < 0
    ? null
    : (IMAGE_MIME_TYPE_BY_EXTENSION.get(
        attachment.name
          .slice(dotIndex + 1)
          .trim()
          .toLowerCase(),
      ) ?? null);
}

const CONTEXT_CHIP_PRESENTATIONS = {
  image: { accent: "#d55665", symbol: "photo" },
  video: { accent: "#d06217", symbol: "play.rectangle" },
  file: { accent: "#0090cd", symbol: "doc" },
  mention: { accent: "#0096af", symbol: "doc" },
  terminal: { accent: "#009f6e", symbol: "terminal" },
  element: { accent: "#b87501", symbol: "cursorarrow.click" },
  "preview-annotation": { accent: "#b87501", symbol: "cursorarrow.click" },
  "review-comment": { accent: "#8a70dd", symbol: "text.bubble" },
  "pull-request": { accent: "#7079e4", symbol: "git-pull-request" },
  skill: { accent: "#b261be", symbol: "cube" },
  thread: { accent: "#009c96", symbol: "text.bubble" },
} as const;

/**
 * The size an attachment chip reports beside its name, matching web. Rendered as its own
 * smaller run, so it carries no separator. Only attachment-backed records have bytes.
 */
export function composerChipSizeSuffix(record?: {
  readonly kind?: string;
  readonly sizeBytes?: number;
}): string {
  if (record?.kind !== "file" && record?.kind !== "image") return "";
  return typeof record.sizeBytes === "number" ? formatAttachmentSize(record.sizeBytes) : "";
}

/**
 * A pull request chip is coloured by what the pull request *is*, the way web colours it and
 * the way the forge itself does: green open, grey draft, purple merged, red closed. The glyph
 * stays the same across all four, as it does on web — state is carried by colour alone.
 */
const PULL_REQUEST_CHIP_PRESENTATIONS = {
  open: { accent: "#009f6e", symbol: "git-pull-request" },
  draft: { accent: "#7f8793", symbol: "git-pull-request" },
  merged: { accent: "#8a70dd", symbol: "git-pull-request" },
  closed: { accent: "#d55665", symbol: "git-pull-request" },
} as const;

export function contextChipPresentation(
  kind: string,
  record?: {
    readonly kind?: string;
    readonly name?: string;
    readonly mimeType?: string;
    readonly sectionId?: string;
    readonly pullRequest?: {
      readonly state?: string;
      readonly isDraft?: boolean;
    };
  },
) {
  const presentationKind =
    kind === "file" &&
    videoMimeType({
      name: record?.name ?? "",
      mimeType: record?.mimeType ?? "",
    })
      ? "video"
      : // A picture chosen through the file picker is typed `file`, but it is still a
        // picture: it reads as one to the user and should not wear the generic file chip.
        kind === "file" &&
          imageMimeType({ name: record?.name ?? "", mimeType: record?.mimeType ?? "" }) !== null
        ? "image"
        : kind === "review-comment" && record?.sectionId?.startsWith("pull-request:")
          ? "pull-request"
          : kind;
  if (presentationKind === "pull-request") {
    const pullRequest = record?.pullRequest;
    const state =
      pullRequest?.state === "open" && pullRequest.isDraft === true
        ? "draft"
        : (pullRequest?.state ?? "");
    if (Object.hasOwn(PULL_REQUEST_CHIP_PRESENTATIONS, state)) {
      return PULL_REQUEST_CHIP_PRESENTATIONS[state as keyof typeof PULL_REQUEST_CHIP_PRESENTATIONS];
    }
  }
  return Object.hasOwn(CONTEXT_CHIP_PRESENTATIONS, presentationKind)
    ? CONTEXT_CHIP_PRESENTATIONS[presentationKind as keyof typeof CONTEXT_CHIP_PRESENTATIONS]
    : CONTEXT_CHIP_PRESENTATIONS.file;
}

export function composerContextEditorTokens(text: string, tokens: readonly ComposerInlineToken[]) {
  const references = collectComposerContextReferences(text);
  return [
    ...tokens.filter(
      (token) => !references.some((ref) => token.start < ref.end && token.end > ref.start),
    ),
    ...references.map((ref) => ({
      type: "context" as const,
      value: ref.label,
      ...ref,
    })),
  ].sort((a, b) => a.start - b.start);
}


export interface ComposerDocumentToken {
  type: 'mention' | 'skill' | 'context'; value: string; source: string; start: number; end: number;
  label: string; detail: string; accent: string; symbol: string; iconUri: string | null;
  kind?: string; contextId?: string; image?: boolean;
}
export interface ComposerDocumentProjection {
  confirmedTokens: readonly ComposerInlineToken[]; tokens: ComposerDocumentToken[];
  tokensJson: string; clipboardFragment: string;
}
export interface ComposerClipboardAttachment { id: string; uploadedAttachmentId?: string; uploadEnvironmentId?: string }
export interface ComposerClipboardFragment { version: 1; source: {environmentId:string;threadId?:string;messageId?:string}; records:Obj[] }
/** The source encoder is a size boundary, not an input/import validator. */
export function encodeComposerContextFragment(fragment:ComposerClipboardFragment):string | null {
  const encoded=JSON.stringify(fragment);
  return encoded.length<=16_000_000?encoded:null;
}
export function encodeComposerContextClipboardHtml(text:string,fragment:string,html?:string):string {
  if(html!==undefined) return `<div data-t3-context-fragment="${encodeURIComponent(fragment)}">${html}</div>`;
  const escaped=text.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  return `<pre data-t3-context-fragment="${encodeURIComponent(fragment)}">${escaped}</pre>`;
}
/** Export all actual draft records; the receiver filters to copied references.
 * Rebind only attachments uploaded into this same source environment. */
export function mobileComposerClipboard(environmentId:string,context:MobileMessageContext | undefined,
  attachments:readonly ComposerClipboardAttachment[]):string | null {
  return environmentId && context ? encodeComposerContextFragment({version:1,source:{environmentId},records:context.records.map(record=>{
    if(!('attachmentId' in record)) return {...record};
    const attachment=attachments.find(entry=>entry.id===record.attachmentId);
    return {...record,attachmentId:attachment?.uploadEnvironmentId===environmentId
      ? attachment.uploadedAttachmentId??record.attachmentId : record.attachmentId};
  })}) : '';
}
function basename(path:string):string { const index=Math.max(path.lastIndexOf('/'),path.lastIndexOf('\\')); return index>=0?path.slice(index+1):path; }
/** Caller owns confirmed-token history by exact editor identity. iconUri resolves
 * real app assets synchronously; no IO, URL signing or icon cache lives here. */
export function mobileComposerDocument(input:{value:string;context?:MobileMessageContext;environmentId:string;
  attachments:readonly ComposerClipboardAttachment[];skills:readonly {name:string;displayName?:string}[];
  confirmedTokens?:readonly ComposerInlineToken[];iconUri:(path:string)=>string|null}):ComposerDocumentProjection {
  const confirmedTokens=collectComposerInlineTokens(input.value,{preserveTrailingFrom:input.confirmedTokens});
  const labels=new Map(input.skills.map(skill=>[skill.name,skill.displayName?.trim()||skill.name]));
  const tokens:ComposerDocumentToken[]=composerContextEditorTokens(input.value,confirmedTokens).map(token=>{
    const record=token.type==='context'?input.context?.records.find(entry=>entry.contextId===token.contextId):undefined;
    const appearance=contextChipPresentation(token.type==='context'?token.kind:token.type,record);
    return {...token,...appearance,label:token.type==='skill'?(labels.get(token.value)??token.value)
      :token.type==='context'?`${token.label}${record?'':' · unavailable'}`:basename(token.value),
      detail:token.type==='context'?composerChipSizeSuffix(record):'',
      iconUri:token.type==='mention'?input.iconUri(token.value):record?.kind==='mention'&&typeof record.path==='string'?input.iconUri(record.path):null};
  });
  return {confirmedTokens:confirmedTokens.map(token=>({...token})),tokens,tokensJson:JSON.stringify(tokens),
    clipboardFragment:mobileComposerClipboard(input.environmentId,input.context,input.attachments)??''};
}

// Pinned mobile markdownLinks.ts tables; desktop resolver intentionally differs.
const POSITION_SUFFIX_PATTERN = /:\d+(?::\d+)?$/;
const FILE_ICON_BY_NAME: Readonly<Record<string, string>> = {
  ".babelrc": "babel",
  ".babelrc.json": "babel",
  ".bash_profile": "bash",
  ".bashrc": "bash",
  ".browserslistrc": "browserslist",
  ".dockerignore": "docker",
  ".eslintignore": "eslint",
  ".eslintrc": "eslint",
  ".eslintrc.cjs": "eslint",
  ".eslintrc.js": "eslint",
  ".eslintrc.json": "eslint",
  ".eslintrc.yaml": "eslint",
  ".eslintrc.yml": "eslint",
  ".gitattributes": "git",
  ".gitignore": "git",
  ".gitkeep": "git",
  ".gitmodules": "git",
  ".oxlintrc.json": "oxc",
  ".postcssrc": "postcss",
  ".postcssrc.json": "postcss",
  ".postcssrc.yaml": "postcss",
  ".postcssrc.yml": "postcss",
  ".prettierignore": "prettier",
  ".prettierrc": "prettier",
  ".prettierrc.json": "prettier",
  ".prettierrc.cjs": "prettier",
  ".prettierrc.js": "prettier",
  ".prettierrc.mjs": "prettier",
  ".prettierrc.toml": "prettier",
  ".prettierrc.yaml": "prettier",
  ".prettierrc.yml": "prettier",
  ".stylelintignore": "stylelint",
  ".stylelintrc": "stylelint",
  ".stylelintrc.cjs": "stylelint",
  ".stylelintrc.js": "stylelint",
  ".stylelintrc.json": "stylelint",
  ".stylelintrc.mjs": "stylelint",
  ".stylelintrc.yaml": "stylelint",
  ".stylelintrc.yml": "stylelint",
  ".terraform.lock.hcl": "terraform",
  ".zprofile": "bash",
  ".zshenv": "bash",
  ".zshrc": "bash",
  "agents.md": "agents",
  "babel.config.js": "babel",
  "babel.config.cjs": "babel",
  "babel.config.json": "babel",
  "babel.config.mjs": "babel",
  "biome.json": "biome",
  "biome.jsonc": "biome",
  "bun.lock": "bun",
  "bun.lockb": "bun",
  "bunfig.toml": "bun",
  "claude.md": "claude",
  "compose.yaml": "docker",
  "compose.yml": "docker",
  "docker-compose.yaml": "docker",
  "docker-compose.yml": "docker",
  "docker-compose.override.yml": "docker",
  dockerfile: "docker",
  "eslint.config.js": "eslint",
  "eslint.config.cjs": "eslint",
  "eslint.config.mjs": "eslint",
  "eslint.config.mts": "eslint",
  "eslint.config.ts": "eslint",
  gemfile: "ruby",
  "next.config.js": "nextjs",
  "next.config.mjs": "nextjs",
  "next.config.mts": "nextjs",
  "next.config.ts": "nextjs",
  "package.json": "npm",
  "pnpm-lock.yaml": "pnpm",
  "pnpm-workspace.yaml": "pnpm",
  "postcss.config.js": "postcss",
  "postcss.config.cjs": "postcss",
  "postcss.config.mjs": "postcss",
  "postcss.config.ts": "postcss",
  "prettier.config.js": "prettier",
  "prettier.config.cjs": "prettier",
  "prettier.config.mjs": "prettier",
  rakefile: "ruby",
  "readme.md": "markdown",
  "stylelint.config.js": "stylelint",
  "stylelint.config.cjs": "stylelint",
  "stylelint.config.mjs": "stylelint",
  "svgo.config.js": "svgo",
  "svgo.config.cjs": "svgo",
  "svgo.config.mjs": "svgo",
  "svgo.config.ts": "svgo",
  "tailwind.config.js": "tailwind",
  "tailwind.config.cjs": "tailwind",
  "tailwind.config.mjs": "tailwind",
  "tailwind.config.ts": "tailwind",
  "tsconfig.json": "typescript",
  "vite.config.js": "vite",
  "vite.config.mjs": "vite",
  "vite.config.mts": "vite",
  "vite.config.ts": "vite",
  "webpack.config.js": "webpack",
  "webpack.config.babel.js": "webpack",
  "webpack.config.cjs": "webpack",
  "webpack.config.mjs": "webpack",
  "webpack.config.ts": "webpack",
};

const FILE_ICON_BY_EXTENSION: Readonly<Record<string, string>> = {
  "7z": "zip",
  astro: "astro",
  avif: "image",
  "code-workspace": "vscode",
  bash: "bash",
  bmp: "image",
  bz2: "zip",
  c: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  css: "css",
  csv: "table",
  cts: "typescript",
  db: "database",
  env: "text",
  "env.development": "text",
  "env.local": "text",
  "env.production": "text",
  eot: "font",
  erb: "ruby",
  fish: "bash",
  gif: "image",
  go: "go",
  gql: "graphql",
  graphql: "graphql",
  gz: "zip",
  h: "c",
  hh: "cpp",
  hpp: "cpp",
  hxx: "cpp",
  htm: "html",
  html: "html",
  ico: "image",
  icns: "image",
  ini: "text",
  inl: "cpp",
  jar: "zip",
  jpeg: "image",
  jpg: "image",
  js: "javascript",
  jsx: "react",
  json: "json",
  jsonc: "json",
  less: "css",
  md: "markdown",
  mdx: "markdown",
  "mdx.tsx": "markdown",
  mjs: "javascript",
  mts: "typescript",
  png: "image",
  postcss: "css",
  py: "python",
  pyi: "python",
  pyw: "python",
  pyx: "python",
  rake: "ruby",
  rar: "zip",
  rb: "ruby",
  rs: "rust",
  sass: "sass",
  scss: "sass",
  sh: "bash",
  sql: "database",
  sqlite: "database",
  sqlite3: "database",
  svelte: "svelte",
  svg: "svg",
  swift: "swift",
  tar: "zip",
  tf: "terraform",
  tfstate: "terraform",
  tfvars: "terraform",
  tgz: "zip",
  ts: "typescript",
  tsv: "table",
  tsx: "react",
  txt: "text",
  woff: "font",
  woff2: "font",
  vue: "vue",
  wasm: "wasm",
  webp: "image",
  yml: "yml",
  yaml: "yml",
  zig: "zig",
  zip: "zip",
  zsh: "bash",
};

export function mobileComposerFileIcon(value: string): string {
  const basename = composerFileBasename(value).replace(POSITION_SUFFIX_PATTERN, "").toLowerCase();
  if (videoMimeType({ name: basename, mimeType: "" }) !== null) return "video";
  const exactIcon = FILE_ICON_BY_NAME[basename];
  if (exactIcon) return exactIcon;
  if (basename.startsWith("tsconfig.") && basename.endsWith(".json")) {
    return "typescript";
  }
  const segments = basename.split(".");
  for (let index = 1; index < segments.length; index += 1) {
    const icon = FILE_ICON_BY_EXTENSION[segments.slice(index).join(".")];
    if (icon) return icon;
  }
  return "default";
}


function composerFileBasename(path: string): string {
  // A trailing separator is a valid way to write a directory. Trim it before
  // taking the final segment so the label is never empty.
  const trimmed = path.replace(/[/\\]+$/, "");
  if (trimmed.length === 0) return path;
  const separatorIndex = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return separatorIndex >= 0 ? trimmed.slice(separatorIndex + 1) : trimmed;
}
