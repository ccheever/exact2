// Authored tests (LLP 1017 P7), the driver's half: `contract test <file>`
// turns a file's `test` blocks into steps, and this drives them through the
// session the operations use (`agent.mjs`'s `open`). `agent.mjs` re-exports it.
import { spawnSync } from 'node:child_process';
import { readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { open } from './agent.mjs';
import { driveStore, launchFacts } from './agent-launch.mjs';
import { resolveApp } from './app.mjs';

/** The text `expect text` reads (kanban F19, shop F15): the node's own `text`, else a control's value (a select's options
 * are its choices, not its text; LLP 1087 wizard trials), else its descendants' in order — the web's `textContent`, a button's
 * label — else a field's value. `nodes` is a `tree` reply's, in preorder. */
export function textOf(nodes, node) {
  if (node.props.text != null) return node.props.text;
  if (node.type === 'Control' && node.props.value != null) return node.props.value;
  const at = nodes.indexOf(node), runs = [];
  for (let i = at + 1; i < nodes.length && nodes[i].depth > node.depth; i++) if (nodes[i].props.text != null) runs.push(nodes[i].props.text);
  return runs.length ? runs.join('') : node.props.value;
}

/** Where a native host keeps a drive's scratch stores (host/apple and host/linux `configure_storage`), or
 * null where the driver cannot reach them: an iOS simulator's are in its app container and go with the app. */
export function storeBase(appId, host, env = process.env, home = homedir()) {
  if (host === 'macos' || host === 'mac') return resolve(home, 'Library/Caches/exact', appId, 'agent');
  if (host === 'linux') return resolve(env.XDG_CACHE_HOME?.startsWith('/') ? env.XDG_CACHE_HOME : resolve(home, '.cache'), 'exact', appId, 'agent');
  return null;
}
const alive = pid => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
/** The stores of authored-test runs that are gone (`<storage>.r<pid>-<tag>.t<n>`, the pid no longer running):
 * a run killed before it removed its own. A live run's are left alone, so concurrent runs never share or
 * empty one another's. */
export function sweepTestStores(base, storage) {
  if (!base) return;
  let names = []; try { names = readdirSync(base); } catch { return; }
  const ours = new RegExp(`^${storage.replace(/[.]/g, '\\.')}\\.r(\\d+)-[0-9a-z]+\\.t\\d+$`);
  for (const name of names) { const m = name.match(ours); if (m && !alive(Number(m[1]))) rmSync(resolve(base, name), { recursive: true, force: true }); }
}
/** A launch line's op and the `open` option it sets (`size` aside: it is two numbers). */
const LAUNCH = { epoch: 'epoch', 'time-zone': 'timeZone', locale: 'locale', seed: 'seed' };

/**
 * Run a `test "…"` file against a host. Each test is a session of its own
 * from the first frame, opened with its launch lines — `size`, `epoch`,
 * `time-zone`, `locale`, `seed`, the test's own or the file's (the compiler
 * puts them first), else the drive's flags — with app storage of its own: a scratch store
 * `<storage>.r<pid>-<tag>.t<n>` of this run's (Chrome's profile for it on the web), emptied at
 * launch and removed after the test (with any a killed run left), so an app
 * that keeps its data in storage loads and no test, concurrent run or
 * earlier run sees another's writes. The app's data lands before the first step (and after a `reload`), unless
 * the test says `before data`. A failed expect names the test, the line, and what
 * was seen. Returns `{ passed, failed, results }`.
 */
export async function runTests({ host, browser, file, plan, app, size, env, webDist, device = false, phone, url, seed, locale, timeZone, epoch, storage = 'test' } = {}) {
  const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
  // Cargo owns target selection and freshness, including CARGO_TARGET_DIR.
  const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'test', resolve(file)], { cwd: root, encoding: 'utf8' });
  if (c.status !== 0) throw new Error(c.stderr?.trim() || c.error?.message || 'contract test compiler failed');
  const tests = JSON.parse(c.stdout);
  const results = [];
  // Where the host keeps its stores, as it will see its environment (a drive's env overrides the driver's).
  const launched = { ...process.env, ...(env ?? {}) };
  const id = resolveApp(app).id, chrome = host === 'web' && (browser ?? launched.EXACT_WEB_BROWSER ?? 'chrome') === 'chrome';
  const base = device ? null : chrome ? driveStore(id, storage, env).base : host === 'web' ? null : storeBase(id, host, launched, launched.HOME || homedir());
  // A run's own names where the driver can remove them; a simulator's (one drive at a time: a launch ends the
  // last) reuse one store a test, emptied at launch, so they cannot pile up in its app container.
  const tag = base ? `.r${process.pid}-${Math.random().toString(36).slice(2, 8)}` : '';
  sweepTestStores(base, storage);
  for (const [n, t] of tests.entries()) {
    const failures = [];
    const store = base || host !== 'web' ? `${storage}${tag}.t${n}` : storage, fresh = { ...(env ?? {}), EXACT_AGENT_STORAGE_FRESH: '1' };
    // A test's launch lines lead its steps and override the drive's flags (habits F7).
    const facts = { size, seed, locale, timeZone, epoch };
    const lines = [];
    let beforeData = false;
    for (const st of t.steps) {
      if (st.op === 'before-data') beforeData = true;
      else if (st.op === 'size') facts.size = [st.width, st.height];
      else if (LAUNCH[st.op]) facts[LAUNCH[st.op]] = st.value;
      else break;
      lines.push(st.line);
    }
    // A zone or locale the driver refuses fails this test at its line, not the run.
    try { launchFacts({ ...facts, env: env ?? {} }); } catch (e) {
      results.push({ name: t.name, failures: [`${t.name}: ${lines.length ? `line ${lines.join(', ')}` : "the drive's launch flags"}: ${e.message}`] });
      continue;
    }
    const launch = (environment) => open({ host, browser, plan, ...facts, env: environment, app, webDist, device, phone, url, storage: store });
    let s = await launch(fresh);
    // The app's data lands before the first step, as `clock data` lands it: activation and every request in flight,
    // the clock unmoved and no timer fired (habits, pomodoro, kanban: a store opened at launch raced the first step).
    // `before data` does not wait (what has landed then is the host's: a native app ran on real time before the
    // driver connected). An unsettled wait leaves the expects to name what is still in flight.
    const data = async () => { if (!beforeData) await s.clock('data'); };
    // `reload`: the app restarts on the store it had (mail F19, kanban F25): the web page loads again in its
    // profile, keeping what the origin stored; a native app relaunches on the same scratch store, not emptied.
    const reload = async () => {
      if (s.host === 'web') { await s.carrier.reset({ keep: true }); s.now = 0; s.logCursor = 0; return; }
      await s.close(); s = await launch(env);
    };
    // The clock stands still between steps: what an input started (a reply,
    // a mutation's `then`, a timer, a transition) lands at a clock step. A
    // failed expect after an input with none says so (kanban F19).
    let input = null;
    // An input the host could not perform fails its step: an unsupported drag or a refused tap did nothing to assert on.
    const delivered = (r) => { if (r?.error || r?.delivery === 'unsupported') throw new Error(r.error ?? r.reason ?? 'the host does not support this input'); };
    // With no input since the clock last moved, a request still in flight (the boot's own, or one a jump
    // left on real time) is named: the expect read the value before its reply (workout F1).
    const fail = async (message) => {
      if (input != null) return failures.push(`${message} (the clock has not moved since line ${input}'s input: a reply, a mutation's \`then\` or a transition lands at \`clock settle\`; a timer fires when the clock reaches its time, \`clock +N\`)`);
      const pending = ((await s.state().catch(() => ({}))).pending ?? []).filter((p) => !p.device).map((p) => p.name);
      failures.push(pending.length ? `${message} (${pending.length} request${pending.length === 1 ? '' : 's'} still in flight: ${pending.join(', ')}; a reply lands at a \`clock\` step, as \`clock settle\`)` : message);
    };
    try {
      try { await data(); } catch (e) { failures.push(`${t.name}: waiting for the app's data before the first step: ${e.message}`); }
      for (const st of failures.length ? [] : t.steps) {
        const at = `${t.name}: line ${st.line}`;
        try {
          switch (st.op) {
            case 'size': case 'epoch': case 'time-zone': case 'locale': case 'seed': case 'before-data': break; // the session opened with it
            // The driver's `tap` forms (feed F10): `into` brings a virtualized list's row into view by its key.
            case 'tap': delivered(await s.tap(st.target, st.form === 'into' ? { into: { key: st.key } } : st.form !== 'press' ? { [st.form]: true } : st.modifiers ? { modifiers: st.modifiers } : undefined)); input = st.line; break;
            case 'drag': delivered(await s.tap(st.target, { drag: { dx: st.dx, dy: st.dy, ...(st.from ? { from: st.from } : {}), ...(st.mouse ? { mouse: true } : {}), ...(st.press != null ? { press: st.press } : {}), ...(st.over != null ? { over: st.over } : {}), ...(st.hold != null ? { hold: st.hold } : {}) } })); input = st.line; break;
            case 'type': {
              // `append`: after the field's value as the tree shows it, the text a keyboard would add (feed F8).
              let text = st.text;
              if (st.append) {
                const { nodes } = await s.tree(), field = nodes.find((n) => n.props.testId === st.target && !n.inactive) ?? nodes.find((n) => n.props.testId === st.target);
                if (field && typeof field.props.value !== 'string') throw new Error(`type … append: "${st.target}" shows no text value to append to`);
                text = (field?.props.value ?? '') + text;
              }
              delivered(await s.type(st.target, text)); input = st.line; break;
            }
            case 'reload': await reload(); await data(); input = null; break;
            case 'key': delivered(await s.type(st.target, { key: st.key })); input = st.line; break;
            // A held picker, by the node its answer arrives at or its capability (files F11); paths are the test file's.
            case 'pick': delivered(st.paths.length ? await s.type(`@${st.target}`, st.paths.map((p) => resolve(dirname(resolve(file)), p)).join('\n') + '\n') : await s.tap(`@${st.target}`, { choice: 'cancel' })); input = st.line; break;
            case 'clipboard': delivered(await s.type(st.target, { clipboard: st.edit, text: st.text })); input = st.line; break;
            case 'clock': await s.clock(st.arg); input = null; break;
            case 'resize': delivered(await s.resize(st.width, st.height)); input = st.line; break;
            case 'screenshot': await s.screenshot(st.path); break;
            case 'expect-tree': {
              const tree = await s.tree();
              const found = tree.nodes.some((n) => n.props.testId === st.target);
              if (found !== st.present) await fail(`${at}: expected testId "${st.target}" ${st.present ? 'present' : 'absent'}, it was ${found ? 'present' : 'absent'}`);
              break;
            }
            case 'expect-text': {
              const { nodes } = await s.tree();
              // As a target is found: a covered screen's copy only when no active one carries it (shop F16).
              const matches = nodes.filter((n) => n.props.testId === st.target), n = matches.find((m) => !m.inactive) ?? matches[0];
              const got = n ? textOf(nodes, n) : undefined;
              if (got !== st.value) await fail(`${at}: text of "${st.target}" is ${n ? JSON.stringify(got) : 'absent (no view carries that testId)'}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            case 'expect-state': {
              const state = await s.state();
              const bag = { ...(state.resources ?? {}), ...(state.derives ?? {}), ...(state.slots ?? {}) };
              // A field of a record at any depth, `name.field` (feed F10).
              const [name, ...fields] = st.name.split('.');
              if (!(name in bag)) { failures.push(`${at}: no state named "${name}"`); break; }
              let got = bag[name], path = name, missing = null;
              for (const field of fields) {
                if (got === null || typeof got !== 'object' || !(field in got)) { missing = field; break; }
                got = got[field]; path += `.${field}`;
              }
              if (missing != null) { failures.push(`${at}: ${path} has no field "${missing}" (${got !== null && typeof got === 'object' && !Array.isArray(got) ? `its fields: ${Object.keys(got).join(', ')}` : `it is ${JSON.stringify(got).slice(0, 200)}`})`); break; }
              if (JSON.stringify(got) !== JSON.stringify(st.value)) await fail(`${at}: ${st.name} is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            // The runner's voice table, the whole record (LLP 1096 D10): a voice of that
            // source matching every clause given, or none; a dropped call is not a voice.
            case 'expect-sound': {
              const sounds = (await s.state(undefined, undefined, false, false, { sounds: 'all' })).sounds;
              const clauses = ['at', 'gain', 'ends', 'by'].filter((k) => st[k] != null), said = clauses.map((k) => ` ${k} ${st[k]}`).join('');
              if (!sounds) { failures.push(`${at}: the app declares no sound, so the runner keeps no voice table`); break; }
              const first = sounds.voices[0]?.at ?? Infinity;
              if (sounds.evicted > 0 && (st.at != null ? st.at < first : !st.present)) { failures.push(`${at}: voices before t=${first} are no longer recorded (the record keeps the last ${sounds.recorded})`); break; }
              const found = sounds.voices.some((v) => v.src === st.src && clauses.every((k) => v[k] === st[k]));
              if (found !== st.present) {
                const of = sounds.voices.filter((v) => v.src === st.src).slice(-8).map((v) => `#${v.id} at ${v.at} gain ${v.gain} ends ${v.ends} by ${v.by}`);
                // A voice is recorded by the commit that issued it: no clock hint applies.
                failures.push(`${at}: expected ${st.present ? 'a' : 'no'} voice of "${st.src}"${said}; ${of.length ? `its voices: ${of.join('; ')}` : 'it has no voice'}`);
              }
              break;
            }
            default: failures.push(`${at}: unknown step ${st.op}`);
          }
        } catch (e) {
          failures.push(`${at}: ${e.message}`);
          break;
        }
      }
    } finally {
      await s.close();
    }
    results.push({ name: t.name, failures });
    if (base) rmSync(resolve(base, store), { recursive: true, force: true });
  }
  const failed = results.filter((r) => r.failures.length).length;
  return { passed: results.length - failed, failed, results };
}
