// The lexers in a test: data/highlights.wasm (tools/generate/gen-highlights.mjs) under Bun's WebAssembly, answering
// highlightCode as the data crate does (data/src/highlights.rs): a whole text in mode 3, and a document (a long text,
// a streaming block) from its first line not yet final, within a call's budget, so a test sees the tokens the app
// paints once the root's pump has handed a text to them (app.contract highlightStep).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { highlightAsk, highlightPending, highlightStore, WHOLE_BYTES } from '../../src/highlight/highlights';
import { highlight, type Token } from '../../src/highlight/timeline-highlight';
import { APP } from './app-files';

const PAGE = 65_536;
/** A call's budget in characters: highlights.rs BUDGET over the fuel it plans a byte to take. */
export const CALL_CHARS = Math.floor(16_000_000 / 450);
let lexers: { memory: WebAssembly.Memory; highlight: () => void } | null = null;

/** The module's imports, as its own JavaScript answers them (dist/highlighter.js #idClass). */
function make() {
  const module = new WebAssembly.Module(readFileSync(join(APP, 'data', 'highlights.wasm')));
  let memory: WebAssembly.Memory | null = null;
  const pointAt = (ptr: number, len: number) => new TextDecoder().decode(new Uint8Array(memory!.buffer, ptr, len));
  const instance = new WebAssembly.Instance(module, { env: {
    is_id_start: (ptr: number, len: number) => /^\p{ID_Start}$/u.test(pointAt(ptr, len)) ? 1 : 0,
    is_id_continue: (ptr: number, len: number) => /^[‌‍\p{ID_Continue}]$/u.test(pointAt(ptr, len)) ? 1 : 0,
  } });
  memory = instance.exports.memory as WebAssembly.Memory;
  return { memory, highlight: instance.exports.highlight as () => void };
}

/** The UTF-16 line records of `text` under lexer `language` (highlights.rs Lexers::whole). */
export function lineRecords(language: number, text: string): number[] {
  lexers ??= make();
  const bytes = new TextEncoder().encode(text);
  const pages = 1 + Math.ceil((bytes.length + 96) / PAGE), have = lexers.memory.buffer.byteLength / PAGE;
  if (pages > have) lexers.memory.grow(pages - have);
  const data = new Uint8Array(lexers.memory.buffer), view = new DataView(lexers.memory.buffer);
  data.set(bytes, PAGE);
  data[PAGE + bytes.length] = 0;
  view.setUint8(0, language); view.setUint8(1, 3); view.setUint32(2, bytes.length, true);
  lexers.highlight();
  const after = new DataView(lexers.memory.buffer), start = after.getUint32(6, true), length = after.getUint32(10, true);
  return Array.from(new Uint32Array(lexers.memory.buffer, start, length >> 2));
}

type Request = { id: number; language: number; doc: number; from: number; text: string };
type Result = { id: number; line: number; records: number[] };
/** A document: its text, the lines answered as final, and how far it is lexed (UTF-16 units). */
const docs = new Map<number, { language: number; text: string; delivered: number; done: number }>();
/** What the lexers have lexed, in characters: a test reads what a stream cost. */
export const lexed = { chars: 0 };

/** highlightCode(requests) as highlights.rs answers it (`line` -1: lines left; -2: start the document again). */
export function highlightCode(requests: readonly Request[], budget = CALL_CHARS): Result[] {
  const results: Result[] = [];
  let spent = 0;
  const lex = (request: Request, share: number): Result => {
    const doc = docs.get(request.doc)!, run = Math.max(0, Math.min(doc.text.length - doc.done, share));
    doc.done += run; spent += run; lexed.chars += run;
    if (doc.done < doc.text.length) return { id: request.id, line: -1, records: [] };
    // The whole text's records, from the start of the first line not yet final.
    const all = lineRecords(doc.language, doc.text), out: number[] = [];
    let breaks = 0, base = 0;
    for (let at = 0; at + 1 < all.length; at += 2) {
      if (breaks >= doc.delivered) out.push(all[at]! - base, all[at + 1]!);
      if (all[at + 1] === 0xffffffff && ++breaks === doc.delivered) base = all[at]!;
    }
    const line = doc.delivered;
    doc.delivered = breaks;
    return { id: request.id, line, records: out };
  };
  for (const request of requests) {
    if (!request.doc) {
      const chars = new TextEncoder().encode(request.text).length > WHOLE_BYTES ? -1 : request.text.length;
      if (chars < 0) { results.push({ id: request.id, line: -2, records: [] }); continue; }
      if (spent > 0 && spent + chars > budget) continue;
      spent += chars; lexed.chars += chars;
      results.push({ id: request.id, line: 0, records: lineRecords(request.language, request.text) });
      continue;
    }
    const doc = docs.get(request.doc);
    if (request.from === 0) docs.set(request.doc, { language: request.language, text: request.text, delivered: 0, done: 0 });
    else if (!doc || doc.text.length !== request.from || doc.language !== request.language) { results.push({ id: request.id, line: -2, records: [] }); continue; }
    else if (request.text) {
      // An append lexes again from the start of the line it continues.
      doc.done = Math.min(doc.done, doc.text.lastIndexOf('\n') + 1);
      doc.text += request.text;
    }
    results.push(lex(request, Math.min(budget / 2, budget - spent)));
  }
  // What is left of the budget goes to the documents still lexing.
  for (const [at, result] of results.entries()) {
    const request = requests.find(entry => entry.id === result.id)!;
    if (result.line === -1 && spent < budget) results[at] = lex(request, budget - spent);
  }
  return results;
}

/** One round of the root's pump: the ask, the lexers' answer and the store. */
export function highlightRound(budget = CALL_CHARS): { finished: boolean; requests: Request[] } {
  const { requests } = highlightAsk();
  return { finished: highlightStore(highlightCode(requests, budget)).finished, requests };
}

/** The root's pump until no text waits. Whether a text got its tokens. */
export function settleHighlights(budget = CALL_CHARS): boolean {
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
