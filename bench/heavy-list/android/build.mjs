#!/usr/bin/env bun
// Release builds of the original Heavy List on C9, main's painter, Views and Compose.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bakeOutput, buildBake, developmentBuildEnv, resolveApp, verifyBakeFiles } from '../../../scripts/app.mjs';
import { apkFootprint, versions } from '../../../host/android/build.mjs';
import { listAssets } from '../../../host/web/serve.mjs';

const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../../..');
const appDir = resolve(here, '../exact-heavylist');
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const write = (path, text) => { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, text); };
const run = (command, args, env, cwd = root, capture = false) => {
  const result = spawnSync(command, args, { cwd, env, ...(capture ? { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 } : { stdio: 'inherit' }) });
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.error?.message})${result.stderr ? ': ' + result.stderr : ''}`);
  return capture ? result.stdout.trim() : undefined;
};
function options(args) {
  const out = { only: 'all', abi: 'arm64-v8a', output: resolve(root, 'target/heavy-list-android'), mainRoot: process.env.EXACT_MAIN_ROOT };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--reuse-native') { out.reuseNative = true; continue; }
    const key = { '--only': 'only', '--abi': 'abi', '--output': 'output', '--main-root': 'mainRoot', '--relink-native': 'relinkNative' }[args[i]];
    if (!key || !args[i + 1]) throw new Error('Usage: bun bench/heavy-list/android/build.mjs [--only all|c9|main|views|compose] [--abi arm64-v8a|x86_64] [--main-root checkout] [--output directory] [--reuse-native] [--relink-native prior-stage.json]');
    out[key] = args[++i];
  }
  if (out.reuseNative && out.relinkNative) throw new Error('Choose either native reuse or archive relinking');
  if (!['all', 'c9', 'main', 'views', 'compose'].includes(out.only)) throw new Error('Unknown build variant');
  if ((out.only === 'all' || out.only === 'main') && !out.mainRoot) throw new Error('--main-root is required for the public main painter');
  return out;
}
function tools(env, abi) {
  const target = { 'arm64-v8a': ['aarch64-linux-android', 'aarch64-linux-android'], x86_64: ['x86_64-linux-android', 'x86_64-linux-android'] }[abi];
  if (!target) throw new Error('ABI must be arm64-v8a or x86_64');
  const sdk = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT;
  if (!sdk) throw new Error('ANDROID_HOME must name the provisioned SDK');
  const ndk = env.ANDROID_NDK_HOME ?? resolve(sdk, 'ndk', versions.ndk);
  const prebuilt = { darwin: 'darwin-x86_64', linux: 'linux-x86_64', win32: 'windows-x86_64' }[process.platform];
  if (!prebuilt) throw new Error('Unsupported NDK build host');
  const clang = resolve(ndk, 'toolchains/llvm/prebuilt', prebuilt, 'bin', `${target[1]}29-clang++${process.platform === 'win32' ? '.cmd' : ''}`);
  if (!existsSync(clang)) throw new Error(`NDK ${versions.ndk} compiler missing: ${clang}`);
  const revision = readFileSync(resolve(ndk, 'source.properties'), 'utf8');
  if (!revision.includes(`Pkg.Revision = ${versions.ndk}`)) throw new Error(`NDK must be ${versions.ndk}`);
  const gradle = env.EXACT_GRADLE ?? 'gradle';
  if (!run(gradle, ['--version'], env, root, true).includes(`Gradle ${versions.gradle}\n`)) throw new Error(`Gradle must be ${versions.gradle}`);
  const key = target[0].replaceAll('-', '_');
  return { clang, gradle, target: target[0], env: { ...env, ANDROID_HOME: sdk, ANDROID_SDK_ROOT: sdk,
    [`CARGO_TARGET_${key.toUpperCase()}_LINKER`]: clang,
    [`CC_${key}`]: clang.replace(/-clang\+\+(\.cmd)?$/, '-clang$1'),
    [`CXX_${key}`]: clang, [`AR_${key}`]: resolve(dirname(clang), 'llvm-ar') } };
}
function dataset() {
  const messages = resolve(appDir, 'data/messages.json'), assets = resolve(appDir, 'assets');
  if (!existsSync(messages) || !existsSync(assets)) throw new Error('Run bench/heavy-list/prepare.sh first (the published dataset is required)');
  const images = readdirSync(assets).filter(name => name.endsWith('.jpg')).sort();
  if (images.length !== 104) throw new Error(`Expected 104 original JPEGs, found ${images.length}`);
  const parsed = JSON.parse(readFileSync(messages));
  if (parsed.messages?.length !== 10000) throw new Error('Expected 10000 original messages');
  const files = Object.fromEntries(images.map(name => [name, hash(resolve(assets, name))]));
  const published = readFileSync(resolve(here, '../data.sha256'), 'utf8').split(/\r?\n/).filter(line => line && !line.startsWith('#'));
  if (published.length !== images.length + 1) throw new Error('Published dataset inventory changed');
  for (const line of published) {
    const match = line.match(/^([a-f0-9]{64})  (messages\.json|images\/[^/]+\.jpg)$/);
    if (!match) throw new Error(`Invalid published checksum row: ${line}`);
    const actual = match[2] === 'messages.json' ? hash(messages) : files[match[2].slice('images/'.length)];
    if (actual !== match[1]) throw new Error(`Dataset differs from published SHA-256: ${match[2]}`);
  }
  const descriptor = JSON.stringify({ messages_sha256: hash(messages), images: files });
  return { messages, assets, images, files, descriptor, sha256: createHash('sha256').update(descriptor).digest('hex') };
}
function project(folder, variant, config, tool, feed) {
  const native = variant === 'c9' || variant === 'main', compose = variant === 'compose';
  const activity = `dev.exact.heavybench.${variant === 'c9' ? 'C9' : variant === 'main' ? 'Main' : compose ? 'Compose' : 'Views'}Activity`;
  const appId = `dev.exact.heavybench.${variant}`;
  write(resolve(folder, 'settings.gradle.kts'), `pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositories { google(); mavenCentral() } }\nrootProject.name = "HeavyList-${variant}"\ninclude(":app")\n`);
  write(resolve(folder, 'gradle.properties'), 'android.useAndroidX=true\norg.gradle.jvmargs=-Xmx2g -Dfile.encoding=UTF-8\norg.gradle.parallel=false\norg.gradle.workers.max=2\n');
  const deps = variant === 'c9' ? `implementation("androidx.compose.ui:ui-text:${versions.compose}")` : compose ?
    `implementation("androidx.activity:activity:${versions.activity}"); implementation("androidx.compose.foundation:foundation:${versions.compose}"); implementation("androidx.compose.ui:ui:${versions.compose}")` : variant === 'views' ? 'implementation("androidx.recyclerview:recyclerview:1.4.0")' : '';
  write(resolve(folder, 'app/build.gradle.kts'), `plugins { id("com.android.application") version "${versions.agp}"${compose ? `; id("org.jetbrains.kotlin.plugin.compose") version "${versions.kotlin}"` : ''} }\nandroid {\n    namespace = "dev.exact.heavybench"\n    compileSdk = ${versions.compileSdk}\n    buildToolsVersion = "${versions.buildTools}"\n    ndkVersion = "${versions.ndk}"\n    defaultConfig { applicationId = "${appId}"; minSdk = ${variant === 'main' ? 30 : 29}; targetSdk = 36; versionCode = 1; versionName = "1"; ndk { abiFilters += "${config.abi}" } }\n    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }\n    ${compose ? 'buildFeatures { compose = true }' : ''}\n    signingConfigs.getByName("debug").storeFile = file("debug.keystore")\n    buildTypes.getByName("release") { isDebuggable = false; isMinifyEnabled = true; isShrinkResources = true; proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro"); signingConfig = signingConfigs.getByName("debug") }\n}\ndependencies { ${deps} }\n`);
  const keep = variant === 'c9' ? '-keep,allowoptimization class com.exact.android.Native { *; }\n-keep,allowoptimization class com.exact.android.TextEngine { java.nio.ByteBuffer measure(java.nio.ByteBuffer,int); void fonts(java.nio.ByteBuffer); void wake(); }\n-keep,allowoptimization class dev.exact.heavybench.C9Config { *; }\n' : variant === 'main' ? '-keep,allowoptimization class dev.exact.heavybench.MainNative { *; }\n' : '';
  write(resolve(folder, 'app/proguard-rules.pro'), keep);
  write(resolve(folder, 'app/src/main/AndroidManifest.xml'), `<manifest xmlns:android="http://schemas.android.com/apk/res/android"><application android:label="Heavy List ${variant}" android:theme="@android:style/Theme.Material.Light.NoActionBar" android:allowBackup="false" android:supportsRtl="true"><profileable android:shell="true"/><activity android:name="${activity}" android:exported="true" android:windowSoftInputMode="adjustResize"><intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter></activity></application></manifest>\n`);
  const sources = resolve(folder, 'app/src/main/kotlin');
  rmSync(sources, { recursive: true, force: true }); mkdirSync(sources, { recursive: true });
  if (variant === 'c9') {
    cpSync(resolve(root, 'host/android/kotlin'), sources, { recursive: true });
    cpSync(resolve(root, 'host/android/benchmark'), sources, { recursive: true });
    cpSync(resolve(root, 'host/android/tests'), sources, { recursive: true });
  }
  if (variant === 'main') {
    const environment = resolve(sources, 'com/exact/android/NativeEnvironment.kt');
    mkdirSync(dirname(environment), { recursive: true });
    copyFileSync(resolve(root, 'host/android/kotlin/com/exact/android/NativeEnvironment.kt'), environment);
  }
  for (const leaf of ['common', variant]) {
    const source = resolve(here, leaf);
    if (!existsSync(source)) throw new Error(`Benchmark sources missing: ${source}`);
    cpSync(source, sources, { recursive: true, filter: path => !path.endsWith('.cpp') && !path.endsWith('.md') });
  }
  const assets = resolve(folder, 'app/src/main/assets'); rmSync(assets, { recursive: true, force: true }); mkdirSync(assets, { recursive: true });
  const images = resolve(assets, native ? 'assets' : 'images'); mkdirSync(images, { recursive: true });
  for (const name of feed.images) copyFileSync(resolve(feed.assets, name), resolve(images, name));
  if (!native) copyFileSync(feed.messages, resolve(assets, 'messages.json'));
  write(resolve(assets, 'benchmark-assets.sha256'), feed.descriptor);
  const keystore = resolve(folder, 'app/debug.keystore');
  if (!existsSync(keystore)) run(tool.env.JAVA_HOME ? resolve(tool.env.JAVA_HOME, 'bin/keytool') : 'keytool',
    ['-genkeypair', '-keystore', keystore, '-storepass', 'android', '-alias', 'androiddebugkey', '-keypass', 'android', '-dname', 'CN=Android Debug,O=Android,C=US', '-validity', '10000', '-keyalg', 'RSA', '-keysize', '2048'], tool.env);
  return { appId, activity, assets };
}
function copyMainInputs(mainRoot, feed) {
  if (!mainRoot || !existsSync(resolve(mainRoot, 'host/linux/src/android.rs'))) throw new Error('--main-root must name the current main checkout with its public Android Handle');
  const destination = resolve(mainRoot, 'bench/heavy-list/exact-heavylist');
  for (const name of ['app.contract', 'data/src/lib.rs', 'data/Cargo.toml']) if (hash(resolve(appDir, name)) !== hash(resolve(destination, name))) throw new Error(`Main benchmark input differs: ${name}`);
  mkdirSync(resolve(destination, 'data'), { recursive: true }); copyFileSync(feed.messages, resolve(destination, 'data/messages.json'));
  mkdirSync(resolve(destination, 'assets'), { recursive: true });
  for (const name of feed.images) copyFileSync(resolve(feed.assets, name), resolve(destination, 'assets', name));
  if (!existsSync(resolve(destination, 'android-main/Cargo.toml'))) throw new Error('Install the authored android-main adapter in the main checkout first (README)');
  return destination;
}
function nativeSourceManifest(nativeRoot, env) {
  const names = run('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], env, nativeRoot, true).split('\0').filter(Boolean);
  // Covers authored/untracked native modules, Cargo inputs, shader/include files,
  // and schema authority; generated target files and Kotlin-only APK inputs are separate.
  const selected = [...new Set(names)].filter(name => /\.(rs|toml|json|wgsl|c|cpp|h|hpp|s|asm)$/.test(name)
    || /(^|\/)Cargo\.lock$/.test(name)).sort();
  const files = Object.fromEntries(selected.map(name => [name, hash(resolve(nativeRoot, name))]));
  return { files, sha256: createHash('sha256').update(JSON.stringify(files)).digest('hex') };
}
function nativeIdentity(variant, config, tool, feed) {
  const nativeRoot = variant === 'main' ? resolve(config.mainRoot) : root;
  const app = resolve(nativeRoot, 'bench/heavy-list/exact-heavylist');
  const adapter = variant === 'main' ? 'android-main' : 'android';
  const diff = run('git', ['diff', '--binary', 'HEAD'], tool.env, nativeRoot, true);
  return { variant, abi: config.abi, target: tool.target, dataset_sha256: feed.sha256,
    revision: run('git', ['rev-parse', 'HEAD'], tool.env, nativeRoot, true),
    native_source_manifest: nativeSourceManifest(nativeRoot, tool.env),
    native_environment: Object.fromEntries(Object.entries(tool.env).filter(([name]) => name === 'BENCH_LIVE' || name.startsWith('CARGO_PROFILE_RELEASE_') || /^(EXACT_UPDATE_|EXACT_(ASSET_ROOTS|BAKE_TRUST|TYPESCRIPT_PLACEMENT|RUST_PLACEMENT|RUST_BUNDLE|GPU_PRODUCT|GPU_MODULES)$)/.test(name)).sort(([a], [b]) => a.localeCompare(b))),
    rustc_version: run('rustc', ['--version'], tool.env, nativeRoot, true),
    tracked_diff_sha256: createHash('sha256').update(diff).digest('hex'),
    files: Object.fromEntries(['Cargo.toml', 'Cargo.lock', 'app.json', 'app.contract', 'android-bake.rs', 'data/Cargo.toml', 'data/src/lib.rs',
      `${adapter}/Cargo.toml`, `${adapter}/build.rs`, `${adapter}/src/lib.rs`].map(name => [name, hash(resolve(app, name))])),
    bridge_sha256: hash(variant === 'c9' ? resolve(root, 'host/android/jni/exact_jni.cpp') : resolve(here, 'main/main_jni.cpp')),
    extra_bridge_sha256: variant === 'c9' ? hash(resolve(here, 'c9/c9_config.cpp')) : null,
    rustflags: tool.env.RUSTFLAGS ?? null, encoded_rustflags: tool.env.CARGO_ENCODED_RUSTFLAGS ?? null,
    rust_profile: 'release', ndk: versions.ndk, compiler_version: run(tool.clang, ['--version'], tool.env, root, true) };
}
function nativeLibrary(folder, variant, config, tool, feed) {
  const library = resolve(folder, 'app/src/main/jniLibs', config.abi, variant === 'c9' ? 'libexact_app.so' : 'libexact_heavylist_main.so');
  mkdirSync(dirname(library), { recursive: true });
  const identity = nativeIdentity(variant, config, tool, feed), stagePath = resolve(folder, 'native-stage-v2.json');
  if (config.reuseNative) {
    if (!existsSync(stagePath)) throw new Error('Native reuse requires an existing hash-bound native-stage-v2.json');
    const stage = JSON.parse(readFileSync(stagePath));
    if (JSON.stringify(stage.identity) !== JSON.stringify(identity) || stage.library !== library
      || stage.native_sha256 !== hash(library) || stage.archive_sha256 !== hash(stage.archive)) throw new Error('Native reuse inputs/products changed; rebuild the native stage');
    console.log(`Reusing verified native stage: ${library}`);
    return { library, archive: stage.archive };
  }
  let archive, bake, parentStage;
  if (config.relinkNative) {
    const priorPath = resolve(config.relinkNative);
    const prior = JSON.parse(readFileSync(priorPath));
    if (JSON.stringify(prior.identity) !== JSON.stringify(identity)
      || prior.archive_sha256 !== hash(prior.archive) || prior.native_sha256 !== hash(prior.library))
      throw new Error('Archive relink inputs/products changed; rebuild the native stage');
    archive = prior.archive;
    parentStage = { path: priorPath, sha256: hash(priorPath), native_sha256: prior.native_sha256 };
    console.log(`Relinking verified Rust archive: ${archive}`);
  } else if (variant === 'c9') {
    // EXACT_APP_DIR selects the original nested benchmark workspace and full data provider.
    process.env.EXACT_APP_DIR = appDir;
    const app = resolveApp('exact-heavylist');
    bake = buildBake(app, 'android', tool.target, { env: { ...tool.env, EXACT_APP_DIR: appDir }, profile: 'release' });
    archive = bake.products.find(product => product.path.endsWith('.a'))?.path;
    if (!archive) throw new Error('C9 bake produced no native static library');
    verifyBakeFiles(bake.compat, readFileSync(resolve(bakeOutput(app, tool.env), `android-${tool.target}.plan`)), listAssets(resolve(folder, 'app/src/main/assets')));
  } else {
    const mainRoot = resolve(config.mainRoot ?? '');
    const destination = copyMainInputs(mainRoot, feed);
    const targetDir = process.env.EXACT_MAIN_CARGO_TARGET_DIR ?? resolve(mainRoot, 'target');
    if (resolve(targetDir) === resolve(process.env.CARGO_TARGET_DIR ?? resolve(root, 'target'))) throw new Error('Main must use its own Cargo target, never C9 target');
    const mainBake = resolve(targetDir, 'heavy-list-main-bake');
    const env = { ...tool.env, CARGO_TARGET_DIR: targetDir, EXACT_BAKE_OUTPUT: mainBake };
    mkdirSync(mainBake, { recursive: true });
    run('cargo', ['build', '--manifest-path', resolve(destination, 'Cargo.toml'), '-p', 'exact-heavylist-android-main', '--release', '--target', tool.target, '--offline'], env, mainRoot);
    archive = resolve(targetDir, tool.target, 'release/libexact_heavylist_main.a');
    const compat = JSON.parse(readFileSync(resolve(mainBake, `android-${tool.target}.json`)));
    verifyBakeFiles(compat, readFileSync(resolve(mainBake, `android-${tool.target}.plan`)), listAssets(resolve(folder, 'app/src/main/assets')));
  }
  const bridge = variant === 'c9' ? resolve(root, 'host/android/jni/exact_jni.cpp') : resolve(here, 'main/main_jni.cpp');
  const extra = variant === 'c9' ? [resolve(here, 'c9/c9_config.cpp')] : [];
  const linkCompiler = variant === 'main' ? tool.clang.replace('29-clang', '30-clang') : tool.clang;
  const linkLibraries = variant === 'main' ? ['-ljnigraphics'] : [];
  run(linkCompiler, ['-std=c++17', '-O3', '-DNDEBUG', '-fPIC', '-shared', '-fvisibility=hidden', '-Wall', '-Wextra', '-Werror', bridge, ...extra, archive,
    '-static-libstdc++', '-Wl,--gc-sections', '-Wl,--exclude-libs,ALL', '-Wl,--pack-dyn-relocs=android+relr', '-Wl,--use-android-relr-tags', '-Wl,-z,max-page-size=16384', '-Wl,--no-undefined', '-llog', '-landroid', ...linkLibraries, '-ldl', '-lm', '-o', library], tool.env);
  write(stagePath, JSON.stringify({ identity, library, archive, native_sha256: hash(library), archive_sha256: hash(archive),
    link: { api: variant === 'main' ? 30 : 29, compiler: linkCompiler, libraries: linkLibraries, no_undefined: true },
    ...(parentStage ? { archive_relink_parent: parentStage, rust_archive_recompiled: false } : {}) }, null, 2) + '\n');
  return { library, archive, bake };
}
function packagedNative(apk, variant, abi, env) {
  const path = `lib/${abi}/${variant === 'c9' ? 'libexact_app.so' : 'libexact_heavylist_main.so'}`;
  const result = spawnSync('unzip', ['-p', apk, path], { env, maxBuffer: 128 * 1024 * 1024 });
  if (result.status !== 0 || !result.stdout?.length) throw new Error(`Packaged native library unreadable: ${path}`);
  return { path, bytes: result.stdout.length, sha256: createHash('sha256').update(result.stdout).digest('hex') };
}
function sourceCards(directory, suffixes) {
  const files = [];
  function walk(path) {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const name = resolve(path, entry.name);
      if (entry.isDirectory()) walk(name);
      else if (suffixes.some(suffix => name.endsWith(suffix))) files.push(name);
    }
  }
  walk(directory);
  return Object.fromEntries(files.sort().map(path => [path.slice(directory.length + 1), hash(path)]));
}
export { nativeIdentity };
export function build(config) {
  const tool = tools({ ...developmentBuildEnv(), ...process.env }, config.abi), feed = dataset();
  const variants = config.only === 'all' ? ['c9', 'main', 'views', 'compose'] : [config.only];
  const products = {};
  for (const variant of variants) {
    const folder = resolve(config.output, variant); mkdirSync(folder, { recursive: true });
    const identity = project(folder, variant, config, tool, feed);
    const compiled = variant === 'c9' || variant === 'main' ? nativeLibrary(folder, variant, config, tool, feed) : null;
    run(tool.gradle, ['--no-daemon', '--console=plain', '-p', folder, ':app:assembleRelease', '--max-workers=2'], tool.env);
    const apk = resolve(folder, 'heavy-list-' + variant + '.apk'); copyFileSync(resolve(folder, 'app/build/outputs/apk/release/app-release.apk'), apk);
    products[variant] = { ...identity, apk, apk_sha256: hash(apk), footprint: apkFootprint(apk),
      ...(compiled ? { native_library: compiled.library, pre_agp_native_sha256: hash(compiled.library), native_archive: compiled.archive, native_archive_sha256: hash(compiled.archive), packaged_native: packagedNative(apk, variant, config.abi, tool.env) } : {}),
      release: { debuggable: false, minified: true, resource_shrunk: true, rust_profile: 'release', abi: config.abi },
      dataset_sha256: feed.sha256, published_checksums_sha256: hash(resolve(here, '../data.sha256')), messages_sha256: hash(feed.messages), images: feed.files,
      build_script_sha256: hash(fileURLToPath(import.meta.url)), packaged_kotlin_sources: sourceCards(resolve(folder, 'app/src/main/kotlin'), ['.kt']),
      ...(compiled ? { adapter_sources: sourceCards(resolve(variant === 'main' ? config.mainRoot : root, 'bench/heavy-list/exact-heavylist', variant === 'main' ? 'android-main' : 'android'), ['.rs', '.toml']), native_bridge_sha256: hash(resolve(variant === 'main' ? here : root, variant === 'main' ? 'main/main_jni.cpp' : 'host/android/jni/exact_jni.cpp')) } : {}),
      source_head: run('git', ['rev-parse', 'HEAD'], tool.env, variant === 'main' ? resolve(config.mainRoot) : root, true) };
    write(resolve(folder, 'build-receipt.json'), JSON.stringify(products[variant], null, 2) + '\n');
  }
  write(resolve(config.output, `products-${config.only}.json`), JSON.stringify(products, null, 2) + '\n');
  return products;
}
if (import.meta.main) build(options(process.argv.slice(2)));
