// The app's icon and asset catalog, from its manifest, for an Apple bundle
// (host/apple/build.mjs assembles it).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, dirname, relative, resolve, isAbsolute } from 'node:path';
import { chmodSync, closeSync, lstatSync, openSync, readlinkSync, readSync, copyFileSync, realpathSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
// The build's stamp, which build.mjs merges into Info.plist beside the icon keys.
export { buildInfo } from './buildinfo.mjs';

const run = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { ...opts, stdio: opts.stdio === 'ignore' ? ['ignore', 'ignore', 'pipe'] : opts.stdio ?? 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})${r.stderr?.length ? ': ' + String(r.stderr).trim() : ''}`);
  return r;
};

/** The app icon from the manifest's first square icon of at least 512 px
 * (`icons`, the web manifest's own field): loose PNGs named by
 * `CFBundleIcons` on iOS, an `.icns` built by `iconutil` on macOS. Returns
 * the plist keys to merge; nothing when the app declares no such icon. */
export function appIcon(app, dir, platform, { catalog = false } = {}) {
  const icon = (app.manifest.icons ?? []).find((i) => { const m = /^(\d+)x(\d+)$/.exec(i.sizes ?? ''); return m && m[1] === m[2] && Number(m[1]) >= 512; });
  if (!icon) return {};
  const source = resolve(app.dir, icon.src);
  if (!existsSync(source)) throw new Error(`host/apple: ${app.name}'s icon ${icon.src} does not exist`);
  const sized = (px, out) => run('sips', ['-z', String(px), String(px), source, '--out', out], { stdio: 'ignore' });
  if (platform === 'ios') {
    for (const [name, px] of [['AppIcon60x60@2x.png', 120], ['AppIcon60x60@3x.png', 180], ['AppIcon76x76@2x~ipad.png', 152], ['AppIcon83.5x83.5@2x~ipad.png', 167]]) sized(px, resolve(dir, name));
    // A distributed build also compiles the icon into Assets.car: App Store
    // Connect requires the asset catalog, not loose PNGs.
    if (catalog) {
      const set = resolve(catalog, 'AppIcon.appiconset');
      mkdirSync(set, { recursive: true });
      sized(1024, resolve(set, 'icon.png'));
      writeFileSync(resolve(set, 'Contents.json'), JSON.stringify({ images: [{ filename: 'icon.png', idiom: 'universal', platform: 'ios', size: '1024x1024' }], info: { author: 'exact', version: 1 } }));
    }
    const primary = (files) => ({ CFBundlePrimaryIcon: { CFBundleIconFiles: files, CFBundleIconName: 'AppIcon' } });
    return { CFBundleIcons: primary(['AppIcon60x60']), 'CFBundleIcons~ipad': primary(['AppIcon60x60', 'AppIcon76x76', 'AppIcon83.5x83.5']) };
  }
  const set = mkdtempSync(resolve(dir, '.icon-')) + '.iconset';
  mkdirSync(set);
  for (const base of [16, 32, 128, 256, 512]) {
    sized(base, resolve(set, `icon_${base}x${base}.png`));
    sized(base * 2, resolve(set, `icon_${base}x${base}@2x.png`));
  }
  run('iconutil', ['-c', 'icns', set, '-o', resolve(dir, 'AppIcon.icns')], { stdio: 'ignore' });
  rmSync(set, { recursive: true, force: true });
  return { CFBundleIconFile: 'AppIcon' };
}

/** Catalogs follow the app's native module directory selection. Their bytes
 * are native binary inputs, not portable plan assets or live update payloads. */
