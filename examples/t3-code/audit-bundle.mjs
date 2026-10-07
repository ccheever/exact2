#!/usr/bin/env bun
// The self-containment audit of the downloadable T3 Code (Exact) bundle (20261005-portable-app-download
// item 2). It reads a built `.app` and, with --t3-home, the runtime tree a first launch unpacked into a
// scratch home, and fails on anything that would tie the app to the machine that built it:
//
//   bun examples/t3-code/audit-bundle.mjs <T3 Code (Exact).app> [--t3-home <scratch>/.t3]
//       [--forbid <path>]… [--build-root <path>]… [--allowlist <file>] [--json <report.json>]
//
//   arch          a Mach-O file that is not exactly arm64 (`lipo -archs`)
//   minos         a Mach-O file whose minimum macOS is above the stated minimum (`vtool -show-build`)
//   dylib         a dependent library outside /usr/lib and /System that is not inside the bundle (`otool -L`),
//                 or a dylib whose own install name is an absolute path
//   rpath         an absolute LC_RPATH outside /usr/lib and /System (`otool -l`)
//   machine-path  this machine's paths in any file: the home folder, the repository, ~/.bun, ~/.t3, the
//                 temporary folder, and every --forbid (the reference checkout, the mc-orch tree). Never
//                 allowed, whatever the allowlist says
//   build-path    a path shaped like a build machine's (/Users/, /private/var/folders, /opt/homebrew,
//                 /Volumes/, /.bun/, DerivedData) or under a --build-root (package-app.mjs's fixed export
//                 folder): a finding unless bundle-allowlist.json lists it with its reason (the release
//                 runtime's own bytes carry its CI machine's and its libraries' paths)
//   dev-file      a leftover development file (source maps, app.contract.d.ts, UI-PARITY-TODO.tmp.md,
//                 tests and fixtures, .git, .DS_Store)
//   unexpected    a bundle file that no `files` pattern of the allowlist covers (the file-list diff)
//   runtime-part  the bundled archive's SHA-256 is not the manifest's, or the manifest is not the pin's
//   runtime-tree  after the first launch: a path that differs from the manifest, an executable that is
//                 not 0755, a Mach-O file (t3, every .node, spawn-helper…) that fails
//                 `codesign --verify --deep --strict`, a symlink out of the runtime folder, no
//                 `.install-complete` with the version
//   signature     `codesign --verify --deep --strict` of the app fails
//   info-plist    LSMinimumSystemVersion is not the stated minimum, or the bundle id is not the example's
//   distribution  Contents/Resources/distribution.json is missing or not {"flavor":"packaged"}
//
// Exit 0 with no findings, 1 with any. Reads only the paths it is given (and runs Xcode's tools).
import { spawnSync } from 'node:child_process';
import { existsSync, lstatSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isMachO, sha256File } from './stage-runtime.mjs';

const here = dirname(fileURLToPath(import.meta.url));
export const defaultAllowlist = join(here, 'bundle-allowlist.json');

/** The generic shapes of a build machine's paths (allowlistable with a reason). */
export const BUILD_PATTERNS = ['/Users/', '/private/var/folders', '/var/folders/', '/opt/homebrew', '/Volumes/', '/.bun/', '/.bun-', 'DerivedData'];

/** This machine's own paths: the home folder, this checkout, ~/.bun, ~/.t3, its temporary folder, plus `extra` (each also as its real path). */
export function machinePaths(extra = [], { home = homedir(), repo = resolve(here, '../..'), temporary = tmpdir() } = {}) {
  const paths = [home, repo, join(home, '.bun'), join(home, '.t3'), temporary, ...extra].filter(Boolean).map(path => resolve(path));
  const real = paths.flatMap(path => { try { return [realpathSync(path)]; } catch { return []; } });
  // Longest first, so a hit is named by the most specific path; never the bare root.
  return [...new Set([...paths, ...real])].filter(path => path.length > 1).sort((a, b) => b.length - a.length);
}

const run = (cmd, args) => spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 64 << 20 });

