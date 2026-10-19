// The helps against the scripts they describe (LLP 1116 D4, LLP 1116.009 D6), read from source with no browser
// and no build: each verb's help names exactly the flags its script parses, each help and the index stay short,
// and the index's forms name only flags their verb parses. scripts/help.test.mjs asserts the same; caps.mjs runs
// `helpParity` before every push, so a flag with no help row, or a help row a flag's edit hid, stops the push
// rather than turning main red.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
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
  const exact = read('scripts/exact.mjs'), between = (from, to) => exact.slice(exact.indexOf(from), exact.indexOf(to, exact.indexOf(from)));
  const appCLI = read('scripts/app-cli.mjs');
  const readiness = appCLI.slice(appCLI.indexOf("if (verb === 'status'"), appCLI.indexOf('if (!verbs[verb]'));
  // The dev loops read `--pair-ttl` through stage 10 A's pairTtlOf; the rest of that module is `tamber pair`'s.
  const pairing = read('scripts/tamber-pairing.mjs'), from = pairing.indexOf('export function pairTtlOf('), to = pairing.indexOf('\n}\n', from);
  if (!(from >= 0 && to > from)) throw new Error('scripts/tamber-pairing.mjs has no pairTtlOf, where the dev loops read --pair-ttl');
  const pairTtl = pairing.slice(from, to);
  const apple = parsed(read('host/apple/build.mjs'), read('host/apple/devices.mjs'), read('host/apple/link.mjs'), read('host/apple/bundle.mjs'));
  const agent = parsed(read('scripts/agent-launch.mjs'), read('scripts/agent-widget.mjs'));
  return [
    ['web', parsed(read('host/web/dev.mjs'), read('scripts/install-page.mjs'), read('scripts/tamber-remote.mjs'), pairTtl, readiness)],
    ['web-build', parsed(read('host/web/build.mjs'))],
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
  let flags;
  try { flags = scriptFlags(root); } catch (e) { fail('help:sources', `the helps' flags could not be read from their scripts: ${e.message}`, 'scripts/help-parity.mjs scriptFlags names the files and slices each verb parses; follow the move.'); return out; }
  for (const [verb, theirs] of flags) {
    if (!HELP[verb]) { fail('help:verb', `${where} has no help for ${verb}, whose script parses ${theirs.join(' ') || 'no flags'}.`, `Add HELP['${verb}'].`); continue; }
    const ours = new Set(knownFlags(verb).keys()), parses = new Set(theirs);
    const unsaid = theirs.filter((f) => !ours.has(f)), stale = [...ours].filter((f) => !parses.has(f)).sort();
    if (unsaid.length) fail('help:unsaid', `${verb} parses ${unsaid.join(', ')}, which its help does not name.`, `Add a row for each to HELP['${verb}'] in ${where} (flags, more, or internal for one a builder never types), keeping the help at most 36 lines: another flag's row can take it after ' · '.`);
    if (stale.length) fail('help:stale', `${verb}'s help names ${stale.join(', ')}, which its script does not parse.`, `A row in HELP['${verb}'] that an edit replaced or that names a flag that is gone: put the row back as it was, or drop the flag from it.`);
  }
  for (const verb of Object.keys(HELP)) {
    const lines = helpText(verb, { all: true }).split('\n').length;
    if (lines > 36) fail('help:long', `${verb}'s help is ${lines} lines, over 36.`, 'It is replayed in an agent\'s context: put two short rows on one line (`a · b`), or move one to `more`.');
  }
  for (const verb of ['web', 'test', 'agent']) {
    const bytes = helpText(verb).length;
    if (HELP[verb] && bytes > 1800) fail('help:long', `${verb} --help is ${bytes} bytes, over 1800.`, 'A first build reads it: move a row to `more`, which --help all shows.');
  }
  const index = indexText({ verbs: Object.keys(HELP).filter((v) => !['new', 'add'].includes(v)) });
  const indexLines = index.split('\n').length, indexBytes = Buffer.byteLength(index);
  if (indexLines > 36 || indexBytes > 2800) fail('help:long', `the help index is ${indexLines} lines and ${indexBytes} bytes, over 36 and 2800.`, 'It is the one read a builder makes: shorten a verb\'s line or a form.');
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