export function appleAssetCatalogInventory(app, platform) {
  const own = resolve(app.dir, platform, 'modules');
  const folder = existsSync(own) ? own : resolve(app.dir, 'modules/apple');
  if (!existsSync(folder)) return [];
  return readdirSync(folder).filter(name => name.endsWith('.xcassets')).sort().map(name => {
    const path = resolve(folder, name);
    return { path: relative(app.dir, path), files: catalogFiles(path) };
  });
}
function catalogFiles(root) {
  const files = [];
  const walk = path => {
    const info = lstatSync(path);
    if (info.isSymbolicLink()) throw new Error(`Apple asset catalogs must not contain symlinks: ${path}`);
    if (info.isDirectory()) for (const name of readdirSync(path).sort()) walk(resolve(path, name));
    else if (info.isFile()) files.push({ name: relative(root, path), sha256: createHash('sha256').update(readFileSync(path)).digest('hex') });
    else throw new Error(`Unsupported Apple asset catalog input: ${path}`);
  };
  if (!lstatSync(root).isDirectory()) throw new Error(`Apple asset catalog must be a directory: ${root}`);
  walk(root);
  return files;
}

/** One actool pass includes the app's catalogs, generated icon and launch
 * colours: separate passes silently replace Assets.car. No unchanged build
 * reruns actool; the cache key includes source bytes, target and toolchain. */
