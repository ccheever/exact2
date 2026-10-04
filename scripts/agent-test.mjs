// Authored tests (LLP 1017 P7), the driver's half: `contract test <file>`
// turns a file's `test` blocks into steps, and this drives them through the
// session the operations use (`agent.mjs`'s `open`). `agent.mjs` re-exports it.
import { spawnSync } from 'node:child_process';
import { readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { open } from './agent.mjs';
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
/**
 * Run a `test "…"` file against a host. Each test is a session of its own
 * from the first frame — at its `size` when its first step names one, else
 * the drive's — with app storage of its own: on the web its fresh profile; on a native host
 * a scratch store `<storage>.r<pid>-<tag>.t<n>` of this run's, emptied at
 * launch and removed after the test (with any a killed run left), so an app
 * that keeps its data in storage loads and no test, concurrent run or
 * earlier run sees another's writes. A failed expect names the test, the line, and what
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
  const base = host === 'web' || device ? null : storeBase(resolveApp(app).id, host, launched, launched.HOME || homedir());
  // A run's own names where the driver can remove them; a simulator's (one drive at a time: a launch ends the
  // last) reuse one store a test, emptied at launch, so they cannot pile up in its app container.
  const tag = base ? `.r${process.pid}-${Math.random().toString(36).slice(2, 8)}` : '';
  sweepTestStores(base, storage);
  for (const [n, t] of tests.entries()) {
    const failures = [];
    const store = host === 'web' ? storage : `${storage}${tag}.t${n}`, fresh = host === 'web' ? env : { ...(env ?? {}), EXACT_AGENT_STORAGE_FRESH: '1' };
    const own = t.steps[0]?.op === 'size' ? [t.steps[0].width, t.steps[0].height] : size;
    const s = await open({ host, browser, plan, size: own, env: fresh, app, webDist, device, phone, url, seed, locale, timeZone, epoch, storage: store });
    // The clock stands still between steps: what an input started (a reply,
    // a mutation's `then`, a timer, a transition) lands at a clock step. A
    // failed expect after an input with none says so (kanban F19).
    let input = null;
    // An input the host could not perform fails its step: an unsupported drag or a refused tap did nothing to assert on.
    const delivered = (r) => { if (r?.error || r?.delivery === 'unsupported') throw new Error(r.error ?? r.reason ?? 'the host does not support this input'); };
    const fail = (message) => failures.push(input == null ? message : `${message} (the clock has not moved since line ${input}'s input: a reply, a mutation's \`then\` or a transition lands at \`clock settle\`; a timer fires when the clock reaches its time, \`clock +N\`)`);
    try {
      for (const st of t.steps) {
        const at = `${t.name}: line ${st.line}`;
        try {
          switch (st.op) {
            case 'size': break; // the session opened at it
            case 'tap': delivered(await s.tap(st.target, st.hover ? { hover: true } : undefined)); input = st.line; break;
            case 'drag': delivered(await s.tap(st.target, { drag: { dx: st.dx, dy: st.dy, ...(st.press != null ? { press: st.press } : {}), ...(st.over != null ? { over: st.over } : {}), ...(st.hold != null ? { hold: st.hold } : {}) } })); input = st.line; break;
            case 'type': delivered(await s.type(st.target, st.text)); input = st.line; break;
            case 'key': delivered(await s.type(st.target, { key: st.key })); input = st.line; break;
            case 'clock': await s.clock(st.arg); input = null; break;
            case 'screenshot': await s.screenshot(st.path); break;
            case 'expect-tree': {
              const tree = await s.tree();
              const found = tree.nodes.some((n) => n.props.testId === st.target);
              if (found !== st.present) fail(`${at}: expected testId "${st.target}" ${st.present ? 'present' : 'absent'}, it was ${found ? 'present' : 'absent'}`);
              break;
            }
            case 'expect-text': {
              const { nodes } = await s.tree();
              // As a target is found: a covered screen's copy only when no active one carries it (shop F16).
              const matches = nodes.filter((n) => n.props.testId === st.target), n = matches.find((m) => !m.inactive) ?? matches[0];
              const got = n ? textOf(nodes, n) : undefined;
              if (got !== st.value) fail(`${at}: text of "${st.target}" is ${n ? JSON.stringify(got) : 'absent (no view carries that testId)'}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            case 'expect-state': {
              const state = await s.state();
              const bag = { ...(state.resources ?? {}), ...(state.derives ?? {}), ...(state.slots ?? {}) };
              if (!(st.name in bag)) { failures.push(`${at}: no state named "${st.name}"`); break; }
              const got = bag[st.name];
              if (JSON.stringify(got) !== JSON.stringify(st.value)) fail(`${at}: ${st.name} is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
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
