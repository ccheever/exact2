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
