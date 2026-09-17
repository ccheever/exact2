// The dev connection owns complete, verified generations (LLP 1023 D3).
// A new epoch resets ordering; each commit replaces the entire asset roster.
const es = typeof EventSource === "undefined" ? null : new EventSource("/__dev");
const encoder = new TextEncoder();
let overlay = null;
function show(message) {
  if (!message) { overlay?.remove(); overlay = null; return; }
  overlay ??= document.body.appendChild(Object.assign(document.createElement("pre"), { style: "position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647" }));
  overlay.textContent = message + "\n(click to dismiss)";
  overlay.onclick = () => show(null);
}

// SHA-256 also works on trusted-LAN HTTP pages where SubtleCrypto is absent.
// The constants are the fractional square/cube roots of the first primes.
// All integer additions below are reduced modulo 2^32.
let hashConstants;
async function digest(bytes) {
  if (globalThis.crypto?.subtle) return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (n) => n.toString(16).padStart(2, "0")).join("");
  if (!hashConstants) {
    const primes = [];
    for (let n = 2; primes.length < 64; n++) if (primes.every((p) => n % p)) primes.push(n);
    hashConstants = { initial: primes.slice(0, 8).map((n) => (Math.sqrt(n) % 1 * 0x100000000) | 0),
      round: primes.map((n) => (Math.cbrt(n) % 1 * 0x100000000) | 0) };
  }
  const hash = hashConstants.initial.slice(), words = new Int32Array(64);
  const padded = new Uint8Array(Math.ceil((bytes.length + 9) / 64) * 64);
  padded.set(bytes); padded[bytes.length] = 128;
  const view = new DataView(padded.buffer);
  view.setUint32(padded.length - 8, Math.floor(bytes.length / 0x20000000));
  view.setUint32(padded.length - 4, bytes.length * 8);
  const rotate = (n, by) => n >>> by | n << (32 - by);
  for (let start = 0; start < padded.length; start += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getInt32(start + i * 4);
    for (let i = 16; i < 64; i++) {
      const a = words[i - 15], b = words[i - 2];
      words[i] = words[i - 16] + (rotate(a, 7) ^ rotate(a, 18) ^ a >>> 3) + words[i - 7] + (rotate(b, 17) ^ rotate(b, 19) ^ b >>> 10);
    }
    let [a, b, c, d, e, f, g, h] = hash;
    for (let i = 0; i < 64; i++) {
      const t1 = (h + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + (e & f ^ ~e & g) + hashConstants.round[i] + words[i]) | 0;
      const t2 = ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + (a & b ^ a & c ^ b & c)) | 0;
      h = g; g = f; f = e; e = d + t1 | 0; d = c; c = b; b = a; a = t1 + t2 | 0;
    }
    [a, b, c, d, e, f, g, h].forEach((n, i) => { hash[i] = hash[i] + n | 0; });
  }
  return hash.map((n) => (n >>> 0).toString(16).padStart(8, "0")).join("");
}
const validIdentity = (m) => m && /^[0-9a-f]{32}$/.test(m.epoch) && Number.isSafeInteger(m.seq) && m.seq >= 0 && /^[0-9a-f]{64}$/.test(m.generation);
const utf8Compare = (a, b) => {
  const aa = encoder.encode(a), bb = encoder.encode(b);
  for (let i = 0; i < Math.min(aa.length, bb.length); i++) if (aa[i] !== bb[i]) return aa[i] - bb[i];
  return aa.length - bb.length;
};
const validName = (name) => typeof name === "string" && /^(assets|deck|shaders)\//.test(name)
  && !name.includes("\\") && !name.includes(":") && !name.includes("\0")
  && name.split("/").every((part) => part && part !== "." && part !== "..");
