// The Android pedometer's sums (LLP 1133.026 §15 stage 3): `android/Count.kt` and `count/CountCheck.kt` compiled by
// the Kotlin compiler Gradle keeps in its cache, which the Android build downloaded, and run on the JDK.
//
// usage: bun packages/pedometer/test/count.mjs   (a JDK 17 or newer in JAVA_HOME, and one Android build done)
import { existsSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, pkg = resolve(here, '..');
const cache = resolve(process.env.GRADLE_USER_HOME ?? join(homedir(), '.gradle'), 'caches/modules-2/files-2.1');
/** The newest cached jar of `group:name`, or of `group:name:version`. */
const jar = (group, name, version) => {
  const dir = resolve(cache, group, name);
  const versions = existsSync(dir) ? readdirSync(dir).filter((v) => !version || v === version).sort((a, b) => a.localeCompare(b, 'en', { numeric: true })) : [];
  for (const v of versions.reverse()) {
    for (const hash of readdirSync(resolve(dir, v))) {
      const file = readdirSync(resolve(dir, v, hash)).find((f) => f === `${name}-${v}.jar`);
      if (file) return { path: resolve(dir, v, hash, file), version: v };
    }
  }
  throw new Error(`no ${group}:${name}${version ? `:${version}` : ''} in ${cache}: build an Android app once (bun scripts/agent-android.mjs build <app>)`);
};
const compiler = jar('org.jetbrains.kotlin', 'kotlin-compiler-embeddable');
const stdlib = jar('org.jetbrains.kotlin', 'kotlin-stdlib', compiler.version).path;
const classpath = [compiler.path, stdlib, jar('org.jetbrains.kotlin', 'kotlin-script-runtime', compiler.version).path,
  jar('org.jetbrains.kotlin', 'kotlin-reflect', compiler.version).path, jar('org.jetbrains.kotlin', 'kotlin-daemon-embeddable', compiler.version).path,
  jar('org.jetbrains.kotlinx', 'kotlinx-coroutines-core-jvm').path, jar('org.jetbrains', 'annotations').path].join(':');
const java = process.env.JAVA_HOME ? resolve(process.env.JAVA_HOME, 'bin/java') : 'java';
const out = mkdtempSync(join(tmpdir(), 'exact-count-'));
try {
  const built = spawnSync(java, ['-cp', classpath, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-cp', stdlib, '-d', out,
    resolve(pkg, 'android/Count.kt'), resolve(here, 'count/CountCheck.kt')], { encoding: 'utf8' });
  if (built.status !== 0) {
    console.error(built.stdout + built.stderr);
    process.exit(1);
  }
  const ran = spawnSync(java, ['-cp', `${out}:${stdlib}`, 'CountCheckKt'], { stdio: 'inherit' });
  process.exitCode = ran.status ?? 1;
} finally {
  rmSync(out, { recursive: true, force: true });
}