/** Every path under `root` (relative, sorted), with its lstat. */
export function walk(root) {
  const out = [];
  const visit = (dir) => {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name), stat = lstatSync(path);
      out.push({ path: relative(root, path), abs: path, stat });
      if (stat.isDirectory()) visit(path);
    }
  };
  visit(root);
  return out;
}

const printable = (byte) => byte >= 0x20 && byte < 0x7f;
/** The printable run a hit sits in, from the hit's start to the end of its token. */
function hitText(buffer, at) {
  let end = at;
  while (end < buffer.length && end - at < 240 && printable(buffer[end]) && !'"\'`<>,;() '.includes(String.fromCharCode(buffer[end]))) end++;
  return buffer.toString('latin1', at, end);
}

/** Each occurrence of each pattern in `buffer`, as `{pattern, text}` (counted once per distinct text). */
export function scanBytes(buffer, patterns) {
  const hits = new Map();
  for (const pattern of patterns) {
    const needle = Buffer.from(pattern, 'latin1');
    for (let at = buffer.indexOf(needle); at !== -1; at = buffer.indexOf(needle, at + needle.length)) {
      const text = hitText(buffer, at), key = `${pattern}\0${text}`;
      const hit = hits.get(key);
      if (hit) hit.count++; else hits.set(key, { pattern, text, count: 1 });
    }
  }
  return [...hits.values()];
}

/** A dotted version as numbers, for comparing minos with the minimum. */
const versionParts = (text) => String(text).split('.').map(part => Number(part) || 0);
export function versionAbove(version, minimum) {
  const a = versionParts(version), b = versionParts(minimum);
  for (let i = 0; i < Math.max(a.length, b.length); i++) if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) > (b[i] ?? 0);
  return false;
}

/** lipo, vtool and otool's answers for one Mach-O file. */
export function machOFacts(path) {
  const archs = run('lipo', ['-archs', path]).stdout.trim().split(/\s+/).filter(Boolean);
  const minos = [...run('vtool', ['-show-build', path]).stdout.matchAll(/^\s*minos\s+(\S+)/gm)].map(match => match[1]);
  // otool reads `name(member)` as an archive member, so "T3 Code (Exact)" is read through a plain link.
  let link = null;
  if (/[()]/.test(path)) { link = join(mkdtempSync(join(tmpdir(), 'audit-otool-')), 'macho'); symlinkSync(path, link); }
  try { return { archs, minos, ...otoolFacts(link ?? path) }; } finally { if (link) rmSync(dirname(link), { recursive: true, force: true }); }
}