const sameOrigin = (path, base) => {
  const url = new URL(path, base);
  if (url.origin !== location.origin || !["http:", "https:"].includes(url.protocol)) throw new Error("the dev generation leaves the app origin");
  return url.href;
};
async function readBounded(url, limit, signal) {
  const response = await fetch(url, { cache: "no-store", signal });
  sameOrigin(response.url || url, location.href);
  if (!response.ok) throw new Error(`generation fetch HTTP ${response.status}: ${url}`);
  if (Number(response.headers.get("content-length")) > limit) throw new Error("generation response exceeds its declared size");
  const reader = response.body.getReader(), chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      length += value.length;
      if (length > limit) throw new Error("generation response exceeds its declared size");
      chunks.push(value);
    }
  } catch (error) { await reader.cancel(); throw error; }
  const bytes = new Uint8Array(length);
  let at = 0; for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return { bytes, url: response.url || url };
}
async function readEnvelope(url, signal) {
  const response = await readBounded(sameOrigin(url, location.href), 64 * 1024, signal);
  return { envelope: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(response.bytes)), url: response.url };
}
async function fetchGeneration(message, signal) {
  const started = performance.now();
  const { envelope, url } = await readEnvelope(message.envelope, signal);
  const identity = envelope.dev;
  if (envelope.exact !== 1 || !validIdentity(identity) || identity.epoch !== message.epoch
      || identity.seq !== message.seq || identity.generation !== message.generation || identity.program !== message.program || !Array.isArray(envelope.assets)) throw new Error("the fetched envelope does not name the announced generation");
  await globalThis.exact.ready;
  if (envelope.app?.id !== globalThis.exact.compat?.inputs?.app) throw new Error("the dev generation names another app");
  const assets = new Map(), names = new Set();
  const load = async (card, name) => {
    if (!card || !Number.isSafeInteger(card.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024 || !/^[0-9a-f]{64}$/.test(card.sha256)) throw new Error(`invalid payload card: ${name}`);
    const path = sameOrigin(card.url, url);
    const { bytes } = await readBounded(path, card.bytes, signal);
    if (bytes.length !== card.bytes || await digest(bytes) !== card.sha256) throw new Error(`payload integrity failed: ${name}`);
    return { bytes, url: path, sha256: card.sha256 };
  };
  for (const card of envelope.assets) {
    if (!validName(card.name) || names.has(card.name)) throw new Error("invalid or duplicate asset name");
    names.add(card.name);
  }
  const moduleKeys = ['native', 'receipt', 'web'];
  if (envelope.module !== undefined && (!envelope.module || Object.keys(envelope.module).sort().join(',') !== moduleKeys.join(','))) throw new Error('invalid module manifest');
  const moduleCards = envelope.module ? moduleKeys.map(key => envelope.module[key]) : [];
  const rust = envelope.rust;
  if (rust && (!rust.wasm || Object.keys(rust).some(k => !["wasm", "native", "tiered"].includes(k)))) throw new Error("invalid Rust manifest");
  const rustCards = rust ? Object.values(rust).flatMap(v => [v.receipt, v.module]) : [];
  const cards = [envelope.plan, ...envelope.assets, ...moduleCards, ...rustCards];
  if (cards.some((card) => !Number.isSafeInteger(card?.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024)
      || cards.reduce((sum, card) => sum + card.bytes, 0) > 256 * 1024 * 1024) throw new Error("generation exceeds the payload budget");
  // All admitted payloads are independent. One bounded queue avoids separate
  // network rounds for the plan, assets and module without raising concurrency.
  const jobs = [[envelope.plan, "app.plan"], ...envelope.assets.map(card => [card, card.name]),
    ...(envelope.module ? ['receipt', 'web'].map(key => [envelope.module[key], `module ${key}`]) : []),
    ...(rust ? [[rust.wasm.receipt, 'Rust receipt'], [rust.wasm.module, 'Rust Wasm']] : [])];
  const loaded = new Array(jobs.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, jobs.length) }, async () => {
    while (next < jobs.length) {
      const index = next++;
      loaded[index] = await load(...jobs[index]);
    }
  }));
  const plan = loaded[0];
  envelope.assets.forEach((card, index) => assets.set(card.name, loaded[index + 1]));
  const moduleAt = 1 + envelope.assets.length;
  const module = envelope.module ? { receipt: loaded[moduleAt].bytes, script: loaded[moduleAt + 1].bytes } : null;
  const rustPayload = rust ? { receipt: loaded.at(-2).bytes, module: loaded.at(-1).bytes } : null;
  const canonical = { assets: [...envelope.assets].sort((a, b) => utf8Compare(a.name, b.name)).map((a) => ({ bytes: a.bytes, name: a.name, sha256: a.sha256 })),
    ...(envelope.module ? { module: Object.fromEntries(moduleKeys.map(key => [key, { bytes: envelope.module[key].bytes, sha256: envelope.module[key].sha256 }])) } : {}),
    plan: { bytes: plan.bytes.length, sha256: plan.sha256 },
    ...(rust ? { rust: Object.fromEntries(Object.keys(rust).sort().map(k => [k, { module: {bytes:rust[k].module.bytes,sha256:rust[k].module.sha256}, receipt:{bytes:rust[k].receipt.bytes,sha256:rust[k].receipt.sha256}, ...(k !== "wasm" ? {target:rust[k].target} : {}) }])) } : {}) };
  if (await digest(encoder.encode(JSON.stringify(canonical))) !== identity.generation) throw new Error("the generation digest does not bind its complete manifest");
  return { ...identity, plan: plan.bytes, assets, module, rust: rustPayload, fetchMs: performance.now() - started };
}

