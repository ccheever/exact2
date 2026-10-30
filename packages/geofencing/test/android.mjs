// @exact/geofencing on Android (LLP 1133.021 §15 stage 3): the geofencing fixture's contract (`apps/geofence-fixture`)
// in an app `exact new` made outside the repo, built as the Canvas reader's APK, which compiles Geofencing.kt and
// Receivers.kt against Play services' location in the app's one locked, verified Gradle graph (LLP 1133.008 D8 item
// 5), and the host's region store (`dev.exact.canvas.ExactRegions`). It checks what the two grant lines derive, the
// fragment's two receivers merged under the allow-list (LLP 1133.030 D9), unexported, and their classes compiled in
// under their names. No fence is armed: the APK is not installed anywhere, and a crossing on a phone is
// checklist.md's, with a simulated location only.
//
// usage: bun packages/geofencing/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { androidFixture, apkOf, checker, root } from '../../maps/test/android.mjs';

const { check, failed } = checker();
const source = resolve(root, 'apps/geofence-fixture');
// Its grant lines are its data crate's, which a TypeScript fixture writes in `app.ts`.
const grants = JSON.parse(readFileSync(resolve(source, 'data/src/lib.rs'), 'utf8').match(/fn grants\(&self\) -> &str \{\s*("(?:[^"\\]|\\.)*")/)[1]);
const fixture = androidFixture('geofencefixture', {
  contract: readFileSync(resolve(source, 'app.contract'), 'utf8'), uses: ['geofencing'], grants,
  strings: JSON.parse(readFileSync(resolve(source, 'strings/en.json'), 'utf8')),
});
try {
  const r = fixture.build(['--write-locks']);
  check('the APK builds, compiling Geofencing.kt, Receivers.kt and the region store', r.status === 0, (r.stdout + r.stderr).slice(-3000));
  const { roster, manifest, lock, dex } = apkOf(fixture.app, fixture.out);
  check("the lock names Play services' location", /^com\.google\.android\.gms:play-services-location:21\.4\.0=/m.test(lock), lock.slice(0, 400) || 'no gradle.lockfile');
  check('the roster binds the module', roster.includes('"geofencing" to { c -> exact.geofencing.GeofencingModule(c) }'), roster);
  const permission = (p) => new RegExp(`<uses-permission\\s+android:name="android\\.permission\\.${p}"`).test(manifest);
  const missing = ['ACCESS_FINE_LOCATION', 'ACCESS_COARSE_LOCATION', 'ACCESS_BACKGROUND_LOCATION', 'RECEIVE_BOOT_COMPLETED'].filter((p) => !permission(p));
  check('the grants derive fine, coarse and background location, and the boot broadcast', !missing.length, `missing ${missing.join(', ')}`);
  const receiver = (cls) => manifest.match(new RegExp(`<receiver[^>]*android:name="exact\\.geofencing\\.${cls}"[^>]*>`))?.[0] ?? '';
  const crossing = receiver('GeofenceReceiver'), boot = receiver('GeofenceBootReceiver');
  check('the fragment merged: both receivers, neither exported', [crossing, boot].every((e) => /android:exported="false"/.test(e)), [crossing, boot]);
  check('the boot receiver hears BOOT_COMPLETED', /android\.intent\.action\.BOOT_COMPLETED/.test(manifest), manifest.slice(0, 3000));
  // R8 keeps the classes a manifest names by their names; the module and the store it may rename.
  const absent = ['GeofenceReceiver', 'GeofenceBootReceiver'].filter((c) => !dex.includes(`Lexact/geofencing/${c};`));
  check("the receivers' classes are compiled in under their names", !absent.length, `missing ${absent.join(', ')}`);
} finally {
  fixture.done();
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
