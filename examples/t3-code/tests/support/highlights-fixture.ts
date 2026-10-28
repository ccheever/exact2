// The lexers in a test: data/highlights.wasm (tools/generate/gen-highlights.mjs) under Bun's WebAssembly, run as the
// data crate runs them (data/src/highlights.rs records), so a test sees the tokens the app paints once the root's pump
// has handed a text to them (app.contract highlightStep).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { highlightAsk, highlightPending, highlightStore } from '../../src/highlight/highlights';
import { highlight, type Token } from '../../src/highlight/timeline-highlight';
import { APP } from './app-files';

const PAGE = 65_536;
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

/** The UTF-16 line records of `text` under lexer `language` (highlights.rs `records`). */
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

/** The root's pump until no text waits: each ask's texts lexed and stored. Whether a text got its tokens. */
export function settleHighlights(): boolean {
  let finished = false;
  while (highlightPending()) {
    const { requests } = highlightAsk();
    finished = highlightStore(requests.map(request => ({ id: request.id, records: lineRecords(request.language, request.text) }))).finished || finished;
  }
  return finished;
}

/** The tokens `text` paints once the lexers have answered for it. */
export function highlighted(text: string, language: string, maxLineLength = 0): Token[] {
  highlight(text, language, maxLineLength);
  settleHighlights();
  return highlight(text, language, maxLineLength);
}
