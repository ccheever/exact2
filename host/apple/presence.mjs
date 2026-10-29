// Locked secrets on a Mac (LLP 1133.010 D7). A Mac keeps a locked secret only
// in the data-protection keychain, whose access groups come from the app's
// code-signing entitlements, which a provisioning profile must authorize
// (TN3137). It never falls back to the login keychain, whose access model
// has no presence check. Signing a Mac build with such a profile is not
// built, since no Mac it was built on had one: a build says it keeps no
// locked secret, and why, and a release build whose grants say a name must
// be locked is refused.
import { installedProfiles, decodeProfile, profilesFor, byExpiry } from './devices.mjs';

const POLICIES = ['user-presence', 'biometry-any', 'biometry-current-set', 'device-credential'];

/** The grant lines that keep a name under a presence policy: `{ name, optional }` each. */
export function presenceLines(ceiling = '') {
  return String(ceiling).split('\n').map((line) => line.trim().split(/\s+/))
    .filter((w) => w[0] === 'secret.keep' && POLICIES.includes(w[2]))
    .map((w) => ({ name: w[1], optional: w[3] === 'optional' }));
}

/** A macOS profile for `app` that grants its keychain group (`<prefix>.<id>`, or the team's `<prefix>.*`), the
 * newest; EXACT_MAC_PROFILE names one. Development profiles for a development build, distribution ones otherwise. */
export function macPresenceProfile(app, { distribution = false, installed = installedProfiles(),
  named = process.env.EXACT_MAC_PROFILE ? decodeProfile(process.env.EXACT_MAC_PROFILE) : null } = {}) {
  const fits = (p) => p.platforms?.includes('OSX') && p.dev === !distribution && p.expires > new Date() && profilesFor(app.id, [p]).length > 0
    && (p.keychain.includes(`${p.prefix}.${app.id}`) || p.keychain.includes(`${p.prefix}.*`));
  return (named ? [named] : installed).filter(fits).sort(byExpiry)[0] ?? null;
}

/** What a macOS build of `app` says of its presence lines, or refuses: a release build with a name that must be
 * locked and no profile is refused (`grant-needs-profile`), so nothing ships that cannot keep what it grants. */
export function macPresence(app, ceiling, { release = false, distribution = false, log = console.log, find = macPresenceProfile } = {}) {
  const lines = presenceLines(ceiling);
  if (!lines.length) return;
  const mandatory = lines.filter((l) => !l.optional).map((l) => `secret.keep ${l.name}`);
  const profile = find(app, { distribution });
  const effect = `localAuthStatus().presenceSecrets is false${mandatory.length ? `, a set of ${mandatory.join(', ')} throws NotSupportedError` : ''}${mandatory.length < lines.length ? ', an optional name stays plain and protect() rejects' : ''}`;
  const why = profile
    ? `the macOS profile "${profile.name}" (team ${profile.team}) grants ${app.id}'s keychain group, and signing a Mac build with it is not built yet`
    : `no macOS provisioning profile for ${app.id} grants keychain-access-groups; install a development one with Keychain Sharing, or name one with EXACT_MAC_PROFILE`;
  if (release && mandatory.length) {
    throw new Error(`grant-needs-profile: ${mandatory.join(', ')} must be kept locked, and a Mac keeps a locked secret only in the data-protection keychain, which a provisioning profile authorizes (LLP 1133.010 D7): ${why}. Or write \`optional\` on the line.`);
  }
  log(`Locked secrets: this build keeps none, since ${why} (LLP 1133.010 D7): ${effect}.`);
}

/** The RP IDs a Mac needs an associated domain for: every `auth.passkey` line but `localhost`, which only the web admits. */
export function passkeyLines(ceiling = '') {
  return String(ceiling).split('\n').map((line) => line.trim().split(/\s+/))
    .filter((w) => w[0] === 'auth.passkey' && w[1] && w[1] !== 'localhost').map((w) => w[1]);
}

/** The bake's associated domains for an entitlement. A passkey RP ID's `webcredentials:` in a development build is
 * `?mode=developer`, so the device fetches the association from the domain rather than Apple's CDN (LLP 1133.011
 * D6); a release build never asks for it. */