function otoolFacts(path) {
  const loads = run('otool', ['-l', path]).stdout;
  // `otool -L` lists a dylib's own install name first; only what it loads counts.
  const id = /cmd LC_ID_DYLIB\n\s+cmdsize \d+\n\s+name (.+?) \(offset/.exec(loads)?.[1] ?? null;
  const listed = run('otool', ['-L', path]).stdout.split('\n').slice(1).map(line => /^\s+(\S.*?) \(compatibility/.exec(line)?.[1]).filter(Boolean);
  const libraries = id && listed[0] === id ? listed.slice(1) : listed;
  const rpaths = [...loads.matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (.+?) \(offset/g)].map(match => match[1]);
  return { id, libraries, rpaths };
}

/** A dependency is the system's (/usr/lib, /System) or resolves to a file inside the bundle. */
export function dependencyInside(dependency, { file, bundle, rpaths }) {
  if (dependency.startsWith('/usr/lib/') || dependency.startsWith('/System/')) return true;
  const base = dirname(file), executables = join(bundle, 'Contents/MacOS');
  const candidates = dependency.startsWith('@rpath/')
    ? rpaths.map(rpath => join(rpath.replace(/^@executable_path/, executables).replace(/^@loader_path/, base), dependency.slice(7)))
    : dependency.startsWith('@executable_path/') ? [join(executables, dependency.slice(17))]
    : dependency.startsWith('@loader_path/') ? [join(base, dependency.slice(13))] : [];
  return candidates.some(path => (path === bundle || path.startsWith(bundle + sep)) && existsSync(path));
}

const LEFTOVER = [/\.map$/, /\.map\.json$/, /(^|\/)app\.contract\.d\.ts$/, /(^|\/)UI-PARITY-TODO\.tmp\.md$/, /\.test\.(ts|mjs|js|swift)$/, /(^|\/)fixtures?(\/|$)/, /(^|\/)\.git(\/|$)/, /(^|\/)\.DS_Store$/];

/** The allowlist: `files` (globs every bundle path must match) and `allow` (build-path hits with reasons). */
export function readAllowlist(path) {
  const list = JSON.parse(readFileSync(path, 'utf8'));
  for (const entry of list.allow ?? []) if (!entry.reason || !entry.text || !entry.scope) throw new Error(`${path}: every allow entry needs scope, text and reason (${JSON.stringify(entry)})`);
  return { minimumOS: list.minimumOS, bundleId: list.bundleId, files: list.files ?? [], allow: list.allow ?? [] };
}

/** The entry that allows a build-path hit, if one does. */
export function allowedBy(allowlist, scope, file, text) {
  return allowlist.allow.find(entry => entry.scope === scope && new Bun.Glob(entry.file ?? '**').match(file) && new RegExp(entry.text).test(text)) ?? null;
}

function plist(path) {
  const json = run('plutil', ['-convert', 'json', '-o', '-', path]);
  return json.status === 0 ? JSON.parse(json.stdout) : null;
}

/** Scans one tree's files for machine and build paths. */
function scanTree(root, scope, { machine, build = BUILD_PATTERNS, allowlist, findings, allowed, totals }) {
  for (const entry of walk(root)) {
    if (!entry.stat.isFile()) continue;
    const buffer = readFileSync(entry.abs);
    totals.files++; totals.bytes += buffer.length;
    for (const hit of scanBytes(buffer, machine)) {
      // A hit of the home folder that is the repository's (a longer machine path) is said once, as that.
      if (machine.some(path => path.length > hit.pattern.length && hit.text.startsWith(path))) continue;
      findings.push({ rule: 'machine-path', scope, file: entry.path, detail: `${hit.text} (${hit.count}×)` });
    }
    for (const hit of scanBytes(buffer, build)) {
      if (machine.some(path => hit.text.startsWith(path))) continue; // already a machine-path finding
      // One hit, one finding: a /private/tmp/… export path is not also its /tmp/… tail.
      if (build.some(pattern => pattern.length > hit.pattern.length && pattern.startsWith('/') && hit.text.startsWith(pattern))) continue;
      const by = allowedBy(allowlist, scope, entry.path, hit.text);
      if (by) allowed.push({ scope, file: entry.path, text: hit.text, count: hit.count, reason: by.reason });
      else findings.push({ rule: 'build-path', scope, file: entry.path, detail: `${hit.text} (${hit.count}×)` });
    }
  }
}

function checkMachO(file, rel, scope, { bundle, minimumOS, findings, totals }) {
  totals.machO++;
  const facts = machOFacts(file);
  if (facts.archs.join(' ') !== 'arm64') findings.push({ rule: 'arch', scope, file: rel, detail: `lipo -archs: ${facts.archs.join(' ') || 'none'}` });
  if (!facts.minos.length) findings.push({ rule: 'minos', scope, file: rel, detail: 'no LC_BUILD_VERSION minos' });
  for (const minos of facts.minos) if (versionAbove(minos, minimumOS)) findings.push({ rule: 'minos', scope, file: rel, detail: `minos ${minos} > ${minimumOS}` });
  // The system's own (the Swift runtime's /usr/lib/swift) is not the build machine's.
  for (const rpath of facts.rpaths) if (rpath.startsWith('/') && !/^\/(usr\/lib|System)\//.test(`${rpath}/`)) findings.push({ rule: 'rpath', scope, file: rel, detail: `LC_RPATH ${rpath}` });
  if (facts.id?.startsWith('/')) findings.push({ rule: 'dylib', scope, file: rel, detail: `install name ${facts.id}` });
  if (scope === 'bundle') {
    for (const dependency of facts.libraries) {
      if (!dependencyInside(dependency, { file, bundle, rpaths: facts.rpaths })) findings.push({ rule: 'dylib', scope, file: rel, detail: dependency });
    }
  } else {
    // The release's own: system libraries, or its own files by relative load paths.
    for (const dependency of facts.libraries) if (!/^(\/usr\/lib\/|\/System\/|@rpath\/|@loader_path\/|@executable_path\/)/.test(dependency)) findings.push({ rule: 'dylib', scope, file: rel, detail: dependency });
  }
  return facts;
}

export function audit(app, { t3Home = null, forbid = [], buildRoots = [], allowlistPath = defaultAllowlist, machine = machinePaths(forbid) } = {}) {
  const bundle = resolve(app), findings = [], allowed = [], totals = { files: 0, bytes: 0, machO: 0, runtimeFiles: 0 };
  // A build folder outside this machine's own paths (package-app.mjs's fixed export) is a build path:
  // a finding unless the allowlist names it, as /Users/ and the rest are.
  const roots = buildRoots.flatMap(path => { const out = [resolve(path)]; try { out.push(realpathSync(path)); } catch {} return out; });
  const build = [...new Set([...roots.sort((a, b) => b.length - a.length), ...BUILD_PATTERNS])];
  const allowlist = readAllowlist(allowlistPath);
  const minimumOS = allowlist.minimumOS;
  const facts = { bundle, minimumOS, findings, totals };
  const resources = join(bundle, 'Contents/Resources');
  if (!existsSync(join(bundle, 'Contents/Info.plist'))) throw new Error(`${bundle} is not an app bundle`);

  // The bundle: every file listed, every Mach-O file's facts, every byte scanned.
  const entries = walk(bundle);
  for (const entry of entries) {
    if (LEFTOVER.some(pattern => pattern.test(entry.path))) findings.push({ rule: 'dev-file', scope: 'bundle', file: entry.path, detail: 'a development file' });
    if (!entry.stat.isFile() && !entry.stat.isSymbolicLink()) continue;
    if (!allowlist.files.some(pattern => new Bun.Glob(pattern).match(entry.path))) findings.push({ rule: 'unexpected', scope: 'bundle', file: entry.path, detail: 'no `files` pattern covers it' });
    if (entry.stat.isFile() && isMachO(entry.abs)) checkMachO(entry.abs, entry.path, 'bundle', facts);
  }
  scanTree(bundle, 'bundle', { machine, build, allowlist, findings, allowed, totals });

  // The app: signature, Info.plist, the packaged marker.
  const verify = run('codesign', ['--verify', '--deep', '--strict', bundle]);
  if (verify.status !== 0) findings.push({ rule: 'signature', scope: 'bundle', file: '.', detail: verify.stderr.trim() });
  const signer = run('codesign', ['-dv', bundle]).stderr;
  const info = plist(join(bundle, 'Contents/Info.plist')) ?? {};
  if (info.LSMinimumSystemVersion !== minimumOS) findings.push({ rule: 'info-plist', scope: 'bundle', file: 'Contents/Info.plist', detail: `LSMinimumSystemVersion ${info.LSMinimumSystemVersion ?? 'missing'}, expected ${minimumOS}` });
  if (info.CFBundleIdentifier !== allowlist.bundleId) findings.push({ rule: 'info-plist', scope: 'bundle', file: 'Contents/Info.plist', detail: `CFBundleIdentifier ${info.CFBundleIdentifier ?? 'missing'}, expected ${allowlist.bundleId}` });
  let distribution = null;
  try { distribution = JSON.parse(readFileSync(join(resources, 'distribution.json'), 'utf8')); } catch {}
  if (distribution?.flavor !== 'packaged') findings.push({ rule: 'distribution', scope: 'bundle', file: 'Contents/Resources/distribution.json', detail: distribution ? `flavor ${JSON.stringify(distribution.flavor)}` : 'missing' });

  // The runtime parts: archive = manifest = pin.
  const runtime = join(resources, 't3-runtime');
  let pin = null, manifest = null;
  try { pin = JSON.parse(readFileSync(join(runtime, 'runtime-pin.json'), 'utf8')); } catch {}
  try { manifest = JSON.parse(readFileSync(join(runtime, 'runtime-manifest.json'), 'utf8')); } catch {}
  if (!pin || !manifest) findings.push({ rule: 'runtime-part', scope: 'bundle', file: 'Contents/Resources/t3-runtime', detail: `missing ${!pin ? 'runtime-pin.json' : 'runtime-manifest.json'}` });
  else {
    for (const key of ['version', 'asset', 'sha256', 'size']) if (manifest[key] !== pin[key]) findings.push({ rule: 'runtime-part', scope: 'bundle', file: 'Contents/Resources/t3-runtime/runtime-manifest.json', detail: `${key} ${manifest[key]}, the pin says ${pin[key]}` });
    const archive = join(runtime, pin.asset);
    const digest = existsSync(archive) ? sha256File(archive) : null;
    if (digest !== manifest.sha256) findings.push({ rule: 'runtime-part', scope: 'bundle', file: `Contents/Resources/t3-runtime/${pin.asset}`, detail: digest ? `sha256 ${digest}, the manifest says ${manifest.sha256}` : 'missing' });
  }

  // The first launch's tree: what the manifest says, executables 0755, signatures, no escaping links.
  let installed = null;
  if (t3Home && manifest) {
    const versionDir = join(resolve(t3Home), 'runtime/versions', manifest.version);
    installed = versionDir;
    let sentinel = null;
    try { sentinel = readFileSync(join(versionDir, '.install-complete'), 'utf8').trim(); } catch {}
    if (sentinel !== manifest.version) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: '.install-complete', detail: sentinel === null ? 'missing' : `holds ${sentinel}` });
    const expected = new Set(manifest.entries.map(entry => entry.path));
    for (const entry of manifest.entries) {
      const path = join(versionDir, entry.path);
      let stat = null;
      try { stat = lstatSync(path); } catch {}
      if (!stat) { findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: 'missing' }); continue; }
      const mode = stat.mode & 0o7777;
      if (entry.type === 'link') {
        if (!stat.isSymbolicLink() || readlinkSync(path) !== entry.link) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: 'not the manifest\'s link' });
        const target = resolve(dirname(path), readlinkSync(path));
        if (!(target === versionDir || target.startsWith(versionDir + sep))) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: `symlink out of the runtime folder: ${readlinkSync(path)}` });
      } else if (entry.type === 'dir') {
        if (!stat.isDirectory() || mode !== entry.mode) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: `folder mode ${mode.toString(8)}, the manifest says ${entry.mode.toString(8)}` });
      } else {
        totals.runtimeFiles++;
        if (!stat.isFile() || mode !== entry.mode || stat.size !== entry.size) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: `mode ${mode.toString(8)} size ${stat.size}, the manifest says ${entry.mode.toString(8)} ${entry.size}` });
        else if (sha256File(path) !== entry.sha256) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: 'sha256 differs from the manifest' });
        if (entry.mode & 0o111 && mode !== 0o755) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: `an executable of mode ${mode.toString(8)}, not 755` });
        if (stat.isFile() && isMachO(path)) {
          checkMachO(path, entry.path, 'runtime', facts);
          const signed = run('codesign', ['--verify', '--deep', '--strict', path]);
          if (signed.status !== 0) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: `codesign --verify --deep --strict: ${signed.stderr.trim()}` });
        }
      }
    }
    for (const entry of existsSync(versionDir) ? walk(versionDir) : []) {
      if (entry.path !== '.install-complete' && !expected.has(entry.path)) findings.push({ rule: 'runtime-tree', scope: 'runtime', file: entry.path, detail: 'not in the manifest' });
    }
    if (existsSync(versionDir)) scanTree(versionDir, 'runtime', { machine, build, allowlist, findings, allowed, totals });
  }

  return {
    app: bundle, t3Home: t3Home ? resolve(t3Home) : null, installed, minimumOS, bundleId: info.CFBundleIdentifier ?? null,
    signature: { verify: verify.status === 0 ? 'valid on disk, satisfies its Designated Requirement' : verify.stderr.trim(),
      kind: /Signature=adhoc/.test(signer) ? 'ad hoc' : (/Authority=([^\n]+)/.exec(signer)?.[1] ?? 'unknown'), teamId: /TeamIdentifier=(.+)/.exec(signer)?.[1]?.replace(/^not set$/, '') || null },
    machine, totals, findings, allowed: summarizeAllowed(allowed),
  };
}

