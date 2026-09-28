// A Rust data module on the exact3 runtime (LLP 1029.000's seam, ABI 3,
// `logic/abi/src/lib.rs`): the app's importless module wasm and its plan,
// fetched after first pixel; once bound and activated, every answer is a
// synchronous call, as the runner's `DataSource::answer` is. Until then the
// runtime keeps each resource's compiled value and asks again at `ready`.
const ABI = 3, utf8 = new TextEncoder(), text = new TextDecoder();

function writer() {
  let buf = new Uint8Array(256), at = 0;
  const room = n => { if (at + n > buf.length) { const b = new Uint8Array((at + n) * 2); b.set(buf); buf = b; } };
  const w = {
    u8(v) { room(1); buf[at++] = v; },
    u32(v) { room(4); new DataView(buf.buffer).setUint32(at, v, true); at += 4; },
    f64(v) { room(8); new DataView(buf.buffer).setFloat64(at, v, true); at += 8; },
    bytes(b) { w.u32(b.length); room(b.length); buf.set(b, at); at += b.length; },
    str(s) { w.bytes(utf8.encode(s)); },
    done: () => buf.subarray(0, at),
  };
  return w;
}
// A value by its declared type (`n b s u ?T [T {T…}`): the runtime's arrays
// are records or lists, and `null` is none, only by type.
function encode(w, v, t, i = 0) {
  const c = t[i++];
  if (c === 'n') { w.u8(0); w.f64(v); }
  else if (c === 'b') { w.u8(1); w.u8(v ? 1 : 0); }
  else if (c === 's') { w.u8(2); w.str(v); }
  else if (c === 'u') w.u8(3);
  else if (c === '?') { if (v == null) { w.u8(4); return skip(t, i); } w.u8(5); return encode(w, v, t, i); }
  else if (c === '[') { w.u8(6); w.u32(v.length); let end = skip(t, i); for (const x of v) end = encode(w, x, t, i); return v.length ? end : skip(t, i); }
  else if (c === '{') { w.u8(7); const types = []; while (t[i] !== '}') { types.push(i); i = skip(t, i); } w.u32(types.length); types.forEach((ti, k) => encode(w, v[k], t, ti)); return i + 1; }
  return i;
}
function skip(t, i) {
  const c = t[i++];
  if (c === '?' || c === '[') return skip(t, i);
  if (c === '{') { while (t[i] !== '}') i = skip(t, i); return i + 1; }
  return i;
}
function reader(b) {
  const d = new DataView(b.buffer, b.byteOffset, b.byteLength); let at = 0;
  const r = {
    u8: () => b[at++],
    u32: () => { const v = d.getUint32(at, true); at += 4; return v; },
    f64: () => { const v = d.getFloat64(at, true); at += 8; return v; },
    str: () => { const n = r.u32(), s = text.decode(b.subarray(at, at + n)); at += n; return s; },
    value() {
      const tag = r.u8();
      if (tag === 0) return r.f64();
      if (tag === 1) return r.u8() === 1;
      if (tag === 2) return r.str();
      if (tag === 3 || tag === 4) return null;
      if (tag === 5) return r.value();
      const n = r.u32(), out = new Array(n);
      for (let k = 0; k < n; k++) out[k] = r.value();
      return out;
    },
  };
  return r;
}

export async function install(data, sources) {
  const [wasm, plan] = await Promise.all([
    fetch('./rust/wasm/app.module.wasm').then(r => r.arrayBuffer()),
    fetch('./app.plan').then(r => r.arrayBuffer()),
  ]);
  const { instance } = await WebAssembly.instantiate(wasm, {});
  const e = instance.exports;
  if (e.exact_logic_abi() !== ABI) throw new Error(`Rust module ABI ${e.exact_logic_abi()} is not ${ABI}`);
  const session = e.exact_logic_create();
  const call = bytes => {
    const p = e.exact_logic_alloc(bytes.length);
    new Uint8Array(e.memory.buffer, p, bytes.length).set(bytes);
    const rc = e.exact_logic_call(session, p, bytes.length);
    e.exact_logic_dealloc(p, bytes.length);
    if (rc !== 0) throw new Error('Rust module rejected the call');
    const r = reader(new Uint8Array(e.memory.buffer, e.exact_logic_output(session), e.exact_logic_output_len(session)).slice());
    if (r.u32() !== ABI) throw new Error('logic ABI version differs');
    return r;
  };
  const result = r => {
    const tag = r.u8();
    if (tag === 0) return { v: r.value() };
    if (tag >= 2 && tag <= 4) throw new Error(r.str());
    throw new Error(`a Rust answer of kind ${tag} (host work) is not carried by the spike`);
  };
  const op = (code, fill) => { const w = writer(); w.u32(ABI); w.u8(code); fill?.(w); return call(w.done()); };
  let r = op(1, w => w.bytes(new Uint8Array(plan))); r.u8(); result(r);
  r = op(2); r.u8(); result(r);
  data.answer = (source, args) => {
    const r = op(3, w => { w.str(source); w.u8(6); w.u32(args.length); const t = sources[source] ?? ''; let i = 0; for (const a of args) i = encode(w, a, t, i); w.u32(0); });
    if (r.u8() !== 2) throw new Error('expected a call reply');
    r.u8(); // observed a store read
    if (r.u32() !== 0) throw new Error('a Rust store write is not carried by the spike');
    return result(r);
  };
  for (const f of data.q.splice(0)) f();
}
