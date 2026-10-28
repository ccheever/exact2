// @exact/geocoding on Android (LLP 1133.021 §15 stage 3): a fixture `exact new` made outside the repo, calling the
// three names, built as the Canvas reader's APK, which compiles Geocoding.kt against the platform's `Geocoder` (no
// Maven library, so no Gradle lock). It checks the build and the roster. No address is looked up: the APK is not
// installed anywhere (checklist.md's Android part is a person's).
//
// usage: bun packages/geocoding/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { androidFixture, apkOf, checker } from '../../maps/test/android.mjs';

const contract = `// The geocoding fixture: the platform's geocoder, forward and reverse.

use geocode, reverseGeocode, geocodingStatus, Place, GeocodingStatus from "@exact/geocoding"

component GeocodingFixture
  resource coder = geocodingStatus() as shape GeocodingStatus else empty()
  mutation found as shape Place[]
  action look
    send found = geocode(address="1 Ferry Building, San Francisco")
  action name
    send found = reverseGeocode(latitude=37.7955, longitude=-122.3937)
  view
    main testId="fixture" padding=16 gap=12 display="flex" flex-direction="column"
      text \`available: \${coder.available.toString()}\` testId="status"
      button press=look testId="look"
        text "Find the Ferry Building"
      button press=name testId="name"
        text "Name this place"
`;

const { check, failed } = checker();
const fixture = androidFixture('geocodingfixture', { contract, uses: ['geocoding'] });
try {
  const r = fixture.build();
  check('the APK builds, compiling Geocoding.kt', r.status === 0, (r.stdout + r.stderr).slice(-3000));
  const { roster, lock } = apkOf(fixture.app, fixture.out);
  check('the roster binds the module', roster.includes('"geocoding" to { c -> exact.geocoding.GeocodingModule(c) }'), roster);
  check('it names no Maven library', lock === '', lock.slice(0, 400));
} finally {
  fixture.done();
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