/** Allowed hits grouped by their allowlist reason (with file and hit counts). */
function summarizeAllowed(allowed) {
  const groups = new Map();
  for (const hit of allowed) {
    const group = groups.get(hit.reason) ?? { reason: hit.reason, scope: hit.scope, files: new Set(), hits: 0, examples: [] };
    group.files.add(hit.file); group.hits += hit.count;
    if (group.examples.length < 3 && !group.examples.includes(hit.text)) group.examples.push(hit.text);
    groups.set(hit.reason, group);
  }
  return [...groups.values()].map(group => ({ ...group, files: group.files.size }));
}

/** The report as text: totals, findings, then the allowed hits with their reasons. */
export function formatReport(report, redact = []) {
  const clean = (text) => redact.reduce((value, [from, to]) => value.split(from).join(to), String(text));
  const lines = [
    `audit-bundle: ${clean(report.app)}`,
    `  signature: ${report.signature.kind}${report.signature.teamId ? ` (team ${report.signature.teamId})` : ' (no team)'}; codesign --verify --deep --strict: ${clean(report.signature.verify)}`,
    `  bundle id ${report.bundleId}; stated minimum macOS ${report.minimumOS}`,
    `  scanned ${report.totals.files} files (${(report.totals.bytes / 1048576).toFixed(1)} MiB), ${report.totals.machO} Mach-O files${report.installed ? `; runtime tree ${clean(report.installed)} (${report.totals.runtimeFiles} files checked against the manifest)` : '; no first-launch tree (--t3-home not given)'}`,
    `  machine paths searched: ${report.machine.map(clean).join(', ')}`,
    `  findings: ${report.findings.length}`,
    ...report.findings.map(finding => `    ${finding.rule} [${finding.scope}] ${clean(finding.file)}: ${clean(finding.detail)}`),
    `  allowed (bundle-allowlist.json): ${report.allowed.reduce((sum, group) => sum + group.hits, 0)} hits`,
    ...report.allowed.map(group => `    [${group.scope}] ${group.hits} hits in ${group.files} files — ${group.reason}\n      e.g. ${group.examples.map(clean).join(' | ')}`),
  ];
  return lines.join('\n');
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const take = (flag) => { const values = []; for (let i = 0; i < args.length; i++) if (args[i] === flag) values.push(args[++i]); return values; };
  const app = args.find((arg, i) => !arg.startsWith('--') && !['--t3-home', '--forbid', '--build-root', '--allowlist', '--json'].includes(args[i - 1]));
  if (!app) { console.error('usage: bun audit-bundle.mjs <App.app> [--t3-home <dir>] [--forbid <path>]… [--build-root <path>]… [--allowlist <file>] [--json <out>]'); process.exit(2); }
  const report = audit(app, { t3Home: take('--t3-home')[0] ?? null, forbid: take('--forbid'), buildRoots: take('--build-root'), allowlistPath: take('--allowlist')[0] ?? defaultAllowlist });
  console.log(formatReport(report));
  const out = take('--json')[0];
  if (out) writeFileSync(out, `${JSON.stringify(report, null, 2)}\n`);
  process.exit(report.findings.length ? 1 : 0);
}
