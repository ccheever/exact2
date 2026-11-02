import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, openSync, closeSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

// Run against a built standalone Caltrain executable, without the agent or dev menu.
test.skipIf(!process.env.EXACT_RELAUNCH_APP)('relaunch cancels with quit, then relaunches from beforeunload and preserves writes with clean state', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relaunch-'));
  const token = `relaunch${process.pid}${Date.now()}`, stage = `${token}Stage`, cancelled = `${token}Cancelled`;
  const executable = resolve(process.env.EXACT_RELAUNCH_APP);
  const source = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), log = join(dir, 'stderr');
  writeFileSync(source, `component App
  state ${stage} = 0 persist
  state ${cancelled} = false persist
  state presses = 0
  state guarded = true
  task go mount
    every(1000, tick)
  action quitting
    if guarded
      preventDefault()
    else if ${stage} == 3
      ${stage} = 4
      relaunch()
  action tick
    if ${stage} == 0
      presses = 2
      ${stage} = 1
    else if ${stage} == 1
      ${stage} = 2
      relaunch()
    else if ${stage} == 2
      ${cancelled} = presses == 2
      guarded = false
      ${stage} = 3
    else if ${stage} == 4
      guarded = false
      ${stage} = presses == 0 && ${cancelled} ? 5 : 6
  view
    main beforeunload=quitting
      text "Relaunch verification"
`);
  const build = spawnSync('target/debug/contract', ['build', source, '-o', plan], { encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const env = { ...process.env, EXACT_PLAN: plan, EXACT_DEV_MENU: '0' };
  for (const key of ['EXACT_AGENT', 'EXACT_STORE', 'EXACT_SMOKE', 'EXACT_DEV_PLAN']) delete env[key];
  const fd = openSync(log, 'w');
  const child = spawn(executable, ['--relaunch-test', token, 'argument with spaces'], { env, stdio: ['ignore', 'ignore', fd] });
  closeSync(fd);
  const started = new Set([child.pid]);
  let exited = false, quitRequested = false, replacement;
  child.on('exit', () => { exited = true; });
  const domain = 'com.exact.caltrain';
  try {
    const until = Date.now() + 30000;
    let value;
    while (Date.now() < until) {
      // The replacement is this test's child too; identify it by the unique argument.
      const rows = spawnSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).stdout.split('\n');
      const apps = rows.filter(line => line.includes(executable) && line.includes(token) && !line.includes('--exact-relaunch-wait'));
      for (const row of apps) started.add(Number(row.trim().split(/\s+/)[0]));
      const read = spawnSync('defaults', ['read', domain, `exact.state.${stage}`], { encoding: 'utf8' });
      value = read.stdout.trim();
      if (value === '3' && !quitRequested) {
        quitRequested = true;
        child.kill('SIGTERM');
      }
      if (value === '5' || value === '6') {
        expect(apps.length).toBe(1);
        replacement = Number(apps[0].trim().split(/\s+/)[0]);
        expect(apps[0]).toContain(`--relaunch-test ${token} argument with spaces`);
        break;
      }
      await new Promise(done => setTimeout(done, 100));
    }
    expect(value, readFileSync(log, 'utf8')).toBe('5');
    expect(exited).toBe(true);
    expect(quitRequested).toBe(true);
    expect(replacement).not.toBe(child.pid);
    console.log(`relaunch: PID ${child.pid} -> ${replacement}; cancellation, beforeunload, clean state, stored writes, arguments, dev menu off`);
  } finally {
    for (const pid of started) { try { process.kill(pid, 'SIGKILL'); } catch {} }
    for (const key of [stage, cancelled]) spawnSync('defaults', ['delete', domain, `exact.state.${key}`]);
    rmSync(dir, { recursive: true, force: true });
  }
}, 45000);
