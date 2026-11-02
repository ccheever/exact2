const { createHash } = require('node:crypto');
const { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { homedir, tmpdir } = require('node:os');
const { spawnSync } = require('node:child_process');

const fail = (message) => { throw new Error(message); };
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`${command} failed (${result.status}): ${result.stderr || result.stdout || ''}`.trim());
  return result.stdout?.trim() ?? '';
};
const digest = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const plistValue = (profile, key) => {
  const plist = run('security', ['cms', '-D', '-i', profile]);
  const decoded = join(tmpdir(), `t3-code-profile-${process.pid}.plist`);
  writeFileSync(decoded, plist);
  try { return run('plutil', ['-extract', key, 'raw', '-o', '-', decoded]); }
  finally { rmSync(decoded, { force: true }); }
};

function provisioningProfilePath(teamId, bundleIdentifier) {
  const directory = join(process.env.HOME ?? homedir(), 'Library/MobileDevice/Provisioning Profiles');
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith('.mobileprovision')) continue;
    const profile = join(directory, entry.name);
    try {
      if (plistValue(profile, 'Entitlements.application-identifier') === `${teamId}.${bundleIdentifier}`) return profile;
    } catch {
      // Ignore unrelated or stale profile files.
    }
  }
  fail(`EAS prepared no provisioning profile for ${bundleIdentifier}`);
}

function signingIdentity(teamId) {
  const listed = run('security', ['list-keychains', '-d', 'user']);
  const keychains = [...listed.matchAll(/\"([^\"]+)\"/g)].map(match => match[1]);
  for (const name of readdirSync(tmpdir())) {
    if (/^eas-build-[\w-]+\.keychain(?:-db)?$/.test(name)) keychains.push(join(tmpdir(), name));
  }
  const uniqueKeychains = [...new Set(keychains)];
  for (const keychain of uniqueKeychains) {
    if (!existsSync(keychain)) continue;
    let identities;
    try { identities = run('security', ['find-identity', '-v', '-p', 'codesigning', keychain]); }
    catch { continue; }
    const lines = identities.split('\n').filter(line => /Distribution/.test(line));
    const line = lines.find(value => value.includes(`(${teamId})`)) ?? lines[0];
    const identity = line?.match(/\b[0-9A-F]{40}\b/i)?.[0];
    if (identity) {
      run('security', ['list-keychains', '-d', 'user', '-s', ...uniqueKeychains, keychain]);
      return { identity, keychain };
    }
  }
  fail(`EAS prepared no iOS distribution identity for team ${teamId}`);
}

exports.default = async function archive(ctx, { inputs, env }) {
  const teamId = inputs.team_id?.value;
  const buildNumber = inputs.build_number?.value;
  if (!/^[A-Z0-9]{6,20}$/.test(teamId ?? '')) fail('EAS must provide an Apple team ID');
  if (!/^[1-9]\d*$/.test(buildNumber ?? '')) fail('EAS must allocate a positive remote build number');

  const source = resolve(ctx.workingDirectory);
  const output = resolve(source, '.exact/t3-production');
  mkdirSync(output, { recursive: true });
  const ipa = join(output, 'T3Code.ipa');
  const profile = provisioningProfilePath(teamId, 'com.exact.t3code.ios');
  const signing = signingIdentity(teamId);
  const command = ['host/apple/build.mjs', '--device', 't3-code-ios', '--archive', ipa];
  const buildEnv = {
    ...process.env,
    ...env,
    EXACT_APP_DIR: join(source, 'examples/t3-code/mobile'),
    EXACT_BUILD_NUMBER: buildNumber,
    EXACT_VERSION: '1.0.0',
    EXACT_IDENTITY: signing.identity,
    EXACT_PROFILE: profile,
    EXACT_UPDATE_TRUST: 'production',
    EXACT_UPDATE_GENESIS: '1',
    EXACT_NON_EXEMPT_ENCRYPTION: 'false',
  };
  run('bun', command, { cwd: source, env: buildEnv, stdio: 'inherit' });
  writeFileSync(join(output, 'build.json'), JSON.stringify({
    appCommit: run('git', ['rev-parse', 'HEAD'], { cwd: source }),
    buildNumber,
    version: '1.0.0',
    bundleIdentifier: 'com.exact.t3code.ios',
    ipaSha256: digest(ipa),
    provisioningProfileUUID: plistValue(profile, 'UUID'),
    source: command,
  }, null, 2) + '\n', { mode: 0o600 });
};
