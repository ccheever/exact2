const { createHash } = require('node:crypto');
const { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { tmpdir } = require('node:os');
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

function credentialParts(value) {
  const credentials = value?.ios ?? value;
  const certificate = credentials?.distributionCertificate;
  const profile = credentials?.provisioningProfileBase64 ?? credentials?.provisioningProfile?.dataBase64;
  if (!certificate?.dataBase64 || !certificate.password || !profile) fail('EAS did not provide an iOS distribution certificate and provisioning profile');
  return { certificate, profile };
}

function installSigning(credentials, directory) {
  const { certificate, profile } = credentialParts(credentials);
  const p12 = join(directory, 'distribution.p12');
  const mobileprovision = join(directory, 'T3Code.mobileprovision');
  writeFileSync(p12, Buffer.from(certificate.dataBase64, 'base64'), { mode: 0o600 });
  writeFileSync(mobileprovision, Buffer.from(profile, 'base64'), { mode: 0o600 });
  const uuid = plistValue(mobileprovision, 'UUID');
  const profileDestination = join(process.env.HOME, 'Library/MobileDevice/Provisioning Profiles', `${uuid}.mobileprovision`);
  mkdirSync(join(process.env.HOME, 'Library/MobileDevice/Provisioning Profiles'), { recursive: true });
  writeFileSync(profileDestination, readFileSync(mobileprovision), { mode: 0o600 });

  const keychain = join(directory, 'T3Code-signing.keychain-db');
  const keychainPassword = `${uuid}-${Date.now()}`;
  const keychains = run('security', ['list-keychains', '-d', 'user'])
    .split('\n').map(value => value.trim().replace(/^"|"$/g, '')).filter(Boolean);
  try {
    run('security', ['create-keychain', '-p', keychainPassword, keychain]);
    run('security', ['set-keychain-settings', '-lut', '21600', keychain]);
    run('security', ['unlock-keychain', '-p', keychainPassword, keychain]);
    run('security', ['import', p12, '-k', keychain, '-P', certificate.password, '-T', '/usr/bin/codesign']);
    run('security', ['set-key-partition-list', '-S', 'apple-tool:,apple:,codesign:', '-s', '-k', keychainPassword, keychain]);
    run('security', ['list-keychains', '-d', 'user', '-s', ...keychains, keychain]);
    const identities = run('security', ['find-identity', '-v', '-p', 'codesigning', keychain]);
    const identity = identities.match(/([0-9A-F]{40})/i)?.[1];
    if (!identity) fail('The EAS distribution certificate did not produce a codesigning identity');
    return { identity, keychain, keychainPassword, profile: mobileprovision, profileDestination, uuid, keychains };
  } catch (error) {
    run('security', ['list-keychains', '-d', 'user', '-s', ...keychains], { stdio: 'ignore' });
    run('security', ['delete-keychain', keychain], { stdio: 'ignore' });
    rmSync(profileDestination, { force: true });
    throw error;
  }
}

function cleanupSigning(signing) {
  if (!signing) return;
  run('security', ['list-keychains', '-d', 'user', '-s', ...signing.keychains], { stdio: 'ignore' });
  run('security', ['delete-keychain', signing.keychain], { stdio: 'ignore' });
  if (existsSync(signing.profileDestination)) rmSync(signing.profileDestination, { force: true });
}

exports.default = async function archive(ctx, { inputs, env }) {
  const buildNumber = inputs.build_number?.value;
  if (!/^[1-9]\d*$/.test(buildNumber ?? '')) fail('EAS must allocate a positive remote build number');
  const directory = mkdtempSync(join(tmpdir(), 't3-code-eas-'));
  const output = resolve(ctx.workingDirectory, '.exact/t3-production');
  mkdirSync(output, { recursive: true });
  let signing;
  try {
    signing = installSigning(inputs.credentials?.value, directory);
    const source = resolve(ctx.workingDirectory);
    const ipa = join(output, 'T3Code.ipa');
    const command = ['host/apple/build.mjs', '--device', 't3-code-ios', '--archive', ipa];
    const buildEnv = {
      ...process.env,
      ...env,
      EXACT_APP_DIR: join(source, 'examples/t3-code/mobile'),
      EXACT_BUILD_NUMBER: buildNumber,
      EXACT_VERSION: '1.0.0',
      EXACT_IDENTITY: signing.identity,
      EXACT_PROFILE: signing.profile,
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
      provisioningProfileUUID: signing.uuid,
      source: command,
    }, null, 2) + '\n', { mode: 0o600 });
  } finally {
    cleanupSigning(signing);
    rmSync(directory, { recursive: true, force: true });
  }
};
