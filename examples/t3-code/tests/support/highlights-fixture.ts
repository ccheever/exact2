// The lexers in a test: data/highlights.wasm (tools/generate/gen-highlights.mjs) under Bun's WebAssembly, answering
// highlightCode by the data crate's rules and constants (data/src/highlights.rs): a whole text in mode 3, and a
// document (a long text, a streaming block) through the module's live tokenizer (dist/live.js), staged and lexed a
// budget at a time and answered from its first line not yet final, so a test sees the tokens the app paints once the
// root's pump has handed a text to them (app.contract highlightStep). Bun meters no fuel: a call spends what
// highlights.rs plans for the work it does, not what wasmi would measure.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { highlightAsk, highlightPending, highlightStore, WHOLE_BYTES } from '../../src/highlight/highlights';
import { highlight, type Token } from '../../src/highlight/timeline-highlight';
import { APP } from './app-files';

const PAGE = 65_536;
/** highlights.rs's budget and plans. */
export const BUDGET = 16_000_000;
export const PLANNED_PER_BYTE = 410, PLANNED_PER_LINE = 4_000;
const STAGE_PLANNED_PER_BYTE = 12, STAGE_PLANNED_PER_LINE = 400;
const LONG_LINE = 16 * 1024, WORK = 4_096, SLICE_PLANNED = 1_800_000, DOCUMENTS = 8, KEEP_PAGES = 64;
/** liveStats: lines wait to be lexed, and the first of them. */
const WAITING = 9, NEXT_LINE = 10;
/** highlightCode's `line` for a document with lines left, one to start again, and lexers that trapped. */
const MORE = -1, AGAIN = -2, FAILED = -3;

type Live = {
  memory: WebAssembly.Memory; highlight: () => void;
  liveStage(len: number): number; liveInitDoc(ptr: number, len: number, lang: number): void; liveApplyEdits(ptr: number, n: number): void;
  liveRun(lines: number, end: number, work: number): number; liveStats(at: number): number; liveLineCount(): number;
  liveLineLen(line: number): number; liveLineByteLen(line: number): number; liveLineFlags(line: number): number;
  liveLineTokCount(line: number): number; liveLineTokPtr(line: number): number;
};
let module: WebAssembly.Module | null = null, shared: Live | null = null;
const encoder = new TextEncoder();

/** An instance of the module, with its imports as its own JavaScript answers them (dist/highlighter.js #idClass). */
function instance(): Live {
  module ??= new WebAssembly.Module(readFileSync(join(APP, 'data', 'highlights.wasm')));
  let memory: WebAssembly.Memory | null = null;
  const pointAt = (ptr: number, len: number) => new TextDecoder().decode(new Uint8Array(memory!.buffer, ptr, len));
  const made = new WebAssembly.Instance(module, { env: {
    is_id_start: (ptr: number, len: number) => /^\p{ID_Start}$/u.test(pointAt(ptr, len)) ? 1 : 0,
    is_id_continue: (ptr: number, len: number) => /^[‌‍\p{ID_Continue}]$/u.test(pointAt(ptr, len)) ? 1 : 0,
  } });
  memory = made.exports.memory as WebAssembly.Memory;
  return made.exports as unknown as Live;
}

const lineCount = (text: string) => text.split('\n').length;

/** The UTF-16 line records of `text` under lexer `language` (highlights.rs Lexers::whole). */
export function lineRecords(language: number, text: string): number[] {
  shared ??= instance();
  const bytes = encoder.encode(text);
  const pages = 1 + Math.ceil((bytes.length + 96) / PAGE), have = shared.memory.buffer.byteLength / PAGE;
  if (pages > have) shared.memory.grow(pages - have);
  const data = new Uint8Array(shared.memory.buffer), view = new DataView(shared.memory.buffer);
  data.set(bytes, PAGE);
  data[PAGE + bytes.length] = 0;
  view.setUint8(0, language); view.setUint8(1, 3); view.setUint32(2, bytes.length, true);
  shared.highlight();
  const after = new DataView(shared.memory.buffer), start = after.getUint32(6, true), length = after.getUint32(10, true);
  return Array.from(new Uint32Array(shared.memory.buffer, start, length >> 2));
}

