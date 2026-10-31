// The helps against the scripts they describe (LLP 1116 D4, LLP 1116.009 D6), read from source with no browser
// and no build: each verb's help names exactly the flags its script parses, every row it names is shown, each help
// and the index stay short, and the index's forms name only flags their verb parses. scripts/help.test.mjs asserts
// these and more (the helps' wording and the index's forms, which no flag edit moves); caps.mjs runs
// `helpParity` before every push, so a flag with no help row, or a help row a flag's edit hid, stops the push
// rather than turning main red.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { appVerbs } from './app-cli.mjs';
import { HELP, helpText, indexText, knownFlags } from './help.mjs';

/** The `--flags` the `sources` parse, as the scripts spell a read: `args.includes('--x')`, `arg('--x'`,
 * `argv[i] === '--x'`, `value('--x')`, `['--x', '--y'].find(`; sorted. */
export function parsed(...sources) {
  const flags = new Set();
  for (const source of sources) {
    for (const re of [/\b(?:args|argv|rest|process\.argv)\.(?:includes|indexOf)\('(--[a-z-]+)'\)/g, /\barg\('(--[a-z-]+)'/g, /argv\[i\] === '(--[a-z-]+)'/g, /\bvalue\('(--[a-z-]+)'\)/g]) for (const [, flag] of source.matchAll(re)) flags.add(flag);
    for (const [, list] of source.matchAll(/\[((?:'--[a-z-]+'(?:, )?)+)\]\.(?:find|includes)\(/g)) for (const [, flag] of list.matchAll(/'(--[a-z-]+)'/g)) flags.add(flag);
  }
  return [...flags].sort();
}

/** Each verb whose script's flags are read from source, with the flags it parses: `[verb, flags]`. Throws when a
 * source moved out from under a slice this reads. */
export function scriptFlags(root) {
  const read = (file) => readFileSync(resolve(root, file), 'utf8');
  // A slice from one marker to the next after it, or a throw naming the one gone (never an empty slice, which would
  // parse no flags and agree with a help that names none).
  const slice = (file, text, from, to) => {
    const start = text.indexOf(from), end = start < 0 ? -1 : text.indexOf(to, start + from.length);
    if (start < 0 || end < 0) throw new Error(`${file} has no ${JSON.stringify(start < 0 ? from : to)}${start < 0 ? '' : ` after ${JSON.stringify(from)}`}, where a verb's flags are read`);
    return text.slice(start, end);
  };
  const exact = read('scripts/exact.mjs'), between = (from, to) => slice('scripts/exact.mjs', exact, from, to);
  const readiness = slice('scripts/app-cli.mjs', read('scripts/app-cli.mjs'), "if (verb === 'status'", 'if (!verbs[verb]');
  // The dev loops read `--pair-ttl` through stage 10 A's pairTtlOf; the rest of that module is `tamber pair`'s.
  const pairTtl = slice('scripts/tamber-pairing.mjs', read('scripts/tamber-pairing.mjs'), 'export function pairTtlOf(', '\n}\n');
  const apple = parsed(read('host/apple/build.mjs'), read('host/apple/devices.mjs'), read('host/apple/link.mjs'), read('host/apple/bundle.mjs'));
  const agent = parsed(read('scripts/agent-launch.mjs'), read('scripts/agent-widget.mjs'));
  return [
    ['web', parsed(read('host/web/dev.mjs'), read('scripts/install-page.mjs'), read('scripts/tamber-remote.mjs'), read('scripts/tamber-relay.mjs'), pairTtl, readiness)],
    ['web-build', parsed(read('host/web/build.mjs'))],
    ['serve', parsed(read('host/web/serve.mjs'))],
    ['agent', agent], ['test', agent], ['verify', parsed(read('scripts/verify.mjs'))],
    ['ios', apple], ['mac', apple],
    ['linux', parsed(read('scripts/build-linux.mjs'))], ['android', parsed(read('scripts/agent-android.mjs'))],
    ['feedback', parsed(read('scripts/feedback.mjs'))],
    ['hatch', parsed(between('export function hatch(', '\nexport function pattern('))],
    ['add', parsed(between('export function add(', '\n/** `exact hatch'))],
    ['pattern', parsed(between('export function pattern(', '\n/** The guides'))],
    ['docs', parsed(between('export function docs(', '\nasync function main('))],
    ['new', parsed(between("if (verb === 'new')", '\n  }'))],
  ];
}

/** What is wrong with the helps in `root`, each `{ code, message, detail }` (caps.mjs's shape); none when they
 * agree with their scripts and fit. Synchronous and quick: a few file reads and regular expressions. */
export function helpParity(root) {
  const out = [], fail = (code, message, detail) => out.push({ code, message, detail });
  const where = 'scripts/help.mjs';
  let flags = [];
  // (The checks after the flags' need no script, so they run either way.)
  try { flags = scriptFlags(root); } catch (e) { fail('help:sources', `the helps' flags could not be read from their scripts: ${e.message}.`, 'scripts/help-parity.mjs scriptFlags names the files and markers each verb\'s flags are read between: follow the move there.'); }
  for (const [verb, theirs] of flags) {
    if (!HELP[verb]) { fail('help:verb', `${where} has no help for ${verb}, whose script parses ${theirs.join(' ') || 'no flags'}.`, `Add HELP['${verb}'].`); continue; }
    const ours = new Set(knownFlags(verb).keys()), parses = new Set(theirs);
    const unsaid = theirs.filter((f) => !ours.has(f)), stale = [...ours].filter((f) => !parses.has(f)).sort();
    if (unsaid.length) fail('help:unsaid', `${verb} parses ${unsaid.join(', ')}, which its help does not name.`, `Add a row for each to HELP['${verb}'] in ${where} (flags, more, or internal for one a builder never types), keeping the help at most 36 lines: another flag's row can take it after ' · '.`);
    if (stale.length) fail('help:stale', `${verb}'s help names ${stale.join(', ')}, which its script does not parse.`, `A row in HELP['${verb}'] that an edit replaced or that names a flag that is gone: put the row back as it was, or drop the flag from it.`);
  }
  for (const verb of Object.keys(HELP)) {
    const lines = helpText(verb, { all: true }).split('\n').length;
    if (lines > 36) fail('help:long', `${verb}'s help (--help all) is ${lines} lines, over 36.`, 'It is replayed in an agent\'s context: put two short rows on one line (`a · b`), or shorten one.');
  }
  for (const verb of ['web', 'test', 'agent']) {
    if (!HELP[verb]) { fail('help:verb', `${where} has no help for ${verb}.`, `Add HELP['${verb}'].`); continue; }
    const bytes = helpText(verb).length, all = helpText(verb, { all: true });
    if (bytes > 1800) fail('help:long', `${verb} --help is ${bytes} bytes, over 1800.`, 'A first build reads it: move a row to `more`, which --help all shows.');
    // Every row's flag is in --help all: a row another edit replaced is gone from it.
    for (const [row] of [...HELP[verb].flags, ...(HELP[verb].more ?? [])]) {
      for (const spec of row.split(' · ')) if (!all.includes(spec.split(' ')[0])) fail('help:hidden', `${verb} --help all does not show ${spec.split(' ')[0]}, which a row of its help names.`, `Its row in HELP['${verb}'] in ${where} is not rendered: an edit to another row took its place.`);
    }
  }
  // The index an app's exact.mjs prints, and a game's: each lists its own verbs (app-cli.mjs `appVerbs`).
  for (const game of [false, true]) {
    const index = indexText({ verbs: Object.keys(appVerbs('app', '.', { game })) }), kind = game ? 'a game\'s' : 'an app\'s';
    const indexLines = index.split('\n').length, indexBytes = Buffer.byteLength(index);
    if (indexLines > 36) fail('help:long', `${kind} help index is ${indexLines} lines, over 36.`, 'It is the one read a builder makes: shorten a verb\'s line or a form.');
    if (indexBytes > 2800) fail('help:long', `${kind} help index is ${indexBytes} bytes, over 2800.`, 'It is the one read a builder makes: shorten a verb\'s line or a form.');
  }
  for (const [verb, entry] of Object.entries(HELP)) {
    if (verb === 'contract') continue; // the compiler's own subcommand flags
    const known = knownFlags(verb), rows = [...entry.flags, ...(entry.more ?? [])];
    for (const form of entry.common ?? []) {
      for (const [flag] of form.matchAll(/--[a-z][a-z-]*/g)) {
        if (!known.has(flag) && !rows.some(([row]) => row.includes(flag))) fail('help:form', `${verb}'s form \`${form}\` names ${flag}, which its script does not parse.`, `Fix the form in HELP['${verb}'].common in ${where}.`);
      }
    }
  }
  return out;
}
