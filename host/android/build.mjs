#!/usr/bin/env bun
// Build optimized Android carriers and independent Compose/platform Views references.
// bun host/android/build.mjs android-core [--run] [--bench] [--compare] [--serial emulator-5554]
// --compare [--heavy-list] [--scroll-samples 900] [--frame-rounds 3] [--scroll-speeds 240,1200,4800]
// JDK17, Gradle9.3.1, SDK37.0/build-tools36 and NDK28.2 are external build tools.
import { spawnSync } from 'node:child_process';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bakeOutput, buildBake, claimBuildOutput, developmentBuildEnv, resolveApp, rustPolicy, verifyBakeFiles } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent, listAssets } from '../web/serve.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const versions = Object.freeze({ agp: '9.1.1', gradle: '9.3.1', kotlin: '2.2.10', compose: '1.12.1', activity: '1.13.0', ndk: '28.2.13676358', compileSdk: 37, buildTools: '36.0.0' });
const targets = {
  'arm64-v8a': { rust: 'aarch64-linux-android', clang: 'aarch64-linux-android' },
  'x86_64': { rust: 'x86_64-linux-android', clang: 'x86_64-linux-android' },
};
const run = (cmd, args, env, cwd = root, capture = false) => {
  const result = spawnSync(cmd, args, { env, cwd, ...(capture ? { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 } : { stdio: 'inherit' }) });
  if (result.status !== 0) throw new Error(`${cmd} failed (${result.status ?? result.error?.message})${result.stderr ? ': ' + result.stderr.trim() : ''}`);
  return capture ? result.stdout.trim() : result;
};
const xml = value => String(value).replace(/[<>&"']/g, c => ({'<':'&lt;', '>':'&gt;', '&':'&amp;', '"':'&quot;', "'":'&apos;'}[c]));
const kotlin = value => JSON.stringify(String(value)).replaceAll('$', '\\$');
const write = (path, value) => {
  mkdirSync(dirname(path), { recursive: true });
  if (!existsSync(path) || readFileSync(path, 'utf8') !== value) writeFileSync(path, value);
};
const integer = (value, fallback, label) => {
  const number = value ?? fallback;
  if (!Number.isSafeInteger(number)) throw new Error(`host.android.${label} must be an integer`);
  return number;
};

/** APK bytes on disk and compressed/uncompressed ZIP payloads; ZIP/signature/alignment bytes are separate. */
export function apkFootprint(apk) {
  const zip = readFileSync(apk);
  let end = zip.length - 22;
  while (end >= Math.max(0, zip.length - 65557) && zip.readUInt32LE(end) !== 0x06054b50) end--;
  if (end < Math.max(0, zip.length - 65557)) throw new Error(`APK has no ZIP end record: ${apk}`);
  const count = zip.readUInt16LE(end + 10), offset = zip.readUInt32LE(end + 16);
  if (count === 0xffff || offset === 0xffffffff) throw new Error('APK footprint does not accept ZIP64');
  const categories = Object.fromEntries(['dex', 'native', 'assets', 'resources', 'metadata'].map(name => [name, { compressed_bytes: 0, uncompressed_bytes: 0, files: 0 }]));
  let cursor = offset, compressed = 0, uncompressed = 0;
  const entries = [];
  for (let index = 0; index < count; index++) {
    if (zip.readUInt32LE(cursor) !== 0x02014b50) throw new Error(`APK central directory entry ${index} is invalid`);
    const compressedBytes = zip.readUInt32LE(cursor + 20), uncompressedBytes = zip.readUInt32LE(cursor + 24);
    const nameLength = zip.readUInt16LE(cursor + 28), extraLength = zip.readUInt16LE(cursor + 30), commentLength = zip.readUInt16LE(cursor + 32);
    const name = zip.subarray(cursor + 46, cursor + 46 + nameLength).toString('utf8');
    const category = /\.dex$/.test(name) ? 'dex' : name.startsWith('lib/') ? 'native' : name.startsWith('assets/') ? 'assets'
      : name.startsWith('res/') || name === 'resources.arsc' ? 'resources' : 'metadata';
    categories[category].compressed_bytes += compressedBytes;
    categories[category].uncompressed_bytes += uncompressedBytes;
    categories[category].files++;
    compressed += compressedBytes; uncompressed += uncompressedBytes;
    entries.push({ path: name, category, compressed_bytes: compressedBytes, uncompressed_bytes: uncompressedBytes });
    cursor += 46 + nameLength + extraLength + commentLength;
  }
  return { apk_bytes: zip.length, payload_compressed_bytes: compressed, payload_uncompressed_bytes: uncompressed,
    zip_signing_alignment_bytes: zip.length - compressed, categories, entries };
}

function writeProject(app, out, env, config) {
  const { abi, minSdk, targetSdk, versionCode, host, kind = 'exact' } = config;
  const compose = kind === 'compose', exact = kind === 'exact';
  const suffix = exact ? '' : '-' + kind, label = exact ? '' : compose ? ' Compose' : ' Views';
  const appId = exact ? app.id : `${app.id}.${kind}`;
  const namespace = `com.exact.${exact ? 'android' : kind}`;
  const activity = `${namespace}.${exact ? 'Exact' : compose ? 'Compose' : 'Views'}Activity`;
  write(resolve(out, 'settings.gradle.kts'), `pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositories { google(); mavenCentral() } }\nrootProject.name = ${kotlin(app.name + suffix)}\ninclude(":app")\n`);
  write(resolve(out, 'gradle.properties'), 'android.useAndroidX=true\norg.gradle.jvmargs=-Xmx2g -Dfile.encoding=UTF-8\norg.gradle.parallel=true\n');
  const plugin = compose ? `; id("org.jetbrains.kotlin.plugin.compose") version "${versions.kotlin}"` : '';
  const dependencies = kind === 'views' ? '' : `implementation("androidx.compose.ui:ui-text:${versions.compose}"); ` +
    (compose ? `implementation("androidx.activity:activity:${versions.activity}"); implementation("androidx.compose.foundation:foundation:${versions.compose}"); implementation("androidx.compose.ui:ui:${versions.compose}")` : '');
  write(resolve(out, 'app/build.gradle.kts'), `plugins { id("com.android.application") version "${versions.agp}"${plugin} }\nandroid {\n    namespace = "${namespace}"\n    compileSdk = ${versions.compileSdk}\n    buildToolsVersion = "${versions.buildTools}"\n    ndkVersion = "${versions.ndk}"\n    defaultConfig {\n        applicationId = ${kotlin(appId)}\n        minSdk = ${minSdk}\n        targetSdk = ${targetSdk}\n        versionCode = ${versionCode}\n        versionName = ${kotlin(host.versionName ?? '0.1.0')}\n        ndk { abiFilters += ${kotlin(abi)} }\n    }\n    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }\n    ${compose ? 'buildFeatures { compose = true }' : ''}\n    signingConfigs.getByName("debug").storeFile = file("debug.keystore")\n    buildTypes.getByName("release") {\n        isDebuggable = false\n        isMinifyEnabled = true\n        isShrinkResources = true\n        proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")\n        signingConfig = signingConfigs.getByName("debug")\n    }\n}\ndependencies { ${dependencies} }\n`);
  // RegisterNatives/FindClass/GetMethodID address these exact classes and callback signatures.
  write(resolve(out, 'app/proguard-rules.pro'), exact ? `-keep,allowoptimization class com.exact.android.Native { *; }\n-keep,allowoptimization class com.exact.android.TextEngine {\n    java.nio.ByteBuffer measure(java.nio.ByteBuffer,int);\n    void fonts(java.nio.ByteBuffer);\n    void wake();\n}\n` : '');
  write(resolve(out, 'app/src/main/AndroidManifest.xml'), `<manifest xmlns:android="http://schemas.android.com/apk/res/android">\n    <application android:label="${xml(app.displayName + label)}" android:theme="@android:style/Theme.Material.Light.NoActionBar" android:allowBackup="false" android:supportsRtl="true">\n        <profileable android:shell="true"/>\n        <activity android:name="${activity}" android:exported="true" android:windowSoftInputMode="adjustResize">\n            <intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter>\n        </activity>\n    </application>\n</manifest>\n`);
  if (!existsSync(resolve(out, 'app/debug.keystore'))) {
    const keytool = env.JAVA_HOME ? resolve(env.JAVA_HOME, 'bin', 'keytool') : 'keytool';
    run(keytool, ['-genkeypair', '-keystore', resolve(out, 'app/debug.keystore'), '-storepass', 'android', '-alias', 'androiddebugkey', '-keypass', 'android',
      '-dname', 'CN=Android Debug,O=Android,C=US', '-validity', '10000', '-keyalg', 'RSA', '-keysize', '2048'], env);
  }
  return { appId, activity };
}

function copyBenchmarkSources(out) {
  const common = resolve(root, 'host/android/benchmark');
  if (existsSync(common)) cpSync(common, resolve(out, 'app/src/main/kotlin'), { recursive: true });
}

export function buildAndroid(app, options = {}) {
  if (app.hasGpu || app.manifest.game || app.modules.tags.length) throw new Error('Android does not yet support GPU or native module artifacts');
  if (rustPolicy(app.manifest, 'android') !== 'off') throw new Error('Android currently requires rust.platforms.android=false (embedded Rust only)');
  const abi = options.abi ?? 'arm64-v8a', target = targets[abi];
  if (!target) throw new Error(`unsupported Android ABI ${abi}; use ${Object.keys(targets).join(' or ')}`);
  const host = app.manifest.host?.android ?? {};
  const minSdk = integer(host.minSdk, 29, 'minSdk'), targetSdk = integer(host.targetSdk, 36, 'targetSdk');
  const versionCode = integer(host.versionCode, 1, 'versionCode');
  if (minSdk < 29 || targetSdk < minSdk || targetSdk > versions.compileSdk) throw new Error('Android requires 29 <= minSdk <= targetSdk <= compileSdk (37)');
  if (!/^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)+$/.test(app.id)) throw new Error(`app.id is not an Android application ID: ${app.id}`);
  let env = { ...developmentBuildEnv(), ...options.env };
  const sdk = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT;
  if (!sdk) throw new Error('Set ANDROID_HOME to an SDK with platforms;android-37.0, build-tools;36.0.0, platform-tools and ndk;28.2.13676358');
  const ndk = env.ANDROID_NDK_HOME ?? resolve(sdk, 'ndk', versions.ndk);
  if (!existsSync(resolve(ndk, 'source.properties')) || !new RegExp(`^Pkg\\.Revision\\s*=\\s*${versions.ndk.replaceAll('.', '\\.')}\\s*$`, 'm').test(readFileSync(resolve(ndk, 'source.properties'), 'utf8'))) {
    throw new Error(`Android build requires NDK ${versions.ndk}; ANDROID_NDK_HOME names ${ndk}`);
  }
  const prebuilt = { darwin: 'darwin-x86_64', linux: 'linux-x86_64', win32: 'windows-x86_64' }[process.platform];
  if (!prebuilt) throw new Error(`no NDK toolchain for ${process.platform}`);
  const clang = resolve(ndk, 'toolchains/llvm/prebuilt', prebuilt, 'bin', `${target.clang}${minSdk}-clang++${process.platform === 'win32' ? '.cmd' : ''}`);
  if (!existsSync(clang)) throw new Error(`missing NDK compiler ${clang}; install ndk;${versions.ndk}`);
  const gradle = env.EXACT_GRADLE ?? 'gradle';
  const actual = run(gradle, ['--version'], env, root, true);
  if (!actual.includes(`Gradle ${versions.gradle}\n`)) throw new Error(`Android build requires Gradle ${versions.gradle}; set EXACT_GRADLE to that executable`);
  const targetKey = target.rust.replaceAll('-', '_');
  env = { ...env, ANDROID_HOME: sdk, ANDROID_SDK_ROOT: sdk,
    [`CARGO_TARGET_${targetKey.toUpperCase()}_LINKER`]: clang,
    [`CC_${targetKey}`]: clang.replace(/-clang\+\+(\.cmd)?$/, '-clang$1'),
    [`CXX_${targetKey}`]: clang, [`AR_${targetKey}`]: resolve(dirname(clang), 'llvm-ar') };
  const out = resolve(app.target, 'apps', app.name, 'android', abi);
  const release = claimBuildOutput(app, resolve(out, '.build.lock'));
  try {
    const receipt = buildBake(app, 'android', target.rust, { env, profile: 'release' });
    if (receipt.compat.inputs.store.L !== '0') throw new Error('Android requires deploy.store.android="0"; delivery is not implemented');
    const archive = receipt.products.find(product => product.path.endsWith('.a'))?.path;
    if (!archive) throw new Error('Android bake did not produce its Rust static library');
    const lib = resolve(out, 'app/src/main/jniLibs', abi, 'libexact_app.so');
    mkdirSync(dirname(lib), { recursive: true });
    run(clang, ['-std=c++17', '-O3', '-DNDEBUG', '-fPIC', '-shared', '-fvisibility=hidden', '-Wall', '-Wextra', '-Werror',
      resolve(root, 'host/android/jni/exact_jni.cpp'), archive, '-static-libstdc++', '-Wl,--gc-sections', '-Wl,--exclude-libs,ALL',
      '-Wl,--pack-dyn-relocs=android+relr', '-Wl,--use-android-relr-tags', '-Wl,-z,max-page-size=16384', '-llog', '-landroid', '-ldl', '-lm', '-o', lib], env);
    const assets = resolve(out, 'app/src/main/assets');
    rmSync(assets, { recursive: true, force: true });
    mkdirSync(assets, { recursive: true });
    for (const directory of ['assets', 'deck']) copyStaticTreeIfPresent(resolve(app.dir, directory), resolve(assets, directory));
    verifyBakeFiles(receipt.compat, readFileSync(resolve(bakeOutput(app, env), `android-${target.rust}.plan`)), listAssets(assets));
    const kotlinSources = resolve(out, 'app/src/main/kotlin');
    rmSync(kotlinSources, { recursive: true, force: true });
    cpSync(resolve(root, 'host/android/kotlin'), kotlinSources, { recursive: true });
    copyBenchmarkSources(out);
    const identity = writeProject(app, out, env, { abi, minSdk, targetSdk, versionCode, host });
    run(gradle, ['--no-daemon', '--console=plain', '-p', out, ':app:assembleRelease'], env);
    const apk = resolve(out, `${app.name}.apk`);
    copyFileSync(resolve(out, 'app/build/outputs/apk/release/app-release.apk'), apk);
    console.log(`Android APK: ${apk}`);
    if (options.run || options.bench) {
      const adb = resolve(sdk, 'platform-tools', process.platform === 'win32' ? 'adb.exe' : 'adb');
      const serial = options.serial ? ['-s', options.serial] : [];
      run(adb, [...serial, 'install', '-r', apk], env);
      if (!options.bench) run(adb, [...serial, 'shell', 'am', 'start', '-W', '-n', `${app.id}/com.exact.android.ExactActivity`], env);
    }
    return { apk, library: lib, receipt, ...identity, versions, footprint: apkFootprint(apk),
      release: { debuggable: false, minified: true, resourceShrunk: true, rustProfile: 'release', signing: 'local-development' } };
  } finally { release(); }
}

function buildReference(app, options, kind) {
  if (app.name !== 'android-core') throw new Error('--compare requires the android-core benchmark fixture');
  const abi = options.abi ?? 'arm64-v8a';
  if (!targets[abi]) throw new Error(`unsupported Android ABI ${abi}`);
  const env = { ...developmentBuildEnv(), ...options.env };
  const sdk = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT;
  if (!sdk) throw new Error('Set ANDROID_HOME to the Android SDK');
  const host = app.manifest.host?.android ?? {};
  const out = resolve(app.target, 'apps', app.name, 'android', abi, kind);
  const release = claimBuildOutput(app, resolve(out, '.build.lock'));
  try {
    const kotlinSources = resolve(out, 'app/src/main/kotlin');
    rmSync(kotlinSources, { recursive: true, force: true });
    cpSync(resolve(app.dir, kind), kotlinSources, { recursive: true });
    copyBenchmarkSources(out);
    const identity = writeProject(app, out, env, { abi, host, kind, minSdk: integer(host.minSdk, 29, 'minSdk'),
      targetSdk: integer(host.targetSdk, 36, 'targetSdk'), versionCode: integer(host.versionCode, 1, 'versionCode') });
    run(env.EXACT_GRADLE ?? 'gradle', ['--no-daemon', '--console=plain', '-p', out, ':app:assembleRelease'], env);
    const apk = resolve(out, `${app.name}-${kind}.apk`);
    copyFileSync(resolve(out, 'app/build/outputs/apk/release/app-release.apk'), apk);
    console.log(`${kind} reference APK: ${apk}`);
    return { apk, ...identity, versions, footprint: apkFootprint(apk),
      release: { debuggable: false, minified: true, resourceShrunk: true, signing: 'local-development' } };
  } finally { release(); }
}

/** Independent Kotlin/Compose reference, with no Exact native library or host classes. */
export function buildCompose(app, options = {}) { return buildReference(app, options, 'compose'); }

/** Independent platform widget reference, with no Compose, Exact or native library. */
export function buildViews(app, options = {}) { return buildReference(app, options, 'views'); }

/** Bounded, opt-in fixture measurement; release carrier stays non-debuggable. */
export async function benchmarkAndroid(app, product, options = {}) {
  if (app.name !== 'android-core') throw new Error('--bench requires the android-core benchmark fixture');
  const env = { ...developmentBuildEnv(), ...options.env };
  const sdk = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT;
  const adb = resolve(sdk, 'platform-tools', process.platform === 'win32' ? 'adb.exe' : 'adb');
  const serial = options.serial ? ['-s', options.serial] : [];
  run(adb, [...serial, 'shell', 'am', 'force-stop', app.id], env);
  run(adb, [...serial, 'logcat', '-c'], env);
  run(adb, [...serial, 'shell', 'am', 'start', '-W', '-n', `${app.id}/com.exact.android.ExactActivity`, '--ez', 'exact_benchmark', 'true', '--ei', 'exact_samples', '120'], env);
  const deadline = Date.now() + 120_000;
  let announced = 0;
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 1000));
    const log = run(adb, [...serial, 'logcat', '-d', '-v', 'raw', '-s', 'ExactBenchmark:I', '*:S'], env, root, true);
    const rows = log.split('\n').filter(line => line.startsWith('Result:')).map(line => JSON.parse(line.slice(7)));
    for (const row of rows.slice(announced)) console.log(`Benchmark ${row.workload}: native p50=${row.native_commit.p50_ms}ms, apply p50=${row.decode_and_apply.p50_ms}ms`);
    announced = rows.length;
    const complete = log.split('\n').find(line => line.startsWith('Complete:'));
    if (complete) {
      const report = { ...JSON.parse(complete.slice(9)), results: rows };
      if (rows.length !== 5) throw new Error(`benchmark completed with ${rows.length} workload records (expected 5)`);
      const path = resolve(dirname(product.apk), 'android-benchmark.json');
      write(path, JSON.stringify(report, null, 2) + '\n');
      console.log(`Android benchmark: ${path}`);
      return report;
    }
  }
  throw new Error('Android benchmark did not complete within 120s; inspect adb logcat');
}