/** What the lexers have lexed, in UTF-8 bytes of whole texts and whole lines: a test reads what a stream cost. */
export const lexed = { bytes: 0 };

/** A text the lexers keep (highlights.rs Document): an instance of its own, the text it holds and the end of it not yet
 *  staged, and the lines answered as final. */
class Document {
  readonly live = instance();
  /** The text's length in UTF-16 units: what the module holds and `unstaged`. */
  length: number;
  delivered = 0;
  used = 0;
  constructor(readonly id: number, readonly language: number, public unstaged: string) {
    this.length = unstaged.length;
    this.live.liveInitDoc(this.live.liveStage(0), 0, language);
  }

  /** highlights.rs Document::stage: the start of `unstaged`, whole lines while they are planned to fit. */
  stage(allowance: number, first: boolean): number {
    if (!this.unstaged) return 0;
    let planned = STAGE_PLANNED_PER_BYTE * this.live.liveLineByteLen(this.live.liveLineCount() - 1), cut = 0;
    while (cut < this.unstaged.length) {
      const end = this.unstaged.indexOf('\n', cut) + 1 || this.unstaged.length;
      const cost = STAGE_PLANNED_PER_BYTE * encoder.encode(this.unstaged.slice(cut, end)).length + STAGE_PLANNED_PER_LINE;
      if (planned + cost > allowance && !(first && cut === 0)) break;
      planned += cost;
      cut = end;
    }
    if (!cut) return 0;
    this.append(this.unstaged.slice(0, cut));
    this.unstaged = this.unstaged.slice(cut);
    return planned;
  }

  /** One edit at the last line's end (dist/live.js #stageEdits). */
  private append(text: string): void {
    const last = this.live.liveLineCount() - 1, column = this.live.liveLineLen(last), bytes = encoder.encode(text);
    const ptr = this.live.liveStage(28 + bytes.length) >>> 0, view = new DataView(this.live.memory.buffer);
    [1, last, column, last, column, 28, bytes.length].forEach((word, index) => view.setUint32(ptr + 4 * index, word, true));
    new Uint8Array(this.live.memory.buffer).set(bytes, ptr + 28);
    this.live.liveApplyEdits(ptr, 0);
  }

  /** highlights.rs Document::run: whole lines while they fit `budget`, else a slice, with `first` at least one step. */
  run(budget: number, first: boolean): [number, boolean] {
    let spent = 0;
    for (;;) {
      if (this.live.liveStats(WAITING) === 0) return [spent, true];
      const next = this.live.liveStats(NEXT_LINE), count = this.live.liveLineCount();
      if (next >= count) throw new Error('a document waits past its last line');
      let end = next, planned = 0, handed = 0;
      while (end < count) {
        const bytes = this.live.liveLineByteLen(end) + 1, cost = PLANNED_PER_BYTE * bytes + PLANNED_PER_LINE;
        if (bytes > LONG_LINE || spent + planned + cost > budget) break;
        planned += cost; handed += bytes; end++;
      }
      if (end === next && !first && spent + SLICE_PLANNED > budget) return [spent, false];
      first = false;
      if (end > next) { this.live.liveRun(end - next, end, 0); spent += planned; lexed.bytes += handed; }
      else { this.live.liveRun(1, next + 1, WORK); spent += SLICE_PLANNED; }
    }
  }

  /** highlights.rs Document::records: the lines from `from` on, offsets from that line's start. */
  records(from: number): number[] {
    const count = this.live.liveLineCount(), records: number[] = [];
    let start = 0;
    for (let line = from; line < count; line++) {
      const tokens = this.live.liveLineTokCount(line), flags = this.live.liveLineFlags(line);
      if (tokens > 0) {
        const at = this.live.liveLineTokPtr(line) >>> 0, view = new DataView(this.live.memory.buffer);
        for (let token = 0; token < tokens; token++) {
          // dist/live.js getLineRecords: `packed24` words of the type and the end, or `wide32` pairs.
          if (flags & 4) records.push(start + view.getUint32(at + token * 8, true), view.getUint32(at + token * 8 + 4, true));
          else { const word = view.getUint32(at + token * 4, true); records.push(start + (word & 0xffffff), word >>> 24); }
        }
      }
      if (line + 1 < count) { start += this.live.liveLineLen(line) + (flags & 3); records.push(start, 0xffffffff); }
    }
    return records;
  }

