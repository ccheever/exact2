// 20261005-portable-app-download item 2: audit-bundle.mjs on synthetic bundles. Each test builds a
// small `.app` (tiny Mach-O files from clang, an Info.plist, the runtime's pin, manifest and archive)
// in a scratch folder, breaks one thing, and expects exactly that rule to fire.
import { afterAll, beforeAll, describe, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { audit, allowedBy, dependencyInside, formatReport, machinePaths, readAllowlist, scanBytes, versionAbove } from './audit-bundle.mjs';
import { manifestOf, sha256File } from './stage-runtime.mjs';

const sh = (cmd: string, args: string[], input?: string) => {
  const result = spawnSync(cmd, args, { encoding: 'utf8', input });
  if (result.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${result.stderr}`);
  return result.stdout;
};
const C = 'int main(void) { return 0; }\n';
/** A tiny Mach-O file: arm64 for macOS 14 unless `extra` says otherwise (a later -arch or -mmacosx-version-min wins). */
const cc = (out: string, extra: string[] = [], source = C) => {
  writeFileSync(`${out}.c`, source);
  const arch = extra.includes('-arch') ? [] : ['-arch', 'arm64'];
  sh('xcrun', ['clang', `${out}.c`, '-o', out, ...arch, '-mmacosx-version-min=14.0', ...extra]);
  rmSync(`${out}.c`);
};

let scratch = '', parts = '', machine: string[] = [];
const ID = 'com.exact.t3code.macos', VERSION = '1.0.0', ASSET = `t3-${VERSION}-darwin-arm64.tar.gz`;

beforeAll(() => {
  scratch = mkdtempSync(join(tmpdir(), 'audit-bundle-test-'));
  parts = join(scratch, 'parts');
  mkdirSync(parts);
  cc(join(parts, 'exe'));
  cc(join(parts, 'exe-x86'), ['-arch', 'x86_64']);
  cc(join(parts, 'exe-15'), ['-mmacosx-version-min=15.0']);
  cc(join(parts, 'exe-rpath'), ['-Wl,-rpath,/opt/homebrew/lib']);
  cc(join(parts, 'node.node'), ['-bundle'], 'int x = 1;\n');
  // A dylib named by its build path, and an executable that loads it from there.
  cc(join(parts, 'libbuilt.dylib'), ['-dynamiclib', '-install_name', join(parts, 'libbuilt.dylib')], 'int built(void) { return 1; }\n');
  cc(join(parts, 'exe-dylib'), [join(parts, 'libbuilt.dylib')], 'int built(void);\nint main(void) { return built(); }\n');
  // The release-shaped runtime: t3, a native addon, a client file.
  const tree = join(parts, `t3-${VERSION}-darwin-arm64`);
  mkdirSync(join(tree, 'node_modules/pty'), { recursive: true });
  mkdirSync(join(tree, 'client'), { recursive: true });
  cpSync(join(parts, 'exe'), join(tree, 't3'));
  chmodSync(join(tree, 't3'), 0o755);
  cpSync(join(parts, 'node.node'), join(tree, 'node_modules/pty/pty.node'));
  writeFileSync(join(tree, 'client/index.html'), '<html></html>\n');
  sh('/usr/bin/tar', ['-czf', join(parts, ASSET), '-C', parts, `t3-${VERSION}-darwin-arm64`]);
  const entries = manifestOf(tree), files = entries.filter((entry: { type: string }) => entry.type === 'file');
  const archive = { version: VERSION, asset: ASSET, sha256: sha256File(join(parts, ASSET)), size: Number(readFileSync(join(parts, ASSET)).length) };
  writeFileSync(join(parts, 'runtime-pin.json'), JSON.stringify(archive));
  writeFileSync(join(parts, 'runtime-manifest.json'), JSON.stringify({ ...archive, root: `t3-${VERSION}-darwin-arm64`, stripComponents: 1, files: files.length,
    bytes: files.reduce((sum: number, entry: { size: number }) => sum + entry.size, 0), entries }));
  writeFileSync(join(parts, 'allowlist.json'), JSON.stringify({
    minimumOS: '14.0', bundleId: ID,
    files: ['Contents/Info.plist', 'Contents/_CodeSignature/CodeResources', 'Contents/MacOS/*', 'Contents/Resources/distribution.json', 'Contents/Resources/notes.txt', 'Contents/Resources/t3-runtime/*'],
    allow: [{ scope: 'bundle', file: 'Contents/Resources/notes.txt', text: '^/Users/runner/', reason: 'a CI path named in a note' },
      { scope: 'runtime', file: '**', text: '^/private/var/folders/', reason: 'a library literal' }],
  }));
  machine = machinePaths(['/Users/someone-else/exact2'], { home: '/Users/builder', repo: '/Users/builder/exact2', temporary: '/Users/builder/tmp' });
});
afterAll(() => { rmSync(scratch, { recursive: true, force: true }); });

type Options = { exe?: string; extra?: [string, string][]; plist?: Record<string, string>; distribution?: string | null; notes?: string; tamper?: boolean; sign?: boolean };
/** A bundle from the parts; `options` breaks one thing. */
function bundle(name: string, options: Options = {}) {
  const app = join(scratch, name, 'T3 Code (Exact).app'), contents = join(app, 'Contents');
  mkdirSync(join(contents, 'MacOS'), { recursive: true });
  mkdirSync(join(contents, 'Resources/t3-runtime'), { recursive: true });
  cpSync(join(parts, options.exe ?? 'exe'), join(contents, 'MacOS/T3 Code (Exact)'));
  for (const [from, to] of options.extra ?? []) cpSync(join(parts, from), join(contents, to));
  const plist = { CFBundleExecutable: 'T3 Code (Exact)', CFBundleIdentifier: ID, CFBundlePackageType: 'APPL', LSMinimumSystemVersion: '14.0', ...options.plist };
  writeFileSync(join(contents, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict>${Object.entries(plist).map(([k, v]) => `<key>${k}</key><string>${v}</string>`).join('')}</dict></plist>\n`);
  if (options.distribution !== null) writeFileSync(join(contents, 'Resources/distribution.json'), options.distribution ?? '{"flavor":"packaged"}\n');
  writeFileSync(join(contents, 'Resources/notes.txt'), options.notes ?? 'built on CI at /Users/runner/work/t3code\n');
  for (const file of ['runtime-pin.json', 'runtime-manifest.json', ASSET]) cpSync(join(parts, file), join(contents, 'Resources/t3-runtime', file));
  if (options.tamper) writeFileSync(join(contents, 'Resources/t3-runtime', ASSET), 'not the archive');
  if (options.sign !== false) sh('codesign', ['--force', '--sign', '-', '--timestamp=none', app]);
  return app;
}
/** The tree a first launch unpacks, under `<home>/runtime/versions/<version>`. */
function firstLaunch(name: string, change?: (dir: string) => void) {
  const home = join(scratch, name, 't3-home'), dir = join(home, 'runtime/versions', VERSION);
  mkdirSync(dir, { recursive: true });
  sh('/usr/bin/tar', ['-xzf', join(parts, ASSET), '-C', dir, '--strip-components=1']);
  writeFileSync(join(dir, '.install-complete'), `${VERSION}\n`);
  change?.(dir);
  return home;
}
const run = (app: string, t3Home: string | null = null) => audit(app, { t3Home, allowlistPath: join(parts, 'allowlist.json'), machine });
const rules = (report: { findings: { rule: string }[] }) => [...new Set(report.findings.map(finding => finding.rule))].sort();

describe('audit-bundle', () => {
  test('a clean bundle and its first-launch tree have no findings; allowed hits carry their reasons', () => {
    const report = run(bundle('clean'), firstLaunch('clean'));
    expect(report.findings).toEqual([]);
    expect(report.signature).toMatchObject({ kind: 'ad hoc', teamId: null });
    expect(report.allowed.map((group: { reason: string }) => group.reason)).toEqual(['a CI path named in a note']);
    expect(report.totals.machO).toBe(3);
    expect(formatReport(report)).toContain('findings: 0');
  });

  test('architecture, minimum macOS and an absolute rpath', () => {
    expect(rules(run(bundle('x86', { exe: 'exe-x86' })))).toEqual(['arch']);
    expect(run(bundle('minos', { exe: 'exe-15' })).findings[0]).toMatchObject({ rule: 'minos', detail: 'minos 15.0 > 14.0' });
    const rpath = run(bundle('rpath', { exe: 'exe-rpath' }));
    expect(rules(rpath)).toEqual(['build-path', 'rpath']);
    expect(rpath.findings.find((finding: { rule: string }) => finding.rule === 'rpath')).toEqual({ rule: 'rpath', scope: 'bundle', file: 'Contents/MacOS/T3 Code (Exact)', detail: 'LC_RPATH /opt/homebrew/lib' });
  });

  test('a library loaded from the build machine, and a dylib named by its build path', () => {
    const report = run(bundle('dylib', { exe: 'exe-dylib', extra: [['libbuilt.dylib', 'MacOS/libbuilt.dylib']] }));
    expect(report.findings.filter((finding: { rule: string }) => finding.rule === 'dylib').map((finding: { detail: string }) => finding.detail).sort())
      .toEqual([join(parts, 'libbuilt.dylib'), `install name ${join(parts, 'libbuilt.dylib')}`].sort());
  });

  test('a machine path is never allowed; a build path only with its allowlist entry', () => {
    const machineHit = run(bundle('machine', { notes: 'cache at /Users/builder/exact2/target/x and /Users/someone-else/exact2/kernel\n' }));
    expect(machineHit.findings.map((finding: { rule: string; detail: string }) => `${finding.rule} ${finding.detail}`))
      .toEqual(['machine-path /Users/someone-else/exact2/kernel (1×)', 'machine-path /Users/builder/exact2/target/x (1×)']);
    const buildHit = run(bundle('build', { notes: 'from /opt/homebrew/bin/node\n' }));
    expect(buildHit.findings).toEqual([{ rule: 'build-path', scope: 'bundle', file: 'Contents/Resources/notes.txt', detail: '/opt/homebrew/bin/node (1×)' }]);
  });

  test('a build root is a build path: allowed only by its entry', () => {
    const root = join(scratch, 'export-root');
    const app = bundle('build-root', { notes: `compiled from ${root}/vendor/x.js\n` });
    const report = audit(app, { allowlistPath: join(parts, 'allowlist.json'), machine, buildRoots: [root] });
    expect(report.findings).toEqual([{ rule: 'build-path', scope: 'bundle', file: 'Contents/Resources/notes.txt', detail: `${root}/vendor/x.js (1×)` }]);
    writeFileSync(join(parts, 'allowlist-root.json'), JSON.stringify({ ...JSON.parse(readFileSync(join(parts, 'allowlist.json'), 'utf8')),
      allow: [{ scope: 'bundle', file: 'Contents/Resources/notes.txt', text: `^${root}/vendor/`, reason: 'the fixed export folder' }] }));
    expect(audit(app, { allowlistPath: join(parts, 'allowlist-root.json'), machine, buildRoots: [root] }).findings).toEqual([]);
  });

  test('leftover development files and files no pattern covers', () => {
    const app = bundle('leftover', { sign: false });
    writeFileSync(join(app, 'Contents/Resources/app.js.map.json'), '{}');
    mkdirSync(join(app, 'Contents/Resources/.git'));
    writeFileSync(join(app, 'Contents/Resources/.git/HEAD'), 'ref: refs/heads/main\n');
    mkdirSync(join(app, 'Contents/Resources/.icon-AbC123'));
    mkdirSync(join(app, 'Contents/Resources/empty'));
    sh('codesign', ['--force', '--sign', '-', '--timestamp=none', app]);
    const report = run(app);
    expect(rules(report)).toEqual(['dev-file', 'unexpected']);
    expect(report.findings.filter((finding: { rule: string }) => finding.rule === 'dev-file').map((finding: { file: string }) => finding.file))
      .toEqual(['Contents/Resources/.git', 'Contents/Resources/.git/HEAD', 'Contents/Resources/.icon-AbC123', 'Contents/Resources/app.js.map.json', 'Contents/Resources/empty']);
  });

  test('the packaged marker, the bundle id and LSMinimumSystemVersion', () => {
    expect(run(bundle('marker-missing', { distribution: null })).findings).toEqual([{ rule: 'distribution', scope: 'bundle', file: 'Contents/Resources/distribution.json', detail: 'missing' }]);
    expect(run(bundle('marker-dev', { distribution: '{"flavor":"development"}' })).findings[0]).toMatchObject({ rule: 'distribution', detail: 'flavor "development"' });
    expect(run(bundle('original-id', { plist: { CFBundleIdentifier: 'com.t3tools.t3code' } })).findings[0]).toMatchObject({ rule: 'info-plist', detail: 'CFBundleIdentifier com.t3tools.t3code, expected com.exact.t3code.macos' });
    expect(run(bundle('plist-15', { plist: { LSMinimumSystemVersion: '15.0' } })).findings[0]).toMatchObject({ rule: 'info-plist', detail: 'LSMinimumSystemVersion 15.0, expected 14.0' });
  });

  test('a runtime part that is not the manifest\'s, and a bundle changed after signing', () => {
    expect(run(bundle('tampered', { tamper: true })).findings.map((finding: { rule: string }) => finding.rule)).toEqual(['runtime-part']);
    const app = bundle('resealed');
    writeFileSync(join(app, 'Contents/Resources/notes.txt'), 'changed after signing at /Users/runner/x\n');
    expect(rules(run(app))).toEqual(['signature']);
  });

  test('the first-launch tree: modes, signatures, escaping links, the manifest and the sentinel', () => {
    const app = bundle('tree');
    const report = run(app, firstLaunch('tree', (dir) => {
      chmodSync(join(dir, 't3'), 0o700);
      sh('codesign', ['--remove-signature', join(dir, 'node_modules/pty/pty.node')]);
      symlinkSync('../../../../../escape', join(dir, 'client/out'));
      writeFileSync(join(dir, '.install-complete'), '0.9.0\n');
    }));
    const said = report.findings.map((finding: { rule: string; file: string; detail: string }) => `${finding.rule} ${finding.file}: ${finding.detail}`);
    expect(new Set(report.findings.map((finding: { rule: string }) => finding.rule))).toEqual(new Set(['runtime-tree']));
    for (const expected of [
      /^runtime-tree \.install-complete: holds 0\.9\.0$/,
      /^runtime-tree client\/out: not in the manifest$/,
      /^runtime-tree t3: an executable of mode 700, not 755$/,
      /^runtime-tree t3: mode 700 size \d+, the manifest says 755 \d+$/,
      /^runtime-tree node_modules\/pty\/pty\.node: codesign --verify --deep --strict: .*not signed/,
    ]) expect(said.some((line: string) => expected.test(line))).toBe(true);
    // The link out of the folder is not the manifest's (it has none), so it is named as an extra path.
    expect(said.filter((line: string) => line.includes('pty.node')).length).toBeGreaterThanOrEqual(2);
  });
});

describe('audit-bundle parts', () => {
  test('scanBytes names each hit by its token', () => {
    expect(scanBytes(Buffer.from('a\0/Users/runner/work/x.js"rest /Users/runner/work/x.js\0'), ['/Users/'])).toEqual([{ pattern: '/Users/', text: '/Users/runner/work/x.js', count: 2 }]);
  });
  test('versions compare by number', () => {
    expect(versionAbove('14.0', '14.0')).toBe(false);
    expect(versionAbove('13.5', '14.0')).toBe(false);
    expect(versionAbove('14.1', '14.0')).toBe(true);
    expect(versionAbove('15', '14.0')).toBe(true);
  });
  test('a dependency inside the bundle by @rpath, @executable_path or @loader_path', () => {
    const app = bundle('inside');
    const file = join(app, 'Contents/MacOS/T3 Code (Exact)');
    writeFileSync(join(app, 'Contents/MacOS/libx.dylib'), '');
    expect(dependencyInside('@rpath/libx.dylib', { file, bundle: app, rpaths: ['@executable_path'] })).toBe(true);
    expect(dependencyInside('@executable_path/libx.dylib', { file, bundle: app, rpaths: [] })).toBe(true);
    expect(dependencyInside('@loader_path/libx.dylib', { file, bundle: app, rpaths: [] })).toBe(true);
    expect(dependencyInside('@rpath/libmissing.dylib', { file, bundle: app, rpaths: ['@executable_path'] })).toBe(false);
    expect(dependencyInside('/usr/lib/libSystem.B.dylib', { file, bundle: app, rpaths: [] })).toBe(true);
    expect(dependencyInside('/opt/homebrew/lib/libz.dylib', { file, bundle: app, rpaths: [] })).toBe(false);
  });
  test('the committed allowlist is well formed and covers what it allows with reasons', () => {
    const list = readAllowlist(join(import.meta.dir, 'bundle-allowlist.json'));
    expect(list.minimumOS).toBe('14.0');
    expect(list.bundleId).toBe('com.exact.t3code.macos');
    for (const entry of list.allow) expect(entry.reason.length).toBeGreaterThan(20);
    expect(allowedBy(list, 'bundle', 'Contents/MacOS/T3 Code (Exact)', '/Users/daehyeonmun/x')).toBeNull();
  });
});