if (import.meta.main) {
  try {
    const args = process.argv.slice(2), options = {};
    let name;
    for (let i = 0; i < args.length; i++) {
      if (args[i] === '--run') options.run = true;
      else if (args[i] === '--bench') options.bench = true;
      else if (args[i] === '--compare') options.compare = true;
      else if (args[i] === '--heavy-list') options.heavyList = true;
      else if (args[i] === '--scroll-samples' || args[i] === '--frame-rounds') {
        const flag = args[i], raw = args[++i], value = Number(raw);
        const minimum = flag === '--scroll-samples' ? 0 : 1;
        if (!raw?.trim() || !Number.isSafeInteger(value) || value < minimum || (flag === '--scroll-samples' && value > 6000)) {
          throw new Error(`${flag} must be an integer ${flag === '--scroll-samples' ? 'in 0..6000' : '>= 1'}`);
        }
        options[flag === '--scroll-samples' ? 'scrollSamples' : 'frameRounds'] = value;
      } else if (args[i] === '--scroll-speeds') {
        const raw = args[++i], parts = raw?.split(',');
        const speeds = parts?.map(value => Number(value.trim()));
        if (!parts?.length || parts.some(value => !value.trim()) || new Set(speeds).size !== speeds.length ||
          speeds.some(value => !Number.isFinite(value) || value < 0 || value > 20_000)) {
          throw new Error('--scroll-speeds must be distinct comma-separated velocities in 0..20000 dp/s');
        }
        options.scrollSpeeds = speeds;
      } else if (args[i] === '--serial' || args[i] === '--abi') {
        const key = args[i].slice(2); options[key] = args[++i];
        if (!options[key]) throw new Error(`missing --${key} value`);
      } else if (args[i].startsWith('-') || name) throw new Error(`unknown argument ${args[i]}`);
      else name = args[i];
    }
    if (!options.compare && ['heavyList', 'scrollSamples', 'frameRounds', 'scrollSpeeds'].some(key => key in options)) {
      throw new Error('heavy-list and scroll/frame options require --compare');
    }
    if (options.heavyList && (options.scrollSamples === 0 || options.scrollSpeeds?.some(value => value === 0))) {
      throw new Error('--heavy-list requires positive scroll samples and velocities');
    }
    const app = resolveApp(name ?? (process.env.EXACT_APP_DIR ? undefined : 'android-core'));
    const product = buildAndroid(app, options);
    if (options.compare) {
      const compose = buildCompose(app, options);
      const views = buildViews(app, options);
      const { compareAndroid } = await import('./compare.mjs');
      await compareAndroid(app, { exact: product, compose, views }, { ...options, versions });
    } else if (options.bench) await benchmarkAndroid(app, product, options);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
