// Authored tests (LLP 1017 P7), the driver's half: `contract test <file>`
// turns a file's `test` blocks into steps, and this drives them through the
// session the operations use (`agent.mjs`'s `open`). `agent.mjs` re-exports it.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open } from './agent.mjs';
import { resolveApp } from './app.mjs';

/** The text `expect text` reads (kanban F19, shop F15): the node's own `text`, else its descendants' in order — the web's `textContent`, a button's label — else a field's value. `nodes` is a `tree` reply's, in preorder. */
export function textOf(nodes, node) {
  if (node.props.text != null) return node.props.text;
  const at = nodes.indexOf(node), runs = [];
  for (let i = at + 1; i < nodes.length && nodes[i].depth > node.depth; i++) if (nodes[i].props.text != null) runs.push(nodes[i].props.text);
  return runs.length ? runs.join('') : node.props.value;
}

/** One authored-test run at a time for an app on a native host: its `<storage>.t<n>` stores are reused, and
 * a second run's launch would empty a store the first is using. The lock is a directory holding its owner's
 * `<pid> <token>`; release removes it only while it is still ours. A dead owner's lock (or one never signed,
 * after a second) is taken over under a second directory, the recovery's own mutex, and only once the lock
 * is still the same directory with the same dead owner — so two waiters cannot both take it over. */
export async function testStoreLock(appId, host, dir = tmpdir()) {
  const lock = resolve(dir, `exact-test-stores-${appId}-${host === 'mac' ? 'macos' : host}`), owner = resolve(lock, 'owner');
  const token = `${process.pid} ${Math.random().toString(36).slice(2)}`, recovery = `${lock}.recovery`;
  const read = () => { try { return { ino: statSync(lock).ino, who: readFileSync(owner, 'utf8') }; } catch (e) { return e.code === 'ENOENT' && existsSync(lock) ? { ino: statSync(lock).ino, who: '' } : null; } };
  const dead = who => { const pid = Number(who.split(' ')[0]); if (!(pid > 0)) return true; try { process.kill(pid, 0); return false; } catch (e) { return e.code !== 'EPERM'; } };
  for (let waited = 0, seen = null; ; waited += 200) {
    try {
      mkdirSync(lock); writeFileSync(owner, token);
      return () => { try { if (readFileSync(owner, 'utf8') === token) rmSync(lock, { recursive: true, force: true }); } catch {} };
    } catch (e) { if (e.code !== 'EEXIST') throw e; }
    const now = read();
    if (now && dead(now.who) && (now.who || (seen?.ino === now.ino && waited >= 1000))) {
      // A recovery left behind by a waiter that died mid-way goes after ten seconds.
      try { if (Date.now() - statSync(recovery).mtimeMs > 10_000) rmSync(recovery, { recursive: true, force: true }); } catch {}
      try {
        mkdirSync(recovery);
        try { const again = read(); if (again && again.ino === now.ino && again.who === now.who) rmSync(lock, { recursive: true, force: true }); }
        finally { rmSync(recovery, { recursive: true, force: true }); }
        continue;
      } catch (e) { if (e.code !== 'EEXIST') throw e; }
    }
    if (!seen) console.error(`waiting for another authored-test run of ${appId} on ${host} (${lock})`);
    seen = now ?? seen;
    await new Promise(done => setTimeout(done, 200));
  }
}
/**
 * Run a `test "…"` file against a host. Each test is a session of its own
 * from the first frame — at its `size` when its first step names one, else
 * the drive's — with app storage of its own: a scratch store, `<storage>.t<n>` emptied at
 * launch where a store outlives its drive (native; the page's is its fresh
 * profile), so an app that keeps its data in storage loads, no test sees
 * another's or an earlier run's writes, and repeated runs reuse one store a
 * test (one run at a time an app and host: `testStoreLock`). A failed expect names the test, the line, and what
 * was seen. Returns `{ passed, failed, results }`.
 */
export async function runTests({ host, browser, file, plan, app, size, env, webDist, device = false, phone, url, seed, locale, timeZone, epoch, storage = 'test' } = {}) {
  const root = resolve(new URL('..', import.meta.url).pathname);
  // Cargo owns target selection and freshness, including CARGO_TARGET_DIR.
  const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'test', resolve(file)], { cwd: root, encoding: 'utf8' });
  if (c.status !== 0) throw new Error(c.stderr?.trim() || c.error?.message || 'contract test compiler failed');
  const tests = JSON.parse(c.stdout);
  const results = [];
  const release = host === 'web' ? () => {} : await testStoreLock(resolveApp(app).id, device ? `${host}-device` : host);
  try {
  for (const [n, t] of tests.entries()) {
    const failures = [];
    const store = host === 'web' ? storage : `${storage}.t${n}`, fresh = host === 'web' ? env : { ...(env ?? {}), EXACT_AGENT_STORAGE_FRESH: '1' };
    const own = t.steps[0]?.op === 'size' ? [t.steps[0].width, t.steps[0].height] : size;
    const s = await open({ host, browser, plan, size: own, env: fresh, app, webDist, device, phone, url, seed, locale, timeZone, epoch, storage: store });
    // The clock stands still between steps: what an input started (a reply,
    // a mutation's `then`, a timer, a transition) lands at a clock step. A
    // failed expect after an input with none says so (kanban F19).
    let input = null;
    const fail = (message) => failures.push(input == null ? message : `${message} (the clock has not moved since line ${input}'s input: a reply, a mutation's \`then\`, a timer or a transition lands at a \`clock\` step, as \`clock settle\`)`);
    try {
      for (const st of t.steps) {
        const at = `${t.name}: line ${st.line}`;
        try {
          switch (st.op) {
            case 'size': break; // the session opened at it
            case 'tap': await s.tap(st.target, st.hover ? { hover: true } : undefined); input = st.line; break;
            case 'drag': await s.tap(st.target, { drag: { dx: st.dx, dy: st.dy, ...(st.press != null ? { press: st.press } : {}), ...(st.over != null ? { over: st.over } : {}), ...(st.hold != null ? { hold: st.hold } : {}) } }); input = st.line; break;
            case 'type': await s.type(st.target, st.text); input = st.line; break;
            case 'key': await s.type(st.target, { key: st.key }); input = st.line; break;
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
  }
  } finally { release(); }
  const failed = results.filter((r) => r.failures.length).length;
  return { passed: results.length - failed, failed, results };
}
