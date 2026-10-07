// The app's icon and asset catalog, from its manifest, for an Apple bundle
// (host/apple/build.mjs assembles it).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';

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

/** All iOS asset sets share one actool pass: each pass replaces Assets.car.
 * `kept` names a directory of the app's and the toolchain's stamp: the icons
 * and the catalog are then made once for each icon file, colour, platform and
 * toolchain and copied into `dir`, since making them is over a second of
 * every simulator build. */
export function iosAssets(app, dir, device, { catalog = false, kept = null } = {}) {
  if (kept) {
    const icons = (app.manifest.icons ?? []).map((icon) => [icon, existsSync(resolve(app.dir, icon.src)) ? createHash('sha256').update(readFileSync(resolve(app.dir, icon.src))).digest('hex') : null]);
    const key = createHash('sha256').update(JSON.stringify([icons, app.manifest.background_color ?? null, app.manifest.background_color_dark ?? null,
      app.manifest.host?.ios?.minimumOS ?? null, device, catalog, kept.stamp])).digest('hex').slice(0, 16);
    const made = resolve(kept.dir, `assets-${key}`);
    if (!existsSync(resolve(made, 'keys.json'))) {
      const making = `${made}.${process.pid}.tmp`;
      rmSync(making, { recursive: true, force: true });
      mkdirSync(making, { recursive: true });
      writeFileSync(resolve(making, 'keys.json'), JSON.stringify(iosAssets(app, making, device, { catalog })));
      rmSync(made, { recursive: true, force: true });
      renameSync(making, made);
      for (const old of readdirSync(kept.dir)) if (/^assets-[0-9a-f]{16}$/.test(old) && resolve(kept.dir, old) !== made) rmSync(resolve(kept.dir, old), { recursive: true, force: true });
    }
    for (const file of readdirSync(made)) if (file !== 'keys.json') cpSync(resolve(made, file), resolve(dir, file), { recursive: true });
    return JSON.parse(readFileSync(resolve(made, 'keys.json'), 'utf8'));
  }
  const work = mkdtempSync(resolve(tmpdir(), 'exact-ios-assets-'));
  try {
    const assets = resolve(work, 'Assets.xcassets');
    const keys = { ...appIcon(app, dir, 'ios', { catalog: catalog ? assets : false }), ...launchScreen(app, assets) };
    const hasIcon = existsSync(resolve(assets, 'AppIcon.appiconset'));
    if (catalog && !hasIcon) throw new Error(`host/apple: ${app.name}'s distribution bundle requires an AppIcon; declare a square icon of at least 512 px`);
    if (hasIcon || keys.UILaunchScreen) {
      writeFileSync(resolve(assets, 'Contents.json'), JSON.stringify({ info: { author: 'exact', version: 1 } }));
      const partial = resolve(work, 'partial.plist');
      run('xcrun', ['actool', assets, '--compile', dir, '--platform', device ? 'iphoneos' : 'iphonesimulator',
        '--minimum-deployment-target', app.manifest.host?.ios?.minimumOS ?? '17.0',
        ...(hasIcon ? ['--app-icon', 'AppIcon', '--target-device', 'iphone', '--target-device', 'ipad'] : []),
        '--output-partial-info-plist', partial, '--output-format', 'human-readable-text'], { stdio: 'ignore' });
      Object.assign(keys, JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', partial], { encoding: 'utf8', stdio: 'pipe' }).stdout));
    }
    if (catalog) {
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
 * `UILaunchScreen` names colours only from an asset catalog, so this
 * writes its colour set for the shared compile. Returns the plist keys to merge. */
function launchScreen(app, catalog) {
  const light = app.manifest.background_color, dark = app.manifest.background_color_dark;
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
