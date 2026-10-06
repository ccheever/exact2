// The grant admission, shared by the wasm host (glue.js, through navigation.js's re-export), the
// JavaScript target (admission.js) and the lazy host modules (http-body.js, module-glue.js). A module
// of its own: a page whose entry admits nothing (no TypeScript source, no fetch before a lazy chunk)
// does not carry it in its first download; the chunk that asks loads it.

// Match only the sealed, typed output of exact-runner's Rust grant parser.
// App code is the page, so this is parity admission rather than a sandbox.
const INVALID_GRANTS = 'the grant set was not validated';

const grantFNV = text => {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
};
// Rust's JSON writer and JavaScript's serializer use this one spelling for the
// two ECMAScript line separators before the seal is calculated.
const grantBody = set => JSON.stringify({ version: 1, entries: set.entries, error: set.error })
  .replaceAll('\u2028', '\\u2028').replaceAll('\u2029', '\\u2029');
const networkGrants = new Set(['fetch', 'fetch-subdomains', 'websocket']);
const pathGrants = new Set(['fs-read', 'fs-write', 'sqlite-open']);
const nameGrants = new Set(['env-read', 'secret-keep', 'storage-kv']);
const validGrantName = name => typeof name === 'string' && name.length >= 1 && name.length <= 64
  && /^[a-z0-9._-]+$/.test(name) && !/^\.+$/.test(name);
