// The publisher and its origin, driven: stream heads, admission and sequence
// allocation, classification tables, signing keys, source snapshots, stream
// commits and origin handles (`scripts/deploy.mjs`, `scripts/origin.mjs`).
// `bun test ./scripts/deploy.test.mjs`.
import { test } from 'bun:test';
// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = `${process.env.CARGO_HOME ?? `${process.env.HOME}/.cargo`}/bin`;
if (!(process.env.PATH ?? '').split(':').includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}:${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;
import { spawn, spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign as cryptoSign } from 'node:crypto';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readlinkSync, realpathSync, renameSync, writeFileSync, rmSync, symlinkSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { isAbsolute, join, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { copyStaticTreeIfPresent, webEnvelope } from '../host/web/serve.mjs';
import { resolveApp, withAppFixture } from './app.mjs';
import { canonicalBytes, classify, classifyArtifacts, cohortReceipt, defaultRelease, deployRun, inspectHead, materializeSnapshot, portableAssetNames, publishStream, renderTable, snapshotOf, streamHead } from './deploy.mjs';
import { blobPath, DirectoryOrigin, HttpsOrigin, OriginUnavailable, sha256 } from './origin.mjs';
// Minimal compiler receipts for origin protocol fixtures below; capability
// behavior is tested independently with declared requirements.
const fixtureBuild = (id, bundle) => ({version:1,compat:{id,inputs:{store:{L:'A'},executors:[]},target:'fixture'},
  graph:{version:1,sources:{},artifacts:[{name:'app.plan',requires:{},sha256:bundle.plan.sha256,bytes:bundle.plan.bytes?.length??0}]},binary:{sha256:'0'.repeat(64)}});
const DEPLOY = join(dirname(fileURLToPath(import.meta.url)), 'deploy.mjs');

// Each case is checked while the file loads and reported as a bun:test test.
function result(name, ok, detail = '') {
  test(name, () => { if (!ok) throw new Error(detail || `${name}: failed`); });
}
async function rejects(action, matches) {
  try { await action(); return false; } catch (error) { return matches(error); }
}
{
  const id = 'com.exact.names';
  const bytes = Buffer.alloc(36 + Buffer.byteLength(id));
  bytes.write('EXPL'); bytes.writeUInt32LE(4, 4); bytes.writeBigUInt64LE(1n, 16);
  bytes.writeUInt32LE(Buffer.byteLength(id), 32); bytes.write(id, 36);
  const app = { name: 'internal-slug', id, displayName: 'Cross-platform Display', manifest: { name: 'Web Install Name' } };
  const web = webEnvelope(app, bytes, []);
  const mismatchedPlanRefused = await rejects(() => webEnvelope({ ...app, id: 'com.exact.another' }, bytes, []), error => error.message.includes(`plan is for ${id}`));
  const stream = streamHead({ app, bundle: { plan: { bytes, sha256: web.plan.sha256, formatVersion: 4, kernelSchema: '0000000000000001' }, assets: [] },
    stream: { channel: 'prod', compatibilityId: 'a'.repeat(32) }, seq: 1, release: 'test' });
  result('web and stream envelopes use the cross-platform display name', app.name !== app.manifest.name
    && app.manifest.name !== app.displayName && web.app.name === app.displayName
    && stream.app.name === web.app.name && mismatchedPlanRefused);
}
// "Current" means the production client admits the complete authenticated
// head. An unusable head contributes no rollback floor: repair advances from
// the largest immutable record whose embedded envelope verifies, or refuses
// when no such history exists. Raw JSON spellings rejected by Rust are also
// rejected before JavaScript can normalize them. The locked publisher repeats
// the same admission in case the head changed after the table was printed.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-head-admission-'));
  const compatibilityId = 'a'.repeat(32);
  const stream = { channel: 'prod', compatibilityId };
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const publicRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64');
  const app = { id: 'com.exact.admission', displayName: 'Admission', dir,
    manifest: { deploy: { signing: { key: 'test', keys: { test: publicRaw } } } } };
  const planBytes = Buffer.from('same plan');
  const assetBytes = Buffer.from('same asset');
  const bundle = {
    plan: { bytes: planBytes, sha256: createHash('sha256').update(planBytes).digest('hex'), formatVersion: 4, kernelSchema: '0'.repeat(16) },
    assets: [{ name: 'assets/icon.png', bytes: assetBytes, sha256: createHash('sha256').update(assetBytes).digest('hex') }],
  };
  const signer = { keyId: 'test', sign: (head) => ({ keyId: 'test', ed25519: cryptoSign(null, canonicalBytes(head), privateKey).toString('base64') }) };
  const signed = (edit = () => {}) => {
    const head = streamHead({ app, bundle, stream, seq: 7, release: 'old' });
    head.cohort = cohortReceipt(fixtureBuild(compatibilityId,bundle));
    edit(head);
    head.signature = signer.sign(head);
    return head;
  };
  const found = (head) => {
    const bytes = Buffer.from(JSON.stringify(head) + '\n');
    return { json: head, bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
  };
  const rawFound = (text) => {
    const bytes = Buffer.from(text + '\n');
    return { json: JSON.parse(text), bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
  };
  const valid = signed();
  // A host display-name change repairs metadata without losing signed capabilities.
  const renameDir=mkdtempSync(join(tmpdir(),'exact-renamed-cohort-')), renameOrigin=new DirectoryOrigin(renameDir);
  await renameOrigin.putHead(stream,found(valid).bytes,{previousDigest:null});
  await renameOrigin.put(`.exact/${stream.channel}/${compatibilityId}/releases/old.json`,Buffer.from(JSON.stringify({platform:'linux',envelope:valid})),{immutable:true});
  const renamed={...app,displayName:'Renamed'}, build=fixtureBuild(compatibilityId,bundle);
  const renameArgs={app:renamed,opts:{platform:['linux']},origin:renameOrigin,channel:stream.channel,snapshot:{commit:'rename',dirty:false,changes:[],repo:dir},release:'rename',bundle,compat:{linux:build.compat},builds:{linux:build},platforms:['linux'],wantOrigin:false};
  const renameTable=await classify(renameArgs), renameRow=renameTable.rows.find(r=>r.kind==='stream');
  const renamePublished=await publishStream({origin:renameOrigin,row:renameRow,bundle,compat:build.compat,app:renamed,signer,release:'rename',snapshot:renameArgs.snapshot,opts:{},log:()=>{},build});
  const changed=structuredClone(build);changed.graph.artifacts[0].requires.sources={camera:{params:[],result:'String'}};
  const afterRename=await classify({...renameArgs,release:'unsupported',builds:{linux:changed}});
  const renamedHead=await renameOrigin.head(stream);
  result('display-name repair preserves the authenticated cohort and still refuses new capabilities',renameRow.action==='bundle'&&renamePublished.seq===8
    &&renamedHead.json.app.name==='Renamed'&&renamedHead.json.stream.seq===8&&inspectHead(renamedHead,renamed,stream).authenticated
    &&afterRename.rows.find(r=>r.kind==='stream').reason.includes('sources.camera'));
  const foreign='deadbeefdeadbeefdeadbeefdeadbeef', forged=structuredClone(renamedHead.json);forged.stream.compatibilityId=foreign;
  await renameOrigin.putHead({...stream,compatibilityId:foreign},Buffer.from(JSON.stringify(forged)),{previousDigest:null});
  const independent=await classify({...renameArgs,release:'unrelated'});
  result('an unauthenticated unrelated stream cannot block a healthy cohort',independent.rows.some(r=>r.compatibilityId===compatibilityId&&r.action==='current')
    &&independent.rows.some(r=>r.compatibilityId===foreign&&r.action==='binary')&&independent.notes.some(n=>n.includes(foreign)&&n.includes('does not verify')));
  rmSync(renameDir,{recursive:true,force:true});
  const history = Buffer.from(JSON.stringify({ envelope: valid }) + '\n');
  const tableFor = (candidate, withHistory = true) => classify({ app, opts: { platform: [] },
    origin: {
      kind: 'directory', describe: () => dir,
      get: async (name) => withHistory && name.endsWith('/releases/old.json') ? history : null,
      head: async () => candidate === null ? null : candidate?.bytes ? candidate : found(candidate),
      list: async (name) => withHistory && name.endsWith('/releases') ? ['old.json'] : [],
    },
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, builds:{linux:fixtureBuild(compatibilityId,bundle)}, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const missingSignature = signed(); delete missingSignature.signature;
  const badSignature = signed(); badSignature.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const forgedLow = signed((head) => { head.stream.seq = 2; }); forgedLow.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const forgedHuge = signed((head) => { head.stream.seq = Number.MAX_SAFE_INTEGER; }); forgedHuge.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const noSeq = signed((head) => { head.stream.seq = 'seven'; });
  const decimal = rawFound(JSON.stringify(valid).replace('"seq":7', '"seq":7.0'));
  const exponent = rawFound(JSON.stringify(valid).replace('"seq":7', '"seq":7e0'));
  const zeroHead = signed((head) => { head.unknownNumber = 0; });
  const negativeZero = rawFound(JSON.stringify(zeroHead).replace('"unknownNumber":0', '"unknownNumber":-0'));
  const scalarHead = { ...valid, unknownText: String.fromCharCode(0xd800) };
  const loneSurrogate = rawFound(JSON.stringify(scalarHead));
  const variants = [
    missingSignature,
    badSignature,
    forgedLow,
    forgedHuge,
    noSeq,
    decimal,
    exponent,
    negativeZero,
    loneSurrogate,
    signed((head) => { head.stream.channel = 'beta'; }),
    signed((head) => { head.exact = 2; }),
    signed((head) => { delete head.plan.bytes; }),
  ];
  const validTable = await tableFor(valid);
  const reordered = { cohort: valid.cohort, signature: valid.signature, stream: valid.stream, release: valid.release,
    plan: valid.plan, exact: valid.exact, assets: valid.assets, app: valid.app };
  const prettyBytes = Buffer.from(JSON.stringify(reordered, null, 2) + '\n');
  const pretty = { json: reordered, bytes: prettyBytes, sha256: createHash('sha256').update(prettyBytes).digest('hex') };
  const numberedKeys = signed((head) => { head.unknownKeys = { 2: 'two', 10: 'ten' }; });
  const numberedCanonical = canonicalBytes(numberedKeys).toString('utf8');
  const surrogateValueRefused = await rejects(() => canonicalBytes({ value: String.fromCharCode(0xd800) }), error => error.message.includes('not a Unicode scalar value'));
  const surrogateKeyRefused = await rejects(() => canonicalBytes({ [String.fromCharCode(0xdc00)]: 'value' }), error => error.message.includes('not a Unicode scalar value'));
  const repaired = await Promise.all(variants.map((candidate) => tableFor(candidate)));
  const missingHeadTable = await tableFor(null);
  const emptyHeadTable = await tableFor(null, false);
  const unlistableTable = await classify({ app, opts: { platform: [] },
    origin: {
      kind: 'https', describe: () => 'https://origin.example', get: async () => null,
      head: async () => null, list: async () => null,
    },
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, builds:{linux:fixtureBuild(compatibilityId,bundle)}, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const noHistoryRefused = await rejects(async () => await tableFor(badSignature, false), error => error.message.includes('release history has no authenticated sequence floor'));
  let unknownHistoryRefused = false;
  let unknownHistoryWrote = false;
  const unknownOrigin = {
    kind: 'object', describe: () => 'unknown-object-origin', get: async () => null,
    head: async () => null, list: async () => null,
    withLock: async (_stream, body) => body(),
    put: async () => { unknownHistoryWrote = true; },
    putHead: async () => { unknownHistoryWrote = true; },
  };
  try {
    await publishStream({ origin: unknownOrigin,
      row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
      bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'unknown-history',
      snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  } catch (error) { unknownHistoryRefused = error.message.includes('cannot enumerate its authenticated release history'); }
  const badHistoryOrigin = (bytes) => ({
    kind: 'directory', describe: () => 'bad-history-origin',
    get: async (name) => name.endsWith('/releases/broken.json') ? bytes : null,
    head: async () => null, list: async () => ['broken.json'],
  });
  const foreignHistory = Buffer.from(JSON.stringify({ envelope: signed((head) => { head.app.id = 'com.exact.foreign'; }) }) + '\n');
  const badHistories = [Buffer.from('{not json'), Buffer.from(JSON.stringify({ envelope: missingSignature }) + '\n'), foreignHistory];
  const badHistoryClassifyRefused = [];
  for (const bytes of badHistories) {
    try {
      await classify({ app, opts: { platform: [] }, origin: badHistoryOrigin(bytes),
        channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
        release: 'next', web: dir, bundle, builds:{linux:fixtureBuild(compatibilityId,bundle)}, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
        platforms: ['linux'], wantOrigin: false });
      badHistoryClassifyRefused.push(false);
    } catch (error) { badHistoryClassifyRefused.push(error.message.includes('nonempty release history has no authenticated sequence floor')); }
  }
  let corruptHistoryPublishRefused = false;
  let corruptHistoryWrote = false;
  try {
    await publishStream({ origin: {
      ...badHistoryOrigin(Buffer.from('{not json')), withLock: async (_stream, body) => body(),
      put: async () => { corruptHistoryWrote = true; }, putHead: async () => { corruptHistoryWrote = true; },
    }, row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'corrupt-history',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  } catch (error) { corruptHistoryPublishRefused = error.message.includes('nonempty release history has no authenticated sequence floor'); }
  const vanishedTable = await classify({ app, opts: { platform: [] }, origin: badHistoryOrigin(null),
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, builds:{linux:fixtureBuild(compatibilityId,bundle)}, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const vanishedHistoryUnavailable = vanishedTable.rows[0].action === 'unavailable'
    && vanishedTable.rows[0].reason.includes('disappeared while establishing');
  const origin = new DirectoryOrigin(dir);
  mkdirSync(join(dir, '.exact', stream.channel, compatibilityId), { recursive: true });
  writeFileSync(join(dir, '.exact', stream.channel, compatibilityId, 'exact.json'), found(forgedHuge).bytes);
  await origin.put(`.exact/${stream.channel}/${compatibilityId}/releases/old.json`, history, { immutable: true });
  const locked = await publishStream({ origin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'current', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'locked-repair',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const repairedHead = await origin.head(stream);
  const missingDir = mkdtempSync(join(tmpdir(), 'exact-missing-head-'));
  const missingOrigin = new DirectoryOrigin(missingDir);
  await missingOrigin.put(`.exact/${stream.channel}/${compatibilityId}/releases/old.json`, history, { immutable: true });
  const missingPublished = await publishStream({ origin: missingOrigin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'after-missing',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const hiddenDir = mkdtempSync(join(tmpdir(), 'exact-hidden-history-'));
  const hiddenOrigin = new DirectoryOrigin(hiddenDir);
  await hiddenOrigin.put(`.exact/${stream.channel}/${compatibilityId}/releases/.old.json`, history, { immutable: true });
  const hiddenTable = await classify({ app, opts: { platform: [] }, origin: hiddenOrigin,
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'after-hidden', web: dir, bundle, builds:{linux:fixtureBuild(compatibilityId,bundle)}, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const hiddenPublished = await publishStream({ origin: hiddenOrigin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'after-hidden',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const hiddenReleaseCli = spawnSync(process.execPath,
    [DEPLOY, 'caltrain', '--release', '.hidden', '--json'],
    { cwd: join(dirname(DEPLOY), '..'), encoding: 'utf8' });
  result('deploy calls only an admitted authenticated head current', validTable.rows[0].action === 'current'
    && validTable.rows[0].seq === 7 && inspectHead(found(valid), app, stream).usable && inspectHead(pretty, app, stream).usable
    && inspectHead(found(numberedKeys), app, stream).usable
    && numberedCanonical.includes('"unknownKeys":{"10":"ten","2":"two"}')
    && surrogateValueRefused && surrogateKeyRefused
    && missingHeadTable.rows[0].action === 'bundle' && missingHeadTable.rows[0].seq === 8
    && emptyHeadTable.rows[0].action === 'bundle' && emptyHeadTable.rows[0].seq === 1
    && unlistableTable.rows[0].action === 'unavailable'
    && unlistableTable.rows[0].reason.includes('cannot enumerate its authenticated release history')
    && repaired.every((table) => table.rows[0].action === 'bundle' && table.rows[0].seq === 8
      && table.rows[0].changes[0].name === 'exact.json' && table.rows[0].changes[0].change === 'repair')
    && noHistoryRefused && unknownHistoryRefused && !unknownHistoryWrote
    && badHistoryClassifyRefused.every(Boolean) && corruptHistoryPublishRefused && !corruptHistoryWrote
    && vanishedHistoryUnavailable
    && locked.action === 'published' && locked.seq === 8
    && missingPublished.action === 'published' && missingPublished.seq === 8
    && hiddenTable.rows[0].seq === 8 && hiddenPublished.seq === 8
    && hiddenReleaseCli.status === 1 && hiddenReleaseCli.stderr.includes('start with a letter or digit')
    && locked.changes[0].change === 'repair' && inspectHead(repairedHead, app, stream).usable,
  JSON.stringify({ missingHead: missingHeadTable.rows[0], emptyHead: emptyHeadTable.rows[0], unlistable: unlistableTable.rows[0],
    noHistoryRefused, unknownHistoryRefused, unknownHistoryWrote, badHistoryClassifyRefused, corruptHistoryPublishRefused, corruptHistoryWrote,
    vanishedHistoryUnavailable, hiddenTable: hiddenTable.rows[0], hiddenPublished: { action: hiddenPublished.action, seq: hiddenPublished.seq },
    hiddenReleaseCli: { status: hiddenReleaseCli.status, stderr: hiddenReleaseCli.stderr },
    locked: { action: locked.action, seq: locked.seq }, missingPublished: { action: missingPublished.action, seq: missingPublished.seq } }));
  rmSync(hiddenDir, { recursive: true, force: true });
  rmSync(missingDir, { recursive: true, force: true });
  rmSync(dir, { recursive: true, force: true });
}
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-list-outage-'));
  const cohort = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  const origin = {
    kind: 'directory', describe: () => dir, get: async () => null, head: async () => null,
    list: async (name) => { if (name.endsWith('/releases')) return null; throw new OriginUnavailable('EACCES'); },
  };
  const table = await classify({ app: { id: 'com.exact.test', displayName: 'Test', dir, manifest: {} }, opts: { platform: [] },
    origin, channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'test', web: dir, bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] },
    builds:{linux:fixtureBuild(cohort,{plan:{sha256:'0'.repeat(64)}})}, compat: { linux: { id: cohort, inputs: { store: { L: 'A' }, executors: [] } } }, platforms: ['linux'], wantOrigin: false });
  result('stream discovery outage retains the classified cohort row', table.rows.filter(r=>r.kind==='stream').length === 1
    && table.rows[0].action === 'bundle' && table.notes.some((note) => note.includes('stream discovery unavailable')),
  JSON.stringify(table));
  rmSync(dir, { recursive: true, force: true });
}
// A dry-run renders network availability per row, including declared retired
// streams. A corrupt response remains a hard refusal rather than masquerading
// as a transient network problem.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-origin-table-'));
  writeFileSync(join(dir, 'index.html'), 'web');
  const app = { id: 'com.exact.test', displayName: 'Test', dir,
    manifest: { deploy: { streams: [{ channel: 'prod', compatibilityId: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' }] } } };
  const compat = {
    web: { id: 'wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww', inputs: {} },
    linux: { id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', inputs: { store: { L: 'A' }, executors: [] } },
  };
  const table = await classify({ app, opts: { platform: [] },
    origin: new HttpsOrigin('https://127.0.0.1:1'), channel: 'prod',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir }, release: 'test', web: dir,
    bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] }, compat, platforms: ['linux'], wantOrigin: true });
  const unavailable = table.rows.filter((row) => row.action === 'unavailable');
  result('deploy tables preserve every row when HTTPS is unavailable', unavailable.length === 3
    && unavailable.some((row) => row.kind === 'origin')
    && unavailable.some((row) => row.platform === 'linux')
    && unavailable.some((row) => row.compatibilityId.startsWith('bbbb')),
  JSON.stringify(table.rows));
  rmSync(dir, { recursive: true, force: true });
}
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-bad-head-'));
  const cohort = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  mkdirSync(join(dir, '.exact', 'prod', cohort), { recursive: true });
  writeFileSync(join(dir, '.exact', 'prod', cohort, 'exact.json'), '{not json');
  let message = '';
  try {
    await classify({ app: { id: 'com.exact.test', displayName: 'Test', dir, manifest: {} }, opts: { platform: [] },
      origin: new DirectoryOrigin(dir), channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
      release: 'test', web: dir, bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] },
      compat: { linux: { id: cohort, inputs: { store: { L: 'A' }, executors: [] } } }, platforms: ['linux'], wantOrigin: false });
  } catch (error) { message = error.message; }
  result('deploy does not hide a malformed head as unavailable', message.includes('is not JSON'), message);
  rmSync(dir, { recursive: true, force: true });
}
// One comparison drives dev and deploy; neither id equality nor a binary
// edit is a proxy for a bundle dependency.
{
  const candidate=fixtureBuild('a'.repeat(32),{plan:{sha256:'0'.repeat(64)}});
  candidate.compat.inputs={app:'test',store:{L:'A'},executors:['native'],grantCeiling:'net.fetch https://x/',gpuSurfaces:[{name:'map',interface:'v1'}],dataCrate:{tree:'old'}};
  candidate.graph.sources={station:{params:['String'],result:'String'}};
  candidate.graph.artifacts[0].requires={sources:candidate.graph.sources,executors:['native'],grantCeiling:'net.fetch https://x/'};
  const installed=cohortReceipt(candidate);
  const host=structuredClone(candidate);host.binary.sha256='1'.repeat(64);
  const same=classifyArtifacts(host,installed);
  const moved=structuredClone(host);moved.compat.id='b'.repeat(32);moved.compat.inputs.dataCrate.tree='new';
  const old=classifyArtifacts(moved,installed);
  const needs=structuredClone(moved);needs.graph.artifacts[0].requires.sources={camera:{params:[],result:'String'}};
  const absent=classifyArtifacts(needs,installed);
  const shader=structuredClone(moved);shader.graph.artifacts.push({name:'shaders/map.wgsl',requires:{gpuSurfaces:[{name:'map',interface:'v2'}]}});
  const unsupported=classifyArtifacts(shader,installed);
  const grants=structuredClone(moved);grants.graph.artifacts[0].requires.grantCeiling='net.fetch https://new/';
  const refused=classifyArtifacts(grants,installed);
  result('artifact graph separates binary changes, compatible older cohorts, and named missing dependencies',
    same.binary&&same.bundle&&old.binary&&old.bundle&&old.warnings[0].includes('will run the old code')
    &&!absent.bundle&&absent.missing[0].includes('sources.camera')&&!unsupported.bundle&&unsupported.missing[0].includes('gpuSurfaces.map')
    &&!refused.bundle&&refused.missing[0].includes('https://new/')&&!classifyArtifacts(moved,null).bundle,
    JSON.stringify({same,old,absent,unsupported,refused}));
}
// Signing-key creation is one exclusive filesystem operation. Two publishers
// racing for an id cannot both report a public half and silently replace the
// private half behind the first one's result.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-keygen-'));
  const run = () => new Promise((done) => {
    const child = spawn(process.execPath, [DEPLOY, 'keygen', 'race', '--keys', dir, '--json']);
    let stdout = '', stderr = '';
    child.stdout.on('data', (bytes) => { stdout += bytes; });
    child.stderr.on('data', (bytes) => { stderr += bytes; });
    child.on('close', (code) => done({ code, stdout, stderr }));
  });
  const attempts = await Promise.all([run(), run()]);
  const winners = attempts.filter((attempt) => attempt.code === 0);
  const losers = attempts.filter((attempt) => attempt.code !== 0);
  let matches = false;
  try {
    const reported = Buffer.from(JSON.parse(winners[0]?.stdout ?? '{}').publicKey ?? '', 'base64');
    const actual = createPublicKey(createPrivateKey(readFileSync(join(dir, 'race.pem'))))
      .export({ type: 'spki', format: 'der' }).subarray(-32);
    matches = reported.equals(actual);
  } catch { /* the result below names the failed invariant */ }
  result('concurrent keygen has one truthful winner', winners.length === 1 && losers.length === 1
    && losers[0].stderr.includes('a signing key is never overwritten') && matches,
  attempts.map((attempt) => `exit ${attempt.code}: ${attempt.stdout}${attempt.stderr}`).join('\n'));
  rmSync(dir, { recursive: true, force: true });
}

{ // Edit diagnostics own and remove large captured Git trees, including on callback failure.
  const app = resolveApp('caltrain'), previous = process.env.CARGO_TARGET_DIR;
  let source, run, isolated = false, installed = false, caught = false;
  try {
    await withAppFixture(app, async f => {
      source = f.sourceRoot; run = f.run;
      isolated = f.app.dir !== app.dir && f.env.EXACT_APP_DIR === f.app.dir
        && f.env.CARGO_TARGET_DIR.startsWith(run + '/') && f.env.EXACT_WEB_DIST.startsWith(source + '/')
        && spawnSync('git', ['rev-parse', '--show-toplevel'], { cwd: f.app.dir, env: f.env, encoding: 'utf8' }).stdout.trim() === source;
      // The capture carries the pinned node_modules (a TypeScript bake runs
      // Rolldown from it), installed as an output, not committed as source.
      installed = existsSync(join(f.exactRoot, 'node_modules/.bin/rolldown'))
        && spawnSync('git', ['ls-files', 'node_modules'], { cwd: f.exactRoot, env: f.env, encoding: 'utf8' }).stdout === '';
      writeFileSync(join(f.app.dir, 'diagnostic-private.txt'), 'captured only');
      for (let i = 0; i < 5000; i++) writeFileSync(join(source, `cleanup-${i}`), 'private');
      throw new Error('expected diagnostic callback failure');
    });
  } catch (error) { caught = error.message === 'expected diagnostic callback failure'; }
  result('diagnostic source and outputs are isolated and cleaned on failure', isolated && installed && caught
    && !existsSync(source ?? '/missing') && !existsSync(run ?? '/missing')
    && !existsSync(join(app.dir, 'diagnostic-private.txt')) && process.env.CARGO_TARGET_DIR === previous);
}

// A release remains recognizable to a person without being the bake's lock
// or directory identity. Even an explicitly reused correlation id gets a
// separate stage, and generated ids in the same clock tick do not collide.
{
  const fixture = mkdtempSync(join(tmpdir(), 'exact-source-snapshot-'));
  const init = (repo, files) => {
    mkdirSync(repo, { recursive: true });
    spawnSync('git', ['init', '-q'], { cwd: repo });
    for (const [name, bytes] of Object.entries(files)) {
      mkdirSync(dirname(join(repo, name)), { recursive: true });
      writeFileSync(join(repo, name), bytes);
    }
    spawnSync('git', ['add', '-A'], { cwd: repo });
    return spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid', 'commit', '-qm', 'fixture'], { cwd: repo }).status === 0;
  };
  const internal = join(fixture, 'internal');
  const external = join(fixture, 'external');
  const exact = join(fixture, 'exact2');
  const cargoDep = join(fixture, 'cargo-dep');
  const linked = join(fixture, 'linked');
  const filtered = join(fixture, 'filtered');
  const outsideLink = join(fixture, 'outside-link.rs');
  const outsideTarget = join(fixture, 'outside-main.rs');
  let initialized = init(internal, { '.gitignore': '/apps/test/assets/ignored.txt\n/generated/\n/target/\n',
    'apps/test/app.contract': 'app Test\n', 'apps/test/assets/dist/published.txt': 'nested old\n',
    'host/runtime.rs': 'old\n', 'removed.txt': 'remove me\n' })
    && init(cargoDep, { '.gitignore': '/crates/fixture-dep/ignored.rs\n/generated/\n/target/\n',
      'Cargo.toml': '[workspace]\nmembers=["crates/fixture-dep"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\n',
      'shared.txt': '1\n',
      'generated/value.txt': 'ignored repository input\n',
      'crates/fixture-dep/Cargo.toml': '[package]\nname="fixture-dep"\nversion.workspace=true\nedition.workspace=true\n',
      'crates/fixture-dep/src/lib.rs': 'pub fn value() -> &\'static str { include_str!("../../../shared.txt").trim() }\n' })
    && init(external, { 'app.contract': 'app External\n',
      'Cargo.toml': '[package]\nname="external"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nfixture-dep={path="../cargo-dep/crates/fixture-dep"}\n',
      'src/lib.rs': 'pub fn value() -> &\'static str { fixture_dep::value() }\n',
      'src/main.rs': 'fn main() { println!("{}:{}", fixture_dep::value(), option_env!("RACE_VALUE").unwrap_or("sealed")); }\n' })
    && init(exact, { 'host/runtime.rs': 'old\n' })
    && init(linked, { 'app.contract': 'app Linked\n', 'src/main.rs': 'local\n' })
    && init(filtered, { 'app.contract': 'app Filtered\n', 'source.txt': 'WORKING SOURCE BYTES\n',
      'target-one.txt': 'ONE\n', 'target-two.txt': 'TWO\n' });
  if (initialized) {
    writeFileSync(outsideLink, 'external mutable bytes\n');
    writeFileSync(outsideTarget, 'fn main() { println!("live"); }\n');
    rmSync(join(linked, 'src/main.rs'));
    symlinkSync(outsideLink, join(linked, 'src/main.rs'));
    spawnSync('git', ['add', 'src/main.rs'], { cwd: linked });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'link'], { cwd: linked }).status === 0;
  }
  if (initialized) {
    spawnSync('git', ['config', 'filter.worktree.clean', 'sed s/WORKING/STORED/g'], { cwd: filtered });
    spawnSync('git', ['config', 'filter.worktree.smudge', 'sed s/STORED/WORKING/g'], { cwd: filtered });
    spawnSync('git', ['config', 'filter.worktree.required', 'true'], { cwd: filtered });
    writeFileSync(join(filtered, '.gitattributes'), 'source.txt filter=worktree\n');
    spawnSync('git', ['add', '--renormalize', '.'], { cwd: filtered });
    spawnSync('git', ['add', '.gitattributes'], { cwd: filtered });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'filter'], { cwd: filtered }).status === 0;
  }
  if (initialized) {
    symlinkSync('target-one.txt', join(filtered, 'alias.txt'));
    symlinkSync(join(realpathSync(filtered), 'alias.txt'), join(filtered, 'inside.txt'));
    spawnSync('git', ['add', 'alias.txt', 'inside.txt'], { cwd: filtered });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'internal absolute link'], { cwd: filtered }).status === 0;
  }
  if (initialized) {
    initialized = spawnSync('cargo', ['generate-lockfile'], { cwd: external, stdio: 'ignore' }).status === 0;
    spawnSync('git', ['add', 'Cargo.lock'], { cwd: external });
    initialized = initialized && spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'lock'], { cwd: external }).status === 0;
  }
  const fixtureApp = (name, dir, workspace) => ({ name, dir, workspace, target: join(workspace, 'target'),
    crate: (kind) => `${name}-${kind}`, manifest: { app: { id: `com.exact.${name}`, name }, host: {}, deploy: {} },
    id: `com.exact.${name}`, displayName: name, origin: null, declared: false });
  const internalApp = fixtureApp('test', join(internal, 'apps/test'), internal);
  const externalApp = fixtureApp('external', external, external);
  const linkedApp = fixtureApp('linked', linked, linked);
  const filteredApp = fixtureApp('filtered', filtered, filtered);
  const escapingSymlinkRefused = await rejects(() => snapshotOf(linkedApp, {}, linked), error => error.message.includes('symlink src/main.rs') && error.message.includes('absolute'));
  const linkRaceBin = join(fixture, 'link-race-bin');
  const linkRaceDone = join(fixture, 'link-race-done');
  mkdirSync(linkRaceBin);
  const linkRaceGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  writeFileSync(join(linkRaceBin, 'git'), `#!/bin/sh\n"${linkRaceGit}" "$@"\ncode=$?\ncase " $* " in\n  *" write-tree "*)\n    if [ ! -e "$EXACT_LINK_RACE_DONE" ]; then\n      ln -shf target-two.txt "$EXACT_LINK_RACE_ALIAS"\n      : > "$EXACT_LINK_RACE_DONE"\n    fi\n    ;;\nesac\nexit "$code"\n`);
  chmodSync(join(linkRaceBin, 'git'), 0o755);
  const linkRacePath = process.env.PATH;
  process.env.PATH = `${linkRaceBin}:${linkRacePath}`;
  process.env.EXACT_LINK_RACE_DONE = linkRaceDone;
  process.env.EXACT_LINK_RACE_ALIAS = join(filtered, 'alias.txt');
  let filteredSnapshot;
  try { filteredSnapshot = snapshotOf(filteredApp, {}, filtered); }
  finally {
    process.env.PATH = linkRacePath;
    delete process.env.EXACT_LINK_RACE_DONE;
    delete process.env.EXACT_LINK_RACE_ALIAS;
    rmSync(join(filtered, 'alias.txt'));
    symlinkSync('target-one.txt', join(filtered, 'alias.txt'));
  }
  const filteredMaterialized = materializeSnapshot(filteredSnapshot, deployRun(filteredApp.target, 'filtered'), filteredApp);
  const checkoutFilterPreserved = filteredSnapshot.id === filteredSnapshot.commit
    && readFileSync(join(filteredMaterialized.app.dir, 'source.txt'), 'utf8') === 'WORKING SOURCE BYTES\n'
    && !isAbsolute(readlinkSync(join(filteredMaterialized.app.dir, 'inside.txt')))
    && readFileSync(join(filteredMaterialized.app.dir, 'inside.txt'), 'utf8') === 'ONE\n';
  const linkedTarget = join(fixture, 'shared-target');
  mkdirSync(linkedTarget);
  symlinkSync(linkedTarget, join(internal, 'target'));
  const projectTmp = join(internal, 'project-tmp');
  mkdirSync(projectTmp);
  const priorTmp = process.env.TMPDIR;
  process.env.TMPDIR = projectTmp;
  // Staged additions deleted from disk are absent from the captured worktree;
  // they must neither resurrect index bytes nor make git add fail.
  writeFileSync(join(internal, 'staged-missing.txt'), 'never captured');
  spawnSync('git', ['add', 'staged-missing.txt'], { cwd: internal });
  rmSync(join(internal, 'staged-missing.txt'));
  const realIndex = spawnSync('git', ['ls-files', '--stage', '-z'], { cwd: internal }).stdout;
  let internalSnapshot;
  try { internalSnapshot = snapshotOf(internalApp, {}, internal); }
  finally {
    if (priorTmp === undefined) delete process.env.TMPDIR;
    else process.env.TMPDIR = priorTmp;
  }
  rmSync(join(internal, 'target'));
  rmSync(projectTmp, { recursive: true });
  // This is the race a before/after fingerprint cannot see: a transient edit
  // exists while the bake is being prepared, then the checkout is restored.
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient during bake\n');
  const cleanRun = deployRun(internalApp.target, 'clean-race');
  const cleanMaterialized = materializeSnapshot(internalSnapshot, cleanRun, internalApp);
  const cleanRaceBytes = readFileSync(join(cleanMaterialized.exactRoot, 'host/runtime.rs'), 'utf8');
  result('capture omits missing staged additions and preserves the real index', !existsSync(join(cleanMaterialized.exactRoot, 'staged-missing.txt'))
    && realIndex.equals(spawnSync('git', ['ls-files', '--stage', '-z'], { cwd: internal }).stdout));
  spawnSync('git', ['reset', '--', 'staged-missing.txt'], { cwd: internal });
  const projectTmpRejected = relative(internal, cleanMaterialized.sourceRoot).startsWith('..');
  writeFileSync(join(internal, 'host/runtime.rs'), 'old\n');
  // Restore the live file immediately after the capture freezes its tree. A
  // later live status read must not relabel those already-captured bytes as a
  // clean HEAD snapshot.
  const raceBin = join(fixture, 'race-bin');
  const raceDone = join(fixture, 'race-done');
  mkdirSync(raceBin);
  const realGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  writeFileSync(join(raceBin, 'git'), `#!/bin/sh\n"${realGit}" "$@"\ncode=$?\ncase " $* " in\n  *" write-tree "*|*" --binary "*)\n    if [ ! -e "$EXACT_CAPTURE_RACE_DONE" ]; then\n      printf 'old\\n' > "$EXACT_CAPTURE_RACE_FILE"\n      : > "$EXACT_CAPTURE_RACE_DONE"\n    fi\n    ;;\nesac\nexit "$code"\n`);
  chmodSync(join(raceBin, 'git'), 0o755);
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient during capture\n');
  const oldPath = process.env.PATH;
  process.env.PATH = `${raceBin}:${oldPath}`;
  process.env.EXACT_CAPTURE_RACE_FILE = join(internal, 'host/runtime.rs');
  process.env.EXACT_CAPTURE_RACE_DONE = raceDone;
  let racedSnapshot;
  try { racedSnapshot = snapshotOf(internalApp, { dirty: true }, internal); }
  finally {
    process.env.PATH = oldPath;
    delete process.env.EXACT_CAPTURE_RACE_FILE;
    delete process.env.EXACT_CAPTURE_RACE_DONE;
  }
  const racedMaterialized = materializeSnapshot(racedSnapshot, deployRun(internalApp.target, 'atomic-race'), internalApp);
  const atomicRaceCaptured = racedSnapshot.dirty && racedSnapshot.id !== racedSnapshot.commit
    && racedSnapshot.changes.some((change) => change.endsWith('host/runtime.rs'))
    && readFileSync(join(racedMaterialized.exactRoot, 'host/runtime.rs'), 'utf8') === 'transient during capture\n'
    && readFileSync(join(internal, 'host/runtime.rs'), 'utf8') === 'old\n';
  writeFileSync(join(internal, 'host/runtime.rs'), 'edited\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'nested edited\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored captured\n');
  rmSync(join(internal, 'removed.txt'));
  mkdirSync(join(internal, 'target'), { recursive: true });
  writeFileSync(join(internal, 'target/generated.bin'), 'not source\n');
  mkdirSync(join(internal, 'generated'));
  writeFileSync(join(internal, 'generated/output.bin'), 'ignored input root\n');
  const siblingRefused = await rejects(() => snapshotOf(internalApp, {}, internal), error => error.message.includes('exact2') && error.message.includes('host/runtime.rs')
    && error.message.includes('assets/ignored.txt') && !error.message.includes('target/generated.bin'));
  const dirtySnapshot = snapshotOf(internalApp, { dirty: true }, internal);
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient replacement\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'transient nested replacement\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'transient ignored replacement\n');
  const dirtyRun = deployRun(internalApp.target, 'dirty-race');
  const dirtyMaterialized = materializeSnapshot(dirtySnapshot, dirtyRun, internalApp);
  writeFileSync(join(internal, 'host/runtime.rs'), 'edited\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'nested edited\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored captured\n');
  const capturedDirtyBytes = readFileSync(join(dirtyMaterialized.exactRoot, 'host/runtime.rs'), 'utf8') === 'edited\n'
    && readFileSync(join(dirtyMaterialized.app.dir, 'assets/dist/published.txt'), 'utf8') === 'nested edited\n'
    && readFileSync(join(dirtyMaterialized.app.dir, 'assets/ignored.txt'), 'utf8') === 'ignored captured\n'
    && !existsSync(join(dirtyMaterialized.exactRoot, 'removed.txt'))
    && readFileSync(join(dirtyMaterialized.exactRoot, 'generated/output.bin'), 'utf8') === 'ignored input root\n';
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored changed\n');
  const dirtyAgain = snapshotOf(internalApp, { dirty: true }, internal);
  const changedSnapshotRefused = await rejects(() => snapshotOf(internalApp, { dirty: true, snapshot: dirtySnapshot.id }, internal), error => error.message.includes('same complete source set'));
  const ignoredRootRefused = await rejects(() => snapshotOf(externalApp, {}, exact), error => error.message.includes('cargo') && error.message.includes('generated/value.txt'));
  const externalSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  const partialPinRefused = await rejects(() => snapshotOf(externalApp, { dirty: true, snapshot: externalSnapshot.commit }, exact), error => error.message.includes('complete source set'));
  const pinned = snapshotOf(externalApp, { dirty: true, snapshot: externalSnapshot.id.slice(0, 12) }, exact);
  const externalRun = deployRun(externalApp.target, 'external');
  const externalMaterialized = materializeSnapshot(externalSnapshot, externalRun, externalApp);
  const stagedGitProbe = spawnSync('git', ['rev-parse', '--show-toplevel'], {
    cwd: externalMaterialized.exactRoot, encoding: 'utf8',
  });
  const stagedGitSealed = stagedGitProbe.status !== 0 && stagedGitProbe.stdout.trim() === '';
  const ignoredRootCaptured = readFileSync(join(dirname(externalMaterialized.app.dir), 'cargo-dep/generated/value.txt'), 'utf8')
    === 'ignored repository input\n';
  writeFileSync(join(cargoDep, 'shared.txt'), '2\n');
  mkdirSync(join(external, '.cargo'));
  writeFileSync(join(external, '.cargo/config.toml'), '[env]\nRACE_VALUE="live ancestor"\n');
  const stagedRun = spawnSync('cargo', ['run', '--quiet', '--locked'], { cwd: externalMaterialized.app.workspace,
    env: { ...process.env, CARGO_TARGET_DIR: join(fixture, 'cargo-target') }, encoding: 'utf8' });
  const stagedDependencyStayedCaptured = stagedRun.status === 0 && stagedRun.stdout.trim() === '1:sealed';
  rmSync(join(external, '.cargo'), { recursive: true });
  writeFileSync(join(cargoDep, 'shared.txt'), '1\n');
  // Cargo accepts absolute path dependencies, but an immutable deploy cannot:
  // the staged manifest would otherwise reach back into the mutable checkout.
  const relativeManifest = readFileSync(join(external, 'Cargo.toml'), 'utf8');
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest.replace('../cargo-dep/crates/fixture-dep', join(cargoDep, 'crates/fixture-dep')));
  const absoluteSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  writeFileSync(join(cargoDep, 'shared.txt'), '2\n');
  const absoluteDependencyRefused = await rejects(() => materializeSnapshot(absoluteSnapshot, deployRun(externalApp.target, 'absolute-dependency'), externalApp), error => error.message.includes('resolves outside the captured source root'));
  writeFileSync(join(cargoDep, 'shared.txt'), '1\n');
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest);
  writeFileSync(join(external, 'Cargo.toml'), `${relativeManifest}\n[[bin]]\nname="outside"\npath=${JSON.stringify(outsideTarget)}\n`);
  const absoluteTargetSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  const absoluteTargetRefused = await rejects(() => materializeSnapshot(absoluteTargetSnapshot, deployRun(externalApp.target, 'absolute-target'), externalApp), error => error.message.includes('target outside') && error.message.includes('resolves outside'));
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest);
  writeFileSync(join(cargoDep, 'crates/fixture-dep/ignored.rs'), 'pub const CAPTURED: bool = true;\n');
  const ignoredDependencyRefused = await rejects(() => snapshotOf(externalApp, {}, exact), error => error.message.includes('cargo') && error.message.includes('crates/fixture-dep/ignored.rs'));
  const dependencyDirty = snapshotOf(externalApp, { dirty: true }, exact);
  const dependencyRun = deployRun(externalApp.target, 'external-dependency');
  const dependencyMaterialized = materializeSnapshot(dependencyDirty, dependencyRun, externalApp);
  const capturedDependency = readFileSync(join(dirname(dependencyMaterialized.app.dir), 'cargo-dep/crates/fixture-dep/ignored.rs'), 'utf8')
    === 'pub const CAPTURED: bool = true;\n';
  rmSync(join(cargoDep, 'crates/fixture-dep/ignored.rs'));
  const rendered = renderTable({ release: 'test', snapshot: externalSnapshot, channel: 'prod',
    origin: { kind: 'directory', location: '/origin' }, notes: [], rows: [] });
  writeFileSync(join(exact, 'host/runtime.rs'), 'edited\n');
  const dependencyRefused = await rejects(() => snapshotOf(externalApp, {}, exact), error => error.message.includes('exact2') && error.message.includes('host/runtime.rs'));
  result('deploy snapshots every source repository the bake reads', initialized
    && internalSnapshot.id === internalSnapshot.commit && internalSnapshot.sources.length === 1
    && internalSnapshot.sources[0].roles.join(',') === 'app,exact2'
    && cleanRaceBytes === 'old\n' && projectTmpRejected && checkoutFilterPreserved && atomicRaceCaptured
    && siblingRefused && dirtySnapshot.dirty && dirtySnapshot.id !== dirtyAgain.id
    && capturedDirtyBytes && changedSnapshotRefused
    && dirtySnapshot.changes.some((change) => change.endsWith('host/runtime.rs'))
    && dirtySnapshot.changes.some((change) => change.endsWith('apps/test/assets/dist/published.txt'))
    && dirtySnapshot.changes.some((change) => change.endsWith('apps/test/assets/ignored.txt'))
    && dirtySnapshot.changes.some((change) => change.endsWith('removed.txt'))
    && !dirtySnapshot.changes.some((change) => change.includes('target/generated.bin'))
    && dirtySnapshot.changes.some((change) => change.includes('generated/output.bin'))
    && /^[0-9a-f]{40}$/.test(externalSnapshot.id) && externalSnapshot.id !== externalSnapshot.commit
    && externalSnapshot.sources.length === 3 && partialPinRefused && pinned.id === externalSnapshot.id
    && relative(externalMaterialized.app.dir, externalMaterialized.exactRoot) === '../exact2'
    && stagedDependencyStayedCaptured && absoluteDependencyRefused && absoluteTargetRefused
    && stagedGitSealed && escapingSymlinkRefused
    && ignoredRootRefused && ignoredRootCaptured
    && ignoredDependencyRefused && capturedDependency
    && rendered.includes(`snapshot ${externalSnapshot.id.slice(0, 12)}`)
    && dependencyRefused,
  JSON.stringify({ initialized, internalSnapshot, cleanRaceBytes, projectTmpRejected, checkoutFilterPreserved, atomicRaceCaptured,
    siblingRefused, dirtySnapshot, dirtyAgain: dirtyAgain.id,
    capturedDirtyBytes, changedSnapshotRefused, externalSnapshot, partialPinRefused, pinned: pinned.id,
    externalLayout: relative(externalMaterialized.app.dir, externalMaterialized.exactRoot),
    stagedDependencyStayedCaptured, absoluteDependencyRefused, absoluteTargetRefused,
    stagedGitSealed, escapingSymlinkRefused,
    ignoredRootRefused, ignoredRootCaptured,
    ignoredDependencyRefused, capturedDependency, dependencyRefused }));
  rmSync(fixture, { recursive: true, force: true });
}
{
  const target = mkdtempSync(join(tmpdir(), 'exact-deploy-run-'));
  const now = new Date('2026-09-04T12:34:56.789Z');
  const ids = new Set(Array.from({ length: 32 }, () => defaultRelease('a'.repeat(40), now)));
  const first = deployRun(target, 'same-release');
  writeFileSync(join(first, 'still-here'), 'first');
  const second = deployRun(target, 'same-release');
  result('deploy ids and private stages do not collide in one clock tick', ids.size === 32
    && first !== second && readFileSync(join(first, 'still-here'), 'utf8') === 'first');
  rmSync(target, { recursive: true, force: true });
}
// Stream heads point only at immutable blobs. Their immutable audit record is
// prepared before the conditional head swap; a failed record cannot expose a
// head, and a failed head leaves the preceding one and every named blob whole.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-stream-commit-'));
  const origin = new DirectoryOrigin(dir);
  const compatibilityId = 'c'.repeat(32);
  const row = { kind: 'stream', platform: 'linux', channel: 'prod', compatibilityId,
    action: 'bundle', changes: [{ name: 'app.plan', change: 'new' }, { name: 'assets/icon.png', change: 'new' }] };
  const plan = Buffer.from('plan bytes');
  const asset = Buffer.from('asset bytes');
  const bundle = { plan: { bytes: plan, sha256: createHash('sha256').update(plan).digest('hex'), formatVersion: 4, kernelSchema: '0'.repeat(16) },
    assets: [{ name: 'assets/icon.png', bytes: asset, sha256: createHash('sha256').update(asset).digest('hex') }] };
  const app = { id: 'com.exact.test', displayName: 'Test', manifest: { deploy: {} } };
  const signer = { keyId: 'test', sign: () => ({ keyId: 'test', ed25519: Buffer.alloc(64).toString('base64') }) };
  const racedPath = blobPath('d'.repeat(64));
  const raced = await Promise.allSettled([
    origin.put(racedPath, Buffer.from('first'), { immutable: true }),
    origin.put(racedPath, Buffer.from('second'), { immutable: true }),
  ]);
  const racedBytes = await origin.get(racedPath);
  const immutableRaceHeld = raced.filter((attempt) => attempt.status === 'fulfilled').length === 1
    && raced.filter((attempt) => attempt.status === 'rejected').length === 1
    && (racedBytes.equals(Buffer.from('first')) || racedBytes.equals(Buffer.from('second')));
  const order = [];
  const put = origin.put.bind(origin);
  origin.put = async (rel, bytes, options) => { const result = await put(rel, bytes, options); order.push(`put ${rel}`); return result; };
  const putHead = origin.putHead.bind(origin);
  origin.putHead = async (...args) => { order.push('put head'); return putHead(...args); };
  const args = { origin, row, bundle, compat: { id: compatibilityId, inputs: {} }, app, signer,
    release: 'one', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} };
  await publishStream(args);
  const stream = { channel: 'prod', compatibilityId };
  const head = await origin.head(stream);
  const base = `.exact/prod/${compatibilityId}`;
  const recordPath = `${base}/releases/one.json`;
  const record = JSON.parse((await origin.get(recordPath)).toString('utf8'));
  let reusedRefused = false;
  const beforeRetry = order.length;
  try { await publishStream(args); }
  catch (error) { reusedRefused = error.message.includes('already has an immutable record'); }
  const names = await origin.list(base);
  const blobCards = [head.json.plan, ...head.json.assets];
  const recordBeforeHead = order.indexOf(`put ${recordPath}`) < order.indexOf('put head');
  const interrupted = [];
  for (const moment of ['before', 'after']) for (let stop = 1; stop <= 4; stop++) {
    const failedDir = mkdtempSync(join(tmpdir(), `exact-stream-failure-${moment}-${stop}-`));
    const failed = new DirectoryOrigin(failedDir);
    const oldPlan = Buffer.from('old plan');
    const oldBundle = { plan: { ...bundle.plan, bytes: oldPlan, sha256: createHash('sha256').update(oldPlan).digest('hex') }, assets: [] };
    await failed.put(blobPath(oldBundle.plan.sha256), oldPlan, { immutable: true });
    const oldHead = streamHead({ app, bundle: oldBundle, stream, seq: 1, release: 'old' });
    oldHead.signature = signer.sign(oldHead);
    const oldBytes = Buffer.from(JSON.stringify(oldHead) + '\n');
    await failed.withLock(stream, () => failed.putHead(stream, oldBytes));
    const rawPut = failed.put.bind(failed);
    let writes = 0;
    failed.put = async (...putArgs) => {
      writes++;
      if (writes === stop && moment === 'before') throw new Error(`injected before write ${stop}`);
      const result = await rawPut(...putArgs);
      if (writes === stop && moment === 'after') throw new Error(`injected after write ${stop}`);
      return result;
    };
    const rawHead = failed.putHead.bind(failed);
    failed.putHead = async (...headArgs) => {
      writes++;
      if (writes === stop && moment === 'before') throw new Error(`injected before write ${stop}`);
      const result = await rawHead(...headArgs);
      if (writes === stop && moment === 'after') throw new Error(`injected after write ${stop}`);
      return result;
    };
    let stopped = false;
    let result = null;
    try { result = await publishStream({ ...args, origin: failed, release: `failed-${moment}-${stop}` }); }
    catch (error) { stopped = error.message.includes('injected'); }
    const after = await failed.head(stream);
    interrupted.push((stop < 4
      ? stopped && after.bytes.equals(oldBytes)
      : moment === 'before'
        ? stopped && after.bytes.equals(oldBytes)
        : result?.action === 'published' && after.json.release === `failed-${moment}-${stop}`)
      && (await failed.get(blobPath(oldBundle.plan.sha256))).equals(oldPlan));
    rmSync(failedDir, { recursive: true, force: true });
  }
  result('stream publication commits immutable blobs and receipt before its head', recordBeforeHead
    && names.join(',') === 'exact.json,releases' && blobCards.every((card) => card.url === `../../blobs/${card.sha256}`)
    && blobCards.every((card) => existsSync(join(dir, '.exact', 'blobs', card.sha256)))
    && record.head.entryDigest === createHash('sha256').update(canonicalBytes(head.json)).digest('hex')
    && record.envelope.signature.keyId === 'test' && immutableRaceHeld && reusedRefused && order.length === beforeRetry
    && interrupted.every(Boolean));
  rmSync(dir, { recursive: true, force: true });
}
// All origin verbs reject links at the root, intermediate, and leaf boundary.
// Locks are permanent OS ownership, independent of age or claimed host/PID.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-origin-handles-'));
  const originDir = join(dir, 'origin');
  const outside = join(dir, 'outside');
  mkdirSync(originDir); mkdirSync(outside);
  writeFileSync(join(outside, 'secret'), 'private');
  const origin = new DirectoryOrigin(originDir);
  const stream = { channel: 'prod', compatibilityId: 'test' };
  const refusals = [];
  const refuse = async (fn) => { try { await fn(); return false; } catch { return true; } };
  symlinkSync(outside, join(originDir, 'escape'));
  symlinkSync(join(outside, 'secret'), join(originDir, 'linked'));
  symlinkSync(join(outside, 'absent'), join(originDir, 'dangling'));
  refusals.push(await refuse(() => origin.get('escape/secret')),
    await refuse(() => origin.list('escape')), await refuse(() => origin.get('linked')),
    await refuse(() => origin.list('linked')), await refuse(() => origin.put('linked', Buffer.from('public'))),
    await refuse(() => origin.put('dangling', Buffer.from('public'))),
    await refuse(() => origin.put('escape/new/deep/file', Buffer.from('public'))));
  symlinkSync(outside, join(originDir, '.exact'));
  refusals.push(await refuse(() => origin.putHead(stream, Buffer.from('{}'))),
    await refuse(() => origin.withLock(stream, async () => false)));
  rmSync(join(originDir, '.exact'));
  await origin.withLock(stream, async () => true);
  const lockPath = join(originDir, '.exact/prod/test/.lock');
  rmSync(lockPath);
  symlinkSync(join(outside, 'secret'), lockPath);
  refusals.push(await refuse(() => origin.withLock(stream, async () => false)));
  rmSync(lockPath);
  const headPath = join(originDir, '.exact/prod/test/exact.json');
  symlinkSync(join(outside, 'secret'), headPath);
  refusals.push(await refuse(() => origin.putHead(stream, Buffer.from('{}'))));
  rmSync(headPath);
  refusals.push(await refuse(() => copyStaticTreeIfPresent(join(originDir, 'escape/missing'), join(dir, 'static-candidate'))));
  const rootLink = join(dir, 'root-link'); symlinkSync(originDir, rootLink);
  refusals.push(await refuse(() => new DirectoryOrigin(rootLink).get('linked')),
    await refuse(() => new DirectoryOrigin(rootLink).put('new', Buffer.from('public'))));
  let agedHeld = false, otherHostHeld = false, replacedRefused = false, successorHeld = false;
  const priorHost = process.env.HOSTNAME;
  try {
    process.env.HOSTNAME = 'publisher-a';
    await origin.withLock(stream, async () => {
      utimesSync(lockPath, new Date(0), new Date(0));
      agedHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
      process.env.HOSTNAME = 'publisher-b';
      otherHostHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
      renameSync(lockPath, join(dir, 'retired-lock'));
      let entered;
      const ready = new Promise((resolve) => { entered = resolve; });
      let release;
      const pending = new Promise((resolve) => { release = resolve; });
      const successor = new DirectoryOrigin(originDir);
      // Keep the successor held until the old owner's finally has run.
      const held = successor.withLock(stream, async () => { entered(); await pending; });
      await ready;
      replacedRefused = await refuse(() => origin.putHead(stream, Buffer.from('{}')));
      origin.successor = { held, release };
    });
    successorHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
    origin.successor.release(); await origin.successor.held;
  } finally {
    if (priorHost === undefined) delete process.env.HOSTNAME; else process.env.HOSTNAME = priorHost;
  }
  const reusable = await origin.withLock(stream, async () => true);
  await origin.putHead(stream, Buffer.from('{"seq":1}'));
  const conditional = await refuse(() => origin.putHead(stream, Buffer.from('{"seq":2}')));
  result('origin handles reject every linked boundary and retain exclusive lock ownership',
    refusals.every(Boolean) && agedHeld && otherHostHeld && replacedRefused && successorHeld && reusable && conditional
    && readFileSync(join(outside, 'secret'), 'utf8') === 'private' && !existsSync(join(outside, 'new')),
    JSON.stringify({ refusals, agedHeld, otherHostHeld, replacedRefused, successorHeld, reusable, conditional }));
  rmSync(dir, { recursive: true, force: true });
}
// Asset names must stage as distinct files on every client filesystem, APFS
// included: the publisher refuses what exact-update's check_asset_names
// refuses, before a head is signed or admitted.
{
  const refusals = [
    [['Logo.png', 'logo.png'], /differ only by case/],
    [['a', 'a/b.png'], /is a file where/],
    [['deck/B/x.html', 'deck/b'], /is a file where/],
    [['caf\u00e9.png'], /not portable/],
    [['cafe\u0301.png'], /not portable/],
    [['two words.png'], /not portable/],
    [['mark.png', 'mark.png'], /twice/],
  ];
  const refused = refusals.map(([names, why]) => { try { portableAssetNames(names); return false; } catch (error) { return why.test(error.message); } });
  let accepted = true;
  for (const names of [['a/b.png', 'a/c.png', 'A-b_c.1.png'], ['deck/index.html', 'deck/index.html.map']]) {
    try { portableAssetNames(names); } catch { accepted = false; }
  }
  const app = { id: 'com.exact.names', displayName: 'Names', manifest: {} };
  const stream = { channel: 'prod', compatibilityId: 'a'.repeat(32) };
  const planBytes = Buffer.from('plan');
  const card = (name) => ({ name, bytes: Buffer.from(name), sha256: sha256(Buffer.from(name)) });
  const bundle = { plan: { bytes: planBytes, sha256: sha256(planBytes), formatVersion: 4, kernelSchema: '0'.repeat(16) }, assets: [card('Logo.png'), card('logo.png')] };
  const signingRefused = await rejects(() => streamHead({ app, bundle, stream, seq: 1, release: 'names' }), (error) => /differ only by case/.test(error.message));
  const head = streamHead({ app, bundle: { ...bundle, assets: [card('Logo.png')] }, stream, seq: 1, release: 'names' });
  head.assets.push({ ...head.assets[0], name: 'logo.png' });
  const bytes = Buffer.from(JSON.stringify(head));
  const inspected = inspectHead({ bytes, json: head, sha256: sha256(bytes) }, app, stream);
  result('the publisher refuses asset names a client filesystem folds together', refused.every(Boolean) && accepted
    && signingRefused && !inspected.usable && /differ only by case/.test(inspected.problem), JSON.stringify({ refused, accepted, signingRefused, inspected: inspected.problem }));
}

// The next seq sits above everything the stream has authenticated: the head
// and every signed immutable release record. A head rolled back on the
// origin (a restored backup, a stale replica) must not hand out a seq
// clients already hold for another bundle.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-seq-floor-'));
  const compatibilityId = 'b'.repeat(32);
  const stream = { channel: 'prod', compatibilityId };
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const publicRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64');
  const app = { id: 'com.exact.seqfloor', displayName: 'Seq Floor', dir,
    manifest: { deploy: { signing: { key: 'test', keys: { test: publicRaw } } } } };
  const bundleOf = (text) => {
    const plan = Buffer.from(text);
    return { plan: { bytes: plan, sha256: sha256(plan), formatVersion: 4, kernelSchema: '0'.repeat(16) }, assets: [] };
  };
  const published = bundleOf('published plan'), next = bundleOf('next plan');
  const signedAt = (seq) => {
    const head = streamHead({ app, bundle: published, stream, seq, release: `r${seq}` });
    head.cohort = cohortReceipt(fixtureBuild(compatibilityId, published));
    head.signature = { keyId: 'test', ed25519: cryptoSign(null, canonicalBytes(head), privateKey).toString('base64') };
    return head;
  };
  const head = signedAt(7);
  const headBytes = Buffer.from(JSON.stringify(head) + '\n');
  const records = { 'r7.json': signedAt(7), 'r9.json': signedAt(9) };
  const tableWith = (names) => classify({ app, opts: { platform: [] },
    origin: {
      kind: 'directory', describe: () => dir,
      get: async (name) => {
        const leaf = name.split('/').at(-1);
        return names.includes(leaf) ? Buffer.from(JSON.stringify({ envelope: records[leaf] })) : null;
      },
      head: async () => ({ json: head, bytes: headBytes, sha256: sha256(headBytes) }),
      list: async (name) => name.endsWith('/releases') ? names : [],
    },
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle: next, builds: { linux: fixtureBuild(compatibilityId, next) },
    compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const rolledBack = await tableWith(['r7.json', 'r9.json']);
  const inStep = await tableWith(['r7.json']);
  rmSync(dir, { recursive: true, force: true });
  result('a rolled-back head allocates above the authenticated release history',
    rolledBack.rows[0].action === 'bundle' && rolledBack.rows[0].seq === 10 && inStep.rows[0].seq === 8,
    JSON.stringify({ rolledBack: rolledBack.rows[0], inStep: inStep.rows[0] }));
}

{
  // LLP 1069.008 D7: the dry run ends with what the app can reach.
  const reach = [
    { grant: 'net.fetch https://api.example.com/', purpose: null, enforced: 'runtime (all hosts); CSP (served web)' },
    { grant: 'device.microphone purpose.microphone', purpose: 'Records your takes.', enforced: 'OS prompt (iOS, macOS); Permissions-Policy (served web; a static dist sends none); declared by app.ts' },
  ];
  const table = (rows) => renderTable({ release: 'test', snapshot: { commit: '0'.repeat(40), dirty: false }, channel: 'prod',
    origin: { kind: 'directory', location: '/origin' }, notes: [], rows: [], reach: rows }).split('\n');
  const shown = table(reach), none = table(null);
  result('the dry run prints what the app can reach, with who enforces each line',
    shown[1] === 'what this app can reach:' && /^ {2}reach +purpose +enforced by$/.test(shown[2])
      && /^ {2}net\.fetch https:\/\/api\.example\.com\/ +— +runtime \(all hosts\); CSP \(served web\)$/.test(shown[3])
      && shown[4].includes('"Records your takes."') && shown[4].endsWith('declared by app.ts')
      && none.length === 1,
    shown.join('\n'));
}