export function iosAssets(app, dir, device, options = {}) {
  return appleAssets(app, dir, { ...options, device, platform: options.tv ? 'tvos' : 'ios' });
}
export function appleAssets(app, dir, { platform = 'macos', device = false, catalog = false, kept = null, expected, launch = 'system' } = {}) {
  const sourcePlatform = platform === 'tvos' ? 'ios' : platform;
  const catalogs = appleAssetCatalogInventory(app, sourcePlatform);
  if (expected !== undefined && JSON.stringify(catalogs) !== JSON.stringify(expected))
    throw new Error('Apple asset catalogs changed after the bake; rebuild before packaging');
  if (platform !== 'ios' && !catalogs.length) { rmSync(resolve(dir, 'Assets.car'), { force: true }); return {}; }
  if (kept) {
    const icons = (app.manifest.icons ?? []).map(icon => [icon, existsSync(resolve(app.dir, icon.src)) ? createHash('sha256').update(readFileSync(resolve(app.dir, icon.src))).digest('hex') : null]);
    const key = createHash('sha256').update(JSON.stringify([catalogs, icons, app.manifest.background_color ?? null, app.manifest.background_color_dark ?? null, launch,
      app.manifest.host?.[sourcePlatform]?.minimumOS ?? null, platform, device, catalog, kept.stamp,
      // What the compile itself writes: a cache from before `--accent-color` named no accent
      'accent-color'])).digest('hex').slice(0, 16);
    const made = resolve(kept.dir, `assets-${platform}-${key}`);
    if (!existsSync(resolve(made, 'keys.json'))) {
      const making = `${made}.${process.pid}.tmp`;
      rmSync(making, { recursive: true, force: true });
      mkdirSync(making, { recursive: true });
      try {
        writeFileSync(resolve(making, 'keys.json'), JSON.stringify(appleAssets(app, making, { platform, device, catalog, expected: catalogs, launch })));
        rmSync(made, { recursive: true, force: true });
        renameSync(making, made);
      } finally { rmSync(making, { recursive: true, force: true }); }
      for (const old of readdirSync(kept.dir)) if (old.startsWith(`assets-${platform}-`) && resolve(kept.dir, old) !== made) rmSync(resolve(kept.dir, old), { recursive: true, force: true });
    }
    rmSync(resolve(dir, 'Assets.car'), { force: true });
    for (const file of readdirSync(made)) if (file !== 'keys.json') cpSync(resolve(made, file), resolve(dir, file), { recursive: true });
    return JSON.parse(readFileSync(resolve(made, 'keys.json'), 'utf8'));
  }
  const work = mkdtempSync(resolve(tmpdir(), 'exact-apple-assets-'));
  try {
    const sources = catalogs.map(entry => {
      const copied = resolve(work, 'app', entry.path);
      cpSync(resolve(app.dir, entry.path), copied, { recursive: true });
      if (JSON.stringify(catalogFiles(copied)) !== JSON.stringify(entry.files))
        throw new Error('Apple asset catalogs changed while capturing them; rebuild before packaging');
      return copied;
    });
    const assets = resolve(work, 'Generated.xcassets');
    const keys = platform === 'ios' ? { ...appIcon(app, dir, 'ios', { catalog: catalog ? assets : false }), ...launchScreen(app, assets, launch) } : {};
    const hasIcon = existsSync(resolve(assets, 'AppIcon.appiconset'));
    if (catalog && platform === 'ios' && !hasIcon) throw new Error(`host/apple: ${app.name}'s distribution bundle requires an AppIcon; declare a square icon of at least 512 px`);
    if (hasIcon || keys.UILaunchScreen) {
      writeFileSync(resolve(assets, 'Contents.json'), JSON.stringify({ info: { author: 'exact', version: 1 } }));
      sources.push(assets);
    }
    rmSync(resolve(dir, 'Assets.car'), { force: true });
    // An `AccentColor` colour set is the app's accent, as Xcode's default
    // names it: actool writes `NSAccentColorName`, which AppKit applies and
    // ExactKit gives every UIKit window (`AppAccent`) under an `auto` accent.
    const accent = catalogs.some(c => c.files.some(f => /(^|\/)AccentColor\.colorset\/Contents\.json$/.test(f.name)));
    if (sources.length) {
      const partial = resolve(work, 'partial.plist');
      const target = platform === 'macos' ? 'macosx' : platform === 'tvos' ? (device ? 'appletvos' : 'appletvsimulator') : device ? 'iphoneos' : 'iphonesimulator';
      run('xcrun', ['actool', ...sources, '--compile', dir, '--platform', target,
        '--minimum-deployment-target', app.manifest.host?.[sourcePlatform]?.minimumOS ?? (platform === 'macos' ? '14.0' : '17.0'),
        ...(platform === 'macos' ? [] : ['--target-device', ...(platform === 'tvos' ? ['tv'] : ['iphone', '--target-device', 'ipad'])]),
        ...(hasIcon ? ['--app-icon', 'AppIcon'] : []),
        ...(accent ? ['--accent-color', 'AccentColor'] : []),
        '--output-partial-info-plist', partial, '--output-format', 'human-readable-text'], { stdio: 'ignore' });
      Object.assign(keys, JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', partial], { encoding: 'utf8', stdio: 'pipe' }).stdout));
    }
    if (catalog && platform === 'ios') {
      const contents = JSON.parse(run('xcrun', ['assetutil', '--info', resolve(dir, 'Assets.car')], { encoding: 'utf8', stdio: 'pipe' }).stdout);
      if (!contents.some(asset => asset.Name === 'AppIcon')) throw new Error(`host/apple: ${dir}/Assets.car has no AppIcon`);
    }
    return keys;
  } finally { rmSync(work, { recursive: true, force: true }); }
}

/** The launch screen in the app's own background, light and dark
 * (the manifest's `background_color` and `background_color_dark`): iOS crossfades
 * from the launch screen to the first frame, and between two screens of one
 * colour that crossfade is invisible, so the app opens on its first frame.
 * Where the manifest names none, the app's unsaid background is the platform's
 * (LLP 1115 D2): `systemBackground`, which an empty `UILaunchScreen` already is
 * in both schemes, or, where the first route scrolls a grouped list (`launch`,
 * the bake's `launchBackground`, LLP 1116.011 D2.3), `systemGroupedBackground`,
 * which the system shows as #F2F2F7 and black. That is a colour set of its own
 * (a reference to the system colour compiles to its light value alone), so the
 * window under the first frame (`ExactLaunch`, read by the presenter) is the
 * same grey and nothing lighter shows between the launch screen and the first
 * frame (stopwatch, round 13: "blank white screen for ~1 s").
 * `UILaunchScreen` names colours only from an asset catalog, so this
 * writes its colour set for the shared compile. Returns the plist keys to merge. */
function launchScreen(app, catalog, launch = 'system') {
  const named = app.manifest.background_color;
  const light = named ?? (launch === 'grouped' ? '#F2F2F7' : undefined);
  const dark = named ? app.manifest.background_color_dark : launch === 'grouped' ? '#000000' : undefined;
  if (!light) return {};
  const components = (hex, field) => {
    const m = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(hex ?? '');
    if (!m) throw new Error(`${field} must be a #RGB, #RRGGBB or #RRGGBBAA colour, not ${JSON.stringify(hex)}`);
    const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
    const a = h.length === 8 ? parseInt(h.slice(6), 16) : 255;
    return { 'color-space': 'srgb', components: { red: `0x${h.slice(0, 2)}`, green: `0x${h.slice(2, 4)}`, blue: `0x${h.slice(4, 6)}`, alpha: (a / 255).toFixed(3) } };
  };
  const colors = [{ idiom: 'universal', color: components(light, 'background_color') }];
  if (dark) colors.push({ idiom: 'universal', appearances: [{ appearance: 'luminosity', value: 'dark' }], color: components(dark, 'background_color_dark') });
  mkdirSync(resolve(catalog, 'ExactLaunch.colorset'), { recursive: true });
  writeFileSync(resolve(catalog, 'ExactLaunch.colorset', 'Contents.json'), JSON.stringify({ colors, info: { author: 'exact', version: 1 } }));
  return { UILaunchScreen: { UIColorName: 'ExactLaunch' } };
}

const NESTED_BUNDLE = /\.(app|appex|bundle|framework|plugin|xpc|systemextension)$/;

/** Whether a file is Mach-O code, by its magic and not its name: a helper
 *  executable or a `.node` addon has no `.dylib` to go by. Thin 32- and 64-bit,
 *  either byte order, and universal (fat) files. A fat magic is also a Java
 *  class file's, told apart by what follows it: a fat header's architecture
 *  count is small, a class file's version is not. */
function machO(path) {
  const head = Buffer.alloc(8);
  let fd;
  try {
    fd = openSync(path, 'r');
    if (readSync(fd, head, 0, 8, 0) < 8) return false;
  } catch { return false; } finally { if (fd !== undefined) closeSync(fd); }
  const magic = head.readUInt32BE(0);
  if ([0xfeedface, 0xfeedfacf, 0xcefaedfe, 0xcffaedfe].includes(magic)) return true;
  if (magic === 0xcafebabe || magic === 0xcafebabf) return head.readUInt32BE(4) < 32;
  if (magic === 0xbebafeca || magic === 0xbfbafeca) return head.readUInt32LE(4) < 32;
  return false;
}

/** A bundle's main executable, which codesign signs when it signs the bundle
 *  (and signing that path alone seals the bundle, before what it holds): an
 *  app's `Contents/MacOS/<name>`, a versioned framework's `Versions/<current>/<name>`
 *  as the walk reaches it (not through the `Current` link), a shallow bundle's
 *  `<name>`. The plist is XML or, through `plutil`, binary. */
function bundleExecutable(bundle) {
  const executable = (plist) => {
    if (!existsSync(plist)) return null;
    const xml = /<key>CFBundleExecutable<\/key>\s*<string>([^<]+)<\/string>/.exec(readFileSync(plist, 'latin1'))?.[1];
    if (xml) return xml;
    const read = spawnSync('plutil', ['-extract', 'CFBundleExecutable', 'raw', '-o', '-', plist], { encoding: 'utf8' });
    return read.status === 0 ? read.stdout.trim() || null : null;
  };
  let version = null;
  try { version = readlinkSync(resolve(bundle, 'Versions/Current')); } catch {}
  for (const [plist, dir] of [['Contents/Info.plist', 'Contents/MacOS'], ...(version ? [[`Versions/${version}/Resources/Info.plist`, `Versions/${version}`]] : []), ['Info.plist', '.']]) {
    const name = executable(resolve(bundle, plist));
    if (name) return resolve(bundle, dir, name);
  }
  return null;
}

/** Everything in a bundle that carries its own signature, innermost first:
 *  every Mach-O file (an executable, a library, an addon — wherever it sits)
 *  and every nested bundle after what it holds. A bundle is sealed over its
 *  contents, so code re-signed after its container invalidates the container.
 *  Symlinks are skipped: a framework's `Versions/Current` names what is signed
 *  through its real path. Baked code keeps its verified digest and signature;
 *  static assets cannot contain code a release would have to re-sign. */
export function signingOrder(bundle, compat = null, identity = null) {
  const preserved = new Map();
  const embedded = compat?.embedded;
  for (const card of embedded?.assets ?? []) {
    const path = resolve(bundle, 'Contents/Resources', card.name);
    if (!machO(path)) continue;
    if (!card.name.startsWith('rust/')) throw new Error(`Mach-O asset ${card.name}: move code to host.macos.resources before releasing`);
    preserved.set(path, card.sha256);
  }
  if (embedded?.gpu) preserved.set(resolve(bundle, 'Contents/MacOS/libexact_gpu.dylib'), embedded.gpu.sha256);
  for (const [name, card] of Object.entries(embedded?.gpuModules ?? {}))
    preserved.set(resolve(bundle, 'Contents/MacOS', `libexact_gpu_${name.replaceAll('-', '_')}.dylib`), card.sha256);
  for (const [path, digest] of preserved) {
    if (!existsSync(path) || createHash('sha256').update(readFileSync(path)).digest('hex') !== digest)
      throw new Error(`baked code ${path}: digest differs; rebuild before signing`);
    run('codesign', ['--verify', '--strict', ...(identity && identity !== '-' ? ['-R', `=certificate leaf = H"${identity}"`] : []), path], { stdio: 'ignore' });
  }
  const order = [];
  const walk = (dir, main) => {
    for (const name of readdirSync(dir, { withFileTypes: true })) {
      const path = resolve(dir, name.name);
      if (name.isDirectory() && NESTED_BUNDLE.test(name.name) && bundleExecutable(path)) {
        walk(path, bundleExecutable(path));
        order.push(path);
      } else if (name.isDirectory()) walk(path, main);
      else if (name.isFile() && path !== main && !preserved.has(path) && machO(path)) order.push(path);
    }
  };
  walk(resolve(bundle, 'Contents'), bundleExecutable(bundle));
  return [...order, bundle];
}


/** Native files are binary inputs, never plan assets or TypeScript sources. */
export function macResourceMappings(manifest) {
  const mappings = manifest.host?.macos?.resources ?? [];
  const safe = path => typeof path === 'string' && path.length && !isAbsolute(path)
    && !path.includes('\\') && !path.includes('\0') && path.split('/').every(p => p && p !== '.' && p !== '..');
  const reserved = new Set(['assets', 'deck', 'gpu', 'fonts', 'strings', 'web', 'apple', 'linux', 'src', 'data', 'modules', 'node_modules', 'target', 'dist']);
  const overlap = (a, b) => a === b || a.startsWith(b + '/') || b.startsWith(a + '/');
  for (const [i, entry] of mappings.entries()) {
    if (!safe(entry.from) || reserved.has(entry.from.split('/')[0]) || entry.from.startsWith('.'))
      throw new Error('host.macos.resources.from must name a separate app-relative resource directory, outside source, asset and output roots');
    if (!safe(entry.to) || !/^(Resources|Helpers|Frameworks)\/.+/.test(entry.to)
      || ['Resources/assets', 'Resources/deck', 'Resources/shaders', 'Resources/receipt.json', 'Resources/AppIcon.icns'].some(p => overlap(entry.to, p))
      || entry.to.split('/').some(p => p.endsWith('.lproj')))
      throw new Error('host.macos.resources.to must name a private path under Resources/, Helpers/ or Frameworks/');
    if (mappings.slice(0, i).some(other => overlap(other.from, entry.from) || overlap(other.to, entry.to)))
      throw new Error('host.macos.resources entries must not overlap');
    if (Object.keys(manifest.typescript?.sources ?? {}).some(name => overlap(name, entry.from)))
      throw new Error('host.macos.resources cannot overlap typescript.sources');
  }
  return mappings;
}

/** Validate links before copying. Relative links stay inside their declared
 * tree, so a copied helper cannot accidentally depend on the producer machine. */
export function macResourceInventory(app, contents = null) {
  const mappings = macResourceMappings(app.manifest);
  if (!mappings.length) return [];
  const result = [], block = Buffer.alloc(1024 * 1024);
  const inside = (root, path) => path === root || path.startsWith(root + '/');
  const appRoot = realpathSync(contents ?? app.dir);
  for (const { from, to } of mappings) {
    const source = resolve(contents ?? app.dir, contents ? to : from), root = realpathSync(source);
    if (!inside(appRoot, root) || lstatSync(source).isSymbolicLink() || !lstatSync(source).isDirectory())
      throw new Error(`native resource root must be a directory inside the app: ${from}`);
    const walk = path => {
      const stat = lstatSync(path), name = relative(source, path), mode = stat.mode & 0o777;
      const item = { from, to, name, mode };
      if (stat.isSymbolicLink()) {
        item.link = readlinkSync(path);
        if (isAbsolute(item.link) || !inside(source, resolve(dirname(path), item.link)) || !inside(root, realpathSync(path)))
          throw new Error(`native resource link escapes its tree: ${path}`);
      } else if (stat.isDirectory()) item.directory = true;
      else if (stat.isFile()) {
        const hash = createHash('sha256'), fd = openSync(path, 'r');
        try { let n; while ((n = readSync(fd, block, 0, block.length, null))) hash.update(block.subarray(0, n)); }
        finally { closeSync(fd); }
        item.sha256 = hash.digest('hex');
      } else throw new Error(`native resource is not a file, directory or link: ${path}`);
      result.push(item);
      if (item.directory) for (const child of readdirSync(path).sort()) walk(resolve(path, child));
    };
    walk(source);
  }
  return result;
}

export function copyMacResources(app, contents, expected = null) {
  const inventory = macResourceInventory(app);
  if (expected && JSON.stringify(inventory) !== JSON.stringify(expected))
    throw new Error('native resources changed after the binary receipt; rebuild');
  for (const { from, to } of macResourceMappings(app.manifest)) {
    const destination = resolve(contents, to);
    // Never merge with generated bundle contents or traverse an existing link.
    for (let at = destination; at !== resolve(contents); at = dirname(at)) {
      const info = lstatSync(at, { throwIfNoEntry: false });
      if (info && (at === destination || info.isSymbolicLink()))
        throw new Error(`native resource destination already exists: ${at}`);
    }
    mkdirSync(dirname(destination), { recursive: true });
    cpSync(resolve(app.dir, from), destination, { recursive: true, verbatimSymlinks: true });
  }
  // cp preserves regular-file modes; explicitly retain directory modes too.
  for (const item of inventory.toReversed()) if (!Object.hasOwn(item, 'link'))
    chmodSync(resolve(contents, item.to, item.name), item.mode);
  if (JSON.stringify(macResourceInventory(app, contents)) !== JSON.stringify(inventory))
    throw new Error('native resources changed during bundle copy; rebuild');
  return inventory;
}

/** A plist from a plain object: strings, booleans, numbers, arrays, and objects, the four spellings Apple's DTD has. */
const plist = (value, indent = '  ') => {
  if (typeof value === 'string') return `<string>${value.replace(/&/g, '&amp;').replace(/</g, '&lt;')}</string>`;
  if (typeof value === 'boolean') return value ? '<true/>' : '<false/>';
  if (typeof value === 'number') return Number.isInteger(value) ? `<integer>${value}</integer>` : `<real>${value}</real>`;
  if (Array.isArray(value)) return `<array>${value.map((v) => plist(v, indent)).join('')}</array>`;
  return `<dict>\n${Object.entries(value).map(([k, v]) => `${indent}<key>${k}</key>${plist(v, indent + '  ')}`).join('\n')}\n${indent.slice(2)}</dict>`;
};
export const plistFile = (dict) => `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">${plist(dict)}</plist>\n`;

/** A loose `Frameworks/lib….dylib` as `Frameworks/<name>.framework/<name>`,
 * which is the only form App Store Connect accepts for an embedded library
 * (ITMS-90171, ITMS-90432). The presenter loads either (`embeddedModule` in ExactKit). */
export function wrapFramework(frameworks, loose, name, app, platform = 'iPhoneOS') {
  const from = resolve(frameworks, loose);
  if (!existsSync(from)) return;
  const dir = resolve(frameworks, `${name}.framework`);
  mkdirSync(dir, { recursive: true });
  renameSync(from, resolve(dir, name));
  run('install_name_tool', ['-id', `@rpath/${name}.framework/${name}`, resolve(dir, name)], { stdio: 'ignore' });
  writeFileSync(resolve(dir, 'Info.plist'), plistFile({
    CFBundleExecutable: name, CFBundleIdentifier: `${app.id}.${name.toLowerCase().replaceAll('_', '-')}`, CFBundleName: name,
    CFBundlePackageType: 'FMWK', CFBundleVersion: process.env.EXACT_BUILD_NUMBER ?? '1', CFBundleShortVersionString: process.env.EXACT_VERSION ?? '0.1.0',
    CFBundleSupportedPlatforms: [platform], MinimumOSVersion: app.manifest.host?.ios?.minimumOS ?? '17.0',
  }));
}

export function signedGpuProduct(product, { identity = '-', framework = null, app, platform = 'iPhoneOS', hardened = false } = {}) {
  // Cargo puts its own unsigned file back on every build, and a signature
  // carries its signing time: signing in place made the app's bake (which
  // names this product's digest) run on every build. Sign a copy beside it,
  // again only when Cargo's bytes or the identity change.
  const directory = resolve(dirname(product), 'signed', ...(framework ? ['frameworks', basename(product)] : []));
  const container = resolve(directory, framework ? `${framework}.framework` : basename(product));
  const signed = framework ? resolve(container, framework) : container, record = `${container}.source`;
  const source = JSON.stringify([createHash('sha256').update(readFileSync(product)).digest('hex'), identity ?? '-',
    hardened, framework, framework ? [app.id, app.manifest.host?.ios?.minimumOS ?? '17.0', platform, process.env.EXACT_BUILD_NUMBER ?? '1', process.env.EXACT_VERSION ?? '0.1.0'] : null]);
  // An ad-hoc signature (`-`: every simulator build) names no certificate
  // to require; asked for one, the verification failed, and the copy was
  // signed again by every build, which reran the app's bake and compile.
  const current = existsSync(signed) && existsSync(record) && readFileSync(record, 'utf8') === source
    && spawnSync('codesign', ['--verify', '--strict', ...(identity && identity !== '-' ? ['-R', `=certificate leaf = H"${identity}"`] : []), container]).status === 0;
  if (!current) {
    mkdirSync(directory, { recursive: true });
    if (framework) rmSync(container, { recursive: true, force: true });
    copyFileSync(product, resolve(directory, basename(product)));
    if (framework) wrapFramework(directory, basename(product), framework, app, platform);
    run('codesign', ['--force', '--sign', identity ?? '-', ...(hardened ? ['--options', 'runtime'] : []),
      hardened && identity && identity !== '-' ? '--timestamp' : '--timestamp=none', container], {stdio:'ignore'});
    writeFileSync(record, source);
  }
  return signed;
}

export function assertArchiveFrameworks(directory) {
  const loose = readdirSync(directory).filter(name => name.endsWith('.dylib'));
  if (loose.length) throw new Error(`archive has loose libraries in Frameworks: ${loose.join(', ')}`);
}