// Rust `str::trim`/`split_whitespace` use Unicode White_Space, which differs
// from JavaScript's `\s` at U+0085 and U+FEFF.
const rustSpace = '[\\u0009-\\u000d\\u0020\\u0085\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000]';
const rustTrim = value => String(value).replace(new RegExp(`^${rustSpace}+|${rustSpace}+$`, 'g'), '');
const rustWords = value => rustTrim(value).split(new RegExp(`${rustSpace}+`)).filter(Boolean);
const nativeNamespace = value => typeof value === 'string' && /^win:[A-Z]$/.test(value);
const nativeLeaf = value => {
  if (typeof value !== 'string' || !value || value.length > 255 || value === '.' || value === '..'
      || /[\\/:*?"<>|\u0000-\u001f\u007f-\u009f]/.test(value) || /[. ]$/.test(value)
      || /[\ud800-\udfff]/u.test(value)) return false;
  const base = value.split('.')[0].replace(/[a-z]/g, char => char.toUpperCase());
  return !/^(?:CON|PRN|AUX|NUL|CLOCK\$|(?:COM|LPT)[1-9¹²³])$/.test(base);
};
const grantTupleValid = grant => {
  if (!Array.isArray(grant) || typeof grant[0] !== 'string') return false;
  if (networkGrants.has(grant[0])) {
    const address = grant[2]?.startsWith('[')
      || /^(?:https?|wss?|ftp)$/.test(grant[1]) && /^[\d.]+$/.test(grant[2]);
    return grant.length === 4
    && /^[a-z][a-z0-9+.-]*$/.test(grant[1])
    && typeof grant[2] === 'string' && grant[2] === grant[2].toLowerCase() && grant[2].length > 0
    && Number.isInteger(grant[3]) && grant[3] >= 0 && grant[3] <= 65535
    && (grant[0] !== 'fetch-subdomains' || !address && !grant[2].endsWith('.') && grant[2].split('.').filter(Boolean).length >= 2);
  }
  if (pathGrants.has(grant[0])) return grant.length >= 2
    && (['', 'app:', 'doc:'].includes(grant[1]) || nativeNamespace(grant[1]))
    && grant.slice(2).every(component => nativeNamespace(grant[1]) ? nativeLeaf(component)
      : typeof component === 'string' && component && component !== '.' && component !== '..' && !component.includes('/'));
  if (!nameGrants.has(grant[0]) || grant.length !== 2 || typeof grant[1] !== 'string') return false;
  return grant[0] === 'env-read' || validGrantName(grant[1]);
};

const pathTuple = (kind, target) => {
  const native = target.startsWith('\\\\?\\') ? target.slice(4) : target;
  if (/^[a-z]:[/\\]/i.test(native)) {
    const parts = native.slice(3).split(/[/\\]/).filter(Boolean);
    return parts.every(nativeLeaf) ? [kind, `win:${native[0].toUpperCase()}`, ...parts] : null;
  }
  const at = target.indexOf(':/');
  const namespace = at < 0 ? target.startsWith('/') ? '' : null : target.slice(0, at) + ':';
  if (namespace == null || !['', 'app:', 'doc:'].includes(namespace)) return null;
  const rest = at < 0 ? target.slice(1) : target.slice(at + 2);
  const parts = rest.split('/').filter(Boolean);
  return parts.some(part => part === '.' || part === '..') ? null : [kind, namespace, ...parts];
};
const networkTuple = (kind, target) => {
  const wildcard = kind === 'fetch-subdomains';
  if (wildcard && !target.includes('://*.')) return null;
  // Userinfo is refused, as in Rust: `https://a.example@evil.com` is evil.com.
  if (target.includes('@')) return null;
  try {
    const url = new URL(wildcard ? target.replace('://*.', '://') : target);
    const port = Number(url.port || ({ 'http:': 80, 'https:': 443, 'ws:': 80, 'wss:': 443, 'ftp:': 21 })[url.protocol]);
    if (!url.hostname || !Number.isInteger(port)) return null;
    if (wildcard && (!['', '/'].includes(url.pathname) || url.search || url.hash)) return null;
    return [kind, url.protocol.slice(0, -1).toLowerCase(), url.hostname.toLowerCase(), port];
  } catch { return null; }
};
const sourceTuple = source => {
  const words = rustWords(source);
  const capability = words[0];
  if (capability === 'fs.read' || capability === 'fs.write') {
    const rest = rustTrim(source.slice(capability.length));
    if (rest.startsWith('"')) {
      try {
        const target = JSON.parse(rest);
        // serde_json refuses lone UTF-16 surrogates; JSON.parse does not.
        if (typeof target !== 'string' || /[\u0000-\u001f\u007f-\u009f]/.test(target)
            || /[\ud800-\udfff]/u.test(target)) return null;
        return pathTuple(capability === 'fs.read' ? 'fs-read' : 'fs-write', target);
      } catch { return null; }
    }
  }
  if (words.length !== 2) return null;
  const target = words[1];
  if (capability === 'net.fetch') return networkTuple(target.includes('://*.') ? 'fetch-subdomains' : 'fetch', target);
  if (capability === 'net.websocket') return target.includes('*') ? null : networkTuple('websocket', target);
  if (capability === 'fs.read') return pathTuple('fs-read', target);
  if (capability === 'fs.write') return pathTuple('fs-write', target);
  if (capability === 'sqlite.open') return pathTuple('sqlite-open', target);
  if (capability === 'env.read') return ['env-read', target];
  if (capability === 'secret.keep') return ['secret-keep', target];
  if (capability === 'storage.kv') return ['storage-kv', target];
  return null;
};
const exactOnly = source => source.startsWith('#') || source.startsWith('surface.read ')
  || source.startsWith('surface.write ') || source.startsWith('device.') || source.startsWith('auth.');
const entryMatchesSource = ([, source, grant, error]) => {
  if (grant === null) return error !== null || exactOnly(source);
  return error === null && JSON.stringify(sourceTuple(source)) === JSON.stringify(grant);
};

function validateGrantSet(set) {
  if (!set || set.version !== 1 || !Array.isArray(set.entries)
      || (set.error !== null && typeof set.error !== 'string') || !/^[0-9a-f]{16}$/.test(set.seal ?? '')) return false;
  let last = 0, hasError = false;
  for (const entry of set.entries) {
    if (!Array.isArray(entry) || entry.length !== 4 || !Number.isInteger(entry[0]) || entry[0] <= last
        || typeof entry[1] !== 'string' || !entry[1] || rustTrim(entry[1]) !== entry[1]
        || entry[2] !== null && !grantTupleValid(entry[2])
        || entry[3] !== null && typeof entry[3] !== 'string' || !entryMatchesSource(entry)) return false;
    last = entry[0];
    hasError ||= entry[3] !== null;
  }
  return (!hasError || set.error !== null) && grantFNV(grantBody(set)) === set.seal;
}

const freezeGrantSet = set => {
  for (const entry of set.entries) {
    if (entry[2]) Object.freeze(entry[2]);
    Object.freeze(entry);
  }
  Object.freeze(set.entries);
  return Object.freeze(set);
};

// The Rust parser is the only producer. Admission validates an incoming JSON
// value in full before making its authority immutable; a prior validation is
// never a reason to trust a mutable or branded object.
export function createGrantSet(value) {
  if (!validateGrantSet(value)) return freezeGrantSet(makeGrantSet([], INVALID_GRANTS));
  return freezeGrantSet(value);
}

function makeGrantSet(entries, error) {
  const set = { version: 1, entries, error, seal: '' };
  set.seal = grantFNV(grantBody(set));
  return freezeGrantSet(set);
}

export function grantError(set) {
  return validateGrantSet(set) ? set.error : INVALID_GRANTS;
}

export const rawGrantText = set => validateGrantSet(set) ? set.entries.map(entry => entry[1]).join('\n') : '';
export const hasGrant = (set, kind) => validateGrantSet(set) && !set.error && set.entries.some(entry => entry[2]?.[0] === kind);

const normalizedDeclaration = set => validateGrantSet(set)
  ? set.entries.map(([, source, grant, error]) => JSON.stringify([source, grant, error])) : [];

// Module metadata carries the author's spelling while the bake carries the
// Rust parser's normalized set. Blank lines and indentation have no bearing
// on activation; the normalized declarations do.
export function sameGrantDeclaration(set, source) {
  if (typeof source !== 'string' || !validateGrantSet(set)) return false;
  const child = scopedGrantSet(set, source);
  const parentLines = normalizedDeclaration(set), childLines = normalizedDeclaration(child);
  return parentLines.length === childLines.length && parentLines.every((line, index) => line === childLines[index]);
}

export function scopedGrantSet(parent, source) {
  if (source == null) return parent;
  if (!validateGrantSet(parent) || typeof source !== 'string') return makeGrantSet([], 'source scope exceeds the app\'s admitted grants');
  const available = new Map(parent.entries.map(entry => [entry[1], entry]));
  const entries = [], errors = [];
  for (const [index, raw] of source.split('\n').entries()) {
    const line = rustTrim(raw);
    if (!line) continue;
    const found = available.get(line);
    if (!found) return makeGrantSet([], 'source scope exceeds the app\'s admitted grants');
    entries.push([index + 1, line, found[2], found[3]]);
    if (found[3]) errors.push(`line ${index + 1}: ${found[3]}`);
  }
  return makeGrantSet(entries, errors.length ? `the app's grants did not parse: ${errors.join('; ')}` : null);
}

export function unionGrantSets(...sets) {
  if (sets.some(set => !validateGrantSet(set))) return makeGrantSet([], INVALID_GRANTS);
  const entries = [], seen = new Set();
  for (const set of sets) for (const entry of set.entries) if (!seen.has(entry[1])) {
    seen.add(entry[1]);
    entries.push([entries.length + 1, entry[1], entry[2], entry[3]]);
  }
  const error = sets.map(set => set.error).find(Boolean) ?? null;
  return makeGrantSet(entries, error);
}

const grantPort = url => Number(url.port || ({ 'http:': 80, 'https:': 443, 'ws:': 80, 'wss:': 443, 'ftp:': 21 })[url.protocol]);
export function admitsNetwork(set, value, operation = 'fetch') {
  if (grantError(set)) return false;
  let target;
  try { target = new URL(value); } catch { return false; }
  const kind = operation === 'websocket' ? 'websocket' : 'fetch';
  const scheme = target.protocol.slice(0, -1).toLowerCase(), host = target.hostname.toLowerCase(), port = grantPort(target);
  return set.entries.some(([, , grant]) => grant && grant[1] === scheme && grant[3] === port && (
    grant[0] === kind && grant[2] === host
    || kind === 'fetch' && grant[0] === 'fetch-subdomains' && host.length > grant[2].length + 1 && host.endsWith('.' + grant[2])
  ));
}

export function admitsSecret(set, name) {
  return !String(name).startsWith('exact.kept.') && !grantError(set)
    && set.entries.some(([, , grant]) => grant?.[0] === 'secret-keep' && grant[1] === String(name));
}

function grantPathParts(path) {
  if (typeof path !== 'string') return null;
  const at = path.indexOf(':/');
  const namespace = at < 0 ? path.startsWith('/') ? '' : null : path.slice(0, at) + ':';
  if (namespace == null || !['', 'app:', 'doc:'].includes(namespace)) return null;
  const rest = at < 0 ? path.slice(1) : path.slice(at + 2);
  const parts = rest.split('/').filter(Boolean);
  return parts.some(part => part === '.' || part === '..' || part.includes('\0')) ? null : [namespace, ...parts];
}

export function coversPath(set, capability, path) {
  if (grantError(set)) return false;
  const target = grantPathParts(path), kind = ({ 'fs.read': 'fs-read', 'fs.write': 'fs-write', 'sqlite.open': 'sqlite-open' })[capability];
  return !!target && set.entries.some(([, , grant]) => grant?.[0] === kind && grant.slice(1).every((part, index) => target[index] === part));
}
