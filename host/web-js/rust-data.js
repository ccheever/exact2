import { admitsSecret, createRequestExecutor, fetchHostAsset, sameGrantDeclaration, setAppGrantSet } from './admission.js';
import { rustGrantSet, tsGrantSet } from './admission-data.js';
import { boundedHttpBody, streamed } from './http-body.js';
// A Rust data module on the JS runtime (LLP 1029.000's seam, ABI 3,
// `logic/abi/src/lib.rs`): the app's importless module wasm and its plan's
// declarations (`app.bind.plan`, the plan without the bake's answers),
// fetched after first pixel; once bound and activated, every answer is a
// synchronous call, as the runner's `DataSource::answer` is. Until then the
// runtime keeps each resource's compiled value and asks again at `ready`.
const ABI = 3, utf8 = new TextEncoder(), text = new TextDecoder();

function writer() {
  let buf = new Uint8Array(256), at = 0;
  const room = n => { if (at + n > buf.length) { const b = new Uint8Array((at + n) * 2); b.set(buf); buf = b; } };
  const w = {
    u8(v) { room(1); buf[at++] = v; },
    u16(v) { room(2); new DataView(buf.buffer).setUint16(at, v, true); at += 2; },
    u32(v) { room(4); new DataView(buf.buffer).setUint32(at, v, true); at += 4; },
    f64(v) { room(8); new DataView(buf.buffer).setFloat64(at, v, true); at += 8; },
    u64(v) { room(8); new DataView(buf.buffer).setBigUint64(at, BigInt(v), true); at += 8; },
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
    u64: () => { const v = Number(d.getBigUint64(at, true)); at += 8; return v; },
    str: () => { const n = r.u32(), s = text.decode(b.subarray(at, at + n)); at += n; return s; },
    bytes: n => { const x = b.slice(at, at + n); at += n; return x; },
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

export async function install(data, sources, load = p => fetchHostAsset(p).then(r => r.arrayBuffer())) {
  const [wasm, plan] = await Promise.all([load('./rust/wasm/app.module.wasm'), load('./app.bind.plan')]);
  // Bytes, or a module a renderer compiled once for every render.
  const made = await WebAssembly.instantiate(wasm, {}), instance = made.instance ?? made;
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
  // A result (`read_result`): now, or an HTTP request for the host to run.
  const result = r => {
    const tag = r.u8();
    if (tag === 0) return { v: r.value() };
    // The source's own refusal refuses the commit, as the runner's does;
    // UnknownSource (2): a mixed app's TypeScript module may answer it.
    // A call the seam could not carry (`call` above) is the resource's
    // failure instead (LLP 1071 §7).
    if (tag >= 2 && tag <= 4) throw Object.assign(new Error(r.str()), { unknown: tag === 2, refuse: true });
    if (tag === 1 || tag === 6 || tag === 9) {
      const http = tag === 1 ? 'ordered' : `independent:${r.u32()}`;
      const scoped = r.u8() === 1, scope = r.str();
      const method = r.str(), url = r.str(), headers = [];
      for (let n = r.u32(); n--;) headers.push([r.str(), r.str()]);
      const n = r.u32(), body = r.bytes(n);
      return { req: { method, url, headers, body: text.decode(body), raw: body, http, maxResponseBytes: http === 'ordered' ? undefined : Number(http.split(':')[1]), scope: scoped ? scope : null, stream: tag === 9 } };
    }
    // Storage work (LLP 1027.001 D2): a portable request the host runs
    // through the web host's own `storage-request.js`, under its scope.
    if (tag === 5) { const payload = r.bytes(r.u32()), scoped = r.u8() === 1, scope = r.str(); return { req: { storage: text.decode(payload), scope: scoped ? scope : null } }; }
    throw new Error(`a Rust answer of kind ${tag} (surface work) is not carried by the JS target`);
  };
  const op = (code, fill) => { const w = writer(); w.u32(ABI); w.u8(code); fill?.(w); return call(w.done()); };
  let r = op(0); r.u8(); const meta = [r.str(), r.str()];
  if (!sameGrantDeclaration(rustGrantSet, meta[1])) throw new Error('Rust module grants differ from the admitted build');
  r = op(1, w => w.bytes(new Uint8Array(plan))); r.u8(); result(r);
  r = op(2); r.u8(); result(r);
  // One call (`call_request`): source, arguments by type, the store's
  // snapshot; its reply (`call_reply`): a store read, writes, the result.
  const callWith = (code, source, args, store, outcome) => {
    const r = op(code, w => {
      w.str(source); w.u8(6); w.u32(args.length);
      const t = (sources[source] ?? '').split('|')[0]; let i = 0; for (const a of args) i = encode(w, a, t, i);
      const pairs = [...store.map].filter(([k]) => admitsSecret(rustGrantSet, k));
      w.u32(pairs.length); for (const [k, v] of pairs) { w.str(k); w.str(v); }
      if (outcome) {
        if (outcome.storage) { w.u8(5); w.bytes(outcome.storage); }
        // One message of a stream (LLP 1016.000), the ABI's tag 8.
        else if (outcome.streamed) { const m = outcome.streamed; w.u8(8); w.str(m.event); w.str(m.id); w.str(m.data); w.u32(m.coalesced); }
        else if (outcome.failed) { w.u8(outcome.failed); w.str(outcome.message); }
        else { w.u8(0); w.u16(outcome.status); w.u32(outcome.headers.length); for (const [k, v] of outcome.headers) { w.str(k); w.str(v); } w.bytes(outcome.body); }
      }
    });
    if (r.u8() !== 2) throw new Error('expected a call reply');
    const observed = r.u8() === 1;
    const writes = [];
    for (let n = r.u32(); n--;) { const k = r.str(); const v = r.u8() ? r.str() : null; if (!admitsSecret(rustGrantSet, k)) throw new Error(`secret ${k} is not granted`); writes.push([k, v]); }
    let out;
    try { out = result(r); } catch (error) { if (error.refuse) for (const [k, v] of writes) store.set(k, v); throw error; }
    for (const [k, v] of writes) store.set(k, v);
    if (observed) out.store = true;
    return out;
  };
  const rust = (source, args, store) => callWith(3, source, args, store), ts = data.ts;
  data.answer = ts ? (source, args, store, target) => { try { return rust(source, args, store); } catch (e) { if (e.unknown) return ts(source, args, store, target); throw e; } } : rust;
  data.parse = (source, args, outcome, store) => callWith(4, source, args, store, outcome);
  // An overlay (`overlay_request`): what a resource shows while writes affect it; none from the Rust module asks
  // the TypeScript one's, beside it.
  const types = s => (sources[s] ?? '|').split('|'), tsOverlay = data.overlay;
  data.overlay = (source, args, answer, writes) => {
    const [params, result] = types(source);
    const r = op(5, w => {
      w.str(source); w.u8(6); w.u32(args.length); let i = 0; for (const a of args) i = encode(w, a, params, i);
      encode(w, answer, result); w.u32(writes.length);
      for (const x of writes) {
        const [p, res] = types(x.source); w.u64(x.id); w.str(x.mutation); w.str(x.source);
        w.u8(6); w.u32(x.args.length); let k = 0; for (const a of x.args) k = encode(w, a, p, k);
        if (x.reply === undefined) w.u8(0); else { w.u8(1); encode(w, x.reply, res); }
        w.u8(x.answered ? 1 : 0);
      }
    });
    if (r.u8() !== 3) throw new Error('expected an overlay reply');
    const tag = r.u8();
    if (tag === 2) throw new Error(r.str());
    // A source the Rust module does not answer: the TypeScript module's overlay, beside it.
    if (tag === 3) return tsOverlay?.(source, args, answer, writes);
    if (tag === 0) return undefined;
    const value = r.value(), keep = []; for (let n = r.u32(); n--;) keep.push(r.u64());
    return { value, keep };
  };
  // The host runs the request under this child's own authority. An absent
  // request scope is therefore the Rust child set, not the mixed-app union.
  // A stream (`Answer::stream`, LLP 1016.000) is the web host's own reader, under the same authority;
  // its end is the outcome a single request's would be.
  const run = createRequestExecutor(data.appId ?? meta[0], rustGrantSet, boundedHttpBody);
  data.fetch = (req, message, controller) => !req.stream ? run(req) : streamed(req, rustGrantSet, message, controller).then(o => o.kind
    ? { failed: o.kind, message: text.decode(o.body) }
    : { status: o.status, headers: o.headers ? o.headers.split('\n').map(l => [l.slice(0, l.indexOf(': ')), l.slice(l.indexOf(': ') + 2)]) : [], body: o.body });
  // What a loaded capability needs of the module (canvas2d.js's draws).
  data.logic = { exports: e, session, writer, reader, encode, ABI };
  data.appId ??= meta[0];
  data.grants = setAppGrantSet(tsGrantSet, rustGrantSet);
  for (const f of data.q.splice(0)) f();
}