// Fetches may finish in any order. Only the latest request can enter the
// host's acceptance/commit operation; a failed candidate keeps the live one.
function generationClient({ fetchGeneration, apply, applied = () => {}, failed = () => {}, programChanged = () => {} }) {
  let epoch = null, queued = -1, committed = null, attempt = 0, inFlight = null, controller = null;
  const retired = new Set();
  let program = null, stopped = false;
  const stop = () => { stopped = true; attempt++; controller?.abort(); inFlight = null; programChanged(); };
  return {
    receive(message) {
      if (stopped) return Promise.resolve(false);
      if (message?.rebuilt) { stop(); return Promise.resolve(false); }
      if (!validIdentity(message) || retired.has(message.epoch)) return Promise.resolve(false);
      if (message.program !== undefined && !/^[0-9a-f]{64}$/.test(message.program)) return Promise.resolve(false);
      if (program && program !== message.program) { stop(); return Promise.resolve(false); }
      if (message.program) program = message.program;
      if (epoch !== message.epoch) { if (epoch !== null) retired.add(epoch); epoch = message.epoch; queued = -1; attempt++; }
      if (message.seq < queued || message.seq === queued && inFlight) return Promise.resolve(false);
      queued = message.seq;
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      const request = ++attempt, current = () => request === attempt;
      if (committed === message.generation) { inFlight = null; return Promise.resolve(false); }
      const activeController = controller;
      const timeout = setTimeout(() => activeController.abort(), 15000);
      const pending = (async () => {
        try {
          const candidate = await fetchGeneration(message, signal);
          if (!current()) return false;
          let accepted;
          try { accepted = await apply(candidate, current); }
          catch (error) { throw Object.assign(new Error(String(error)), { hostRefused: true }); }
          if (!current()) return false;
          if (!accepted) throw Object.assign(new Error("the host refused the current generation"), { hostRefused: true });
          committed = candidate.generation;
          applied(candidate);
          return true;
        } catch (error) { if (current()) { failed(error, current); } return false; }
        finally { clearTimeout(timeout); if (current()) inFlight = null; }
      })();
      inFlight = pending;
      return pending;
    },
  };
}

if (!es) globalThis.exactDevProtocol = { digest, generationClient, validIdentity };
if (es) {
  globalThis.exact.devError = show;
  // Host affordance, outside the app tree and absent from static/production pages.
  const opening = document.body.appendChild(document.createElement('a'));
  opening.href = '/__dev/open' + location.search + location.hash;
  opening.textContent = 'Open in native…';
  opening.style = 'position:fixed;right:10px;bottom:10px;padding:8px 12px;background:#fff;color:#164bc4;border:1px solid #ccd3df;border-radius:8px;font:13px system-ui;z-index:2147483646';
  // The module loader uses this implementation on trusted LAN HTTP too.
  globalThis.exact.moduleDigest = digest;
  let retry = null;
  const client = generationClient({ fetchGeneration, programChanged: () => { clearTimeout(retry); location.reload(); },
    apply: async (candidate, current) => {
      if (!current()) return false;
      const t = performance.now();
      const accepted = await globalThis.exact.reloadGeneration(candidate.plan, candidate.assets, current, candidate.module, candidate.rust);
      if (accepted) {
        navigator.sendBeacon(`/__dev/reloaded?epoch=${candidate.epoch}&seq=${candidate.seq}&dom=${Date.now()}&fetch=${candidate.fetchMs.toFixed(1)}&boot=${(performance.now() - t).toFixed(1)}`);
        requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?epoch=${candidate.epoch}&seq=${candidate.seq}&paint=${Date.now()}`));
      }
      return accepted;
    },
    applied: () => { clearTimeout(retry); show(null); },
    failed: (error, current) => {
      show(String(error)); console.error("exact dev:", String(error));
      clearTimeout(retry);
      // A deterministic host refusal needs a new edit, not repeated execution
      // of the same candidate every 250 ms. Network discovery still retries.
      if (error.hostRefused) return;
      const discover = async () => {
        if (!current()) return;
        try {
          const { envelope, url } = await readEnvelope("/exact.json", AbortSignal.timeout(15000));
          if (current() && validIdentity(envelope.dev)) client.receive({ ...envelope.dev, envelope: new URL("exact.json", new URL(envelope.plan.url, url)).href });
        } catch { if (current()) retry = setTimeout(discover, 1000); }
      };
      retry = setTimeout(discover, 250);
    },
  });
  es.onmessage = (event) => {
    let message;
    try { message = JSON.parse(event.data); } catch { show("the dev stream sent invalid JSON"); return; }
    if (message.error) { show(message.error); console.error("exact dev:", message.error); return; }
    if (message.fresh) { if (new URLSearchParams(location.search).get('agent') !== '1') location.reload(); return; }
    if (message.gpu !== undefined) {
      // A drive owns its clock and code. Do not change either behind the driver.
      if (new URLSearchParams(location.search).get('agent') === '1') return;
      globalThis.exact.gpuVersion = message.gpu;
      if (globalThis.exact.gpu) (async () => {
        if (message.gpu !== globalThis.exact.gpuVersion) return;
        const result = await globalThis.exact.gpu.swap(message.gpu);
        show(result.errors.join('\n') || null);
        navigator.sendBeacon(`/__dev/gpu?g=${message.gpu}&swap=${result.ms.toFixed(1)}`);
      })().catch(error => { show(String(error)); console.error('exact dev:', error); });
      return;
    }
    if (message.ready === false) return;
    client.receive(message);
  };
}
