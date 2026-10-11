// LLP 1116 D4: every verb of an app's exact.mjs answers `--help` at once and starts nothing; the scripts it
// runs answer `--help` and refuse a flag they do not take; and each verb's help names exactly the flags its
// script parses (so it never names one that does not exist). `bun test ./scripts/help.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { createApp } from '../game/new.mjs';
import { HELP, knownFlags } from './help.mjs';

const ROOT = resolve(import.meta.dir, '..');
const APP_VERBS = ['web', 'web-build', 'test', 'agent', 'ios', 'mac', 'linux', 'android', 'update', 'contract', 'hatch', 'feedback'];
const timed = (args, options) => {
  const t = performance.now(), r = spawnSync(process.execPath, args, { encoding: 'utf8', timeout: 15_000, ...options });
  return { ...r, ms: performance.now() - t };
};

test('each verb of a new app\'s exact.mjs, and no verb, answers help from exact2 and runs nothing', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-help-'));
  try {
    const dir = resolve(parent, 'field-log'), sdk = resolve(parent, 'sdk'), started = resolve(parent, 'started');
    createApp(dir);
    const manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest, commands: { verify: ['bun', 'verify.mjs'] } }, null, 2));
    // A fake exact2 whose every script, run, leaves a mark: a help that built or served anything would.
    mkdirSync(resolve(sdk, 'scripts'), { recursive: true });
    copyFileSync(resolve(ROOT, 'scripts/help.mjs'), resolve(sdk, 'scripts/help.mjs'));
    for (const file of ['host/web/dev.mjs', 'host/web/build.mjs', 'host/apple/build.mjs', 'scripts/agent.mjs', 'scripts/build-linux.mjs', 'scripts/agent-android.mjs', 'scripts/exact.mjs', 'scripts/feedback.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), `require('node:fs').appendFileSync(${JSON.stringify(started)}, ${JSON.stringify(file)} + '\\n');`);
    }
    const exact = (...args) => timed([resolve(dir, 'exact.mjs'), ...args], { cwd: dir, env: { ...process.env, EXACT2: sdk } });
    for (const verb of APP_VERBS) {
      const lead = verb === 'test' ? ['ios'] : verb === 'agent' ? ['web', 'tree'] : [];
      for (const args of [[verb, '--help'], [verb, ...lead, '-h'], ['help', verb]]) {
        const r = exact(...args);
        assert.equal(r.status, 0, `${args.join(' ')}: ${r.stderr}`);
        assert.ok(r.stdout.startsWith(HELP[verb].usage), `${args.join(' ')} printed:\n${r.stdout}`);
        assert.ok(r.ms < 2000, `${args.join(' ')} took ${Math.round(r.ms)} ms`);
      }
    }
    for (const args of [[], ['help'], ['--help'], ['-h']]) {
      const r = exact(...args);
      assert.equal(r.status, 0, r.stderr);
      for (const verb of [...APP_VERBS, 'verify']) assert.match(r.stdout, new RegExp(`^  ${verb} +\\S`, 'm'), `the index names ${verb}`);
      assert.match(r.stdout, /verify +app\.json commands: bun verify\.mjs/);
    }
    assert.equal(exact('help', 'nope').status, 2);
    assert.ok(!existsSync(started), `help ran ${existsSync(started) && readFileSync(started, 'utf8')}`);
    assert.ok(!existsSync(resolve(dir, '.exact/commands.jsonl')), 'help is not a logged command');
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000); // One offline Cargo resolution.

