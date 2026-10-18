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
import { HELP, helpText, indexText, knownFlags } from './help.mjs';

const ROOT = resolve(import.meta.dir, '..');
const APP_VERBS = ['web', 'web-build', 'test', 'agent', 'verify', 'ios', 'mac', 'linux', 'android', 'update', 'lock', 'contract', 'tamber', 'hatch', 'snapback', 'backend', 'pattern', 'feedback'];
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
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest, commands: { seed: ['bun', 'seed.mjs'] } }, null, 2));
    // A fake exact2 whose every script, run, leaves a mark: a help that built or served anything would.
    mkdirSync(resolve(sdk, 'scripts'), { recursive: true });
    for (const file of ['scripts/help.mjs', 'scripts/app-cli.mjs', 'scripts/bun.mjs']) copyFileSync(resolve(ROOT, file), resolve(sdk, file));
    for (const file of ['host/web/dev.mjs', 'host/web/build.mjs', 'host/apple/build.mjs', 'scripts/agent.mjs', 'scripts/verify.mjs', 'scripts/build-linux.mjs', 'scripts/agent-android.mjs', 'scripts/exact.mjs', 'scripts/feedback.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), `require('node:fs').appendFileSync(${JSON.stringify(started)}, ${JSON.stringify(file)} + '\\n');`);
    }
    const exact = (...args) => timed([resolve(dir, 'exact.mjs'), ...args], { cwd: dir, env: { ...process.env, EXACT2: sdk } });
    for (const verb of APP_VERBS) {
      const lead = verb === 'test' || verb === 'verify' ? ['ios'] : verb === 'agent' ? ['web', 'tree'] : [];
      for (const args of [[verb, '--help'], [verb, ...lead, '-h'], ['help', verb], [verb, '--help', 'all']]) {
        const r = exact(...args);
        assert.equal(r.status, 0, `${args.join(' ')}: ${r.stderr}`);
        assert.ok(r.stdout.startsWith(HELP[verb].usage), `${args.join(' ')} printed:\n${r.stdout}`);
        assert.ok(r.ms < 2000, `${args.join(' ')} took ${Math.round(r.ms)} ms`);
        assert.equal(r.stdout.trim(), helpText(verb, { all: args.includes('all') }), `${args.join(' ')}: the short text, or with all the whole`);
      }
    }
    for (const args of [[], ['help'], ['--help'], ['-h']]) {
      const r = exact(...args);
      assert.equal(r.status, 0, r.stderr);
      for (const verb of [...APP_VERBS, 'seed']) assert.match(r.stdout, new RegExp(`^  ${verb} +\\S`, 'm'), `the index names ${verb}`);
      assert.match(r.stdout, /seed +app\.json commands: bun seed\.mjs/);
    }
    // LLP 1116.010 D2 (weather, round 12: `help --help all` and `--help all` reported "no verb all"): every help form the index and
    // the helps name runs as written, and `all` with no verb is the index, never a verb.
    const own = { seed: ['bun', 'seed.mjs'] }, index = indexText({ verbs: APP_VERBS, own }), forms = new Set();
    for (const text of [index, ...APP_VERBS.map((v) => helpText(v))]) for (const [, form] of text.matchAll(/bun exact\.mjs ((?:[\w<>-]+ )?(?:--help|-h)(?: all)?|help(?: all)?)(?=[:\s]|$)/g)) forms.add(form);
    assert.ok(forms.has('<verb> --help') && forms.has('<verb> --help all'), `the forms named: ${[...forms]}`);
    const expected = (args) => { const verb = args.find((a) => HELP[a]); return verb ? helpText(verb, { all: args.includes('all') }) : index; };
    for (const form of forms) for (const verb of form.includes('<verb>') ? APP_VERBS : ['']) {
      const args = form.replace('<verb>', verb).split(' ').filter(Boolean), r = exact(...args);
      assert.equal(r.status, 0, `${args.join(' ')}: ${r.stderr}`);
      assert.equal(r.stdout.trim(), expected(args), `bun exact.mjs ${args.join(' ')}`);
    }
    for (const args of [['--help', 'all'], ['-h', 'all'], ['help', 'all'], ['help', '--help', 'all'], ['help', 'web', 'all'], ['help', 'all', 'web'], ['help', 'seed']]) {
      const r = exact(...args);
      assert.equal(r.status, 0, `${args.join(' ')}: ${r.stderr}`);
      if (args[1] !== 'seed') assert.equal(r.stdout.trim(), expected(args), `bun exact.mjs ${args.join(' ')}`);
    }
    assert.equal(exact('help', 'nope').status, 2);
    assert.equal(exact('help', 'all', 'nope').status, 2);
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
    ['verify', ['scripts/verify.mjs', 'web', '--app', 'caltrain']],
    ['ios', ['host/apple/build.mjs', '--ios', 'caltrain-apple']], ['mac', ['host/apple/build.mjs', 'caltrain-apple']],
    ['linux', ['scripts/build-linux.mjs', 'caltrain']], ['android', ['scripts/agent-android.mjs', 'build', 'caltrain']],
    ['feedback', ['scripts/feedback.mjs', 'send']], ['hatch', ['scripts/exact.mjs', 'hatch', 'avatar']], ['pattern', ['scripts/exact.mjs', 'pattern', 'list-detail']],
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
    ['agent', parsed(read('scripts/agent-launch.mjs'))], ['test', parsed(read('scripts/agent-launch.mjs'))], ['verify', parsed(read('scripts/verify.mjs'))],
    ['ios', apple], ['mac', apple],
    ['linux', parsed(read('scripts/build-linux.mjs'))], ['android', parsed(read('scripts/agent-android.mjs'))],
    ['feedback', parsed(read('scripts/feedback.mjs'))],
    ['hatch', parsed(between('export function hatch(', '\nexport function pattern('))],
    ['pattern', parsed(between('export function pattern(', '\nasync function main('))],
    ['new', parsed(between("if (verb === 'new')", '\n  }'))],
  ]) assert.deepEqual([...knownFlags(verb).keys()].sort(), flags, `${verb}: the help's flags (documented and internal) against its script's`);
  for (const verb of Object.keys(HELP)) {
    const lines = helpText(verb, { all: true }).split('\n').length;
    assert.ok(lines <= 36, `${verb}'s help is ${lines} lines; it is replayed in an agent's context, so keep it short`);
  }
});