  pages(): number { return this.live.memory.buffer.byteLength / PAGE; }
}

type Request = { id: number; language: number; doc: number; from: number; text: string };
type Result = { id: number; line: number; records: number[] };
let documents: Document[] = [], uses = 0;

/** Tests: no documents kept, as a new data crate starts. */
export function resetLexers(): void { documents = []; }

/** highlightCode(requests) as highlights.rs answers it, within `budget` of planned fuel: requests in order, a document
 *  taking at most half the budget at first, what is left going to the documents still lexing. */
export function highlightCode(requests: readonly Request[], budget = BUDGET): Result[] {
  let spent = 0;
  const lex = (id: number, doc: Document, share: number, first: boolean): Result => {
    try {
      const staged = doc.stage(share, first);
      const [used, done] = doc.run(Math.max(0, share - staged), first && staged === 0);
      spent += staged + used;
      if (!done || doc.unstaged) return { id, line: MORE, records: [] };
      const line = doc.delivered, records = doc.records(line);
      doc.delivered = doc.live.liveLineCount() - 1;
      if (doc.pages() > KEEP_PAGES) documents = documents.filter(entry => entry !== doc);
      return { id, line, records };
    } catch {
      documents = documents.filter(entry => entry !== doc);
      return { id, line: FAILED, records: [] };
    }
  };
  const whole = (request: Request): Result | null => {
    const bytes = encoder.encode(request.text).length;
    if (bytes > WHOLE_BYTES) return { id: request.id, line: AGAIN, records: [] };
    const planned = PLANNED_PER_BYTE * bytes + PLANNED_PER_LINE * lineCount(request.text);
    if (spent + planned > budget) return spent > 0 ? null : { id: request.id, line: AGAIN, records: [] };
    spent += planned; lexed.bytes += bytes;
    return { id: request.id, line: 0, records: lineRecords(request.language, request.text) };
  };
  const document = (request: Request, share: number): Result => {
    const start = spent;
    let doc: Document | undefined;
    if (request.from === 0) {
      documents = documents.filter(entry => entry.id !== request.doc);
      if (documents.length >= DOCUMENTS) documents.splice(documents.indexOf(documents.reduce((a, b) => (b.used < a.used ? b : a))), 1);
      documents.push(doc = new Document(request.doc, request.language, request.text));
    } else {
      doc = documents.find(entry => entry.id === request.doc && entry.length === request.from && entry.language === request.language);
      if (!doc) return { id: request.id, line: AGAIN, records: [] };
      doc.unstaged += request.text;
      doc.length += request.text.length;
    }
    doc.used = ++uses;
    return lex(request.id, doc, share - (spent - start), start === 0);
  };
  const outcomes = requests.map(request => request.doc ? document(request, Math.min(Math.floor(budget / 2), Math.max(0, budget - spent))) : whole(request));
  outcomes.forEach((outcome, at) => {
    if (spent >= budget || outcome?.line !== MORE) return;
    const request = requests[at]!, doc = documents.find(entry => entry.id === request.doc);
    outcomes[at] = doc ? lex(request.id, doc, budget - spent, false) : { id: request.id, line: AGAIN, records: [] };
  });
  return outcomes.filter((outcome): outcome is Result => outcome !== null);
}

/** One round of the root's pump: the ask, the lexers' answer and the store. */
export function highlightRound(budget = BUDGET): { finished: boolean; requests: Request[] } {
  const { requests } = highlightAsk();
  return { finished: highlightStore(highlightCode(requests, budget)).finished, requests };
}

/** The root's pump until no text waits. Whether a text got its tokens. */
export function settleHighlights(budget = BUDGET): boolean {
  let finished = false;
  for (let round = 0; highlightPending(); round++) {
    if (round > 10_000) throw new Error('the highlight pump never settled');
    finished = highlightRound(budget).finished || finished;
  }
  return finished;
}

/** The tokens `text` paints once the lexers have answered for it. */
export function highlighted(text: string, language: string, maxLineLength = 0): Token[] {
  highlight(text, language, maxLineLength);
  settleHighlights();
  return highlight(text, language, maxLineLength);
}