test('the scripts a verb runs answer --help and refuse a flag they do not take, before starting anything', () => {
  const env = { ...process.env };
  delete env.EXACT_APP_DIR;
  for (const [verb, args] of [
    ['web', ['host/web/dev.mjs', '--app', 'caltrain']], ['web-build', ['host/web/build.mjs', 'caltrain']],
    ['agent', ['scripts/agent.mjs', 'web', '--app', 'caltrain', 'tree']], ['test', ['scripts/agent.mjs', 'web', '--test', 'x.test.contract']],
    ['ios', ['host/apple/build.mjs', '--ios', 'caltrain-apple']], ['mac', ['host/apple/build.mjs', 'caltrain-apple']],
    ['linux', ['scripts/build-linux.mjs', 'caltrain']], ['android', ['scripts/agent-android.mjs', 'build', 'caltrain']],
    ['feedback', ['scripts/feedback.mjs', 'send']], ['hatch', ['scripts/exact.mjs', 'hatch', 'avatar']],
    ['contract', ['scripts/exact.mjs', 'contract', 'build']], ['new', ['scripts/exact.mjs', 'new', 'x']],
  ]) {
    const help = timed([...args, '--help'], { cwd: ROOT, env });
    assert.equal(help.status, 0, `${args.join(' ')} --help: ${help.stderr}`);
    assert.ok(help.stdout.startsWith(HELP[verb].usage), `${args.join(' ')} --help printed:\n${help.stdout}${help.stderr}`);
    assert.ok(help.ms < 2000, `${args.join(' ')} --help took ${Math.round(help.ms)} ms`);
    if (verb === 'contract') continue; // the compiler parses its own flags
    const refused = timed([...args, '--bogus'], { cwd: ROOT, env });
    assert.equal(refused.status, 2, `${args.join(' ')} --bogus: ${refused.stdout}${refused.stderr}`);
    assert.match(refused.stderr, new RegExp(`^${verb}: unknown flag --bogus\\. It takes ${HELP[verb].flags.length ? '--' : 'no flags'}`));
  }
});

/** The flags a script's own parsing reads, by its idioms: `args.includes('--x')`, `arg('--x', …)`,
 * `argv[i] === '--x'` and a literal list matched against the arguments. */
function parsed(...sources) {
  const flags = new Set();
  for (const source of sources) {
    for (const re of [/\b(?:args|argv|rest|process\.argv)\.(?:includes|indexOf)\('(--[a-z-]+)'\)/g, /\barg\('(--[a-z-]+)'/g, /argv\[i\] === '(--[a-z-]+)'/g]) for (const [, flag] of source.matchAll(re)) flags.add(flag);
    for (const [, list] of source.matchAll(/\[((?:'--[a-z-]+'(?:, )?)+)\]\.(?:find|includes)\(/g)) for (const [, flag] of list.matchAll(/'(--[a-z-]+)'/g)) flags.add(flag);
  }
  return [...flags].sort();
}

test('each verb\'s help names exactly the flags its script parses, in a short text', () => {
  const read = (file) => readFileSync(resolve(ROOT, file), 'utf8');
  const exact = read('scripts/exact.mjs'), between = (from, to) => exact.slice(exact.indexOf(from), exact.indexOf(to, exact.indexOf(from)));
  const apple = parsed(read('host/apple/build.mjs'), read('host/apple/devices.mjs'), read('host/apple/link.mjs'));
  for (const [verb, flags] of [
    ['web', parsed(read('host/web/dev.mjs'), read('scripts/install-page.mjs'))],
    ['web-build', parsed(read('host/web/build.mjs'))],
    ['agent', parsed(read('scripts/agent-launch.mjs'))], ['test', parsed(read('scripts/agent-launch.mjs'))],
    ['ios', apple], ['mac', apple],
    ['linux', parsed(read('scripts/build-linux.mjs'))], ['android', parsed(read('scripts/agent-android.mjs'))],
    ['feedback', parsed(read('scripts/feedback.mjs'))],
    ['hatch', parsed(between('export function hatch(', '\nfunction main('))],
    ['new', parsed(between("if (verb === 'new')", '\n  }'))],
  ]) assert.deepEqual([...knownFlags(verb).keys()].sort(), flags, `${verb}: the help's flags (documented and internal) against its script's`);
  for (const verb of Object.keys(HELP)) {
    const lines = (`${HELP[verb].usage}\n${HELP[verb].body}`.split('\n').length + HELP[verb].flags.length);
    assert.ok(lines <= 34, `${verb}'s help is ${lines} lines; it is replayed in an agent's context, so keep it short`);
  }
});