// LLP 1116.003 D5: the helps a small web app reads first (web, test, agent) say what a first build
// needs; `--help all` names every flag the short text leaves out, and the ports.
test('a first build\'s helps are short, and --help all has every flag', () => {
  for (const verb of ['web', 'test', 'agent']) {
    const short = helpText(verb), all = helpText(verb, { all: true });
    assert.ok(short.length <= 1800, `${verb} --help is ${short.length} bytes`);
    assert.match(short, new RegExp(`^More     bun exact\\.mjs ${verb} --help all: `, 'm'));
    for (const [row] of [...HELP[verb].flags, ...HELP[verb].more]) {
      for (const spec of row.split(' · ')) assert.ok(all.includes(spec.split(' ')[0]), `${verb} --help all names ${spec}`);
    }
    assert.match(all, /^Ports {4}/m, `${verb} --help all says its ports`);
    assert.doesNotMatch(all, /^More /m);
  }
  // LLP 1116.008 D2: web's help does not teach a background dev server (builders followed it into five steps).
  assert.match(helpText('web', { all: true }), /The dev loop is for a person watching\n\s+the app change: verify, test and agent build and serve their own copy, so checking\n\s+the app needs no dev server\./);
  assert.doesNotMatch(helpText('web', { all: true }), /web\.log| &\s|background|\$!/);
  // LLP 1116.007 D2: a builder parses `agent` output, so the short text says --json is one object per line.
  assert.match(helpText('agent'), /^ {2}--json +one compact JSON object per op reply, one per line/m);
  // LLP 1116.007 D3.9: `state <name>` is a state or resource read down a dotted path; a big one prints its sizes.
  assert.match(helpText('agent'), /^ {2}state \[<name>\] \[--full\] +a state, resource or field, dotted: groceries\.0\.done; >8 KB: sizes/m);
});

// LLP 1116.009 D6: builders read `help` and then four or five verbs' `--help`, each a step carried to the end of the build.
// The index is the one read: each verb, then the forms a build uses, in one screen, and every flag a form names is one its
// verb's script parses (a form never names a flag that does not exist).
test('help is the one read: the verbs and the forms a build uses, in one screen, naming only flags the verbs parse', () => {
  const text = indexText({ verbs: Object.keys(HELP).filter((v) => v !== 'new') });
  assert.ok(text.split('\n').length <= 36, `the index is ${text.split('\n').length} lines`);
  assert.ok(Buffer.byteLength(text) <= 2800, `the index is ${Buffer.byteLength(text)} bytes`);
  // What builders used across rounds 10 and 11: verify's platform and `--shots`, test's platform, agent's `--storage`, `--size`
  // and its common operations, web and ios `--run`, contract's build and vocab, pattern.
  for (const form of [
    /^ {2}verify \[ios\] \[--shots <dir>\]/m, /^ {2}test \[web\|ios\]/m, /^ {2}agent \[web\|ios\] \[--storage <name>\] \[--size <w>x<h>\]/m,
    /"tap <target>"/, /"type <target> <text>"/, /"clock data\|settle\|\+<ms>"/, /"state \[<name>\]"/, /"screenshot <png>"/, /"check \[both\]"/, /"resize <w>x<h>"/, /"prefer <feature> <value>"/,
    /^ {2}web \[--port <n>\]/m, /^ {2}ios \[--run\]/m, /^ {2}contract build <file> --json · contract vocab <name>/m, /^ {2}pattern \[<name>\]/m,
  ]) assert.match(text, form);
  for (const [verb, entry] of Object.entries(HELP)) {
    const known = knownFlags(verb);
    for (const form of entry.common ?? []) {
      for (const [flag] of form.matchAll(/--[a-z][a-z-]*/g)) {
        if (verb === 'contract') continue; // the compiler's own subcommand flags
        assert.ok(known.has(flag) || [...HELP[verb].flags, ...(HELP[verb].more ?? [])].some(([row]) => row.includes(flag)), `${verb}'s form names ${flag}, which its script does not parse`);
      }
    }
  }
  // Without a verb's forms (a game's own verbs have none), the index is the verbs.
  assert.doesNotMatch(indexText({ verbs: ['lock'] }), /The forms a build uses/);
});
