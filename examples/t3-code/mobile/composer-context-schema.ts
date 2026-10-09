// Clipboard boundary adapted from T3 Code365aa87982 contracts/composerContext (MIT, LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { Json, Obj } from './shared/domain';

type Decoder = (value: unknown) => Json;
const invalid = (): never => { throw new Error('Invalid composer context'); };
const object = (value: unknown): Obj => value !== null && typeof value === 'object' && !Array.isArray(value)
  ? value as Obj : invalid();
const string = (max = Infinity, trim = false): Decoder => value => {
  if (typeof value !== 'string') return invalid();
  const result = trim ? value.trim() : value;
  return result.length <= max && (!trim || result.length > 0) ? result : invalid();
};
const integer: Decoder = value => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : invalid();
const nullable = (decode: Decoder): Decoder => value => value === null ? null : decode(value);
const array = (decode: Decoder, max: number): Decoder => value => Array.isArray(value) && value.length <= max
  ? Array.from(value, decode) : invalid();
const literal = (...values: Json[]): Decoder => value => values.includes(value as Json) ? value as Json : invalid();
function struct(required: Record<string, Decoder>, optional: Record<string, Decoder> = {}): Decoder {
  return value => {
    const input = object(value), output: Obj = {};
    for (const key of Object.keys(required)) output[key] = required[key]!(input[key]);
    for (const key of Object.keys(optional)) if (Object.hasOwn(input, key)) output[key] = optional[key]!(input[key]);
    return output;
  };
}
const short = string(2048), nullShort = nullable(short);
const id: Decoder = value => {
  const result = string(128, true)(value) as string;
  return /^[a-z0-9_-]+$/i.test(result) ? result : invalid();
};
const base = { version: literal(1), contextId: id, label: string(200) };
const element = {
  pageUrl: short, pageTitle: nullShort, tagName: string(255, true), selector: nullShort,
  htmlPreview: string(8000), componentName: nullShort,
  source: nullable(struct({ functionName: nullShort, fileName: nullShort,
    lineNumber: nullable(integer), columnNumber: nullable(integer) })), styles: string(8000),
};
const attachment = { attachmentId: id, name: string(255, true), mimeType: string(100, true), sizeBytes: integer };
const positive: Decoder = value => { const n = Number(integer(value)); return n > 0 ? n : invalid(); };
const boolean: Decoder = value => typeof value === 'boolean' ? value : invalid();
const decoders: Record<string, Decoder> = {
  image: struct({ ...base, kind: literal('image'), ...attachment }),
  file: struct({ ...base, kind: literal('file'), ...attachment }),
  terminal: struct({ ...base, kind: literal('terminal'), terminalId: string(255, true), terminalLabel: string(255, true),
    lineStart: integer, lineEnd: integer, text: string(64000) }),
  element: struct({ ...base, kind: literal('element'), ...element }),
  'preview-annotation': struct({ ...base, kind: literal('preview-annotation'), annotationId: short,
    pageUrl: short, pageTitle: nullShort, comment: string(8000), targetSummary: short, styleChanges: array(short, 200) }, {
    elements: array(struct(element), 50), elementIds: array(short, 50), regionCount: integer, strokeCount: integer,
    styleChangeDetails: array(struct({ targetId: short, selector: nullShort, property: short,
      previousValue: string(8000), value: string(8000) }), 200), screenshotContextId: id,
  }),
  'review-comment': struct({ ...base, kind: literal('review-comment'), sectionId: string(255, true), sectionTitle: short,
    filePath: string(2048, true), startIndex: integer, endIndex: integer, rangeLabel: short, text: string(16000), diff: string(32000) }, {
    fenceLanguage: string(64), pullRequest: struct({ number: positive, title: short, url: short, headBranch: short,
      baseBranch: short, state: literal('open', 'closed', 'merged'), isDraft: boolean }),
  }),
  mention: struct({ ...base, kind: literal('mention'), path: string(2048, true) }),
  skill: struct({ ...base, kind: literal('skill'), name: string(255, true) }),
  thread: struct({ ...base, kind: literal('thread'), environmentId: string(Infinity, true),
    threadId: string(Infinity, true), title: string(200) }),
};
function json(value: unknown): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return Array.from(value).every(json);
  if (typeof value !== 'object' || Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) return false;
  return Object.values(value).every(json);
}
const unknownRecord = struct({ ...base, kind: value => {
  const kind = string(Infinity, true)(value) as string;
  return /^[a-z][a-z0-9-]{0,39}$/.test(kind) && !Object.hasOwn(decoders, kind) ? kind : invalid();
}, payload: value => {
  const encoded = JSON.stringify(value);
  return encoded !== undefined && encoded.length <= 64000 && json(value) ? value as Json : invalid();
} });

/** Decode, including source trimming and excess-field removal; not a saved-record validity predicate. */
export function decodeMobileComposerContextRecord(value: unknown): Obj | null {
  try {
    const input = object(value), kind = input.kind;
    const decode = typeof kind === 'string' && Object.hasOwn(decoders, kind) ? decoders[kind]! : unknownRecord;
    const record = decode(input) as Obj;
    if (record.kind === 'terminal' && Number(record.lineEnd) < Number(record.lineStart) ||
      record.kind === 'review-comment' && Number(record.endIndex) < Number(record.startIndex)) return null;
    return record;
  } catch { return null; }
}

export interface MobileComposerClipboardFragment {
  version: 1;
  source: { environmentId: string; threadId?: string; messageId?: string };
  records: Obj[];
}
/** The raw array is bounded before individually dropping malformed records. Duplicate IDs survive, as upstream. */
export function decodeMobileComposerClipboardValue(value: unknown): MobileComposerClipboardFragment | null {
  try {
    const input = object(value), source = object(input.source);
    if (input.version !== 1 || !Array.isArray(input.records) || input.records.length > 200) return null;
    const decodedSource: MobileComposerClipboardFragment['source'] = { environmentId: string(Infinity, true)(source.environmentId) as string };
    for (const key of ['threadId', 'messageId'] as const) {
      if (source[key] !== undefined) decodedSource[key] = string(Infinity, true)(source[key]) as string;
      else if (Object.hasOwn(source, key)) decodedSource[key] = undefined;
    }
    return { version: 1, source: decodedSource,
      records: input.records.map(decodeMobileComposerContextRecord).filter((record): record is Obj => record !== null) };
  } catch { return null; }
}