export function associatedDomains(auth, debuggable) {
  const passkeys = new Set((auth?.passkeys ?? []).map((rp) => `webcredentials:${rp}`));
  return (auth?.associatedDomains ?? []).map((d) => debuggable && passkeys.has(d) ? `${d}?mode=developer` : d);
}

/** Sign in with Apple's entitlement, which follows the signing, not the grant (LLP 1133.014 D7): a simulator takes it;
 * a device's profile must authorize it, and reading that from the profile is not built, so a device build signs none. */
export function signInEntitlement(auth, simulator) {
  return (auth?.signIn?.apple ?? []).length && simulator ? { 'com.apple.developer.applesignin': ['Default'] } : {};
}

/** A grant's entitlement's value (`reach.entitlements`): `true`, or the string the build chooses (LLP 1133.028 D15
 * item 1). App Attest's environment is `development` for a debuggable build and `production` for a release; Apple
 * uses production for TestFlight and App Store builds whatever it says. */
export function entitlementValue(name, debuggable) {
  if (name === 'com.apple.developer.devicecheck.appattest-environment') return debuggable ? 'development' : 'production';
  return true;
}

/** The device rows whose entitlement a Mac takes only with a provisioning profile (LLP 1133.028 D17): App Attest's
 * environment and the Declared Age Range. */
const MAC_PROFILE_ROWS = { attestation: 'attestationStatus().available is false and attest fails NotSupportedError',
  'age-range': 'ageRangeStatus().available is false and requestAgeRange fails NotSupportedError' };

/** What a macOS build of `app` says of those rows, or refuses: a development build signs neither entitlement, and a
 * release build granting either is refused (`grant-needs-profile`), as a passkey line is. */
export function macProfileRows(app, ceiling, { release = false, log = console.log } = {}) {
  const rows = String(ceiling).split('\n').map((line) => line.trim().split(/\s+/)[0])
    .filter((w) => w?.startsWith('device.') && MAC_PROFILE_ROWS[w.slice(7)]).map((w) => w.slice(7));
  if (!rows.length) return;
  const lines = rows.map((r) => `device.${r}`).join(', ');
  const one = rows.length === 1;
  const why = `${one ? 'its entitlement needs' : 'their entitlements need'} a macOS provisioning profile for ${app.id}, and signing a Mac build with one is not built yet (LLP 1133.010 D7)`;
  if (release) throw new Error(`grant-needs-profile: ${lines}: ${why}.`);
  log(`${lines}: this build signs ${one ? 'no entitlement for it' : 'neither entitlement'}, since ${why}: ${rows.map((r) => MAC_PROFILE_ROWS[r]).join('; ')} outside the agent.`);
}

/** What a macOS build of `app` says of its passkey lines, or refuses (LLP 1133.011 D6): `webcredentials:` is a
 * restricted entitlement on macOS, which only a provisioning profile authorizes (TN3125), and signing a Mac build
 * with one is not built. So the build signs no associated domain, its ceremonies fail `NotSupportedError`, and a
 * release build with a passkey line is refused (`grant-needs-profile`). */
export function macPasskeys(app, ceiling, { release = false, log = console.log } = {}) {
  const ids = passkeyLines(ceiling);
  if (!ids.length) return;
  const lines = ids.map((rp) => `auth.passkey ${rp}`).join(', ');
  const why = `the Associated Domains entitlement needs a macOS provisioning profile for ${app.id}, and signing a Mac build with one is not built yet (LLP 1133.010 D7)`;
  if (release) throw new Error(`grant-needs-profile: ${lines}: ${why}.`);
  log(`Passkeys: this build signs no associated domain, since ${why}: getClientCapabilities().available is false and create and get fail NotSupportedError.`);
}

/** @ref LLP 1133.034 D13 — the `payment.apple-pay` grants' merchant IDs, where they can be signed: on a simulator, or
 * with a profile whose entitlements (`allows`) include Apple Pay's. Elsewhere (an ad hoc Mac build) Apple Pay is
 * unavailable, and a profile without it would refuse the build. */
export function merchantEntitlement(reach, { simulator = false, team = null, allows = [] } = {}) {
  const merchants = reach?.payments?.merchantIds ?? [];
  const signable = (simulator && !team) || allows.includes('com.apple.developer.in-app-payments');
  return merchants.length && signable ? { 'com.apple.developer.in-app-payments': merchants } : {};
}
