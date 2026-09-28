import { createPrivateKey, createPublicKey, generateKeyPairSync, sign, verify } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

/** A refusal: printed as one line, exit 1. Anything else is a bug and keeps its stack. */
export class Refusal extends Error {}
export const refuse = (message) => { throw new Refusal(message); };

// ---------------------------------------------------------------- the signer

/** One JSON value in the signed representation. Write sorted entries
 * directly: rebuilding an object and then calling `JSON.stringify` is not
 * canonical for integer-like keys because JavaScript enumerates those in
 * numeric order regardless of insertion order. */
function requireUnicodeScalars(text) {
  for (let i = 0; i < text.length; i++) {
    const unit = text.charCodeAt(i);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const low = text.charCodeAt(i + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) refuse('the head carries an unpaired UTF-16 surrogate, not a Unicode scalar value');
      i++;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) refuse('the head carries an unpaired UTF-16 surrogate, not a Unicode scalar value');
  }
  return text;
}

export function canonicalJson(value, dropSignature = false) {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'string') return JSON.stringify(requireUnicodeScalars(value));
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || Object.is(value, -0)) refuse(`the head carries the unsafe or non-canonical integer ${value}; canonical bytes require exact integers (update/src/envelope.rs)`);
    return String(value);
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`;
  if (!value || typeof value !== 'object') refuse(`the head carries a ${typeof value}; canonical bytes require JSON values`);
  const keys = Object.keys(value).filter((key) => !dropSignature || key !== 'signature').map(requireUnicodeScalars)
    .sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
}

/** Refuse a numeric token whose spelling Rust's canonical parser cannot
 * admit. `JSON.parse` alone loses this evidence by normalizing `7.0`, `7e0`,
 * and `-0` to the number 7 or 0 before the signature check sees it. */
export function validateRawIntegers(text) {
  for (let i = 0; i < text.length;) {
    if (text[i] === '"') {
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] !== '\\') { i++; continue; }
        if (text[i + 1] !== 'u') { i += 2; continue; }
        const first = Number.parseInt(text.slice(i + 2, i + 6), 16);
        if (first >= 0xd800 && first <= 0xdbff) {
          const second = text.slice(i + 6, i + 8) === '\\u' ? Number.parseInt(text.slice(i + 8, i + 12), 16) : NaN;
          if (!(second >= 0xdc00 && second <= 0xdfff)) throw new Error('the envelope carries an escaped lone surrogate, not a Unicode scalar value');
          i += 12;
          continue;
        }
        if (first >= 0xdc00 && first <= 0xdfff) throw new Error('the envelope carries an escaped lone surrogate, not a Unicode scalar value');
        i += 6;
      }
      if (text[i] === '"') i++;
      continue;
    }
    if (text[i] !== '-' && (text[i] < '0' || text[i] > '9')) { i++; continue; }
    const token = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(text.slice(i))?.[0];
    if (!token) { i++; continue; } // JSON.parse below reports malformed JSON.
    if (!/^(?:0|-[1-9][0-9]*|[1-9][0-9]*)$/.test(token) || !Number.isSafeInteger(Number(token))) {
      throw new Error(`the envelope carries the non-canonical integer ${token}; canonical bytes require shortest exact integers`);
    }
    i += token.length;
  }
}

/** The bytes the signature covers: the head without its top-level
 * `signature`, keys sorted by UTF-8 bytes and emitted directly. */
export function canonicalBytes(head) {
  if (!head || typeof head !== 'object' || Array.isArray(head)) refuse('the envelope is not a JSON object');
  return Buffer.from(canonicalJson(head, true), 'utf8');
}

/** An Ed25519 public key from its 32 raw bytes: the SubjectPublicKeyInfo prefix the crypto API wants, then the bytes. */
export function publicKeyFromRaw(raw) {
  if (raw.length !== 32) refuse(`an Ed25519 public key is 32 bytes; this one is ${raw.length}`);
  return createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), raw]), format: 'der', type: 'spki' });
}

/** The 32 raw bytes of an Ed25519 public key object. */
const rawPublic = (key) => key.export({ type: 'spki', format: 'der' }).subarray(-32);

/** The signer for the app's `deploy.signing.key`: the PEM at `<keys>/<id>.pem`, refused by path when missing, and refused when the manifest's public key for that id is not this private key's. A dry run never calls this. */
export function loadSigner(app, keysDir) {
  const signing = app.manifest.deploy?.signing ?? {};
  const keyId = signing.key;
  if (!keyId) refuse(`${app.dir}/app.json names no deploy.signing.key: a publish signs every head (LLP 1026 D11); bun scripts/deploy.mjs keygen <id> makes one`);
  const path = resolve(keysDir, `${keyId}.pem`);
  if (!existsSync(path)) refuse(`no signing key at ${path} — bun scripts/deploy.mjs keygen ${keyId} --keys ${keysDir} writes one (and prints the public half for app.json's deploy.signing.keys["${keyId}"])`);
  let privateKey;
  try { privateKey = createPrivateKey(readFileSync(path)); } catch (e) { refuse(`${path} is not a PEM private key: ${e.message}`); }
  if (privateKey.asymmetricKeyType !== 'ed25519') refuse(`${path} is a ${privateKey.asymmetricKeyType} key; the head is signed with Ed25519`);
  const declared = signing.keys?.[keyId];
  if (!declared) refuse(`${app.dir}/app.json's deploy.signing.keys has no "${keyId}": the binary verifies with that key (LLP 1030 D3a); paste the public half keygen printed`);
  const manifestKey = publicKeyFromRaw(Buffer.from(declared, 'base64'));
  const probe = Buffer.from('exact deploy: is this key mine?');
  if (!verify(null, probe, manifestKey, sign(null, probe, privateKey))) refuse(`the manifest's public key for ${keyId} is not this private key's (${path}): a head signed here would be refused by every installed binary`);
  return {
    keyId,
    path,
    publicKey: rawPublic(createPublicKey(privateKey)).toString('base64'),
    /** The `signature` member for `head` (which must not carry one yet). */
    sign(head) { return { keyId, ed25519: sign(null, canonicalBytes(head), privateKey).toString('base64') }; },
  };
}

/** `keygen <id>`: a fresh Ed25519 key at `<keys>/<id>.pem` (mode 0600, never overwritten) and its public half printed for the manifest. */
export function keygen(opts, usage) {
  const id = opts._[1];
  if (!id || !/^[A-Za-z0-9._-]+$/.test(id)) refuse(`keygen <id>: letters, digits, . _ - (it names the PEM and the manifest entry)\n${usage}`);
  const path = resolve(opts.keys, `${id}.pem`);
  if (existsSync(path)) refuse(`${path} exists: a signing key is never overwritten — remove it yourself, or pick another id (a rotation is a new cohort, LLP 1030 D3a)`);
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  mkdirSync(opts.keys, { recursive: true, mode: 0o700 });
  try {
    writeFileSync(path, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600, flag: 'wx' });
  } catch (error) {
    if (error.code === 'EEXIST') refuse(`${path} exists: a signing key is never overwritten — remove it yourself, or pick another id (a rotation is a new cohort, LLP 1030 D3a)`);
    throw error;
  }
  const raw = rawPublic(publicKey).toString('base64');
  if (opts.json) console.log(JSON.stringify({ id, path, publicKey: raw }));
  else console.log(`wrote ${path}\npaste into app.json → deploy.signing.keys["${id}"]: ${JSON.stringify(raw)}`);
  return 0;
}
